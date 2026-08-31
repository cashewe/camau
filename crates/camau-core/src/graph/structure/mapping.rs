use crate::diagnostics::IssueData;
use crate::json_value::JsonValue;

use super::{validate_pointer_field, validate_properties};
use crate::graph::ValueType;

pub(super) fn validate_mapping(value: &JsonValue, path: &str, issues: &mut Vec<IssueData>) {
    let Some(mapping) = value.object() else {
        issues.push(IssueData::new(
            "SCHEMA_TYPE",
            path,
            "mapping must be an object",
        ));
        return;
    };
    validate_properties(
        mapping,
        path,
        &["target", "path-to-source", "default", "type"],
        &["target", "type"],
        issues,
    );
    validate_pointer_field(
        mapping.get("target"),
        &format!("{path}/target"),
        false,
        issues,
    );
    if let Some(source) = mapping.get("path-to-source") {
        validate_pointer_field(
            Some(source),
            &format!("{path}/path-to-source"),
            true,
            issues,
        );
    }
    if !mapping.contains_key("path-to-source") && !mapping.contains_key("default") {
        issues.push(IssueData::new(
            "SCHEMA_REQUIRED",
            path,
            "mapping requires path-to-source or default",
        ));
    }
    let valid_type = matches!(
        mapping.get("type").and_then(JsonValue::string),
        Some("object" | "array" | "string" | "number" | "integer" | "boolean" | "null")
    );
    if mapping.contains_key("type") && !valid_type {
        issues.push(IssueData::new(
            "SCHEMA_VALUE",
            format!("{path}/type"),
            "unsupported mapping type",
        ));
    }
    if let (Some(default), Some(value_type)) = (
        mapping.get("default"),
        mapping
            .get("type")
            .and_then(JsonValue::string)
            .and_then(ValueType::parse),
    ) {
        if !value_type.matches(default) {
            issues.push(IssueData::new(
                "MAPPING_DEFAULT",
                format!("{path}/default"),
                "default does not satisfy the declared type",
            ));
        }
    }
}
