use std::collections::{HashMap, HashSet};

use crate::diagnostics::IssueData;
use crate::json_parsing::{JsonValue, join_pointer};

use super::super::text;

pub(super) fn validate_flows(
    nodes: &[JsonValue],
    by_id: &HashMap<&str, usize>,
    fan_flows: &HashMap<String, String>,
    enabled: bool,
    issues: &mut Vec<IssueData>,
) {
    if !enabled {
        return;
    }

    let mut source_convergence: HashMap<&str, String> = HashMap::new();
    let mut flow_consumption: HashMap<&str, Vec<String>> = HashMap::new();
    let mut convergence_defined = HashSet::new();
    for (index, raw) in nodes.iter().enumerate() {
        let Some(node) = raw.object() else { continue };
        if text(node, "type") != Some("converge") {
            continue;
        }
        let converge_id = text(node, "id").unwrap_or("");
        let Some(inputs) = node.get("inputs").and_then(JsonValue::object) else {
            continue;
        };
        let mut source_seen = HashSet::new();
        for (flow_id, source) in inputs {
            let path = join_pointer(&format!("/nodes/{index}/inputs"), flow_id);
            if fan_flows.contains_key(flow_id) {
                flow_consumption
                    .entry(flow_id)
                    .or_default()
                    .push(path.clone());
            } else if by_id.contains_key(flow_id.as_str())
                || !convergence_defined.insert(flow_id.as_str())
            {
                issues.push(IssueData::new(
                    "ID_COLLISION",
                    path.clone(),
                    "convergence flow identifier collides in the graph namespace",
                ));
            }
            let Some(source_id) = source.string() else {
                continue;
            };
            if !source_seen.insert(source_id) {
                issues.push(IssueData::new(
                    "FLOW_REUSED",
                    path.clone(),
                    "one source is supplied more than once to convergence",
                ));
            }
            if source_convergence
                .insert(source_id, converge_id.to_owned())
                .is_some()
            {
                issues.push(IssueData::new(
                    "FLOW_REUSED",
                    path.clone(),
                    "one source feeds multiple convergence points",
                ));
            }
            if let Some(source_index) = by_id.get(source_id) {
                let source_next = nodes[*source_index]
                    .object()
                    .and_then(|source| source.get("next"))
                    .and_then(JsonValue::string);
                if source_next != Some(converge_id) {
                    issues.push(IssueData::new(
                        "CONVERGE_LINK",
                        path,
                        "convergence input and source next do not agree",
                    ));
                }
            }
        }
        validate_reverse_links(nodes, inputs, converge_id, issues);
    }
    validate_flow_consumption(fan_flows, &flow_consumption, issues);
}

fn validate_reverse_links(
    nodes: &[JsonValue],
    inputs: &indexmap::IndexMap<String, JsonValue>,
    converge_id: &str,
    issues: &mut Vec<IssueData>,
) {
    for (source_index, source) in nodes.iter().enumerate() {
        let Some(source) = source.object() else {
            continue;
        };
        if source.get("next").and_then(JsonValue::string) == Some(converge_id) {
            let source_id = text(source, "id").unwrap_or("");
            if !inputs
                .values()
                .any(|value| value.string() == Some(source_id))
            {
                issues.push(IssueData::new(
                    "CONVERGE_LINK",
                    format!("/nodes/{source_index}/next"),
                    "source points to convergence but is absent from inputs",
                ));
            }
        }
    }
}

fn validate_flow_consumption(
    fan_flows: &HashMap<String, String>,
    consumption: &HashMap<&str, Vec<String>>,
    issues: &mut Vec<IssueData>,
) {
    for (flow_id, definition_path) in fan_flows {
        match consumption.get(flow_id.as_str()).map(Vec::len).unwrap_or(0) {
            0 => issues.push(IssueData::new(
                "FLOW_UNCONSUMED",
                definition_path,
                "fan-out flow is never consumed by convergence",
            )),
            1 => {}
            _ => {
                for path in consumption.get(flow_id.as_str()).unwrap().iter().skip(1) {
                    issues.push(IssueData::new(
                        "FLOW_REUSED",
                        path,
                        "fan-out flow is consumed more than once",
                    ));
                }
            }
        }
    }
}
