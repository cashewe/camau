use std::collections::HashMap;

use crate::json_parsing::{JsonPointer, JsonValue};
use indexmap::IndexMap;

use super::{
    Branch, CompiledGraph, ConvergeInput, GateCase, Mapping, Node, NodeKind, ValueType,
    WeightedRoute, json_number, text,
};

pub fn compile(value: &JsonValue) -> Option<CompiledGraph> {
    let root = value.object()?;
    let raw_nodes = root.get("nodes")?.array()?;
    let mut by_id = HashMap::new();
    for (index, raw) in raw_nodes.iter().enumerate() {
        by_id.insert(raw.object()?.get("id")?.string()?.to_owned(), index);
    }

    let mut nodes = Vec::with_capacity(raw_nodes.len());
    for raw in raw_nodes {
        let object = raw.object()?;
        let id = text(object, "id")?.to_owned();
        let kind = match text(object, "type")? {
            "task" => NodeKind::Task {
                task: text(object, "task")?.to_owned(),
                next: reference(object, "next", &by_id),
            },
            "map-schema" => NodeKind::Map {
                mappings: object
                    .get("mappings")?
                    .array()?
                    .iter()
                    .map(parse_mapping)
                    .collect::<Option<Vec<_>>>()?,
                next: reference(object, "next", &by_id),
            },
            "deterministic-gate" => NodeKind::Deterministic {
                select: JsonPointer::parse(text(object, "select")?)?,
                cases: object
                    .get("cases")?
                    .array()?
                    .iter()
                    .map(|case| parse_case(case, &by_id))
                    .collect::<Option<Vec<_>>>()?,
            },
            "randomised-gate" => {
                let mut routes = object
                    .get("routes")?
                    .array()?
                    .iter()
                    .map(|route| {
                        let route = route.object()?;
                        Some(WeightedRoute {
                            weight: json_number(route.get("weight")?)?.as_f64(),
                            target: *by_id.get(text(route, "target")?)?,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                let scale = routes.iter().map(|route| route.weight).reduce(f64::max)?;
                if !scale.is_finite() || scale <= 0.0 {
                    return None;
                }
                for route in &mut routes {
                    route.weight /= scale;
                }
                let total: f64 = routes.iter().map(|route| route.weight).sum();
                if !total.is_finite() || total <= 0.0 {
                    return None;
                }
                NodeKind::Randomised { routes, total }
            }
            "fan-out" => NodeKind::FanOut {
                branches: object
                    .get("branches")?
                    .array()?
                    .iter()
                    .map(|branch| {
                        let branch = branch.object()?;
                        Some(Branch {
                            flow_id: text(branch, "id")?.to_owned(),
                            target: *by_id.get(text(branch, "target")?)?,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?,
            },
            "converge" => NodeKind::Converge {
                inputs: object
                    .get("inputs")?
                    .object()?
                    .iter()
                    .map(|(flow_id, source)| {
                        Some(ConvergeInput {
                            flow_id: flow_id.clone(),
                            source: *by_id.get(source.string()?)?,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?,
                next: reference(object, "next", &by_id),
            },
            "raise-error" => NodeKind::Raise {
                message: text(object, "message")?.to_owned(),
            },
            _ => return None,
        };
        nodes.push(Node { id, kind });
    }

    Some(CompiledGraph {
        entry: *by_id.get(text(root, "entry")?)?,
        output: *by_id.get(text(root, "output")?)?,
        nodes,
    })
}

fn parse_mapping(value: &JsonValue) -> Option<Mapping> {
    let mapping = value.object()?;
    Some(Mapping {
        target: JsonPointer::parse(text(mapping, "target")?)?,
        source: match text(mapping, "path-to-source") {
            Some(source) => Some(JsonPointer::parse(source)?),
            None => None,
        },
        default: mapping.get("default").cloned(),
        value_type: ValueType::parse(text(mapping, "type")?)?,
    })
}

fn parse_case(value: &JsonValue, by_id: &HashMap<String, usize>) -> Option<GateCase> {
    let case = value.object()?;
    let target = *by_id.get(text(case, "target")?)?;
    match text(case, "operator")? {
        "eq" => Some(GateCase::Eq {
            value: case.get("value")?.clone(),
            target,
        }),
        "lt" => Some(GateCase::Lt {
            value: json_number(case.get("value")?)?,
            target,
        }),
        "le" => Some(GateCase::Le {
            value: json_number(case.get("value")?)?,
            target,
        }),
        "ge" => Some(GateCase::Ge {
            value: json_number(case.get("value")?)?,
            target,
        }),
        "gt" => Some(GateCase::Gt {
            value: json_number(case.get("value")?)?,
            target,
        }),
        "in_range" => Some(GateCase::Range {
            lower: json_number(case.get("lower")?)?,
            upper: json_number(case.get("upper")?)?,
            target,
        }),
        "in_set" => Some(GateCase::Set {
            values: case
                .get("values")?
                .array()?
                .iter()
                .map(|value| value.string().map(str::to_owned))
                .collect::<Option<Vec<_>>>()?,
            target,
        }),
        "otherwise" => Some(GateCase::Otherwise { target }),
        _ => None,
    }
}

fn reference(
    object: &IndexMap<String, JsonValue>,
    key: &str,
    by_id: &HashMap<String, usize>,
) -> Option<usize> {
    object
        .get(key)
        .and_then(JsonValue::string)
        .and_then(|id| by_id.get(id))
        .copied()
}
