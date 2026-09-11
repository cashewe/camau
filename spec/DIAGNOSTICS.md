# Assessment diagnostics

Assessment diagnostics are a stable, machine-readable contract within a package major version. Every `Issue` has one code, message, and JSON Pointer path. There are no warning severities in V1.

## Codes

| Code | Meaning |
|---|---|
| `JSON_PARSE` | A JSON string is malformed or contains invalid Unicode. |
| `JSON_DUPLICATE_KEY` | A JSON string repeats an object key. |
| `JSON_VALUE` | A specification contains a number, string, key, or value outside the supported JSON data model. |
| `SCHEMA_REQUIRED` | A required property is absent. |
| `SCHEMA_UNKNOWN` | An unknown property is present. |
| `SCHEMA_TYPE` | A value has the wrong structural JSON type. |
| `SCHEMA_VALUE` | A structurally constrained value, count, pattern, enum, or range is invalid. |
| `ID_INVALID` | A node, flow, or logical task identifier has invalid syntax. |
| `ID_RESERVED` | A node or flow identifier uses reserved `GWALL`. |
| `ID_COLLISION` | Node or flow identifier definitions collide in the global graph namespace. |
| `REFERENCE_MISSING` | A node reference does not resolve to exactly one node. |
| `ENTRY_INVALID` | Entry is missing, unreachable, or names an entry-ineligible node. |
| `OUTPUT_INVALID` | Output is missing, unreachable, has an outgoing edge, or names an output-ineligible node. |
| `GRAPH_CYCLE` | One strongly connected component contains a cycle, including a self-cycle. |
| `GRAPH_UNREACHABLE` | A node cannot be reached from entry. |
| `GRAPH_DEAD_END` | A successful path terminates without reaching output or reaches output ambiguously. |
| `GRAPH_IMPLICIT_JOIN` | An ordinary node may receive more than one simultaneously active value. |
| `FLOW_INVALID` | A flow ID is introduced, referenced, suspended, resumed, or consumed in a topologically invalid position. |
| `FLOW_REUSED` | A flow is consumed more than once or one source is supplied to multiple convergences without fan-out. |
| `FLOW_UNCONSUMED` | A successful fan-out flow can complete without contributing through convergence to output. |
| `CONVERGE_LINK` | A converge input and its upstream source's `next` do not agree exactly. |
| `CONVERGE_ACTIVITY` | Declared convergence inputs cannot all be active together or one can arrive more than once. |
| `MAPPING_TARGET` | Mapping targets duplicate, overlap, construct an array position, or otherwise conflict. |
| `MAPPING_DEFAULT` | A mapping default does not satisfy its declared type. |
| `GATE_CASE` | A deterministic case has invalid operator-specific values or ordering. |
| `GATE_OTHERWISE` | `otherwise` is absent, repeated, or not the final case. |
| `GATE_OVERLAP` | Two deterministic cases have a statically provable overlap. |
| `RANDOM_WEIGHT` | A randomised route weight is negative or non-finite, or every route weight is zero. |
| `RANDOM_TARGET` | A randomised gate repeats a target. |
| `TASK_MISSING` | Router construction cannot find a referenced logical task in its registry. |
| `TASK_NOT_CALLABLE` | Router construction finds a referenced registry value that is not callable. |

Generic JSON Schema keyword failures are normalized into `SCHEMA_REQUIRED`, `SCHEMA_UNKNOWN`, `SCHEMA_TYPE`, or `SCHEMA_VALUE`. When the catalogue provides a more specific domain code—such as `ID_RESERVED`, `GATE_CASE`, or `RANDOM_WEIGHT`—the assessor emits that code instead and suppresses the generic schema symptom for the same cause. Library-specific validator messages and keyword names are not public codes.

## Paths

`path` points as closely as possible to the offending configuration value. Missing-property issues point to the containing object. Graph-wide issues use the most specific defining location:

- a cycle points to the lexicographically first node ID in that strongly connected component;
- an identifier collision points to each conflicting definition after the first;
- a missing reference points to that reference value;
- a flow issue points to the branch or converge input that introduces or misuses it; and
- a task-binding issue points to the task node's `task` property.

The root pointer is the empty string.

## Completeness and cascade suppression

The assessor reports every independently detectable issue, not every downstream symptom of one defect.

1. A JSON parse failure prevents structural and semantic assessment. Duplicate keys that the strict parser detects before the fatal location may still be reported.
2. A non-object root prevents node assessment. An invalid subtree is skipped only where its missing or mistyped data makes further interpretation unsafe; valid sibling subtrees continue.
3. Identifier and reference checks run before topology checks. Ambiguous duplicate IDs are not arbitrarily resolved.
4. Missing or ambiguous references suppress reachability, dead-end, flow, and convergence findings caused solely by those references.
5. Topology checks still report independent cycles, unreachable components, and valid-subgraph failures.
6. One `GRAPH_CYCLE` is emitted per cyclic strongly connected component rather than per edge or possible traversal.
7. Node-specific mapping, gate, and random-route checks run whenever that node's required structural fields are usable, even if another graph area is invalid.

After suppression, issues are sorted by `path` using Unicode code-point order and then by `code`. Identical code/path pairs are emitted once.

## Rendering

- `to_text()` emits one line per issue containing code, path, and message. A valid assessment emits an explicit success line.
- `to_junit_xml()` emits one test case per issue, each containing one failure. A valid assessment emits one passing test case. XML special characters are escaped.
- `to_github_annotations()` emits one GitHub workflow-command `error` annotation per issue. Newlines, carriage returns, percent signs, colons, and commas are escaped according to GitHub workflow-command rules. When `source` is supplied it is used only as annotation display metadata.

All renderers preserve assessment order and return strings without performing I/O.
