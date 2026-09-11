use std::collections::HashSet;

use camau_core::json_parsing::{JsonValue, join_pointer};
use pyo3::IntoPyObjectExt;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString};

const MAX_CONTAINER_DEPTH: usize = 128;

#[derive(Debug, thiserror::Error)]
#[error("{message} at {path}")]
pub(crate) struct PythonValueError {
    pub(crate) path: String,
    pub(crate) message: String,
}

pub(crate) fn to_python(value: &JsonValue, py: Python<'_>) -> PyResult<Py<PyAny>> {
    match value {
        JsonValue::Null => Ok(py.None()),
        JsonValue::Bool(value) => value.into_py_any(py),
        JsonValue::Integer(value) => value.into_py_any(py),
        JsonValue::Number(value) => value.into_py_any(py),
        JsonValue::String(value) => value.into_py_any(py),
        JsonValue::Array(values) => {
            let list = PyList::empty(py);
            for value in values {
                list.append(to_python(value, py)?)?;
            }
            Ok(list.into_any().unbind())
        }
        JsonValue::Object(values) => {
            let dict = PyDict::new(py);
            for (key, value) in values {
                dict.set_item(key, to_python(value, py)?)?;
            }
            Ok(dict.into_any().unbind())
        }
    }
}

pub(crate) fn from_python(
    value: &Bound<'_, PyAny>,
    require_object: bool,
) -> Result<JsonValue, PythonValueError> {
    if require_object && !value.is_instance_of::<PyDict>() {
        return Err(invalid("", "expected a JSON object"));
    }
    from_python_at(value, "")
}

fn from_python_at(value: &Bound<'_, PyAny>, path: &str) -> Result<JsonValue, PythonValueError> {
    let mut work = vec![ConversionStep::Visit {
        value: value.clone(),
        path: path.to_owned(),
        depth: 0,
    }];
    let mut active = HashSet::new();
    let mut converted = Vec::new();

    while let Some(step) = work.pop() {
        match step {
            ConversionStep::Visit { value, path, depth } => {
                if value.is_none() {
                    converted.push(JsonValue::Null);
                } else if value.is_instance_of::<PyBool>() {
                    converted.push(
                        value
                            .extract::<bool>()
                            .map(JsonValue::Bool)
                            .map_err(|_| invalid(&path, "invalid boolean"))?,
                    );
                } else if value.is_instance_of::<PyInt>() {
                    converted.push(value.extract::<i64>().map(JsonValue::Integer).map_err(
                        |_| invalid(&path, "integer is outside the signed 64-bit range"),
                    )?);
                } else if value.is_instance_of::<PyFloat>() {
                    let number = value
                        .extract::<f64>()
                        .map_err(|_| invalid(&path, "invalid floating-point number"))?;
                    if !number.is_finite() {
                        return Err(invalid(&path, "floating-point numbers must be finite"));
                    }
                    converted.push(JsonValue::Number(number));
                } else if let Ok(text) = value.cast::<PyString>() {
                    converted.push(
                        text.to_str()
                            .map(|value| JsonValue::String(value.to_owned()))
                            .map_err(|_| invalid(&path, "string contains invalid Unicode"))?,
                    );
                } else if let Ok(list) = value.cast::<PyList>() {
                    let identity = value.as_ptr() as usize;
                    validate_container(identity, depth, &path, &mut active)?;
                    let items = list.iter().collect::<Vec<_>>();
                    work.push(ConversionStep::FinishList {
                        identity,
                        length: items.len(),
                    });
                    for (index, item) in items.into_iter().enumerate().rev() {
                        work.push(ConversionStep::Visit {
                            value: item,
                            path: format!("{path}/{index}"),
                            depth: depth + 1,
                        });
                    }
                } else if let Ok(dict) = value.cast::<PyDict>() {
                    let identity = value.as_ptr() as usize;
                    validate_container(identity, depth, &path, &mut active)?;
                    let mut items = Vec::with_capacity(dict.len());
                    for (key, item) in dict.iter() {
                        let key = key.cast::<PyString>().map_err(|_| {
                            invalid(&path, "object keys must be valid Unicode strings")
                        })?;
                        let key = key
                            .to_str()
                            .map_err(|_| {
                                invalid(&path, "object keys must be valid Unicode strings")
                            })?
                            .to_owned();
                        let item_path = join_pointer(&path, &key);
                        items.push((key, item, item_path));
                    }
                    work.push(ConversionStep::FinishObject {
                        identity,
                        keys: items.iter().map(|(key, _, _)| key.clone()).collect(),
                    });
                    for (_, item, item_path) in items.into_iter().rev() {
                        work.push(ConversionStep::Visit {
                            value: item,
                            path: item_path,
                            depth: depth + 1,
                        });
                    }
                } else {
                    return Err(invalid(&path, "value is outside the JSON data model"));
                }
            }
            ConversionStep::FinishList { identity, length } => {
                active.remove(&identity);
                let start = converted.len() - length;
                let values = converted.split_off(start);
                converted.push(JsonValue::Array(values));
            }
            ConversionStep::FinishObject { identity, keys } => {
                active.remove(&identity);
                let start = converted.len() - keys.len();
                let values = converted.split_off(start);
                converted.push(JsonValue::Object(keys.into_iter().zip(values).collect()));
            }
        }
    }

    Ok(converted.pop().expect("conversion produces one root value"))
}

enum ConversionStep<'py> {
    Visit {
        value: Bound<'py, PyAny>,
        path: String,
        depth: usize,
    },
    FinishList {
        identity: usize,
        length: usize,
    },
    FinishObject {
        identity: usize,
        keys: Vec<String>,
    },
}

fn validate_container(
    identity: usize,
    depth: usize,
    path: &str,
    active: &mut HashSet<usize>,
) -> Result<(), PythonValueError> {
    if active.contains(&identity) {
        return Err(invalid(path, "container contains a reference cycle"));
    }
    if depth >= MAX_CONTAINER_DEPTH {
        return Err(invalid(
            path,
            "container nesting exceeds the maximum depth of 128",
        ));
    }
    active.insert(identity);
    Ok(())
}

pub(crate) fn input_py_error(error: PythonValueError, object_required: bool) -> PyErr {
    if object_required && error.path.is_empty() && error.message == "expected a JSON object" {
        PyTypeError::new_err(error.message)
    } else {
        PyValueError::new_err(if error.path.is_empty() {
            error.message
        } else {
            format!("{} at {}", error.message, error.path)
        })
    }
}

fn invalid(path: &str, message: &str) -> PythonValueError {
    PythonValueError {
        path: path.to_owned(),
        message: message.to_owned(),
    }
}
