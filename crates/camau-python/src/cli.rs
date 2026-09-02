use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use camau_core::assessment::assess_json;
use clap::{Parser, Subcommand, ValueEnum};
use pyo3::prelude::*;

const INVALID: u8 = 1;
const ERROR: u8 = 2;

#[derive(Parser)]
#[command(name = "camau", version, about = "Assess Camau routing specifications")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Assess one routing specification.
    Assess {
        /// JSON routing specification to assess.
        specification: PathBuf,

        /// Report format written to stdout.
        #[arg(short, long, value_enum, default_value_t = ReportFormat::Text)]
        format: ReportFormat,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum ReportFormat {
    Text,
    Junit,
    Github,
}

#[pyfunction]
pub(crate) fn main(py: Python<'_>) -> PyResult<u8> {
    let arguments = py
        .import("sys")?
        .getattr("argv")?
        .extract::<Vec<String>>()?;
    Ok(run_from(
        arguments,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    ))
}

fn run_from<I, T>(arguments: I, stdout: &mut impl Write, stderr: &mut impl Write) -> u8
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match Cli::try_parse_from(arguments) {
        Ok(cli) => cli,
        Err(error) => {
            let exit_code = error.exit_code() as u8;
            let _ = if error.use_stderr() {
                write!(stderr, "{error}")
            } else {
                write!(stdout, "{error}")
            };
            return exit_code;
        }
    };

    match cli.command {
        Command::Assess {
            specification,
            format,
        } => assess(&specification, format, stdout, stderr),
    }
}

fn assess(
    specification: &Path,
    format: ReportFormat,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    let source = match fs::read_to_string(specification) {
        Ok(source) => source,
        Err(error) => {
            let _ = writeln!(
                stderr,
                "camau: failed to read '{}': {error}",
                specification.display()
            );
            return ERROR;
        }
    };
    let (_, assessment) = assess_json(&source);
    let report = match format {
        ReportFormat::Text => assessment.to_text(),
        ReportFormat::Junit => assessment.to_junit_xml(),
        ReportFormat::Github => {
            assessment.to_github_annotations(Some(&specification.to_string_lossy()))
        }
    };

    if write_report(stdout, &report).is_err() {
        let _ = writeln!(stderr, "camau: failed to write report");
        return ERROR;
    }

    if assessment.valid() { 0 } else { INVALID }
}

fn write_report(output: &mut impl Write, report: &str) -> io::Result<()> {
    output.write_all(report.as_bytes())?;
    if !report.is_empty() && !report.ends_with('\n') {
        output.write_all(b"\n")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_specification_uses_requested_report_and_failure_exit_code() {
        let path = temporary_specification("{\"entry\":\"x\",\"entry\":\"y\"}");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let exit_code = run_from(
            [
                OsString::from("camau"),
                OsString::from("assess"),
                path.clone().into_os_string(),
                OsString::from("--format"),
                OsString::from("junit"),
            ],
            &mut stdout,
            &mut stderr,
        );

        fs::remove_file(path).unwrap();
        assert_eq!(exit_code, INVALID);
        assert!(stderr.is_empty());
        assert!(String::from_utf8(stdout).unwrap().contains("<testsuite"));
    }

    #[test]
    fn missing_specification_is_an_operational_error() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let exit_code = run_from(
            ["camau", "assess", "this-file-does-not-exist.json"],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(exit_code, ERROR);
        assert!(stdout.is_empty());
        assert!(
            String::from_utf8(stderr)
                .unwrap()
                .contains("failed to read")
        );
    }

    fn temporary_specification(contents: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "camau-cli-test-{}-{:?}.json",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::write(&path, contents).unwrap();
        path
    }
}
