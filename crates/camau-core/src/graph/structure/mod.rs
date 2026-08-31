mod gate;
mod mapping;
mod node;

use indexmap::IndexMap;

use crate::diagnostics::IssueData;
use crate::json_pointer::{join_pointer, valid_identifier, valid_pointer};
use crate::json_value::JsonValue;

pub(super) fn validate_structure(value: &JsonValue, issues: &mut Vec<IssueData>) {
    let Some(root) = value.object() else {
        issues.push(IssueData::new(
            "SCHEMA_TYPE",
            "",
            "specification must be an object",
        ));
        return;
    };
    validate_properties(
        root,
        "",
        &["entry", "output", "nodes"],
        &["entry", "output", "nodes"],
        issues,
    );
    validate_identifier_field(root.get("entry"), "/entry", true, issues);
    validate_identifier_field(root.get("output"), "/output", true, issues);

    let Some(nodes) = root.get("nodes") else {
        return;
    };
    let Some(nodes) = nodes.array() else {
        issues.push(IssueData::new(
            "SCHEMA_TYPE",
            "/nodes",
            "nodes must be an array",
        ));
        return;
    };
    if nodes.is_empty() {
        issues.push(IssueData::new(
            "SCHEMA_VALUE",
            "/nodes",
            "nodes must not be empty",
        ));
    }
    for (index, node) in nodes.iter().enumerate() {
        node::validate_node(node, index, issues);
    }
}

fn validate_properties(
    object: &IndexMap<String, JsonValue>,
    path: &str,
    allowed: &[&str],
    required: &[&str],
    issues: &mut Vec<IssueData>,
) {
    for required in required {
        if !object.contains_key(*required) {
            issues.push(IssueData::new(
                "SCHEMA_REQUIRED",
                path,
                format!("required property {required:?} is missing"),
            ));
        }
    }
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            issues.push(IssueData::new(
                "SCHEMA_UNKNOWN",
                join_pointer(path, key),
                "unknown property",
            ));
        }
    }
}

fn validate_optional_identifier(
    value: Option<&JsonValue>,
    path: &str,
    issues: &mut Vec<IssueData>,
) {
    if value.is_some() {
        validate_identifier_field(value, path, true, issues);
    }
}

fn validate_identifier_field(
    value: Option<&JsonValue>,
    path: &str,
    reserve: bool,
    issues: &mut Vec<IssueData>,
) {
    match value {
        Some(JsonValue::String(value)) => validate_identifier_text(value, path, reserve, issues),
        Some(_) => issues.push(IssueData::new(
            "SCHEMA_TYPE",
            path,
            "identifier must be a string",
        )),
        None => {}
    }
}

fn validate_identifier_text(value: &str, path: &str, reserve: bool, issues: &mut Vec<IssueData>) {
    if !valid_identifier(value) {
        issues.push(IssueData::new(
            "ID_INVALID",
            path,
            "identifier has invalid syntax",
        ));
    } else if reserve && value == "GWALL" {
        issues.push(IssueData::new("ID_RESERVED", path, "GWALL is reserved"));
    }
}

fn validate_pointer_field(
    value: Option<&JsonValue>,
    path: &str,
    allow_root: bool,
    issues: &mut Vec<IssueData>,
) {
    match value {
        Some(JsonValue::String(value)) if !valid_pointer(value, allow_root) => {
            issues.push(IssueData::new("SCHEMA_VALUE", path, "invalid JSON Pointer"))
        }
        Some(JsonValue::String(_)) => {}
        Some(_) => issues.push(IssueData::new(
            "SCHEMA_TYPE",
            path,
            "JSON Pointer must be a string",
        )),
        None => {}
    }
}
