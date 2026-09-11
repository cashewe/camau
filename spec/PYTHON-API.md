# Python API

This document defines the public V1 surface. Names and behaviour are normative; recursive JSON type aliases below are illustrative typing notation.

```python
from collections.abc import Awaitable, Callable, Mapping
from typing import TypeAlias

JsonScalar: TypeAlias = str | int | float | bool | None
JsonValue: TypeAlias = JsonScalar | list["JsonValue"] | dict[str, "JsonValue"]
JsonObject: TypeAlias = dict[str, JsonValue]
AsyncTask: TypeAlias = Callable[[JsonObject], Awaitable[JsonObject]]
```

All exposed result, issue, and exception metadata objects must be ordinary Python objects. Public collections described as immutable must be tuples or read-only mappings.

## Router

```python
class Router:
    def __init__(
        self,
        specification: dict[str, JsonValue] | str,
        tasks: Mapping[str, AsyncTask],
        *,
        seed: int | None = None,
    ) -> None: ...

    async def run(self, payload: JsonObject) -> JsonObject: ...
```

Construction:

- accepts a Python dictionary or JSON string and performs no file I/O;
- runs the same structural and semantic assessment exposed by `Assessor`;
- raises `ConfigurationError` if assessment fails;
- verifies that every referenced logical task name is present and callable, without invoking it;
- permits unused registry entries without validating their values;
- copies and freezes graph and registry state so the router cannot be reconfigured; and
- accepts a non-boolean, non-negative integer seed no greater than `2**64 - 1`, or nondeterministic entropy when omitted.

Execution:

- accepts only a JSON object and returns a new JSON object;
- supports overlapping runs on one asyncio event loop without serializing them;
- does not promise cross-thread or cross-event-loop safety;
- returns no routing metadata and emits no logging or telemetry; and
- does not mutate the caller's payload.

Passing a non-dictionary payload raises `TypeError`. A dictionary containing non-string keys, out-of-range integers, non-finite numbers, invalid Unicode, reference cycles, more than 128 nested containers, or values outside the JSON data model raises `ValueError` at the offending JSON Pointer. These are caller argument errors, not graph, task, or routing failures. The same cycle and depth limits produce a `JSON_VALUE` assessment issue for dictionary specifications and a `TaskResultError` with reason `invalid_json` for task results.

## Numeric data model

JSON integers are signed 64-bit values from `-(2**63)` through `2**63 - 1` and are preserved exactly. JSON decimal and exponent forms are finite IEEE-754 binary64 values; their source spelling and decimal precision are not preserved. A JSON string number that overflows binary64, and any Python integer or float outside these bounds, is invalid.

Integer-to-integer comparisons are exact. Floating-point comparisons use IEEE-754 numeric value. An integer and float compare equal only when the float is finite, integral, within the signed 64-bit range, and represents that exact integer; booleans never participate in numeric comparison. Range ordering follows the same exact cross-representation rule and must not compare by first rounding an integer to binary64. Positive and negative zero compare equally.

A task registry name is case-sensitive and matches `[A-Za-z][A-Za-z0-9._-]*`. The registry uses a separate namespace from graph and flow IDs.

## Task invocation

A registry value may be an `async def` function or any callable object that returns an awaitable. Router construction checks only that it is callable. Runtime behaviour is:

1. Materialize an isolated Python dictionary from the current Rust-owned JSON object.
2. Call the bound task with exactly that positional argument.
3. Require an awaitable without attempting to adapt synchronous return values.
4. Await it and require the resolved value to be a JSON object containing only finite JSON numbers and valid JSON values.
5. Convert the result immediately into Rust-owned JSON.

The task owns transport, status checking, authentication, timeouts, retries, coercion, connection pooling, and task-specific observability. It must raise an exception for an unsuccessful downstream response. A returned error-shaped JSON object is a successful value to the router.

## Assessor

```python
class Assessor:
    @classmethod
    def assess(
        cls,
        specification: dict[str, JsonValue] | str,
    ) -> "Assessment": ...

    @classmethod
    def schema(cls) -> dict[str, JsonValue]: ...
```

