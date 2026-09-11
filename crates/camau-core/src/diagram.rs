use std::collections::HashMap;
use std::fmt::Write as _;

use crate::graph::{
    CompiledGraph, GateCase, JsonNumber, Mapping, NodeKind, ValueType, WeightedRoute,
};
use crate::json_parsing::JsonValue;

#[derive(Clone, Copy)]
enum EdgeKind {
    Sequential,
    Exclusive,
    Concurrent,
}

pub fn render_markdown(graph: &CompiledGraph) -> String {
    let mut output = String::from("# Camau workflow graph\n\n```mermaid\nflowchart TD\n");
    let convergence_labels = convergence_labels(graph);
    let incoming_counts = incoming_counts(graph);
    let _ = writeln!(output, "    input([\"Input\"]) --> n{}", graph.entry);

    for (index, node) in graph.nodes.iter().enumerate() {
        let label = format!(
            "{}<br/><small>{}</small>",
            mermaid_text(&node.id),
            node_type_label(&node.kind)
        );
        let declaration = match node.kind {
            NodeKind::Task { .. } => format!("[\"{label}\"]"),
            NodeKind::Map { .. } => format!("[/\"{label}\"/]"),
            NodeKind::Deterministic { .. } | NodeKind::Randomised { .. } => {
                format!("{{\"{label}\"}}")
            }
            NodeKind::FanOut { .. } => format!("{{{{\"{label}\"}}}}"),
            NodeKind::Converge { .. } => format!("([\"{label}\"])"),
            NodeKind::Raise { .. } => format!("[\"{label}\"]"),
        };
        let _ = writeln!(output, "    n{index}{declaration}");
    }

    for (index, node) in graph.nodes.iter().enumerate() {
        match &node.kind {
            NodeKind::Task { next, .. }
            | NodeKind::Map { next, .. }
            | NodeKind::Converge { next, .. } => {
                if let Some(target) = next {
                    write_edge(
                        &mut output,
                        index,
                        *target,
                        convergence_labels.get(&(index, *target)).cloned(),
                        edge_kind(graph, &incoming_counts, index, *target),
                    );
                }
            }
            NodeKind::Deterministic { cases, .. } => {
                for case in cases {
                    write_edge(
                        &mut output,
                        index,
                        case_target(case),
                        Some(gate_case_label(case)),
                        EdgeKind::Exclusive,
                    );
                }
            }
            NodeKind::Randomised { routes, .. } => {
                let scale = routes
                    .iter()
                    .map(|route| route.weight)
                    .fold(0.0_f64, f64::max);
                let scaled_total = routes.iter().map(|route| route.weight / scale).sum();
                for route in routes {
                    write_edge(
                        &mut output,
                        index,
                        route.target,
                        Some(route_label(route, scale, scaled_total)),
                        EdgeKind::Exclusive,
                    );
                }
            }
            NodeKind::FanOut { branches } => {
                for branch in branches {
                    write_edge(
                        &mut output,
                        index,
                        branch.target,
                        Some(format!("branch: {}", branch.flow_id)),
                        EdgeKind::Concurrent,
                    );
                }
            }
            NodeKind::Raise { .. } => {}
        }
    }

    let _ = writeln!(output, "    n{} --> output([\"Output\"])", graph.output);
    output.push_str("    classDef boundary stroke-width:3px\n");
    output.push_str("    classDef failure stroke:#c62828,stroke-width:2px\n");
    output.push_str("    class input,output boundary\n");
    let failures = graph
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| {
            matches!(node.kind, NodeKind::Raise { .. }).then_some(format!("n{index}"))
        })
        .collect::<Vec<_>>();
    if !failures.is_empty() {
        let _ = writeln!(output, "    class {} failure", failures.join(","));
    }
    output.push_str("```\n\n## How to read this graph\n\n");
    let _ = writeln!(
        output,
        "- Routing starts at {} and succeeds at {}.",
        markdown_code(&graph.nodes[graph.entry].id),
        markdown_code(&graph.nodes[graph.output].id)
    );
    output.push_str("- Dotted arrows are mutually exclusive: exactly one route is active. Thick arrows belong to concurrently active branches. Solid arrows are sequential continuation.\n");
    output.push_str("- Diamond nodes select one route. Hexagonal fan-out nodes start every branch concurrently. Rounded convergence nodes wait for every labelled input.\n");
    output.push_str("- A red failure node stops the run with `GWALL`; it does not lead to the successful output.\n\n");
    output.push_str("## Node details\n\n| Node | Behaviour |\n|---|---|\n");
    for node in &graph.nodes {
        let _ = writeln!(
            output,
            "| {} | {} |",
            markdown_code(&node.id),
            node_description(graph, &node.kind)
        );
    }
    output
}

