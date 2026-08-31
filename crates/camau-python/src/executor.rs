use std::collections::HashMap;
use std::sync::Arc;

use camau_core::execution::{Execution as CoreExecution, Progress as CoreProgress, SharedRng};
use camau_core::graph::CompiledGraph;
use camau_core::json_value::JsonValue;
use pyo3::prelude::*;

use crate::errors::{execution_error, task_result_error};
use crate::python_value::{from_python, to_python};

type TaskRequest = (u64, Py<PyAny>, Py<PyAny>);
type Progress = (Vec<TaskRequest>, Option<Py<PyAny>>);

#[pyclass(module = "camau._camau")]
pub(crate) struct Execution {
    inner: CoreExecution,
    graph: Arc<CompiledGraph>,
    tasks: Arc<HashMap<usize, Py<PyAny>>>,
}

impl Execution {
    pub(crate) fn new(
        graph: Arc<CompiledGraph>,
        tasks: Arc<HashMap<usize, Py<PyAny>>>,
        rng: SharedRng,
        payload: JsonValue,
    ) -> Self {
        Self {
            inner: CoreExecution::new(Arc::clone(&graph), rng, payload),
            graph,
            tasks,
        }
    }

    fn to_python_progress(&self, py: Python<'_>, progress: CoreProgress) -> PyResult<Progress> {
        let requests = progress
            .requests
            .into_iter()
            .map(|request| {
                let callable = self
                    .tasks
                    .get(&request.node)
                    .expect("validated task binding")
                    .clone_ref(py);
                let payload = to_python(&request.payload, py)?;
                Ok((request.ticket, callable, payload))
            })
            .collect::<PyResult<Vec<_>>>()?;
        let output = progress
            .output
            .as_ref()
            .map(|value| to_python(value, py))
            .transpose()?;
        Ok((requests, output))
    }
}

#[pymethods]
impl Execution {
    fn start(&mut self, py: Python<'_>) -> PyResult<Progress> {
        let progress = self
            .inner
            .start()
            .map_err(|failure| execution_error(py, failure))?;
        self.to_python_progress(py, progress)
    }

    fn resume(
        &mut self,
        py: Python<'_>,
        ticket: u64,
        result: &Bound<'_, PyAny>,
    ) -> PyResult<Progress> {
        let Some(pending) = self.inner.pending_task(ticket) else {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(
                "unknown or completed task ticket",
            ));
        };
        let value = match from_python(result, true) {
            Ok(value) => value,
            Err(error) => {
                let reason = if error.message == "expected a JSON object" {
                    "non_object"
                } else {
                    "invalid_json"
                };
                return Err(task_result_error(
                    py,
                    &self.graph.nodes[pending.node].id,
                    &pending.task_name,
                    reason,
                    Some(result.clone().unbind()),
                ));
            }
        };
        let progress = self
            .inner
            .resume(ticket, value)
            .map_err(|failure| execution_error(py, failure))?;
        self.to_python_progress(py, progress)
    }

    fn reject(
        &self,
        py: Python<'_>,
        ticket: u64,
        invalid_value: Py<PyAny>,
        reason: &str,
    ) -> PyResult<()> {
        let Some(pending) = self.inner.pending_task(ticket) else {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(
                "unknown or completed task ticket",
            ));
        };
        Err(task_result_error(
            py,
            &self.graph.nodes[pending.node].id,
            &pending.task_name,
            reason,
            Some(invalid_value),
        ))
    }
}
