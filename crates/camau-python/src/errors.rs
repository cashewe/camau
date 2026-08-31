use camau_core::execution::{ExecutionFailure, MappingFailure, RoutingFailure};
use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyRuntimeError};
use pyo3::prelude::*;

use crate::issue::Assessment;
use crate::python_value::to_python;

create_exception!(_camau, CamauError, PyException);
create_exception!(_camau, ConfigurationError, CamauError);
create_exception!(_camau, MappingError, CamauError);
create_exception!(_camau, TaskResultError, CamauError);
create_exception!(_camau, RoutingSelectionError, CamauError);

pub(crate) fn configuration_error(py: Python<'_>, assessment: Assessment) -> PyResult<PyErr> {
    let error = PyErr::new::<ConfigurationError, _>("routing specification is invalid");
    error
        .value(py)
        .setattr("assessment", Py::new(py, assessment)?)?;
    Ok(error)
}

pub(crate) fn task_result_error(
    py: Python<'_>,
    node_id: &str,
    task_name: &str,
    reason: &str,
    invalid_value: Option<Py<PyAny>>,
) -> PyErr {
    let error = PyErr::new::<TaskResultError, _>(format!(
        "task {task_name:?} at node {node_id:?} returned an invalid result ({reason})"
    ));
    let value = error.value(py);
    let _ = value.setattr("node_id", node_id);
    let _ = value.setattr("task_name", task_name);
    let _ = value.setattr("reason", reason);
    let _ = value.setattr("invalid_value", invalid_value.unwrap_or_else(|| py.None()));
    error
}

pub(crate) fn execution_error(py: Python<'_>, failure: ExecutionFailure) -> PyErr {
    match failure {
        ExecutionFailure::Mapping(failure) => mapping_error(py, &failure),
        ExecutionFailure::Routing(failure) => routing_selection_error(py, &failure),
        ExecutionFailure::AlreadyStarted | ExecutionFailure::UnknownTicket => {
            PyRuntimeError::new_err(failure.to_string())
        }
    }
}

fn mapping_error(py: Python<'_>, failure: &MappingFailure) -> PyErr {
    let error = PyErr::new::<MappingError, _>(failure.to_string());
    let value = error.value(py);
    let _ = value.setattr("node_id", &failure.node_id);
    let _ = value.setattr("target", &failure.target);
    let _ = value.setattr("source", failure.source_path.as_deref());
    let _ = value.setattr("expected_type", failure.expected_type);
    let _ = value.setattr("reason", failure.reason);
    error
}

fn routing_selection_error(py: Python<'_>, failure: &RoutingFailure) -> PyErr {
    let error = PyErr::new::<RoutingSelectionError, _>(failure.to_string());
    let value = error.value(py);
    let gate = failure.gate.as_ref();
    let _ = value.setattr("code", "GWALL");
    let _ = value.setattr("node_id", &failure.node_id);
    let _ = value.setattr("message", &failure.message);
    let _ = value.setattr(
        "gate_node_id",
        gate.map(|context| context.gate_node_id.as_str()),
    );
    let _ = value.setattr(
        "selected_path",
        gate.and_then(|context| context.selected_path.as_deref()),
    );
    let _ = value.setattr("value_found", gate.and_then(|context| context.value_found));
    let selected = gate
        .and_then(|context| context.selected_value.as_ref())
        .and_then(|selected| to_python(selected, py).ok())
        .unwrap_or_else(|| py.None());
    let _ = value.setattr("selected_value", selected);
    let _ = value.setattr(
        "matched_operator",
        gate.and_then(|context| context.matched_operator.as_deref()),
    );
    error
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("CamauError", module.py().get_type::<CamauError>())?;
    module.add(
        "ConfigurationError",
        module.py().get_type::<ConfigurationError>(),
    )?;
    module.add("MappingError", module.py().get_type::<MappingError>())?;
    module.add("TaskResultError", module.py().get_type::<TaskResultError>())?;
    module.add(
        "RoutingSelectionError",
        module.py().get_type::<RoutingSelectionError>(),
    )?;
    Ok(())
}
