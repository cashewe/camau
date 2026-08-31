use camau_core::json_pointer::join_pointer;
use camau_core::json_value::JsonValue;
use indexmap::IndexMap;
use pyo3::IntoPyObjectExt;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString};

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
    if value.is_none() {
        return Ok(JsonValue::Null);
    }
    if value.is_instance_of::<PyBool>() {
        return value
            .extract::<bool>()
            .map(JsonValue::Bool)
            .map_err(|_| invalid(path, "invalid boolean"));
    }
    if value.is_instance_of::<PyInt>() {
        return value
            .extract::<i64>()
            .map(JsonValue::Integer)
            .map_err(|_| invalid(path, "integer is outside the signed 64-bit range"));
    }
    if value.is_instance_of::<PyFloat>() {
        let number = value
            .extract::<f64>()
            .map_err(|_| invalid(path, "invalid floating-point number"))?;
        return if number.is_finite() {
            Ok(JsonValue::Number(number))
        } else {
            Err(invalid(path, "floating-point numbers must be finite"))
        };
    }
    if let Ok(text) = value.cast::<PyString>() {
        return text
            .to_str()
            .map(|value| JsonValue::String(value.to_owned()))
            .map_err(|_| invalid(path, "string contains invalid Unicode"));
    }
    if let Ok(list) = value.cast::<PyList>() {
        let mut values = Vec::with_capacity(list.len());
        for (index, item) in list.iter().enumerate() {
            values.push(from_python_at(&item, &format!("{path}/{index}"))?);
        }
        return Ok(JsonValue::Array(values));
    }
    if let Ok(dict) = value.cast::<PyDict>() {
        let mut values = IndexMap::with_capacity(dict.len());
        for (key, item) in dict.iter() {
            let key = key
                .cast::<PyString>()
                .map_err(|_| invalid(path, "object keys must be valid Unicode strings"))?;
            let key = key
                .to_str()
                .map_err(|_| invalid(path, "object keys must be valid Unicode strings"))?;
            let item_path = join_pointer(path, key);
            values.insert(key.to_owned(), from_python_at(&item, &item_path)?);
        }
        return Ok(JsonValue::Object(values));
    }
    Err(invalid(path, "value is outside the JSON data model"))
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
