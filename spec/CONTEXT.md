# Camau routing context

Camau describes how one JSON object moves through an in-process workflow of gateway-owned tasks. This glossary fixes the vocabulary used throughout the V1 specification.

## Language

**Routing specification**:
A trusted JSON object that declares one workflow graph. It contains nodes and the identifiers connecting them.
_Avoid_: Config file, pipeline script

**Router**:
An immutable, compiled routing specification bound to a registry of tasks and reused for gateway requests.
_Avoid_: Gateway, HTTP router, client

**Workflow graph**:
The finite directed acyclic graph executed by a router, with one entry and one successful output.
_Avoid_: Daisy chain, flow chart

**Node**:
One declared operation in a workflow graph. Every node has a globally unique node ID and one of the built-in node types.
_Avoid_: Step, task

**Task**:
A gateway-owned asynchronous callable bound to a stable logical task name. A task owns downstream integration behaviour and is invoked only by a task node.
_Avoid_: Endpoint, node

**Task node**:
A node that invokes one registered task with the current object.
_Avoid_: Task

**Current object**:
The Rust-owned JSON object presented to the next active node on a path.
_Avoid_: Message, mutable payload

**Active path**:
A route selected for the current run. Every fan-out branch is active; exactly one deterministic- or randomised-gate target is active.
_Avoid_: Possible path

**Branch**:
A concurrently active path created by fan-out and labelled by a branch ID. A branch may contain several sequential nodes before its value converges.
_Avoid_: Task, target

**Flow ID**:
A globally unique, human-readable label used as a fan-out branch ID or convergence input ID. Node IDs and flow IDs share one identifier namespace.
_Avoid_: Task name, dictionary key

**Convergence**:
The operation that awaits two or more concurrently active upstream values and replaces them with one object keyed by flow IDs.
_Avoid_: Gate reunion, implicit join

**Assessment**:
The immutable collection of every structural and semantic issue detected in a routing specification.
_Avoid_: First validation error

**GWALL**:
The stable error code for an explicit raise-error node, named for the Welsh word for an oversight or mistake.
_Avoid_: Task failure, contract failure
