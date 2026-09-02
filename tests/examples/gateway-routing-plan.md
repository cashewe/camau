# Camau workflow graph

```mermaid
flowchart TD
    input(["Input"]) --> n0
    n0[/"normalise-request<br/><small>map schema</small>"/]
    n1{"route-channel<br/><small>deterministic gate</small>"}
    n2{"select-model<br/><small>randomised gate</small>"}
    n3["stable-model<br/><small>task</small>"]
    n4["candidate-model<br/><small>task</small>"]
    n5{{"run-checks<br/><small>fan-out</small>"}}
    n6["fraud-check<br/><small>task</small>"]
    n7[/"format-fraud<br/><small>map schema</small>"/]
    n8["policy-check<br/><small>task</small>"]
    n9["score-request<br/><small>task</small>"]
    n10(["combine-signals<br/><small>convergence</small>"])
    n11{"route-risk<br/><small>deterministic gate</small>"}
    n12["manual-review<br/><small>task</small>"]
    n13["auto-approve<br/><small>task</small>"]
    n14[/"format-response<br/><small>map schema</small>"/]
    n15["unsupported-channel<br/><small>raise error</small>"]
    n16["risk-rejected<br/><small>raise error</small>"]
    n0 --> n1
    n1 -. "= &quot;api&quot;" .-> n2
    n1 -. "= &quot;batch&quot;" .-> n5
    n1 -. "otherwise" .-> n15
    n2 -. "90%" .-> n3
    n2 -. "10%" .-> n4
    n3 -.-> n5
    n4 -.-> n5
    n5 == "branch: fraud-signal" ==> n6
    n5 == "branch: policy-signal" ==> n8
    n5 == "branch: model-signal" ==> n9
    n6 --> n7
    n7 == "flow: fraud-signal" ==> n10
    n8 == "flow: policy-signal" ==> n10
    n9 == "flow: model-signal" ==> n10
    n10 --> n11
    n11 -. "&gt; 0.8" .-> n16
    n11 -. "0.4 to 0.8" .-> n12
    n11 -. "otherwise" .-> n13
    n12 -.-> n14
    n13 -.-> n14
    n14 --> output(["Output"])
    classDef boundary stroke-width:3px
    classDef failure stroke:#c62828,stroke-width:2px
    class input,output boundary
    class n15,n16 failure
```

## How to read this graph

- Routing starts at <code>normalise-request</code> and succeeds at <code>format-response</code>.
- Dotted arrows are mutually exclusive: exactly one route is active. Thick arrows belong to concurrently active branches. Solid arrows are sequential continuation.
- Diamond nodes select one route. Hexagonal fan-out nodes start every branch concurrently. Rounded convergence nodes wait for every labelled input.
- A red failure node stops the run with `GWALL`; it does not lead to the successful output.

## Node details

| Node | Behaviour |
|---|---|
| <code>normalise-request</code> | Builds a new object: <code>/request-id</code> &larr; <code>/request/id</code> (string); <code>/channel</code> &larr; <code>/request/channel</code> (string); <code>/customer</code> &larr; <code>/customer</code> (object). |
| <code>route-channel</code> | Reads <code>/channel</code> and follows the first matching labelled route. |
| <code>select-model</code> | Chooses one labelled route using the normalised weights. |
| <code>stable-model</code> | Calls task <code>stable-predictor</code>. |
| <code>candidate-model</code> | Calls task <code>candidate-predictor</code>. |
| <code>run-checks</code> | Starts all 3 labelled flow branches concurrently. |
| <code>fraud-check</code> | Calls task <code>fraud-checker</code>. |
| <code>format-fraud</code> | Builds a new object: <code>/risk</code> &larr; <code>/risk-score</code> (number); <code>/provider</code> &larr; default <code>"fraud-service"</code> (string). |
| <code>policy-check</code> | Calls task <code>policy-checker</code>. |
| <code>score-request</code> | Calls task <code>risk-scorer</code>. |
| <code>combine-signals</code> | Waits for <code>fraud-signal</code> from <code>format-fraud</code>, <code>policy-signal</code> from <code>policy-check</code>, <code>model-signal</code> from <code>score-request</code> and creates an object keyed by those flow IDs. |
| <code>route-risk</code> | Reads <code>/fraud-signal/risk</code> and follows the first matching labelled route. |
| <code>manual-review</code> | Calls task <code>review-queue</code>. |
| <code>auto-approve</code> | Calls task <code>approval-service</code>. |
| <code>format-response</code> | Builds a new object: <code>/request-id</code> &larr; <code>/request-id</code> (string); <code>/decision</code> &larr; <code>/decision</code> (string); <code>/reviewed</code> &larr; <code>/reviewed</code> or default <code>false</code> (boolean). |
| <code>unsupported-channel</code> | Stops routing with `GWALL`: The request channel is not supported. |
| <code>risk-rejected</code> | Stops routing with `GWALL`: The fraud risk exceeds the gateway threshold. |
