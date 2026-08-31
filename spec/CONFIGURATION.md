# Routing configuration

A routing specification is a trusted JSON object. The package accepts it as a Python dictionary or JSON string; reading and versioning files belongs to the gateway. The package major version governs this format, so the document has no `version` property.

The packaged [camau.schema.json](./camau.schema.json) is the structural authority for local JSON shapes. Cross-property, cross-node, graph, and runtime data-model rules in this document are semantic requirements enforced by the assessor or executor.

| Structural JSON Schema checks | Semantic assessor checks |
|---|---|
| Root and property types, required properties, unknown properties, local array sizes, enums, identifier and pointer syntax | Global identifier uniqueness, references, node capabilities, reachability, cycles, active-path analysis, flow lifecycle, convergence links, mapping target/default compatibility, gate ordering and overlap, randomised target uniqueness |

## Document envelope

```json
{
  "entry": "predict",
  "output": "predict",
  "nodes": [
    {
      "id": "predict",
      "type": "task",
      "task": "predictor"
    }
  ]
}
```

- `entry` identifies the first node to receive the router input.
- `output` identifies the only successful output node.
- `nodes` is a non-empty list of node objects selected by their `type`.
- Unknown properties are errors at every level.

The root and all runtime task boundaries are JSON objects. Nested fields may contain any JSON value. JSON strings are parsed strictly: duplicate object keys, malformed JSON, invalid Unicode, NaN, and infinity are errors.

## Identifiers

Node IDs and flow IDs are case-sensitive and match `[A-Za-z][A-Za-z0-9._-]*`. They share one global namespace and may not collide. `GWALL` is reserved as the raise-error code and may not be used as an identifier.

A flow ID is defined either by a fan-out branch or by a convergence input that labels a source downstream of an earlier convergence. Repeating a still-open fan-out branch ID at its first convergence is a reference to the existing flow ID, not a second definition.

Each fan-out branch ID remains open along its unique sequential path until one convergence input consumes it. A convergence-only flow ID is defined at the convergence where it labels an already-combined upstream source. It occurs nowhere else. A convergence consumes each input flow ID once; its result can later be given a new flow ID by another convergence. This separates the identity of a path from the IDs of the nodes on it and makes staged convergence statically traceable.

When fan-out runs inside an open parent flow, that parent flow is suspended while the child flows are active. The parent must not appear as a convergence input while suspended. Once every successful child flow contributes transitively to one combined value, the parent flow resumes automatically on that value's unique outgoing path. Child flows are consumed by their own convergences and are not visible after the parent resumes. A nested fan-out is invalid if its child results escape along multiple paths, consume the parent early, or fail to recombine into one value before the parent is consumed. Nested raise-error remains an exceptional termination and cancels the whole run.

Logical task names use the same syntax but occupy a separate registry namespace.

## Common control-flow rules

- Every node is reachable from `entry`.
- Every possible successful route reaches `output` exactly once.
- `output` has no `next`.
- A non-output task, map-schema, or converge node has exactly one `next`.
- A concrete node output has at most one `next`. Reusing it in parallel requires fan-out.
- raise-error is an exceptional terminal and has no `next`.
- References name existing nodes of an eligible type.
- The graph is acyclic.
- Two simultaneously active values meet only at converge.
- Mutually exclusive gate routes may feed the same ordinary successor.

The assessor obtains entry, output, and edge eligibility from node capabilities declared by each node implementation.

## Task

```json
{
  "id": "call-control",
  "type": "task",
  "task": "control-predictor",
  "next": "format-control"
}
```

`task` names a bound asynchronous callable. The callable receives the current object and its returned object becomes the current object. `next` is omitted only when this node is `output`; a task feeding convergence still declares that convergence node as `next`.

A task node may be `entry` or `output`.

## Map schema

```json
{
  "id": "format-control",
  "type": "map-schema",
  "mappings": [
    {
      "target": "/score",
      "path-to-source": "/prediction/confidence",
      "type": "number"
    },
    {
      "target": "/model",
      "default": "control",
      "type": "string"
    },
    {
      "target": "/details/request_id",
      "path-to-source": "/request/id",
      "default": "unknown",
      "type": "string"
    }
  ],
  "next": "combine"
}
```

map-schema creates a completely new object from its ordered, non-empty `mappings` list. Each mapping has:

- required `target`: a non-root JSON Pointer identifying an object field in the new object;
- optional `path-to-source`: a JSON Pointer into the current object;
- optional `default`: a literal JSON value; and
- required `type`: one of `object`, `array`, `string`, `number`, `integer`, `boolean`, or `null`.

