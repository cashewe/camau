use camau_core::diagnostics::IssueData;
use camau_core::graph;
use camau_core::json_parser::parse_json;
use camau_core::json_value::JsonValue;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyString};

use crate::issue::Assessment;
use crate::python_value::{from_python, to_python};

#[pyclass(frozen, module = "camau")]
pub(crate) struct Assessor;

#[pymethods]
impl Assessor {
    #[staticmethod]
    fn assess(specification: &Bound<'_, PyAny>) -> Assessment {
        assess_input(specification).1
    }

    #[staticmethod]
    fn schema(py: Python<'_>) -> PyResult<Py<PyAny>> {
        let (schema, problems) = parse_json(include_str!("../../../spec/camau.schema.json"));
        if !problems.is_empty() {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(
                "packaged Camau schema is invalid",
            ));
        }
        to_python(&schema.unwrap(), py)
    }
}

pub(crate) fn assess_input(specification: &Bound<'_, PyAny>) -> (Option<JsonValue>, Assessment) {
    let (value, mut issues) = if let Ok(source) = specification.cast::<PyString>() {
        match source.to_str() {
            Ok(source) => {
                let (value, problems) = parse_json(source);
                (
                    value,
                    problems
                        .into_iter()
                        .map(|problem| IssueData::new(problem.code, problem.path, problem.message))
                        .collect(),
                )
            }
            Err(error) => (
                None,
                vec![IssueData::new(
                    "JSON_VALUE",
                    "",
                    format!("specification string contains invalid Unicode: {error}"),
                )],
            ),
        }
    } else if specification.is_instance_of::<PyDict>() {
        match from_python(specification, false) {
            Ok(value) => (Some(value), Vec::new()),
            Err(error) => (
                None,
                vec![IssueData::new("JSON_VALUE", error.path, error.message)],
            ),
        }
    } else {
        (
            None,
            vec![IssueData::new(
                "JSON_VALUE",
                "",
                "specification must be a dictionary or JSON string",
            )],
        )
    };

    if let Some(value) = &value {
        issues.extend(graph::assess(value));
    }
    let assessment = Assessment::new(issues);
    (value, assessment)
}
