use std::collections::{BTreeMap, HashMap, HashSet};

use crate::diagnostics::IssueData;
use crate::json_pointer::join_pointer;
use crate::json_value::JsonValue;
use indexmap::IndexMap;

use super::super::text;

#[derive(Clone, Default, Eq, Hash, PartialEq)]
struct ActivityContext {
    flows: Vec<(usize, String)>,
    gates: BTreeMap<usize, usize>,
}

pub(super) fn validate_activity(
    nodes: &[JsonValue],
    edges: &[Vec<usize>],
    by_id: &HashMap<&str, usize>,
    entry: usize,
    issues: &mut Vec<IssueData>,
) {
    let mut branch_flows = HashMap::new();
    for (fan_index, node) in nodes.iter().enumerate() {
        let Some(branches) = node
            .object()
            .filter(|node| text(node, "type") == Some("fan-out"))
            .and_then(|node| node.get("branches"))
            .and_then(JsonValue::array)
        else {
            continue;
        };
        for branch in branches {
            if let Some(flow_id) = branch
                .object()
                .and_then(|branch| branch.get("id"))
                .and_then(JsonValue::string)
            {
                branch_flows.insert(flow_id, fan_index);
            }
        }
    }

    let mut indegree = vec![0usize; nodes.len()];
    for targets in edges {
        for target in targets {
            indegree[*target] += 1;
        }
    }
    let mut ready: Vec<usize> = indegree
        .iter()
        .enumerate()
        .filter_map(|(index, degree)| (*degree == 0).then_some(index))
        .collect();
    ready.sort_unstable_by(|left, right| right.cmp(left));
    let mut order = Vec::with_capacity(nodes.len());
    while let Some(node) = ready.pop() {
        order.push(node);
        for target in &edges[node] {
            indegree[*target] -= 1;
            if indegree[*target] == 0 {
                ready.push(*target);
                ready.sort_unstable_by(|left, right| right.cmp(left));
            }
        }
    }

    let mut contexts: Vec<HashSet<ActivityContext>> = vec![HashSet::new(); nodes.len()];
    contexts[entry].insert(ActivityContext::default());
    for node_index in order {
        let Some(node) = nodes[node_index].object() else {
            continue;
        };
        let kind = text(node, "type").unwrap_or("");
        let flow_shapes: HashSet<_> = contexts[node_index]
            .iter()
            .map(|context| &context.flows)
            .collect();
        if !matches!(kind, "converge" | "raise-error" | "fan-out") && flow_shapes.len() > 1 {
            issues.push(IssueData::new(
                "GRAPH_IMPLICIT_JOIN",
                format!("/nodes/{node_index}/id"),
                "ordinary node may receive simultaneously active values",
            ));
        }

        match kind {
            "fan-out" => {
                let Some(branches) = node.get("branches").and_then(JsonValue::array) else {
                    continue;
                };
                for branch in branches {
                    let Some(branch) = branch.object() else {
                        continue;
                    };
                    let Some(flow_id) = text(branch, "id") else {
                        continue;
                    };
                    let Some(target) = text(branch, "target").and_then(|id| by_id.get(id)).copied()
                    else {
                        continue;
                    };
                    let source_contexts: Vec<_> = contexts[node_index].iter().cloned().collect();
                    for mut context in source_contexts {
                        context.flows.push((node_index, flow_id.to_owned()));
                        contexts[target].insert(context);
                    }
                }
            }
            "converge" => {
                let Some(inputs) = node.get("inputs").and_then(JsonValue::object) else {
                    continue;
                };
                let mut compatible: Option<HashSet<ActivityContext>> = None;
                for (flow_id, source) in inputs {
                    let Some(source_index) = source.string().and_then(|id| by_id.get(id)).copied()
                    else {
                        continue;
                    };
                    let mut remainders = HashSet::new();
                    for context in &contexts[source_index] {
                        let mut remainder = context.clone();
                        if branch_flows.contains_key(flow_id.as_str()) {
                            match remainder.flows.last() {
                                Some((_, active_flow)) if active_flow == flow_id => {
                                    remainder.flows.pop();
                                }
                                Some(_)
                                    if remainder
                                        .flows
                                        .iter()
                                        .any(|(_, active_flow)| active_flow == flow_id) =>
                                {
                                    issues.push(IssueData::new(
                                        "FLOW_INVALID",
                                        join_pointer(
                                            &format!("/nodes/{node_index}/inputs"),
                                            flow_id,
                                        ),
                                        "suspended parent flow cannot be consumed before child recombination",
                                    ));
                                    continue;
                                }
                                _ => {
                                    issues.push(IssueData::new(
                                        "FLOW_INVALID",
                                        join_pointer(
                                            &format!("/nodes/{node_index}/inputs"),
                                            flow_id,
                                        ),
                                        "flow is not active at the declared source",
                                    ));
                                    continue;
                                }
                            }
                        }
                        remainders.insert(remainder);
                    }
                    compatible = Some(match compatible {
                        None => remainders,
                        Some(existing) => existing.intersection(&remainders).cloned().collect(),
                    });
                }
                let compatible = compatible.unwrap_or_default();
                if compatible.is_empty() {
                    issues.push(IssueData::new(
                        "CONVERGE_ACTIVITY",
                        format!("/nodes/{node_index}/inputs"),
                        "declared convergence inputs cannot all be active together",
                    ));
                }
                contexts[node_index] = compatible;
                propagate_next(node, node_index, nodes, by_id, &mut contexts);
            }
            "task" | "map-schema" => {
                propagate_next(node, node_index, nodes, by_id, &mut contexts);
            }
            "deterministic-gate" => {
                if let Some(cases) = node.get("cases").and_then(JsonValue::array) {
                    let produced: Vec<_> = contexts[node_index].iter().cloned().collect();
                    for target in cases.iter().filter_map(|case| {
                        case.object()
                            .and_then(|case| case.get("target"))
                            .and_then(JsonValue::string)
                            .and_then(|id| by_id.get(id))
                            .copied()
                    }) {
                        contexts[target].extend(produced.iter().cloned().map(|mut context| {
                            context.gates.insert(node_index, target);
                            context
                        }));
                    }
                }
            }
            "randomised-gate" => {
                if let Some(routes) = node.get("routes").and_then(JsonValue::array) {
                    let produced: Vec<_> = contexts[node_index].iter().cloned().collect();
                    for target in routes.iter().filter_map(|route| {
                        route
                            .object()
                            .and_then(|route| route.get("target"))
                            .and_then(JsonValue::string)
                            .and_then(|id| by_id.get(id))
                            .copied()
                    }) {
                        contexts[target].extend(produced.iter().cloned().map(|mut context| {
                            context.gates.insert(node_index, target);
                            context
                        }));
                    }
                }
            }
            _ => {}
        }
    }
}

fn propagate_next(
    node: &IndexMap<String, JsonValue>,
    node_index: usize,
    nodes: &[JsonValue],
    by_id: &HashMap<&str, usize>,
    contexts: &mut [HashSet<ActivityContext>],
) {
    let Some(target) = node
        .get("next")
        .and_then(JsonValue::string)
        .and_then(|id| by_id.get(id))
        .copied()
    else {
        return;
    };
    if nodes[target]
        .object()
        .and_then(|target| text(target, "type"))
        == Some("converge")
    {
        return;
    }
    let produced: Vec<_> = contexts[node_index].iter().cloned().collect();
    contexts[target].extend(produced);
}
