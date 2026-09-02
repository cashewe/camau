mod assessment;
mod cli;
mod errors;
mod executor;
mod issue;
mod python_value;
mod router;

use pyo3::prelude::*;

#[pymodule]
fn _camau(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(cli::main, module)?)?;
    module.add_class::<assessment::Assessor>()?;
    module.add_class::<issue::Assessment>()?;
    module.add_class::<issue::Issue>()?;
    module.add_class::<router::Router>()?;
    module.add_class::<executor::Execution>()?;
    errors::register(module)?;
    Ok(())
}
