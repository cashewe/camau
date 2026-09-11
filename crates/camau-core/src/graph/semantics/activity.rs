use std::collections::{BTreeMap, HashMap, HashSet};

use crate::diagnostics::IssueData;
use crate::json_parsing::{JsonValue, join_pointer};
use indexmap::IndexMap;

use super::super::algorithms::reachable_from;
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
) -> usize {
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

    let retired_gates = gate_retirements(nodes, edges, by_id);
    let mut contexts: Vec<HashSet<ActivityContext>> = vec![HashSet::new(); nodes.len()];
    contexts[entry].insert(ActivityContext::default());
    let mut max_contexts = 1;
    for node_index in order {
        max_contexts = max_contexts.max(contexts[node_index].len());
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
                        insert_context(target, context, &retired_gates, &mut contexts);
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
                        Some(existing) => existing
                            .iter()
                            .flat_map(|left| {
                                remainders
                                    .iter()
                                    .filter_map(|right| merge_contexts(left, right))
                            })
                            .collect(),
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
                contexts[node_index] = compatible
                    .into_iter()
                    .map(|mut context| {
                        retire_gates(&mut context, &retired_gates[node_index]);
                        context
                    })
                    .collect();
                propagate_next(
                    node,
                    node_index,
                    nodes,
                    by_id,
                    &retired_gates,
                    &mut contexts,
                );
            }
            "task" | "map-schema" => {
                propagate_next(
                    node,
                    node_index,
                    nodes,
                    by_id,
                    &retired_gates,
                    &mut contexts,
                );
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
                        let produced = produced.iter().cloned().map(|mut context| {
                            context.gates.insert(node_index, target);
                            context
                        });
                        for context in produced {
                            insert_context(target, context, &retired_gates, &mut contexts);
                        }
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
                        let produced = produced.iter().cloned().map(|mut context| {
                            context.gates.insert(node_index, target);
                            context
                        });
                        for context in produced {
                            insert_context(target, context, &retired_gates, &mut contexts);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    max_contexts
}

fn propagate_next(
    node: &IndexMap<String, JsonValue>,
    node_index: usize,
    nodes: &[JsonValue],
    by_id: &HashMap<&str, usize>,
    retired_gates: &[HashSet<usize>],
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
    for context in produced {
        insert_context(target, context, retired_gates, contexts);
    }
}

fn merge_contexts(left: &ActivityContext, right: &ActivityContext) -> Option<ActivityContext> {
    if left.flows != right.flows {
        return None;
    }
    let mut merged = left.clone();
    for (gate, target) in &right.gates {
        if merged
            .gates
            .get(gate)
            .is_some_and(|existing| existing != target)
        {
            return None;
        }
        merged.gates.insert(*gate, *target);
    }
    Some(merged)
}

fn insert_context(
    target: usize,
    mut context: ActivityContext,
    retired_gates: &[HashSet<usize>],
    contexts: &mut [HashSet<ActivityContext>],
) {
    retire_gates(&mut context, &retired_gates[target]);
    contexts[target].insert(context);
}

fn retire_gates(context: &mut ActivityContext, retired: &HashSet<usize>) {
    context.gates.retain(|gate, _| !retired.contains(gate));
}

fn gate_retirements(
    nodes: &[JsonValue],
    edges: &[Vec<usize>],
    by_id: &HashMap<&str, usize>,
) -> Vec<HashSet<usize>> {
    let mut retired = vec![HashSet::new(); nodes.len()];
    for (gate, node) in nodes.iter().enumerate() {
        let Some(node) = node.object() else { continue };
        let entries = match text(node, "type") {
            Some("deterministic-gate") => "cases",
            Some("randomised-gate") => "routes",
            _ => continue,
        };
        let targets: HashSet<_> = node
            .get(entries)
            .and_then(JsonValue::array)
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                entry
                    .object()
                    .and_then(|entry| entry.get("target"))
                    .and_then(JsonValue::string)
                    .and_then(|target| by_id.get(target))
                    .copied()
            })
            .collect();
        let mut reach_count = vec![0u8; nodes.len()];
        for target in targets {
            for (index, reachable) in reachable_from(target, edges).into_iter().enumerate() {
                if reachable && reach_count[index] < 2 {
                    reach_count[index] += 1;
                    if reach_count[index] == 2 {
                        retired[index].insert(gate);
                    }
                }
            }
        }
    }
    retired
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::diagnostics::IssueData;
    use crate::graph::{algorithms::outgoing_references, text};
    use crate::json_parsing::{JsonValue, parse_json};

    use super::validate_activity;

    #[test]
    fn branch_local_gate_constraint_is_compatible_with_unconstrained_sibling() {
        let source = r#"{
            "entry":"fan","output":"join","nodes":[
                {"id":"fan","type":"fan-out","branches":[
                    {"id":"left-flow","target":"gate"},{"id":"right-flow","target":"right"}
                ]},
                {"id":"gate","type":"deterministic-gate","select":"/kind","cases":[
                    {"operator":"eq","value":"continue","target":"left"},
                    {"operator":"otherwise","target":"stop"}
                ]},
                {"id":"left","type":"task","task":"left","next":"join"},
                {"id":"right","type":"task","task":"right","next":"join"},
                {"id":"join","type":"converge","inputs":{"left-flow":"left","right-flow":"right"}},
                {"id":"stop","type":"raise-error","message":"stopped"}
            ]
        }"#;

        let (issues, _) = run_activity(source);

        assert!(!issues.iter().any(|issue| issue.code == "CONVERGE_ACTIVITY"));
    }

    #[test]
    fn mutually_exclusive_gate_routes_cannot_converge() {
        let source = r#"{
            "entry":"gate","output":"join","nodes":[
                {"id":"gate","type":"deterministic-gate","select":"/kind","cases":[
                    {"operator":"eq","value":"left","target":"left"},
                    {"operator":"otherwise","target":"right"}
                ]},
                {"id":"left","type":"task","task":"left","next":"join"},
                {"id":"right","type":"task","task":"right","next":"join"},
                {"id":"join","type":"converge","inputs":{"left":"left","right":"right"}}
            ]
        }"#;

        let (issues, _) = run_activity(source);

        assert!(issues.iter().any(|issue| issue.code == "CONVERGE_ACTIVITY"));
    }

    #[test]
    fn sequential_exclusive_rejoins_keep_activity_state_bounded() {
        let gate_count = 32;
        let mut nodes = Vec::new();
        for index in 0..gate_count {
            nodes.push(format!(
                r#"{{"id":"gate-{index}","type":"deterministic-gate","select":"/choice","cases":[{{"operator":"eq","value":{index},"target":"left-{index}"}},{{"operator":"otherwise","target":"right-{index}"}}]}}"#
            ));
            nodes.push(format!(
                r#"{{"id":"left-{index}","type":"task","task":"left","next":"merge-{index}"}}"#
            ));
            nodes.push(format!(
                r#"{{"id":"right-{index}","type":"task","task":"right","next":"merge-{index}"}}"#
            ));
            let next = if index + 1 == gate_count {
                String::new()
            } else {
                format!(r#","next":"gate-{}""#, index + 1)
            };
            nodes.push(format!(
                r#"{{"id":"merge-{index}","type":"task","task":"merge"{next}}}"#
            ));
        }
        let source = format!(
            r#"{{"entry":"gate-0","output":"merge-{}","nodes":[{}]}}"#,
            gate_count - 1,
            nodes.join(",")
        );

        let (issues, max_contexts) = run_activity(&source);

        assert!(issues.is_empty(), "{issues:?}");
        assert!(max_contexts <= 2);
    }

    fn run_activity(source: &str) -> (Vec<IssueData>, usize) {
        let (value, problems) = parse_json(source);
        assert!(problems.is_empty(), "{problems:?}");
        let value = value.unwrap();
        let root = value.object().unwrap();
        let nodes = root.get("nodes").and_then(JsonValue::array).unwrap();
        let by_id: HashMap<_, _> = nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (text(node.object().unwrap(), "id").unwrap(), index))
            .collect();
        let edges = nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                outgoing_references(node.object().unwrap(), index)
                    .into_iter()
                    .filter_map(|(target, _)| by_id.get(target.as_str()).copied())
                    .collect()
            })
            .collect::<Vec<_>>();
        let entry = by_id[text(root, "entry").unwrap()];
        let mut issues = Vec::new();
        let max_contexts = validate_activity(nodes, &edges, &by_id, entry, &mut issues);
        (issues, max_contexts)
    }
}