fn write_edge(
    output: &mut String,
    source: usize,
    target: usize,
    label: Option<String>,
    kind: EdgeKind,
) {
    let label = label.map(|label| mermaid_text(&label));
    let edge = match (kind, label) {
        (EdgeKind::Sequential, Some(label)) => format!("-->|\"{label}\"|"),
        (EdgeKind::Sequential, None) => "-->".to_owned(),
        (EdgeKind::Exclusive, Some(label)) => format!("-. \"{label}\" .->"),
        (EdgeKind::Exclusive, None) => "-.->".to_owned(),
        (EdgeKind::Concurrent, Some(label)) => format!("== \"{label}\" ==>"),
        (EdgeKind::Concurrent, None) => "==>".to_owned(),
    };
    let _ = writeln!(output, "    n{source} {edge} n{target}");
}

fn edge_kind(
    graph: &CompiledGraph,
    incoming_counts: &[usize],
    source: usize,
    target: usize,
) -> EdgeKind {
    if matches!(graph.nodes[target].kind, NodeKind::Converge { .. })
        || matches!(graph.nodes[source].kind, NodeKind::FanOut { .. })
    {
        EdgeKind::Concurrent
    } else if matches!(
        graph.nodes[source].kind,
        NodeKind::Deterministic { .. } | NodeKind::Randomised { .. }
    ) || incoming_counts[target] > 1
    {
        EdgeKind::Exclusive
    } else {
        EdgeKind::Sequential
    }
}

fn incoming_counts(graph: &CompiledGraph) -> Vec<usize> {
    let mut counts = vec![0; graph.nodes.len()];
    for node in &graph.nodes {
        match &node.kind {
            NodeKind::Task { next, .. }
            | NodeKind::Map { next, .. }
            | NodeKind::Converge { next, .. } => {
                if let Some(target) = next {
                    counts[*target] += 1;
                }
            }
            NodeKind::Deterministic { cases, .. } => {
                for case in cases {
                    counts[case_target(case)] += 1;
                }
            }
            NodeKind::Randomised { routes, .. } => {
                for route in routes {
                    counts[route.target] += 1;
                }
            }
            NodeKind::FanOut { branches } => {
                for branch in branches {
                    counts[branch.target] += 1;
                }
            }
            NodeKind::Raise { .. } => {}
        }
    }
    counts
}

