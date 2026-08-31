# V1 acceptance

V1 is complete when every requirement below is met. The two multi-stage workflows replace the earlier illustrative use cases as end-to-end proof; the examples are not separate product features.

## Package and compatibility

- The package builds with cargo and maturin and exposes only Python public interfaces.
- Runtime-sensitive graph assessment, compilation, routing, mapping, JSON ownership, and scheduling are implemented in Rust.
- Rust module errors use `thiserror`; the dependency graph contains no `anyhow`.
- Built wheels install and import on every tested CPython and platform combination in [CONSTRAINTS.md](./CONSTRAINTS.md).
- CI and release notes state the exact CPython/platform matrix and any maintainer-approved `abi3` exception.
- `cargo audit` and `pip-audit` pass, or release notes record the impact, mitigation, and maintainer approval for every accepted advisory.
- The repository contains no Python logging added by the package.

## Public API

- `Router`, `Assessor`, `Assessment`, `Issue`, and the documented exception hierarchy match [PYTHON-API.md](./PYTHON-API.md).
- Router construction accepts a dictionary or JSON string, performs no task calls, and fails with the complete assessment when configuration or task binding is invalid.
- Referenced missing and non-callable tasks produce `TASK_MISSING` and `TASK_NOT_CALLABLE`; invalid unused registry values are ignored.
- A router is immutable after construction and overlapping runs on one event loop do not share execution state.
- Mutating the source specification or registry mapping after construction does not change the router; mutable internal state owned by a bound callable is outside this guarantee.
- `run` accepts and returns JSON objects, never mutates its input, and returns no route metadata.
- The standalone assessor performs no file, task, or network access.

## Structural and semantic assessment

- The packaged [camau.schema.json](./camau.schema.json) accepts every structurally valid node form and rejects unknown properties or malformed forms.
- `Assessor.schema()` returns that same schema as a fresh Python dictionary.
- The assessor reports all independently detectable structural and semantic issues in deterministic path/code order.
- Every code and cascade-suppression rule in [DIAGNOSTICS.md](./DIAGNOSTICS.md) has a fixture.
- Text, JUnit XML, and GitHub annotation renderers produce valid escaped output and perform no file I/O.
- Fixtures cover duplicate keys in JSON strings, invalid JSON values, identifier collisions, missing references, cycles, unreachable nodes, ambiguous outputs, orphaned branches, invalid joins, mapping conflicts, gate overlap, and invalid random weights.

## Node behaviour

- Task nodes call only their named bound callable and replace the current object with its valid result.
- map-schema tests cover every supported type, nested object targets, whole-array copying, defaults, missing sources, type mismatch, and target conflicts.
- Deterministic-gate tests cover `eq`, `lt`, `le`, `ge`, `gt`, inclusive `in_range`, string-only `in_set`, mandatory `otherwise`, missing paths, incompatible types, numeric equality, and rejection of every pairwise overlap in the V1 operator set.
- Randomised-gate tests cover normalization, seeded serial repeatability when no randomised gates are simultaneously active, positive finite weights, unique targets, and per-execution sampling.
- Fan-out proves all roots are scheduled before awaiting a result, carries isolated objects, and preserves configured convergence key order independent of completion order.
- Converge waits for exactly its declared active inputs and produces the specified flow-ID-keyed object.
- raise-error raises `RoutingSelectionError` with `code == "GWALL"` and cancels active siblings.

## Multi-stage workflow A: gated transformation

An integration test must compile and run a workflow with this shape:

```text
input → deterministic gate → selected task → map-schema → output
                           ↘ otherwise → raise-error
```

The test proves that:

- only the matching task executes;
- the task receives an isolated copy of the original object;
- mapping validates rather than coerces values;
- the router returns the mapped object; and
- an unhandled value reaches `GWALL`.

## Multi-stage workflow B: staged convergence

An integration test must compile and run a workflow with this shape:

```text
                          ┌─ task A ─┐
input → fan-out ──────────┼─ task B ─┴─ converge early → adjudication task ─┐
                          └─ task C ────────────────────────────────────────┴─ converge final → map → output
```

The test proves that:

- all three initial branches become active without router throttling;
- A and B converge before their combined result reaches the adjudication task;
- C may complete earlier without allowing final convergence to run early;
- the early result receives a new globally unique flow ID at final convergence;
- every original fan-out path contributes transitively to output; and
- both convergence objects use configured flow IDs, not task names.

## Failure and cancellation

- A task-raised exception cancels unfinished siblings and is re-raised as the exact original instance.
- A downstream failure represented by a raised task exception is not misclassified as a task contract failure.
- `MappingError`, `TaskResultError`, and `RoutingSelectionError` in one active branch cancel unfinished siblings before propagating.
- Failure and cancellation tests prove the router awaits cooperative sibling termination and consumes secondary failures before propagating.
- A returned non-awaitable, non-object, non-JSON value, NaN, or infinity raises `TaskResultError` without interpolating the invalid value into its message.
- Caller or task cancellation cancels the complete run and propagates `asyncio.CancelledError`.
- Mapping errors expose documented context without including complete payload values in messages.
- When multiple tasks fail before cancellation, the propagated exception is one of those exact original instances and all secondary failures are consumed; no deterministic tie-break is asserted.

## Property and stress testing

- Property tests generate acyclic and cyclic graphs and verify assessment never accepts a cycle or panics on user-controlled depth.
- Nested fan-out fixtures prove parent-flow suspension, automatic resumption after all children recombine, rejection of early parent consumption, and rejection of child flows that escape on multiple successful paths.
- Property tests exercise weighted selection without asserting production stability from a seed.
- Repeated and concurrent executions prove branch and request objects are isolated.
- No test depends on execution or task-completion order unless the specification guarantees it.

## Performance evidence

- A reproducible benchmark suite covers the linear and fan-out workloads in [CONSTRAINTS.md](./CONSTRAINTS.md).
- Published results include hardware or CI runner, Python version, build profile, object size, graph shape, warm-up, sample count, p50, and p95.
- A 100,000-run soak measurement reports memory behaviour.
- Missing an objective is documented with its trade-off and explicit maintainer approval rather than hidden or treated as a functional failure.

## Documentation

- The specification files agree on terminology from [CONTEXT.md](./CONTEXT.md).
- Public classes, methods, exceptions, node forms, mapping rules, gate semantics, cancellation, seeding limitations, compatibility, and non-goals are documented.
- At least one gateway example demonstrates a preconfigured async client captured by a bound task, including explicit downstream failure checking.
