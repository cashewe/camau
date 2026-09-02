import shutil
import subprocess
import sysconfig


def run_camau(*arguments: str) -> subprocess.CompletedProcess[str]:
    executable = shutil.which("camau", path=sysconfig.get_path("scripts"))
    assert executable is not None
    return subprocess.run(
        [executable, *arguments],
        check=False,
        capture_output=True,
        text=True,
    )


def test_cli_reports_invalid_specification_in_ci_formats(tmp_path):
    specification = tmp_path / "route.json"
    specification.write_text('{"entry":"x","entry":"y"}', encoding="utf-8")

    junit = run_camau("assess", str(specification), "--format", "junit")
    github = run_camau("assess", str(specification), "--format", "github")

    assert junit.returncode == 1
    assert "<testsuite" in junit.stdout
    assert junit.stderr == ""
    assert github.returncode == 1
    assert "::error" in github.stdout
    assert "file=" in github.stdout
    assert github.stderr == ""


def test_cli_distinguishes_valid_input_from_file_errors(tmp_path):
    specification = tmp_path / "route.json"
    specification.write_text(
        '{"entry":"only","output":"only","nodes":[{"id":"only","type":"task","task":"run"}]}',
        encoding="utf-8",
    )

    valid = run_camau("assess", str(specification))
    missing = run_camau("assess", str(tmp_path / "missing.json"))

    assert valid.returncode == 0
    assert valid.stdout == "Camau assessment valid\n"
    assert valid.stderr == ""
    assert missing.returncode == 2
    assert missing.stdout == ""
    assert "failed to read" in missing.stderr