At least one of `path-to-source` or `default` is required. When both are present, the default is used only if the source path is absent. A present value of the wrong type is an error rather than a reason to use the default. A default must satisfy its declared type.

`type` validates without coercing. `integer` means a finite mathematical integer; `number` includes integers. Booleans are never numbers. The surrounding gateway owns coercion.

Source pointers may select scalars, objects, or complete arrays. Target pointers create missing parent objects. V1 does not construct array positions, although it may copy a complete array value. Duplicate targets and parent/child conflicts such as `/user` together with `/user/name` are errors.

map-schema performs no arithmetic, expressions, concatenation, predicates, arbitrary functions, or general JSON Schema validation. A missing required source or type mismatch raises `MappingError` at runtime.

A map-schema node may be `entry` or `output`.

## Deterministic gate

```json
{
  "id": "route-score",
  "type": "deterministic-gate",
  "select": "/score",
  "cases": [
    {"operator": "lt", "value": 0.5, "target": "low"},
    {
      "operator": "in_range",
      "lower": 0.5,
      "upper": 0.8,
      "target": "medium"
    },
    {
      "operator": "in_set",
      "values": ["manual", "unknown"],
      "target": "review"
    },
    {"operator": "otherwise", "target": "unsupported-score"}
  ]
}
```

The gate selects one value with a JSON Pointer and evaluates cases in declared order. Exactly one `otherwise` case is required and it is last. The selected target receives the current object unchanged.

Supported cases are:

| Operator | Configuration | Runtime match |
|---|---|---|
| `eq` | scalar `value` | exact JSON scalar equality |
| `lt` | numeric `value` | selected number is less than `value` |
| `le` | numeric `value` | selected number is less than or equal to `value` |
| `ge` | numeric `value` | selected number is greater than or equal to `value` |
| `gt` | numeric `value` | selected number is greater than `value` |
| `in_range` | numeric `lower` and `upper` | inclusive numeric interval |
| `in_set` | non-empty unique string `values` | selected string is a member |
| `otherwise` | no comparison value | fallback |

`eq` accepts string, number, boolean, or null configuration values. Numerically equal integer and floating-point representations compare equally. Booleans remain distinct from numbers, null, and strings. Objects and arrays are not comparable. Numeric operators accept only finite numbers, and `lower` must not exceed `upper`.

All numbers follow the signed-64-bit integer and finite binary64 model in [PYTHON-API.md](./PYTHON-API.md). Assessment applies that model to specification literals before overlap or range analysis.

There is no coercion. A missing selected field, incompatible runtime type, or unmatched value selects `otherwise`. Every overlap in the closed V1 operator set is decidable, and the assessor rejects all overlapping cases. Evaluation order remains first-match for forward compatibility, but no valid V1 specification can depend on that tie-break.

A deterministic-gate may be `entry` but not `output`.

## Randomised gate

```json
{
  "id": "experiment",
  "type": "randomised-gate",
  "routes": [
    {"weight": 95, "target": "control"},
    {"weight": 5, "target": "candidate"}
  ]
}
```

At least two routes are required. Each weight is positive and finite; weights need not sum to one because the router normalizes them. Targets are unique. The chosen target receives the current object unchanged.

Sampling occurs independently each time the node executes. An optional router seed controls a serial pseudo-random sequence for tests but does not provide stable per-entity allocation or deterministic concurrent execution.

A randomised-gate may be `entry` but not `output`.

## Fan-out

```json
{
  "id": "parallel-predictors",
  "type": "fan-out",
  "branches": [
    {"id": "control", "target": "call-control"},
    {"id": "candidate", "target": "call-candidate"}
  ]
}
```

At least two branches are required. Each branch defines a globally unique flow ID and target node. All targets receive isolated copies of the current object and are scheduled before any branch is awaited. The router does not throttle them.

A branch may contain multiple nodes and may participate in staged convergence. Every successful active branch must eventually contribute to `output`; a branch ending in raise-error terminates the whole execution exceptionally. A fan-out node may be `entry` but not `output` and has no `next` because its branch targets are its outgoing edges.

Nested fan-out follows the parent-flow suspension and resumption rules above. The assessor derives the resumption point from topology: it is the first downstream convergence result whose dependency ancestry contains every successful child contribution and whose output continues on one path. No additional implicit edge or configuration property is introduced.

## Converge

```json
{
  "id": "combine",
  "type": "converge",
  "inputs": {
    "control": "format-control",
    "candidate": "format-candidate"
  },
  "next": "format-response"
}
```

