# Camau

Camau is a Rust-backed asynchronous JSON workflow router for Python.

## Work with a routing specification

Installing the package also installs the `camau` command:

```console
pip install camau
camau schema > camau.schema.json
camau assess route.json
camau assess route.json --format junit > camau.xml
camau assess route.json --format github
camau diagram route.json > route.md
```

`schema` writes the packaged structural JSON Schema to stdout. `assess` writes its report to stdout. `diagram` validates the specification, then writes a Markdown document containing a Mermaid workflow graph and a human-readable node table to stdout. Exit code 0 means the command succeeded, 1 means assessment found issues, and 2 means the command or file input failed.

Semantic graph rules remain the responsibility of `assess`; schema conformance alone is not sufficient.

See [the example routing specification](./tests/examples/gateway-routing-plan.json) and its [generated Markdown diagram](./tests/examples/gateway-routing-plan.md).
