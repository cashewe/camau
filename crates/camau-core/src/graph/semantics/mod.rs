mod activity;
mod flow;
mod gate_validation;
mod mapping_validation;

use std::collections::HashMap;

use crate::diagnostics::IssueData;
use crate::json_parsing::{JsonValue, valid_identifier};

use super::algorithms::{
    can_reach_terminal, convergence_sources, outgoing_references, reachable_from,
    strongly_connected,
};
use super::{NodeKind, text};
use activity::validate_activity;
use flow::validate_flows;
use gate_validation::validate_gate_overlaps;
use mapping_validation::validate_mapping_targets;

#[derive(Clone, Copy)]
struct Capabilities {
    entry: bool,
    output: bool,
    ordinary_next: bool,
}

impl NodeKind {
    fn capabilities_name(kind: &str) -> Option<Capabilities> {
        match kind {
            "task" | "map-schema" => Some(Capabilities {
                entry: true,
                output: true,
                ordinary_next: true,
            }),
            "converge" => Some(Capabilities {
                entry: false,
                output: true,
                ordinary_next: true,
            }),
            "deterministic-gate" | "randomised-gate" | "fan-out" => Some(Capabilities {
                entry: true,
                output: false,
                ordinary_next: false,
            }),
            "raise-error" => Some(Capabilities {
                entry: false,
                output: false,
                ordinary_next: false,
            }),
            _ => None,
        }
    }
}

