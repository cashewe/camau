use camau_core::diagnostics::{AssessmentData, IssueData};
use pyo3::prelude::*;
use pyo3::types::PyTuple;

#[pyclass(frozen, module = "camau", skip_from_py_object)]
#[derive(Clone)]
pub(crate) struct Issue {
    data: IssueData,
}

#[pymethods]
impl Issue {
    #[getter]
    fn code(&self) -> &str {
        &self.data.code
    }

    #[getter]
    fn message(&self) -> &str {
        &self.data.message
    }

    #[getter]
    fn path(&self) -> &str {
        &self.data.path
    }

    fn __repr__(&self) -> String {
        format!(
            "Issue(code={:?}, message={:?}, path={:?})",
            self.data.code, self.data.message, self.data.path
        )
    }
}

#[pyclass(frozen, module = "camau", skip_from_py_object)]
#[derive(Clone)]
pub(crate) struct Assessment {
    pub(crate) data: AssessmentData,
}

impl Assessment {
    pub(crate) fn new(issues: Vec<IssueData>) -> Self {
        Self {
            data: AssessmentData::new(issues),
        }
    }

    pub(crate) fn from_data(data: AssessmentData) -> Self {
        Self { data }
    }
}

#[pymethods]
impl Assessment {
    #[getter]
    fn valid(&self) -> bool {
        self.data.valid()
    }

    #[getter]
    fn issues(&self, py: Python<'_>) -> PyResult<Py<PyTuple>> {
        let values = self
            .data
            .issues
            .iter()
            .cloned()
            .map(|data| Py::new(py, Issue { data }))
            .collect::<PyResult<Vec<_>>>()?;
        Ok(PyTuple::new(py, values)?.unbind())
    }

    fn to_text(&self) -> String {
        self.data.to_text()
    }

    fn to_junit_xml(&self) -> String {
        self.data.to_junit_xml()
    }

    #[pyo3(signature = (source=None))]
    fn to_github_annotations(&self, source: Option<&str>) -> String {
        self.data.to_github_annotations(source)
    }

    fn __repr__(&self) -> String {
        format!(
            "Assessment(valid={}, issues={})",
            self.data.valid(),
            self.data.issues.len()
        )
    }
}