`Assessor` is task-registry agnostic. It validates task-name syntax but does not know whether a task is registered. It performs no task calls and no file or network I/O. `schema()` returns a fresh Python representation of the packaged structural schema.

Malformed JSON strings and Python dictionaries outside the JSON data model produce assessment issues rooted at the closest known JSON Pointer; they do not escape as parser exceptions.

```python
@dataclass(frozen=True)
class Issue:
    code: str
    message: str
    path: str


@dataclass(frozen=True)
class Assessment:
    issues: tuple[Issue, ...]

    @property
    def valid(self) -> bool: ...

    def to_text(self) -> str: ...
    def to_junit_xml(self) -> str: ...
    def to_github_annotations(self, source: str | None = None) -> str: ...
```

`valid` is true exactly when `issues` is empty. The assessor reports every independently detectable issue rather than stopping at the first. Issues are ordered by configuration JSON Pointer and then stable diagnostic code.

The complete code catalogue and cascade-suppression rules are defined in [DIAGNOSTICS.md](./DIAGNOSTICS.md).

Renderers return strings. Text is intended for people, JUnit XML represents invalid assessment as test failures, and GitHub output uses workflow-command error annotations. `source` is an optional display filename; it is never opened. Renderer escaping must prevent issue content from creating extra records or workflow commands.

Diagnostic codes and configuration compatibility remain stable within a package major version.

## Exceptions

```text
CamauError
├─ ConfigurationError
├─ MappingError
├─ TaskResultError
└─ RoutingSelectionError
```

### ConfigurationError

Raised by `Router` construction when structural or semantic assessment fails, a referenced task is absent, or a referenced registry value is not callable. It exposes the complete `assessment`. Binding failures use `TASK_MISSING` or `TASK_NOT_CALLABLE` at the task node's `/task` path. Invalid unused registry values are ignored.

### MappingError

Raised when map-schema cannot produce a declared field. It exposes `node_id`, `target`, `source` when present, `expected_type`, and a machine-readable reason. Its message must not include the complete input or mapped value.

### TaskResultError

Raised when a task invocation returns a non-awaitable, resolves to a non-object, or returns a value that is not representable as JSON. It exposes `node_id`, `task_name`, a machine-readable reason, and the invalid Python value where one exists. Its message must not interpolate that value.

### RoutingSelectionError

Raised by raise-error. It has stable `code == "GWALL"` and exposes `node_id` and the configured message. It also exposes optional `gate_node_id`, `selected_path`, `value_found`, `selected_value`, and `matched_operator` attributes.

Gate metadata describes only a direct gate-to-raise-error edge. All optional attributes are present and use `None` when not applicable. A direct deterministic gate supplies its node ID, selected pointer, matched operator, whether the pointer resolved, and the resolved value when present. `value_found` distinguishes a missing value from JSON null. A direct randomised gate supplies only `gate_node_id`. Reaching raise-error through any intermediate node supplies no gate metadata. Payload values are attributes only and are not interpolated into the exception message.

### Pass-through exceptions

Every in-run `CamauError` cancels unfinished sibling work before it propagates. An exception raised by a task is not wrapped: the router requests cancellation of unfinished siblings, awaits their termination, suppresses resulting cancellation exceptions, and re-raises the exact exception instance. `asyncio.CancelledError` raised by the caller or a task follows the same cleanup and then propagates unchanged. Bound tasks must cooperate with asyncio cancellation; a task that suppresses cancellation may delay failure propagation.

If several tasks have already failed before cancellation takes effect, the executor may propagate any one of those original exception instances. No branch-order or event-loop-turn tie-break is guaranteed; secondary failures are consumed so they do not become unhandled task errors.

When several task invocations return the same awaitable object, the router schedules that object once and applies its single result to every associated routing ticket. A run-ending failure or caller cancellation requests cancellation once per distinct unfinished awaitable, after every ticket in that run has been abandoned.