pub(super) fn validate_semantics(value: &JsonValue, issues: &mut Vec<IssueData>) {
    let Some(root) = value.object() else { return };
    let Some(raw_nodes) = root.get("nodes").and_then(JsonValue::array) else {
        return;
    };
    let mut ids: HashMap<&str, Vec<usize>> = HashMap::new();
    for (index, node) in raw_nodes.iter().enumerate() {
        if let Some(id) = node
            .object()
            .and_then(|node| node.get("id"))
            .and_then(JsonValue::string)
            .filter(|id| valid_identifier(id) && *id != "GWALL")
        {
            ids.entry(id).or_default().push(index);
        }
    }
    for indexes in ids.values() {
        for index in indexes.iter().skip(1) {
            issues.push(IssueData::new(
                "ID_COLLISION",
                format!("/nodes/{index}/id"),
                "node identifier collides with an earlier definition",
            ));
        }
    }

    let mut flow_definitions: HashMap<String, String> = HashMap::new();
    for (index, node) in raw_nodes.iter().enumerate() {
        let Some(node) = node.object() else { continue };
        if text(node, "type") == Some("fan-out") {
            if let Some(branches) = node.get("branches").and_then(JsonValue::array) {
                for (branch_index, branch) in branches.iter().enumerate() {
                    let Some(flow_id) = branch
                        .object()
                        .and_then(|branch| branch.get("id"))
                        .and_then(JsonValue::string)
                    else {
                        continue;
                    };
                    let path = format!("/nodes/{index}/branches/{branch_index}/id");
                    if ids.contains_key(flow_id)
                        || flow_definitions
                            .insert(flow_id.to_owned(), path.clone())
                            .is_some()
                    {
                        issues.push(IssueData::new(
                            "ID_COLLISION",
                            path,
                            "flow identifier collides in the graph namespace",
                        ));
                    }
                }
            }
        }
    }

    let mut by_id = HashMap::new();
    for (id, indexes) in &ids {
        if indexes.len() == 1 {
            by_id.insert(*id, indexes[0]);
        }
    }
    let mut edges = vec![Vec::<usize>::new(); raw_nodes.len()];
    let mut reference_failure = false;
    for (index, node) in raw_nodes.iter().enumerate() {
        let Some(node) = node.object() else { continue };
        for (target, path) in outgoing_references(node, index) {
            if let Some(target_index) = by_id.get(target.as_str()) {
                edges[index].push(*target_index);
            } else if valid_identifier(&target) && target != "GWALL" {
                issues.push(IssueData::new(
                    "REFERENCE_MISSING",
                    path,
                    format!("node reference {target:?} is missing or ambiguous"),
                ));
                reference_failure = true;
            }
        }
        for (source, path) in convergence_sources(node, index) {
            if !by_id.contains_key(source.as_str())
                && valid_identifier(&source)
                && source != "GWALL"
            {
                issues.push(IssueData::new(
                    "REFERENCE_MISSING",
                    path,
                    format!("convergence source {source:?} is missing or ambiguous"),
                ));
                reference_failure = true;
            }
        }
    }

    let entry = root
        .get("entry")
        .and_then(JsonValue::string)
        .and_then(|id| by_id.get(id))
        .copied();
    let output = root
        .get("output")
        .and_then(JsonValue::string)
        .and_then(|id| by_id.get(id))
        .copied();
    if root.contains_key("entry") && entry.is_none() {
        issues.push(IssueData::new(
            "ENTRY_INVALID",
            "/entry",
            "entry does not name exactly one node",
        ));
    }
    if root.contains_key("output") && output.is_none() {
        issues.push(IssueData::new(
            "OUTPUT_INVALID",
            "/output",
            "output does not name exactly one node",
        ));
    }
    if let Some(entry_index) = entry {
        let kind = raw_nodes[entry_index]
            .object()
            .and_then(|node| text(node, "type"));
        if !kind
            .and_then(NodeKind::capabilities_name)
            .is_some_and(|capabilities| capabilities.entry)
        {
            issues.push(IssueData::new(
                "ENTRY_INVALID",
                "/entry",
                "node type is not eligible as entry",
            ));
        }
    }
    if let Some(output_index) = output {
        let node = raw_nodes[output_index].object();
        let kind = node.and_then(|node| text(node, "type"));
        if !kind
            .and_then(NodeKind::capabilities_name)
            .is_some_and(|capabilities| capabilities.output)
        {
            issues.push(IssueData::new(
                "OUTPUT_INVALID",
                "/output",
                "node type is not eligible as output",
            ));
        }
        if node.is_some_and(|node| node.contains_key("next")) {
            issues.push(IssueData::new(
                "OUTPUT_INVALID",
                format!("/nodes/{output_index}/next"),
                "output node must not have an outgoing edge",
            ));
        }
    }

    for (index, node) in raw_nodes.iter().enumerate() {
        let Some(node) = node.object() else { continue };
        let Some(kind) = text(node, "type") else {
            continue;
        };
        if NodeKind::capabilities_name(kind).is_some_and(|capabilities| capabilities.ordinary_next)
            && Some(index) != output
            && !node.contains_key("next")
        {
            issues.push(IssueData::new(
                "GRAPH_DEAD_END",
                format!("/nodes/{index}"),
                "non-output node requires next",
            ));
        }
    }

    validate_node_specific_semantics(
        raw_nodes,
        &by_id,
        &flow_definitions,
        !reference_failure,
        issues,
    );
    if reference_failure || entry.is_none() || output.is_none() {
        return;
    }
    let entry = entry.unwrap();
    let output = output.unwrap();
    let reachable = reachable_from(entry, &edges);
    for (index, is_reachable) in reachable.iter().enumerate() {
        if !is_reachable {
            issues.push(IssueData::new(
                "GRAPH_UNREACHABLE",
                format!("/nodes/{index}/id"),
                "node is unreachable from entry",
            ));
        }
    }
    let components = strongly_connected(&edges);
    let mut has_cycle = false;
    for component in components {
        if component.len() > 1 || edges[component[0]].contains(&component[0]) {
            has_cycle = true;
            let index = *component
                .iter()
                .min_by_key(|index| {
                    raw_nodes[**index]
                        .object()
                        .and_then(|node| text(node, "id"))
                        .unwrap_or("")
                })
                .unwrap();
            issues.push(IssueData::new(
                "GRAPH_CYCLE",
                format!("/nodes/{index}/id"),
                "workflow graph contains a cycle",
            ));
        }
    }
    let terminal_reachable = can_reach_terminal(output, raw_nodes, &edges);
    for index in 0..raw_nodes.len() {
        if reachable[index] && !terminal_reachable[index] {
            issues.push(IssueData::new(
                "GRAPH_DEAD_END",
                format!("/nodes/{index}/id"),
                "successful path cannot reach output",
            ));
        }
    }
    if !has_cycle {
        validate_activity(raw_nodes, &edges, &by_id, entry, issues);
    }
}

fn validate_node_specific_semantics(
    nodes: &[JsonValue],
    by_id: &HashMap<&str, usize>,
    fan_flows: &HashMap<String, String>,
    flow_checks: bool,
    issues: &mut Vec<IssueData>,
) {
    for (index, raw) in nodes.iter().enumerate() {
        let Some(node) = raw.object() else { continue };
        if text(node, "type") == Some("map-schema") {
            validate_mapping_targets(node, index, issues);
        }
        if text(node, "type") == Some("deterministic-gate") {
            validate_gate_overlaps(node, index, issues);
        }
    }
    validate_flows(nodes, by_id, fan_flows, flow_checks, issues);
}
