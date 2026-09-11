use std::collections::HashMap;
use std::sync::Arc;

use camau_core::diagnostics::IssueData;
use camau_core::execution::{SharedRng, seeded_rng};
use camau_core::graph::{CompiledGraph, NodeKind, compile};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyInt};

use crate::assessment::assess_input;
use crate::errors::configuration_error;
use crate::executor::Execution;
use crate::issue::Assessment;
use crate::python_value::{from_python, input_py_error};

#[pyclass(frozen, module = "camau._camau", name = "_Router")]
pub(crate) struct Router {
    graph: Arc<CompiledGraph>,
    tasks: Arc<HashMap<usize, Py<PyAny>>>,
    rng: SharedRng,
}

enum TaskResolution {
    Bound(Py<PyAny>),
    Missing,
    NotCallable,
}

#[pymethods]
impl Router {
    #[new]
    #[pyo3(signature = (specification, tasks, *, seed=None))]
    fn new(
        py: Python<'_>,
        specification: &Bound<'_, PyAny>,
        tasks: &Bound<'_, PyAny>,
        seed: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let seed = parse_seed(seed)?;
        let (value, assessment) = assess_input(specification);
        if !assessment.data.issues.is_empty() {
            return Err(configuration_error(py, assessment)?);
        }
        let value = value.ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err(
                "assessed routing specification has no parsed value",
            )
        })?;
        let graph = compile(&value).ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err(
                "assessed routing specification could not be compiled",
            )
        })?;
        let mut resolved = HashMap::new();
        for node in &graph.nodes {
            let NodeKind::Task { task, .. } = &node.kind else {
                continue;
            };
            if resolved.contains_key(task) {
                continue;
            }
            let resolution = match tasks.get_item(task) {
                Ok(value) if value.is_callable() => TaskResolution::Bound(value.unbind()),
                Ok(_) => TaskResolution::NotCallable,
                Err(error) if error.is_instance_of::<pyo3::exceptions::PyKeyError>(py) => {
                    TaskResolution::Missing
                }
                Err(error) => return Err(error),
            };
            resolved.insert(task.clone(), resolution);
        }

        let mut bindings = HashMap::new();
        let mut binding_issues = Vec::new();
        for (index, node) in graph.nodes.iter().enumerate() {
            let NodeKind::Task { task, .. } = &node.kind else {
                continue;
            };
            match resolved.get(task).expect("every task name was resolved") {
                TaskResolution::Bound(value) => {
                    bindings.insert(index, value.clone_ref(py));
                }
                TaskResolution::NotCallable => binding_issues.push(IssueData::new(
                    "TASK_NOT_CALLABLE",
                    format!("/nodes/{index}/task"),
                    format!("registered task {task:?} is not callable"),
                )),
                TaskResolution::Missing => binding_issues.push(IssueData::new(
                    "TASK_MISSING",
                    format!("/nodes/{index}/task"),
                    format!("registered task {task:?} is missing"),
                )),
            }
        }
        if !binding_issues.is_empty() {
            return Err(configuration_error(py, Assessment::new(binding_issues))?);
        }
        Ok(Self {
            graph: Arc::new(graph),
            tasks: Arc::new(bindings),
            rng: seeded_rng(seed),
        })
    }

    fn new_run(&self, payload: &Bound<'_, PyAny>) -> PyResult<Execution> {
        let payload = from_python(payload, true).map_err(|error| input_py_error(error, true))?;
        Ok(Execution::new(
            Arc::clone(&self.graph),
            Arc::clone(&self.tasks),
            Arc::clone(&self.rng),
            payload,
        ))
    }
}

fn parse_seed(seed: Option<&Bound<'_, PyAny>>) -> PyResult<Option<u64>> {
    let Some(seed) = seed else { return Ok(None) };
    if seed.is_none() {
        return Ok(None);
    }
    if seed.is_instance_of::<PyBool>() || !seed.is_instance_of::<PyInt>() {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "seed must be a non-boolean unsigned 64-bit integer or None",
        ));
    }
    seed.extract::<u64>().map(Some).map_err(|_| {
        pyo3::exceptions::PyValueError::new_err("seed must be between 0 and 2**64 - 1")
    })
}
