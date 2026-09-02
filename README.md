# Camau

Camau is a Rust-backed asynchronous JSON workflow router for Python.

## Assess a routing specification

Installing the package also installs the `camau` command:

```console
pip install camau
camau assess route.json
camau assess route.json --format junit > camau.xml
camau assess route.json --format github
```

The report is written to stdout. Exit code 0 means the specification is valid, 1 means assessment found issues, and 2 means the command or file input failed.
