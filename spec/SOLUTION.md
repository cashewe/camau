# Solution

`camau` is an immutable, Rust-backed workflow router exposed as a Python package. A gateway constructs a router once during service initialization by supplying a routing specification and a registry of asynchronous callables. Each request calls the compiled router with one JSON object and receives one new JSON object.

See [CONFIGURATION.md](./CONFIGURATION.md) for the graph language, [PYTHON-API.md](./PYTHON-API.md) for the public interface and failures, [ACCEPTANCE.md](./ACCEPTANCE.md) for V1 completion criteria, and [CONTEXT.md](./CONTEXT.md) for canonical terminology.

## Lifecycle

1. The gateway creates its transport clients and preconfigures authentication, timeouts, retries, and connection pools.
2. It binds asynchronous callables using stable logical task names.
3. `Router` assesses the supplied dictionary or JSON string, resolves every referenced task, and compiles an immutable graph.
4. The gateway reuses that router for requests on one asyncio event loop.
5. Each `run` converts the request into Rust-owned JSON, executes the graph, and returns a fresh Python dictionary.

Router construction performs no task calls and no file or network I/O.

## Task boundary

A task is a gateway-owned callable that accepts one JSON object and returns an awaitable resolving to one JSON object. It may call HTTP, gRPC, a queue, a local model, or ordinary Python. A callable must check its own downstream outcome and raise an exception for failure; an error-shaped JSON object is otherwise a successful task result to `camau`.

The package validates task presence and callability at router construction. It validates that invocation returns an awaitable and that the awaited result is a JSON object at runtime.

## Data-flow semantics

- A **task** replaces the current object with its callable's result.
- **map-schema** constructs a new object from the current object.
- A **deterministic-gate** or **randomised-gate** passes the current object unchanged to exactly one selected target.
- **fan-out** schedules every branch root as an asyncio task before awaiting any branch and gives each branch an isolated materialization of the current object.
- **converge** waits for its declared active inputs and constructs a new object whose keys are configured flow IDs and whose values are the corresponding upstream objects. Key order follows configuration order rather than completion order.
- **raise-error** terminates the run with `RoutingSelectionError` and stable code `GWALL`.

The executor keeps per-run state. Compiled graph state and task bindings are immutable. It applies no whole-run lock, queue, semaphore, or fan-out limit.

## Graph semantics

The graph has exactly one entry and one successful output. Ordinary and mapping nodes expose one `next` edge when they are not the output. Gates expose mutually exclusive target edges. Fan-out exposes two or more simultaneously active branch edges. Convergence inputs explicitly name the upstream node to await, while each upstream node still names the convergence node in `next` for human readability and control flow.

Fan-out is not restricted to a paired fork/join structure. Some branches may converge, undergo more work, and later converge with another active branch. A concrete node output has only one `next`; duplicating it requires another explicit fan-out. Every successful active fan-out path must contribute transitively to the declared output. A path ending in raise-error is a valid exceptional path.

A fan-out nested within an open branch temporarily suspends its parent flow. After all successful child flows have contributed to one combined value, that value resumes the parent flow. The assessor rejects a parent consumed while suspended or child results that do not recombine to one value.

Mutually exclusive gate branches may share a downstream node without convergence. The assessor must prove that a node with such incoming edges can receive at most one active value. Multiple simultaneously active values may meet only at a convergence node.

## Ownership and cancellation

Python dictionaries are converted to Rust-owned JSON at each task boundary. Concurrent branches receive separately materialized dictionaries, and the caller receives a fresh output dictionary. A callable cannot affect another branch by mutating its input.

Any in-run failure cancels all active sibling tasks before it propagates. When a task raises, the exact original exception is re-raised. When the caller or a task raises `asyncio.CancelledError`, the entire run is cancelled and cancellation propagates. Mapping, explicit routing, and task-contract failures use package exceptions; configuration failures occur before execution begins.

## Randomness

Randomised gates normalize configured positive finite weights and sample once per execution of the gate. `Router` accepts an optional integer seed. A seeded router promises a repeatable sequence only for the same graph, serial run order, and graph executions with no simultaneously active randomised gates. Concurrent scheduling may consume samples in a different order. Seeding is a testing aid, not stable entity allocation.

## Extensibility

Each Rust node implementation declares internal capabilities such as whether it may be an entry or output and what outgoing-edge shape it permits. The assessor consumes the same descriptors rather than maintaining a second node-type list. V1 exposes no custom-node API.

## V1 non-goals

- Synchronous or streaming execution.
- Transport, authentication, retries, timeouts, response coercion, or service discovery.
- Configuration file loading, hot reload, secrets, or remote configuration.
- Stable entity-based experiment allocation.
- Detached shadow work or successful orphan branches.
- Partial convergence, optional branches, recovery, or fallback policy.
- Arbitrary expressions, arithmetic, or Python callbacks in mappings.
- Custom node types.
- Built-in logging, metrics, traces, or execution metadata.
- Distributed execution, persistence, cycles, or durable workflow state.

These capabilities require user stories before they enter scope.
