from __future__ import annotations

import argparse
import hashlib
import os
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import urllib.error
import urllib.request
import venv
import zipfile
from collections.abc import Callable, Sequence
from email.message import Message
from email.parser import Parser
from pathlib import Path

PROJECT = "camau"
REPOSITORY_URL = "https://github.com/cashewe/camau"
PROJECT_URLS = {
    "Homepage": REPOSITORY_URL,
    "Repository": REPOSITORY_URL,
    "Issues": f"{REPOSITORY_URL}/issues",
    "Documentation": f"{REPOSITORY_URL}#readme",
}
PLATFORM_SUFFIXES = {
    "linux-x64": "manylinux_2_28_x86_64.whl",
    "linux-arm64": "manylinux_2_28_aarch64.whl",
    "windows-x64": "win_amd64.whl",
}
REQUIRED_PACKAGE_FILES = {
    "camau/__init__.py",
    "camau/router.py",
    "camau/types.py",
    "camau/_camau.pyi",
    "camau/py.typed",
}
SDIST_SCHEMA = "crates/camau-python/schema/camau.schema.json"
COPYRIGHT_LINE = "Copyright (c) 2026 John C Ll Stokes"


class ReleaseValidationError(RuntimeError):
    pass


def _load_toml(path: Path) -> dict[str, object]:
    with path.open("rb") as source:
        return tomllib.load(source)


def _project_version(root: Path) -> str:
    project = _load_toml(root / "pyproject.toml")["project"]
    assert isinstance(project, dict)
    version = project["version"]
    assert isinstance(version, str)
    return version


def _workspace_versions(root: Path) -> dict[str, str]:
    manifest = _load_toml(root / "Cargo.toml")
    workspace = manifest["workspace"]
    assert isinstance(workspace, dict)
    workspace_package = workspace["package"]
    assert isinstance(workspace_package, dict)
    workspace_version = workspace_package["version"]
    assert isinstance(workspace_version, str)
    members = workspace["members"]
    assert isinstance(members, list)

    versions: dict[str, str] = {}
    for member in members:
        assert isinstance(member, str)
        member_manifest = _load_toml(root / member / "Cargo.toml")
        package = member_manifest["package"]
        assert isinstance(package, dict)
        name = package["name"]
        assert isinstance(name, str)
        declared = package["version"]
        if isinstance(declared, dict) and declared.get("workspace") is True:
            versions[name] = workspace_version
        elif isinstance(declared, str):
            versions[name] = declared
        else:
            raise ReleaseValidationError(f"{name} has no resolvable Cargo package version")
    return versions


def validate_source(root: Path, tag: str | None) -> str:
    version = _project_version(root)
    mismatches = {
        name: crate_version
        for name, crate_version in _workspace_versions(root).items()
        if crate_version != version
    }
    if mismatches:
        details = ", ".join(f"{name}={value}" for name, value in sorted(mismatches.items()))
        raise ReleaseValidationError(f"Python version {version} does not match Cargo: {details}")

    if tag:
        expected_tag = f"v{version}"
        if tag != expected_tag:
            raise ReleaseValidationError(
                f"release tag {tag!r} does not match package version {version!r}; "
                f"expected {expected_tag!r}"
            )
    return version


def prepare_directory(directory: Path) -> None:
    if directory.exists():
        raise ReleaseValidationError(f"release directory already exists: {directory}")
    directory.mkdir(parents=True)


def _only_artifact(directory: Path, suffix: str) -> Path:
    if not directory.is_dir():
        raise ReleaseValidationError(f"artifact directory does not exist: {directory}")
    entries = list(directory.iterdir())
    matching = [entry for entry in entries if entry.is_file() and entry.name.endswith(suffix)]
    if len(entries) != 1 or len(matching) != 1:
        names = ", ".join(sorted(entry.name for entry in entries)) or "<empty>"
        raise ReleaseValidationError(
            f"expected exactly one {suffix} artifact in {directory}; found: {names}"
        )
    return matching[0]


def _metadata_from_text(raw: str) -> Message:
    return Parser().parsestr(raw)


