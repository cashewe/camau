# Constraints

The words **must**, **must not**, **should**, and **may** are normative.

## Product boundaries

1. The public interface consists of Python objects and the Rust-backed `camau assess` and `camau diagram` commands. Router execution must be asynchronous; construction, assessment, schema access, report rendering, diagram rendering, and CLI execution are synchronous.
2. Runtime-sensitive routing, mapping, graph execution, and JSON ownership must be implemented in Rust and packaged with maturin.
3. The router must invoke gateway-supplied asynchronous callables. It must not own transport, authentication, retries, timeouts, coercion, connection pooling, or service discovery.
4. Router input, task input, task output, and router output must be JSON objects.
5. Routing specifications must be accepted as a Python dictionary or JSON string by the library. Only CLI commands may load a specification file; neither interface reloads configuration.
6. Configuration is trusted, version-controlled application configuration. It must not contain executable expressions, callbacks, credentials, or secrets.

## Graph and execution

1. A specification must describe a finite directed acyclic graph with one entry node and one successful output node.
2. V1 must provide task, fan-out, converge, deterministic-gate, randomised-gate, map-schema, and raise-error nodes.
3. All active paths created by fan-out must eventually converge unless execution terminates exceptionally.
4. Mutually exclusive gate paths may rejoin without convergence because only one path is active.
5. Cycles, missing references, identifier collisions, unreachable nodes, ambiguous outputs, orphaned successful paths, and inconsistent convergence declarations must be rejected before execution.
6. Fan-out branches must be scheduled without router-level throttling. Gateways and bound callables own concurrency and backpressure policy.
7. A single router must be re-entrant for overlapping runs on one asyncio event loop. The package must not queue, serialize, or throttle complete runs. Cross-thread and cross-event-loop use is not guaranteed.
8. JSON objects crossing task or branch boundaries must not share mutable Python state.
9. Caller cancellation or any in-run failure must cancel the entire execution. An unhandled task exception must be re-raised unchanged after siblings are cancelled.

## Configuration and validation

1. The editable routing surface must be explicit, human-readable JSON with visible forward links through `next` and explicit convergence inputs.
2. One packaged JSON Schema must define structural validity and be consumed by both assessment and router construction.
3. Semantic validation that JSON Schema cannot express must be performed by the assessor.
4. Assessment must report all detectable issues in deterministic order and render readable text, JUnit XML, and GitHub workflow annotations. Library renderers perform no file I/O; the CLI reads one named specification and writes its report to standard output.
5. The configuration format is governed by the package major version. Minor and patch releases must remain compatible with valid configurations from the same major version.
6. V1 must not support custom node types, but node implementations must declare their boundary and edge capabilities so new built-in types can be added without duplicating validation rules.

## Compatibility and delivery

1. Each release must test every CPython minor version from 3.11 through the highest stable CPython minor published at release time. The exact matrix must be recorded in CI and release notes.
2. V1 must provide maturin-built wheels for Linux x86-64, Linux ARM64, and Windows x86-64. PyPy, macOS, WebAssembly, and 32-bit platforms are outside the V1 commitment.
3. Wheels must target the CPython 3.11 `abi3` ABI. A non-`abi3` exception requires a documented incompatible dependency and explicit maintainer approval in the release notes; only tested Python versions are guaranteed.
4. Rust modules must use module-scoped errors implemented with `thiserror`; `anyhow` must not be used.
5. Python code must not add logging. Observability belongs to gateway and task code in V1.
6. Validation and execution must not use recursion proportional to user-controlled graph depth.
7. The package must impose no arbitrary graph, fan-out, or payload-size limits. Consumers own resource limits.
8. Build tooling and dependencies must use current, supported releases. Release CI must run `cargo audit` against RustSec and `pip-audit` against the Python Packaging Advisory Database. A known vulnerability requires documented impact, mitigation, and explicit maintainer approval before release.

## Performance objectives

Performance is measured and improved rather than enforced as a correctness limit. Benchmarks must document their runner, inputs, warm-up, and sampling method. Initial objectives, excluding downstream task latency, are:

- p95 routing overhead at or below 1 ms for a ten-node linear graph carrying 10 KiB objects;
- p95 framework overhead at or below 2 ms for ten no-op asynchronous branches followed by convergence, carrying 10 KiB objects; and
- no sustained memory growth across 100,000 repeated executions.

Benchmark regressions must be investigated and explained. Missing an objective requires the benchmark report to state the trade-off and record explicit maintainer approval before release.
