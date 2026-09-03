---
name: camau-routing
description: Create, modify, repair, or review Camau routing specifications and verify their structural, graph, task-binding, and behavioural correctness. Do not use for changes to the Camau engine itself.
---

# Camau routing authoring

Author the same JSON routing specification that a person would maintain. Do not introduce an agent-only format or duplicate Camau's rules in prompts or generated documentation.

## Establish the authoring context

Read [`spec/CONFIGURATION.md`](../../../spec/CONFIGURATION.md) before changing a routing specification. Consult [`spec/CONTEXT.md`](../../../spec/CONTEXT.md) when naming domain concepts and [`spec/DIAGNOSTICS.md`](../../../spec/DIAGNOSTICS.md) when an assessment result needs interpretation.

Identify the intended entry and output object shapes and the available gateway tasks. For each task, establish its exact logical name, purpose, accepted object shape, and returned object shape from gateway code, types, tests, or user-provided contracts. Do not invent task names or payload fields. If required task or object contracts cannot be established, ask for them before claiming behavioural correctness.

Use `camau schema` to obtain the installed structural schema when editor integration or constrained generation can consume JSON Schema. Treat schema conformance as a structural check only; it cannot prove graph semantics or business intent.

## Author and verify

Keep changes limited to the requested routing behaviour and preserve insertion order where convergence output order matters.

After writing or changing a routing specification:

1. Run `camau assess <file>` and repair every reported issue until it exits successfully.
2. Run `camau diagram <file>` and compare the rendered topology with the requested behaviour, including exceptional paths.
3. When the gateway registry is available, construct `Router` with that registry to detect missing or non-callable task bindings without invoking tasks.
4. When expected behaviour is in scope, exercise representative payloads for each gate route, successful fan-out/convergence path, mapping default, and explicit error path affected by the change.

Do not claim that a routing strategy is correct merely because `camau assess` succeeds. Report separately whether structural and graph validity, task binding, and representative behaviour were verified.
