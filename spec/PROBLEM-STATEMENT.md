# Problem statement

Gateway services expose outcome-oriented APIs in front of small backend services, including AI and machine-learning predictors. A gateway consumer should not need to know which model, endpoint version, or combination of backend services produces an outcome.

Routing policy currently leaks into gateway code. Common changes—selecting a predictor from a request field, introducing a candidate model, calling several predictors, or reshaping an intermediate response—therefore require bespoke orchestration code. That code is difficult to review consistently and tends to mix routing policy with HTTP, authentication, retries, and service-specific behaviour.

`camau` is a reusable, configuration-driven routing engine for those gateways. A gateway binds its own asynchronous task callables to stable logical names, builds one immutable router during service initialization, and reuses it for requests. `camau` decides which callables run, runs independent branches concurrently, combines their JSON results, and maps JSON objects between shapes.

`camau` deliberately does not know how a task communicates with a backend. Gateway-owned callables retain responsibility for HTTP or other transports, authentication, connection pooling, retries, timeouts, response-status checks, coercion, and service-specific errors. This keeps routing independent of integration technology and keeps secrets out of routing specifications.

## Users

- Gateway developers bind callables, construct routers, and integrate router output into services.
- Machine-learning teams review and propose routing policy expressed in human-readable JSON.
- Operators validate specifications in CI and manage service concurrency and deployment.
- Gateway consumers receive the selected outcome without learning backend topology.

## V1 outcome

V1 is successful when a gateway can express and execute multi-stage acyclic workflows using task invocation, fan-out, convergence, deterministic gates, randomised gates, schema mapping, and explicit failure nodes. Invalid graphs must be rejected before traffic is served, and the same assessment must be usable independently in CI.