fn convergence_labels(graph: &CompiledGraph) -> HashMap<(usize, usize), String> {
    graph
        .nodes
        .iter()
        .enumerate()
        .flat_map(|(target, node)| match &node.kind {
            NodeKind::Converge { inputs, .. } => inputs
                .iter()
                .map(move |input| ((input.source, target), format!("flow: {}", input.flow_id)))
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect()
}

fn node_type_label(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Task { .. } => "task",
        NodeKind::Map { .. } => "map schema",
        NodeKind::Deterministic { .. } => "deterministic gate",
        NodeKind::Randomised { .. } => "randomised gate",
        NodeKind::FanOut { .. } => "fan-out",
        NodeKind::Converge { .. } => "convergence",
        NodeKind::Raise { .. } => "raise error",
    }
}

fn gate_case_label(case: &GateCase) -> String {
    match case {
        GateCase::Eq { value, .. } => format!("= {}", json_text(value)),
        GateCase::Lt { value, .. } => format!("< {}", number_text(*value)),
        GateCase::Le { value, .. } => format!("<= {}", number_text(*value)),
        GateCase::Ge { value, .. } => format!(">= {}", number_text(*value)),
        GateCase::Gt { value, .. } => format!("> {}", number_text(*value)),
        GateCase::Range { lower, upper, .. } => {
            format!("{} to {}", number_text(*lower), number_text(*upper))
        }
        GateCase::Set { values, .. } => format!(
            "one of {}",
            values
                .iter()
                .map(|value| serde_json::to_string(value).expect("strings serialize"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        GateCase::Otherwise { .. } => "otherwise".to_owned(),
    }
}

fn case_target(case: &GateCase) -> usize {
    match case {
        GateCase::Eq { target, .. }
        | GateCase::Lt { target, .. }
        | GateCase::Le { target, .. }
        | GateCase::Ge { target, .. }
        | GateCase::Gt { target, .. }
        | GateCase::Range { target, .. }
        | GateCase::Set { target, .. }
        | GateCase::Otherwise { target } => *target,
    }
}

fn route_label(route: &WeightedRoute, scale: f64, scaled_total: f64) -> String {
    let percentage = (route.weight / scale) / scaled_total * 100.0;
    if (percentage - percentage.round()).abs() < 1e-9 {
        format!("{}%", percentage.round() as u64)
    } else {
        format!("{percentage:.1}%")
    }
}

fn node_description(graph: &CompiledGraph, kind: &NodeKind) -> String {
    match kind {
        NodeKind::Task { task, .. } => format!("Calls task {}.", markdown_code(task)),
        NodeKind::Map { mappings, .. } => format!(
            "Builds a new object: {}.",
            mappings
                .iter()
                .map(mapping_description)
                .collect::<Vec<_>>()
                .join("; ")
        ),
        NodeKind::Deterministic { select, .. } => format!(
            "Reads {} and follows the first matching labelled route.",
            markdown_code(select.as_str())
        ),
        NodeKind::Randomised { .. } => {
            "Chooses one labelled route using the normalised weights.".to_owned()
        }
        NodeKind::FanOut { branches } => format!(
            "Starts all {} labelled flow branches concurrently.",
            branches.len()
        ),
        NodeKind::Converge { inputs, .. } => format!(
            "Waits for {} and creates an object keyed by those flow IDs.",
            inputs
                .iter()
                .map(|input| format!(
                    "{} from {}",
                    markdown_code(&input.flow_id),
                    markdown_code(&graph.nodes[input.source].id)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        NodeKind::Raise { message } => {
            format!("Stops routing with `GWALL`: {}.", markdown_text(message))
        }
    }
}

fn mapping_description(mapping: &Mapping) -> String {
    let value = match (&mapping.source, &mapping.default) {
        (Some(source), Some(default)) => format!(
            "{} or default {}",
            markdown_code(source.as_str()),
            markdown_code(&json_text(default))
        ),
        (Some(source), None) => markdown_code(source.as_str()),
        (None, Some(default)) => format!("default {}", markdown_code(&json_text(default))),
        (None, None) => unreachable!("valid mappings have a source or default"),
    };
    format!(
        "{} &larr; {} ({})",
        markdown_code(mapping.target.as_str()),
        value,
        value_type_label(mapping.value_type)
    )
}

fn value_type_label(value_type: ValueType) -> &'static str {
    match value_type {
        ValueType::Object => "object",
        ValueType::Array => "array",
        ValueType::String => "string",
        ValueType::Number => "number",
        ValueType::Integer => "integer",
        ValueType::Boolean => "boolean",
        ValueType::Null => "null",
    }
}

fn number_text(number: JsonNumber) -> String {
    match number {
        JsonNumber::Integer(value) => value.to_string(),
        JsonNumber::Float(value) => value.to_string(),
    }
}

fn json_text(value: &JsonValue) -> String {
    match value {
        JsonValue::Null => "null".to_owned(),
        JsonValue::Bool(value) => value.to_string(),
        JsonValue::Integer(value) => value.to_string(),
        JsonValue::Number(value) => value.to_string(),
        JsonValue::String(value) => serde_json::to_string(value).expect("strings serialize"),
        JsonValue::Array(values) => format!(
            "[{}]",
            values.iter().map(json_text).collect::<Vec<_>>().join(",")
        ),
        JsonValue::Object(values) => format!(
            "{{{}}}",
            values
                .iter()
                .map(|(key, value)| format!(
                    "{}:{}",
                    serde_json::to_string(key).expect("strings serialize"),
                    json_text(value)
                ))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

fn mermaid_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('|', "&#124;")
        .replace(['\r', '\n'], " ")
}

fn markdown_code(value: &str) -> String {
    format!("<code>{}</code>", markdown_text(value))
}

fn markdown_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('|', "&#124;")
        .replace('*', "&#42;")
        .replace('_', "&#95;")
        .replace('[', "&#91;")
        .replace(']', "&#93;")
        .replace('`', "&#96;")
        .replace(['\r', '\n'], " ")
}

#[cfg(test)]
mod tests {
    use crate::assessment::assess_json;
    use crate::graph::compile;

    use super::render_markdown;

    #[test]
    fn renders_routes_convergence_and_user_text_safely() {
        let source = r#"{
            "entry":"choose","output":"join","nodes":[
                {"id":"choose","type":"deterministic-gate","select":"/kind","cases":[
                    {"operator":"eq","value":"a|b","target":"split"},
                    {"operator":"otherwise","target":"stop"}
                ]},
                {"id":"split","type":"fan-out","branches":[
                    {"id":"left","target":"call-left"},{"id":"right","target":"call-right"}
                ]},
                {"id":"call-left","type":"task","task":"left-task","next":"join"},
                {"id":"call-right","type":"task","task":"right-task","next":"join"},
                {"id":"join","type":"converge","inputs":{"left":"call-left","right":"call-right"}},
                {"id":"stop","type":"raise-error","message":"not <allowed> | retry"}
            ]
        }"#;
        let (value, assessment) = assess_json(source);
        assert!(assessment.valid(), "{}", assessment.to_text());

        let markdown = render_markdown(&compile(&value.unwrap()).unwrap());

        assert!(markdown.starts_with("# Camau workflow graph\n\n```mermaid"));
        assert!(markdown.contains("branch: left"));
        assert!(markdown.contains("flow: right"));
        assert!(markdown.contains("&quot;a&#124;b&quot;"));
        assert!(markdown.contains("not &lt;allowed&gt; &#124; retry"));
    }

    #[test]
    fn renders_the_gateway_example_with_every_node_type() {
        let source = include_str!("../../../tests/examples/gateway-routing-plan.json");
        let (value, assessment) = assess_json(source);
        assert!(assessment.valid(), "{}", assessment.to_text());

        let markdown = render_markdown(&compile(&value.unwrap()).unwrap());

        assert!(markdown.contains("deterministic gate"));
        assert!(markdown.contains("randomised gate"));
        assert!(markdown.contains("fan-out"));
        assert!(markdown.contains("convergence"));
        assert!(markdown.contains("raise error"));
        assert!(markdown.contains("map schema"));
        assert!(markdown.contains("task"));
        assert!(markdown.contains("n3 -.-> n5"));
        assert!(markdown.contains("n4 -.-> n5"));
        assert!(markdown.contains("n5 == \"branch: fraud-signal\" ==> n6"));
        assert!(markdown.contains("n7 == \"flow: fraud-signal\" ==> n10"));
        assert!(markdown.contains("n10 --> n11"));
    }

    #[test]
    fn normalises_large_randomised_weights_without_overflow() {
        let source = r#"{
            "entry":"choose","output":"done","nodes":[
                {"id":"choose","type":"randomised-gate","routes":[
                    {"weight":1e308,"target":"left"},{"weight":1e308,"target":"right"}
                ]},
                {"id":"left","type":"task","task":"left-task","next":"done"},
                {"id":"right","type":"task","task":"right-task","next":"done"},
                {"id":"done","type":"task","task":"finish"}
            ]
        }"#;
        let (value, assessment) = assess_json(source);
        assert!(assessment.valid(), "{}", assessment.to_text());

        let markdown = render_markdown(&compile(&value.unwrap()).unwrap());

        assert_eq!(markdown.matches("-. \"50%\" .->").count(), 2);
    }
}