def _validate_metadata(metadata: Message, version: str) -> None:
    if metadata["Name"] != PROJECT:
        raise ReleaseValidationError(f"metadata Name is {metadata['Name']!r}, expected {PROJECT!r}")
    if metadata["Version"] != version:
        raise ReleaseValidationError(
            f"metadata Version is {metadata['Version']!r}, expected {version!r}"
        )
    if metadata["License-Expression"] != "MIT":
        raise ReleaseValidationError("metadata does not declare License-Expression: MIT")

    found_urls: dict[str, str] = {}
    for value in metadata.get_all("Project-URL", []):
        label, separator, url = value.partition(",")
        if separator:
            found_urls[label.strip()] = url.strip()
    if found_urls != PROJECT_URLS:
        raise ReleaseValidationError(
            f"metadata project URLs are {found_urls!r}, expected {PROJECT_URLS!r}"
        )


def _validate_license(raw: bytes) -> None:
    text = raw.decode("utf-8")
    lines = text.splitlines()
    if not lines or lines[0] != "MIT License" or COPYRIGHT_LINE not in lines:
        raise ReleaseValidationError("artifact does not contain the canonical Camau MIT license")


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as artifact:
        for chunk in iter(lambda: artifact.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _record_artifact(path: Path) -> None:
    line = f"SHA256 {path.name} {_sha256(path)}"
    print(line)
    summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary_path:
        with Path(summary_path).open("a", encoding="utf-8") as summary:
            summary.write(f"- `{line}`\n")


def validate_wheel(path: Path, version: str, platform: str | None = None) -> None:
    expected_prefix = f"{PROJECT}-{version}-cp311-abi3-"
    if not path.name.startswith(expected_prefix):
        raise ReleaseValidationError(
            f"wheel {path.name!r} does not use expected version and cp311 abi3 tag"
        )
    if platform and not path.name.endswith(PLATFORM_SUFFIXES[platform]):
        raise ReleaseValidationError(
            f"wheel {path.name!r} does not match the {platform} release platform"
        )

    with zipfile.ZipFile(path) as wheel:
        names = set(wheel.namelist())
        missing = REQUIRED_PACKAGE_FILES - names
        if missing:
            raise ReleaseValidationError(f"wheel is missing package files: {sorted(missing)!r}")
        native_extensions = [
            name
            for name in names
            if name.startswith("camau/_camau.") and name.endswith((".so", ".pyd"))
        ]
        if len(native_extensions) != 1:
            raise ReleaseValidationError(
                f"wheel must contain exactly one native extension; found {native_extensions!r}"
            )
        licenses = [name for name in names if name.endswith(".dist-info/licenses/LICENSE")]
        if len(licenses) != 1:
            raise ReleaseValidationError(
                f"wheel must contain exactly one dist-info license; found {licenses!r}"
            )
        _validate_license(wheel.read(licenses[0]))
        metadata_files = [name for name in names if name.endswith(".dist-info/METADATA")]
        if len(metadata_files) != 1:
            raise ReleaseValidationError(
                f"wheel must contain exactly one METADATA file; found {metadata_files!r}"
            )
        metadata = _metadata_from_text(wheel.read(metadata_files[0]).decode("utf-8"))
        _validate_metadata(metadata, version)


SMOKE_CODE = """
import asyncio
import os
from pathlib import Path

import camau
from camau import Assessor, Router
from camau import router, types

repository = Path(os.environ["CAMAU_REPOSITORY"]).resolve()
installed = Path(camau.__file__).resolve()
assert not installed.is_relative_to(repository), (installed, repository)
assert Path(router.__file__).name == "router.py"
assert Path(types.__file__).name == "types.py"
assert (installed.parent / "_camau.pyi").is_file()

specification = {
    "entry": "call",
    "output": "call",
    "nodes": [{"id": "call", "type": "task", "task": "echo"}],
}
assert Assessor.assess(specification).valid
assert Assessor.schema()["title"] == "Camau routing specification"

async def echo(payload):
    return {**payload, "verified": True}

assert asyncio.run(Router(specification, {"echo": echo}).run({"value": 1})) == {
    "value": 1,
    "verified": True,
}
"""


def install_and_smoke_wheel(path: Path, repository_root: Path) -> None:
    with tempfile.TemporaryDirectory(prefix="camau-wheel-") as temporary:
        root = Path(temporary)
        environment = root / "venv"
        venv.EnvBuilder(with_pip=True, clear=True).create(environment)
        executable = environment / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
        subprocess.run(
            [
                str(executable),
                "-I",
                "-m",
                "pip",
                "install",
                "--no-deps",
                "--no-index",
                str(path.resolve()),
            ],
            cwd=root,
            check=True,
        )
        environment_variables = os.environ.copy()
        environment_variables.pop("PYTHONPATH", None)
        environment_variables["CAMAU_REPOSITORY"] = str(repository_root.resolve())
        subprocess.run(
            [str(executable), "-I", "-c", SMOKE_CODE],
            cwd=root,
            env=environment_variables,
            check=True,
        )


def verify_wheel(
    directory: Path, version: str, platform: str | None, repository_root: Path
) -> Path:
    wheel = _only_artifact(directory, ".whl")
    validate_wheel(wheel, version, platform)
    install_and_smoke_wheel(wheel, repository_root)
    _record_artifact(wheel)
    return wheel


def _safe_extract(archive: tarfile.TarFile, destination: Path) -> None:
    resolved_destination = destination.resolve()
    for member in archive.getmembers():
        target = (destination / member.name).resolve()
        if not target.is_relative_to(resolved_destination) or member.issym() or member.islnk():
            raise ReleaseValidationError(f"sdist contains unsafe member {member.name!r}")
    archive.extractall(destination, filter="data")


def validate_sdist(path: Path, version: str) -> str:
    expected_name = f"{PROJECT}-{version}.tar.gz"
    if path.name != expected_name:
        raise ReleaseValidationError(f"sdist is {path.name!r}, expected {expected_name!r}")
    expected_root = f"{PROJECT}-{version}"
    with tarfile.open(path, "r:gz") as archive:
        names = set(archive.getnames())
        required = {
            f"{expected_root}/Cargo.lock",
            f"{expected_root}/LICENSE",
            f"{expected_root}/PKG-INFO",
            f"{expected_root}/pyproject.toml",
            f"{expected_root}/{SDIST_SCHEMA}",
        }
        missing = required - names
        if missing:
            raise ReleaseValidationError(f"sdist is missing files: {sorted(missing)!r}")
        schema_files = [name for name in names if name.endswith("/camau.schema.json")]
        if schema_files != [f"{expected_root}/{SDIST_SCHEMA}"]:
            raise ReleaseValidationError(
                f"sdist must contain the embedded schema exactly once; found {schema_files!r}"
            )
        package_info = archive.extractfile(f"{expected_root}/PKG-INFO")
        assert package_info is not None
        metadata = _metadata_from_text(package_info.read().decode("utf-8"))
        _validate_metadata(metadata, version)
        license_file = archive.extractfile(f"{expected_root}/LICENSE")
        assert license_file is not None
        _validate_license(license_file.read())
    return expected_root


def verify_sdist(directory: Path, version: str, extract_to: Path) -> Path:
    sdist = _only_artifact(directory, ".tar.gz")
    expected_root = validate_sdist(sdist, version)
    if extract_to.exists():
        raise ReleaseValidationError(f"sdist extraction directory already exists: {extract_to}")
    extract_to.mkdir(parents=True)
    with tarfile.open(sdist, "r:gz") as archive:
        _safe_extract(archive, extract_to)
    if not (extract_to / expected_root).is_dir():
        raise ReleaseValidationError("sdist did not extract to its expected root directory")
    _record_artifact(sdist)
    return sdist


def expected_artifact_names(version: str) -> set[str]:
    wheels = {f"{PROJECT}-{version}-cp311-abi3-{suffix}" for suffix in PLATFORM_SUFFIXES.values()}
    return wheels | {f"{PROJECT}-{version}.tar.gz"}


def validate_artifact_names(directory: Path, version: str) -> list[Path]:
    if not directory.is_dir():
        raise ReleaseValidationError(f"artifact directory does not exist: {directory}")
    entries = list(directory.iterdir())
    if any(not entry.is_file() for entry in entries):
        raise ReleaseValidationError("release directory contains nested or non-file entries")
    names = {entry.name for entry in entries}
    expected = expected_artifact_names(version)
    if names != expected or len(entries) != len(expected):
        missing = sorted(expected - names)
        unexpected = sorted(names - expected)
        raise ReleaseValidationError(
            f"release artifact set is incomplete or stale; missing={missing!r}, "
            f"unexpected={unexpected!r}"
        )
    return sorted(entries)


def version_exists_on_pypi(version: str) -> bool:
    request = urllib.request.Request(
        f"https://pypi.org/pypi/{PROJECT}/{version}/json",
        headers={"Accept": "application/json", "User-Agent": "camau-release-guard"},
    )
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            return response.status == 200
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return False
        raise ReleaseValidationError(f"PyPI version check failed with HTTP {error.code}") from error
    except urllib.error.URLError as error:
        raise ReleaseValidationError(f"PyPI version check failed: {error.reason}") from error


def ensure_version_unpublished(
    version: str, lookup: Callable[[str], bool] = version_exists_on_pypi
) -> None:
    if lookup(version):
        raise ReleaseValidationError(
            f"refusing to publish: {PROJECT} {version} already exists on PyPI"
        )


def validate_artifacts(directory: Path, version: str, check_pypi: bool) -> None:
    artifacts = validate_artifact_names(directory, version)
    for artifact in artifacts:
        if artifact.suffix == ".whl":
            validate_wheel(artifact, version)
        else:
            validate_sdist(artifact, version)
        _record_artifact(artifact)
    if check_pypi:
        ensure_version_unpublished(version)
        print(f"PyPI does not contain {PROJECT} {version}; publication may proceed")


def _write_github_output(path: Path | None, version: str) -> None:
    if path:
        with path.open("a", encoding="utf-8") as output:
            output.write(f"version={version}\n")


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Validate Camau release inputs and artifacts")
    subparsers = parser.add_subparsers(dest="command", required=True)

    source = subparsers.add_parser("source")
    source.add_argument("--root", type=Path, default=Path.cwd())
    source.add_argument("--tag")
    source.add_argument("--github-output", type=Path)

    prepare = subparsers.add_parser("prepare")
    prepare.add_argument("--directory", type=Path, required=True)

    wheel = subparsers.add_parser("wheel")
    wheel.add_argument("--directory", type=Path, required=True)
    wheel.add_argument("--version", required=True)
    wheel.add_argument("--platform", choices=sorted(PLATFORM_SUFFIXES))
    wheel.add_argument("--repository-root", type=Path, default=Path.cwd())

    sdist = subparsers.add_parser("sdist")
    sdist.add_argument("--directory", type=Path, required=True)
    sdist.add_argument("--version", required=True)
    sdist.add_argument("--extract-to", type=Path, required=True)

    artifacts = subparsers.add_parser("artifacts")
    artifacts.add_argument("--directory", type=Path, required=True)
    artifacts.add_argument("--version", required=True)
    artifacts.add_argument("--check-pypi", action="store_true")
    return parser


def main(arguments: Sequence[str] | None = None) -> int:
    options = _parser().parse_args(arguments)
    try:
        if options.command == "source":
            version = validate_source(options.root, options.tag or None)
            _write_github_output(options.github_output, version)
            print(f"source metadata agrees on version {version}")
        elif options.command == "prepare":
            prepare_directory(options.directory)
        elif options.command == "wheel":
            verify_wheel(
                options.directory,
                options.version,
                options.platform,
                options.repository_root,
            )
        elif options.command == "sdist":
            verify_sdist(options.directory, options.version, options.extract_to)
        elif options.command == "artifacts":
            validate_artifacts(options.directory, options.version, options.check_pypi)
    except (
        AssertionError,
        KeyError,
        OSError,
        ReleaseValidationError,
        subprocess.CalledProcessError,
    ) as error:
        print(f"release validation failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
