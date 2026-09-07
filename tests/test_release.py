from pathlib import Path

import pytest

from tools.release import (
    ReleaseValidationError,
    ensure_version_unpublished,
    expected_artifact_names,
    validate_artifact_names,
    validate_source,
)


def write_source_metadata(root: Path, python_version: str, rust_version: str) -> None:
    (root / "crates" / "core").mkdir(parents=True)
    (root / "pyproject.toml").write_text(
        f'[project]\nname = "camau"\nversion = "{python_version}"\n', encoding="utf-8"
    )
    (root / "Cargo.toml").write_text(
        f"""[workspace]
members = ["crates/core"]

[workspace.package]
version = "{rust_version}"
""",
        encoding="utf-8",
    )
    (root / "crates" / "core" / "Cargo.toml").write_text(
        """[package]
name = "core"
version.workspace = true
""",
        encoding="utf-8",
    )


def create_named_artifacts(directory: Path, version: str) -> None:
    directory.mkdir()
    for name in expected_artifact_names(version):
        (directory / name).touch()


def test_source_rejects_tag_version_mismatch(tmp_path: Path) -> None:
    write_source_metadata(tmp_path, "1.2.3", "1.2.3")

    with pytest.raises(ReleaseValidationError, match="release tag"):
        validate_source(tmp_path, "v1.2.4")


def test_source_rejects_cargo_version_mismatch(tmp_path: Path) -> None:
    write_source_metadata(tmp_path, "1.2.3", "1.2.4")

    with pytest.raises(ReleaseValidationError, match="does not match Cargo"):
        validate_source(tmp_path, "v1.2.3")


def test_artifact_set_rejects_stale_file(tmp_path: Path) -> None:
    create_named_artifacts(tmp_path / "release", "1.2.3")
    (tmp_path / "release" / "old.whl").touch()

    with pytest.raises(ReleaseValidationError, match="unexpected=.*old.whl"):
        validate_artifact_names(tmp_path / "release", "1.2.3")


def test_artifact_set_rejects_partial_matrix(tmp_path: Path) -> None:
    directory = tmp_path / "release"
    create_named_artifacts(directory, "1.2.3")
    (directory / "camau-1.2.3-cp311-abi3-win_amd64.whl").unlink()

    with pytest.raises(ReleaseValidationError, match="missing=.*win_amd64"):
        validate_artifact_names(directory, "1.2.3")


def test_already_published_version_fails_closed() -> None:
    with pytest.raises(ReleaseValidationError, match="already exists on PyPI"):
        ensure_version_unpublished("1.2.3", lookup=lambda _: True)


def test_unpublished_version_can_proceed() -> None:
    ensure_version_unpublished("1.2.3", lookup=lambda _: False)
