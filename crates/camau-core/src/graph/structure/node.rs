use std::collections::HashSet;

use crate::diagnostics::IssueData;
use crate::json_pointer::join_pointer;
use crate::json_value::JsonValue;

use super::gate::{validate_case, validate_otherwise};
use super::mapping::validate_mapping;
use super::{
    validate_identifier_field, validate_identifier_text, validate_optional_identifier,
    validate_pointer_field, validate_properties,
};
use crate::graph::json_number;

pub(super) fn validate_node(value: &JsonValue, index: usize, issues: &mut Vec<IssueData>) {
    let path = format!("/nodes/{index}");
    let Some(node) = value.object() else {
        issues.push(IssueData::new(
            "SCHEMA_TYPE",
            path,
            "node must be an object",
        ));
        return;
    };
    for field in ["id", "type"] {
        if !node.contains_key(field) {
            issues.push(IssueData::new(
                "SCHEMA_REQUIRED",
                &path,
                format!("required property {field:?} is missing"),
            ));
        }
    }
    validate_identifier_field(node.get("id"), &format!("{path}/id"), true, issues);
    let Some(kind) = node.get("type").and_then(JsonValue::string) else {
        if node.contains_key("type") {
            issues.push(IssueData::new(
                "SCHEMA_TYPE",
                format!("{path}/type"),
                "node type must be a string",
            ));
        }
        return;
    };
    match kind {
        "task" => {
            validate_properties(
                node,
                &path,
                &["id", "type", "task", "next"],
                &["id", "type", "task"],
                issues,
            );
            validate_identifier_field(node.get("task"), &format!("{path}/task"), false, issues);
            validate_optional_identifier(node.get("next"), &format!("{path}/next"), issues);
        }
        "map-schema" => {
            validate_properties(
                node,
                &path,
                &["id", "type", "mappings", "next"],
                &["id", "type", "mappings"],
                issues,
            );
            validate_optional_identifier(node.get("next"), &format!("{path}/next"), issues);
            match node.get("mappings") {
                Some(JsonValue::Array(mappings)) => {
                    if mappings.is_empty() {
                        issues.push(IssueData::new(
                            "SCHEMA_VALUE",
                            format!("{path}/mappings"),
                            "mappings must not be empty",
                        ));
                    }
                    for (mapping_index, mapping) in mappings.iter().enumerate() {
                        validate_mapping(
                            mapping,
                            &format!("{path}/mappings/{mapping_index}"),
                            issues,
                        );
                    }
                }
                Some(_) => issues.push(IssueData::new(
                    "SCHEMA_TYPE",
                    format!("{path}/mappings"),
                    "mappings must be an array",
                )),
                None => {}
            }
        }
        "deterministic-gate" => {
            validate_properties(
                node,
                &path,
                &["id", "type", "select", "cases"],
                &["id", "type", "select", "cases"],
                issues,
            );
            validate_pointer_field(node.get("select"), &format!("{path}/select"), true, issues);
            match node.get("cases") {
                Some(JsonValue::Array(cases)) => {
                    if cases.len() < 2 {
                        issues.push(IssueData::new(
                            "GATE_CASE",
                            format!("{path}/cases"),
                            "deterministic gate requires at least two cases",
                        ));
                    }
                    for (case_index, case) in cases.iter().enumerate() {
                        validate_case(case, &format!("{path}/cases/{case_index}"), issues);
                    }
                    validate_otherwise(cases, &format!("{path}/cases"), issues);
                }
                Some(_) => issues.push(IssueData::new(
                    "SCHEMA_TYPE",
                    format!("{path}/cases"),
                    "cases must be an array",
                )),
                None => {}
            }
        }
        "randomised-gate" => {
            validate_properties(
                node,
                &path,
                &["id", "type", "routes"],
                &["id", "type", "routes"],
                issues,
            );
            match node.get("routes") {
                Some(JsonValue::Array(routes)) => {
                    if routes.len() < 2 {
                        issues.push(IssueData::new(
                            "SCHEMA_VALUE",
                            format!("{path}/routes"),
                            "randomised gate requires at least two routes",
                        ));
                    }
                    let mut targets = HashSet::new();
                    for (route_index, route) in routes.iter().enumerate() {
                        let route_path = format!("{path}/routes/{route_index}");
                        let Some(route) = route.object() else {
                            issues.push(IssueData::new(
                                "SCHEMA_TYPE",
                                route_path,
                                "route must be an object",
                            ));
                            continue;
                        };
                        validate_properties(
                            route,
                            &route_path,
                            &["weight", "target"],
                            &["weight", "target"],
                            issues,
                        );
                        match route.get("weight").and_then(json_number) {
                            Some(weight)
                                if weight.as_f64().is_finite() && weight.as_f64() > 0.0 => {}
                            Some(_) => issues.push(IssueData::new(
                                "RANDOM_WEIGHT",
                                format!("{route_path}/weight"),
                                "weight must be finite and positive",
                            )),
                            None if route.contains_key("weight") => issues.push(IssueData::new(
                                "RANDOM_WEIGHT",
                                format!("{route_path}/weight"),
                                "weight must be a number",
                            )),
                            None => {}
                        }
                        validate_identifier_field(
                            route.get("target"),
                            &format!("{route_path}/target"),
                            true,
                            issues,
                        );
                        if let Some(target) = route.get("target").and_then(JsonValue::string) {
                            if !targets.insert(target) {
                                issues.push(IssueData::new(
                                    "RANDOM_TARGET",
                                    format!("{route_path}/target"),
                                    "randomised target is repeated",
                                ));
                            }
                        }
                    }
                }
                Some(_) => issues.push(IssueData::new(
                    "SCHEMA_TYPE",
                    format!("{path}/routes"),
                    "routes must be an array",
                )),
                None => {}
            }
        }
        "fan-out" => {
            validate_properties(
                node,
                &path,
                &["id", "type", "branches"],
                &["id", "type", "branches"],
                issues,
            );
            match node.get("branches") {
                Some(JsonValue::Array(branches)) => {
                    if branches.len() < 2 {
                        issues.push(IssueData::new(
                            "SCHEMA_VALUE",
                            format!("{path}/branches"),
                            "fan-out requires at least two branches",
                        ));
                    }
                    for (branch_index, branch) in branches.iter().enumerate() {
                        let branch_path = format!("{path}/branches/{branch_index}");
                        let Some(branch) = branch.object() else {
                            issues.push(IssueData::new(
                                "SCHEMA_TYPE",
                                branch_path,
                                "branch must be an object",
                            ));
                            continue;
                        };
                        validate_properties(
                            branch,
                            &branch_path,
                            &["id", "target"],
                            &["id", "target"],
                            issues,
                        );
                        validate_identifier_field(
                            branch.get("id"),
                            &format!("{branch_path}/id"),
                            true,
                            issues,
                        );
                        validate_identifier_field(
                            branch.get("target"),
                            &format!("{branch_path}/target"),
                            true,
                            issues,
                        );
                    }
                }
                Some(_) => issues.push(IssueData::new(
                    "SCHEMA_TYPE",
                    format!("{path}/branches"),
                    "branches must be an array",
                )),
                None => {}
            }
        }
        "converge" => {
            validate_properties(
                node,
                &path,
                &["id", "type", "inputs", "next"],
                &["id", "type", "inputs"],
                issues,
            );
            validate_optional_identifier(node.get("next"), &format!("{path}/next"), issues);
            match node.get("inputs") {
                Some(JsonValue::Object(inputs)) => {
                    if inputs.len() < 2 {
                        issues.push(IssueData::new(
                            "SCHEMA_VALUE",
                            format!("{path}/inputs"),
                            "converge requires at least two inputs",
                        ));
                    }
                    for (flow_id, source) in inputs {
                        validate_identifier_text(
                            flow_id,
                            &join_pointer(&format!("{path}/inputs"), flow_id),
                            true,
                            issues,
                        );
                        validate_identifier_field(
                            Some(source),
                            &join_pointer(&format!("{path}/inputs"), flow_id),
                            true,
                            issues,
                        );
                    }
                }
                Some(_) => issues.push(IssueData::new(
                    "SCHEMA_TYPE",
                    format!("{path}/inputs"),
                    "inputs must be an object",
                )),
                None => {}
            }
        }
        "raise-error" => {
            validate_properties(
                node,
                &path,
                &["id", "type", "message"],
                &["id", "type", "message"],
                issues,
            );
            match node.get("message") {
                Some(JsonValue::String(message)) if message.is_empty() => {
                    issues.push(IssueData::new(
                        "SCHEMA_VALUE",
                        format!("{path}/message"),
                        "message must not be empty",
                    ))
                }
                Some(JsonValue::String(_)) => {}
                Some(_) => issues.push(IssueData::new(
                    "SCHEMA_TYPE",
                    format!("{path}/message"),
                    "message must be a string",
                )),
                None => {}
            }
        }
        _ => issues.push(IssueData::new(
            "SCHEMA_VALUE",
            format!("{path}/type"),
            "unsupported node type",
        )),
    }
}
