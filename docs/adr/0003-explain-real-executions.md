# Explain real executions

`Router.explain` traces a real workflow execution rather than simulating possible paths because task results can determine later routing decisions. It therefore invokes tasks and consumes random samples exactly like `run`; configured `raise-error` nodes become JSON-compatible terminal outcomes, while mapping failures, invalid task results, task exceptions, and cancellation continue to propagate. Current-object snapshots are deduplicated and referenced from node visits so pass-through routing does not repeatedly copy unchanged values into the result.