At least two simultaneously active inputs are required. Each key is a flow ID used in the new object and each value is the upstream node whose value is awaited. Every named upstream node must declare this converge node as `next`. This intentional redundancy is required: `next` defines forward flow, while `inputs` defines readiness and output labelling. The assessor requires exact agreement.

Convergence waits for every declared active input and emits a new object in configured input order. The implementation preserves insertion order from a supplied Python dictionary and source-member order from a JSON string for this purpose. Partial, optional, quorum, and first-completed convergence are not supported.

Convergence can be staged. A later convergence may assign a new globally unique flow ID to the output of an earlier convergence:

```json
{
  "id": "converge-final",
  "type": "converge",
  "inputs": {
    "combined-ab": "converge-early",
    "branch-c": "call-c"
  }
}
```

In this example, `branch-c` references an open fan-out branch and consumes it. `combined-ab` is defined at `converge-final` because its source is downstream of `converge-early`; it labels that already-combined value in the final output. The assessor rejects reusing a consumed flow ID, consuming one flow more than once, assigning a new flow ID to an uncombined fan-out path, or leaving a successful fan-out flow unconsumed.

A prior output may feed only one later node. Sharing it in more than one place requires a new fan-out.

A converge node may be `output` but not `entry` in V1.

### Complete staged-convergence example

```json
{
  "entry": "parallel",
  "output": "format-final",
  "nodes": [
    {
      "id": "parallel",
      "type": "fan-out",
      "branches": [
        {"id": "branch-a", "target": "call-a"},
        {"id": "branch-b", "target": "call-b"},
        {"id": "branch-c", "target": "call-c"}
      ]
    },
    {"id": "call-a", "type": "task", "task": "predictor-a", "next": "converge-early"},
    {"id": "call-b", "type": "task", "task": "predictor-b", "next": "converge-early"},
    {"id": "call-c", "type": "task", "task": "predictor-c", "next": "converge-final"},
    {
      "id": "converge-early",
      "type": "converge",
      "inputs": {
        "branch-a": "call-a",
        "branch-b": "call-b"
      },
      "next": "adjudicate"
    },
    {
      "id": "adjudicate",
      "type": "task",
      "task": "adjudicator",
      "next": "converge-final"
    },
    {
      "id": "converge-final",
      "type": "converge",
      "inputs": {
        "combined-ab": "adjudicate",
        "branch-c": "call-c"
      },
      "next": "format-final"
    },
    {
      "id": "format-final",
      "type": "map-schema",
      "mappings": [
        {"target": "/combined", "path-to-source": "/combined-ab", "type": "object"},
        {"target": "/independent", "path-to-source": "/branch-c", "type": "object"}
      ]
    }
  ]
}
```

`branch-a`, `branch-b`, and `branch-c` are defined by fan-out. The early convergence consumes the first two. `branch-c` remains open until final convergence. `combined-ab` is defined at final convergence because `adjudicate` lies on the unique path downstream of an earlier convergence and no intervening fan-out duplicated that value.

## Raise error

```json
{
  "id": "unsupported-score",
  "type": "raise-error",
  "message": "No route accepts this score"
}
```

raise-error may be targeted anywhere after entry that a normal target is accepted. It has no `next`, cannot be `entry` or successful `output`, and raises `RoutingSelectionError` with stable code `GWALL`. If reached by one fan-out branch, the router cancels all other active work.

## Semantic assessment

After structural schema validation, the assessor reports all independently detectable semantic issues. Stable codes and cascade-suppression rules are defined in [DIAGNOSTICS.md](./DIAGNOSTICS.md). Semantic checks include:

- invalid, reserved, duplicate, or colliding identifiers;
- absent node references and malformed logical task names;
- missing, ineligible, or ambiguous entry and output nodes;
- cycles and unreachable nodes;
- successful paths that terminate before output or reach output more than once;
- ordinary nodes that may receive more than one simultaneously active value;
- fan-out paths that can complete without convergence;
- convergence sources whose `next` does not point back to the convergence node;
- convergence inputs that cannot be active together or can arrive more than once;
- reused, multiply consumed, invalidly introduced, or unconsumed flow IDs;
- invalid mapping target conflicts or default types;
- missing, misplaced, duplicate, overlapping, or malformed gate cases; and
- invalid randomised weights or duplicate randomised targets.

Task availability is deliberately absent from standalone assessment. Router construction adds missing task registrations to its `ConfigurationError` assessment.
