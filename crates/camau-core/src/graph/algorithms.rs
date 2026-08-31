use super::text;
use crate::json_value::JsonValue;
use indexmap::IndexMap;

pub(super) fn outgoing_references(
    node: &IndexMap<String, JsonValue>,
    index: usize,
) -> Vec<(String, String)> {
    let mut references = Vec::new();
    if let Some(next) = node.get("next").and_then(JsonValue::string) {
        references.push((next.to_owned(), format!("/nodes/{index}/next")));
    }
    match text(node, "type") {
        Some("deterministic-gate") => {
            if let Some(cases) = node.get("cases").and_then(JsonValue::array) {
                for (case_index, case) in cases.iter().enumerate() {
                    if let Some(target) = case
                        .object()
                        .and_then(|case| case.get("target"))
                        .and_then(JsonValue::string)
                    {
                        references.push((
                            target.to_owned(),
                            format!("/nodes/{index}/cases/{case_index}/target"),
                        ));
                    }
                }
            }
        }
        Some("randomised-gate") => {
            if let Some(routes) = node.get("routes").and_then(JsonValue::array) {
                for (route_index, route) in routes.iter().enumerate() {
                    if let Some(target) = route
                        .object()
                        .and_then(|route| route.get("target"))
                        .and_then(JsonValue::string)
                    {
                        references.push((
                            target.to_owned(),
                            format!("/nodes/{index}/routes/{route_index}/target"),
                        ));
                    }
                }
            }
        }
        Some("fan-out") => {
            if let Some(branches) = node.get("branches").and_then(JsonValue::array) {
                for (branch_index, branch) in branches.iter().enumerate() {
                    if let Some(target) = branch
                        .object()
                        .and_then(|branch| branch.get("target"))
                        .and_then(JsonValue::string)
                    {
                        references.push((
                            target.to_owned(),
                            format!("/nodes/{index}/branches/{branch_index}/target"),
                        ));
                    }
                }
            }
        }
        _ => {}
    }
    references
}

pub(super) fn reachable_from(entry: usize, edges: &[Vec<usize>]) -> Vec<bool> {
    let mut reachable = vec![false; edges.len()];
    let mut stack = vec![entry];
    while let Some(node) = stack.pop() {
        if std::mem::replace(&mut reachable[node], true) {
            continue;
        }
        stack.extend(edges[node].iter().copied());
    }
    reachable
}

pub(super) fn can_reach_terminal(
    output: usize,
    nodes: &[JsonValue],
    edges: &[Vec<usize>],
) -> Vec<bool> {
    let mut reverse = vec![Vec::new(); edges.len()];
    for (source, targets) in edges.iter().enumerate() {
        for target in targets {
            reverse[*target].push(source);
        }
    }
    let mut result = vec![false; edges.len()];
    let mut stack = vec![output];
    for (index, node) in nodes.iter().enumerate() {
        if node.object().and_then(|node| text(node, "type")) == Some("raise-error") {
            stack.push(index);
        }
    }
    while let Some(node) = stack.pop() {
        if std::mem::replace(&mut result[node], true) {
            continue;
        }
        stack.extend(reverse[node].iter().copied());
    }
    result
}

pub(super) fn strongly_connected(edges: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut visited = vec![false; edges.len()];
    let mut order = Vec::with_capacity(edges.len());
    for start in 0..edges.len() {
        if visited[start] {
            continue;
        }
        let mut stack = vec![(start, 0usize)];
        visited[start] = true;
        while let Some((node, edge_index)) = stack.pop() {
            if edge_index < edges[node].len() {
                stack.push((node, edge_index + 1));
                let target = edges[node][edge_index];
                if !visited[target] {
                    visited[target] = true;
                    stack.push((target, 0));
                }
            } else {
                order.push(node);
            }
        }
    }
    let mut reverse = vec![Vec::new(); edges.len()];
    for (source, targets) in edges.iter().enumerate() {
        for target in targets {
            reverse[*target].push(source);
        }
    }
    visited.fill(false);
    let mut components = Vec::new();
    for start in order.into_iter().rev() {
        if visited[start] {
            continue;
        }
        let mut component = Vec::new();
        let mut stack = vec![start];
        visited[start] = true;
        while let Some(node) = stack.pop() {
            component.push(node);
            for target in &reverse[node] {
                if !visited[*target] {
                    visited[*target] = true;
                    stack.push(*target);
                }
            }
        }
        components.push(component);
    }
    components
}
