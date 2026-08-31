use std::cmp::Ordering;
use std::collections::HashSet;

use crate::diagnostics::IssueData;
use crate::json_value::JsonValue;

use super::{validate_identifier_field, validate_properties};
use crate::graph::{compare_numbers, json_number};

pub(super) fn validate_case(value: &JsonValue, path: &str, issues: &mut Vec<IssueData>) {
    let Some(case) = value.object() else {
        issues.push(IssueData::new(
            "SCHEMA_TYPE",
            path,
            "gate case must be an object",
        ));
        return;
    };
    if !case.contains_key("operator") {
        issues.push(IssueData::new(
            "SCHEMA_REQUIRED",
            path,
            "required property \"operator\" is missing",
        ));
        return;
    }
    let Some(operator) = case.get("operator").and_then(JsonValue::string) else {
        issues.push(IssueData::new(
            "GATE_CASE",
            format!("{path}/operator"),
            "operator must be a string",
        ));
        return;
    };
    let (allowed, required): (&[&str], &[&str]) = match operator {
        "eq" => (
            &["operator", "value", "target"],
            &["operator", "value", "target"],
        ),
        "lt" | "le" | "ge" | "gt" => (
            &["operator", "value", "target"],
            &["operator", "value", "target"],
        ),
        "in_range" => (
            &["operator", "lower", "upper", "target"],
            &["operator", "lower", "upper", "target"],
        ),
        "in_set" => (
            &["operator", "values", "target"],
            &["operator", "values", "target"],
        ),
        "otherwise" => (&["operator", "target"], &["operator", "target"]),
        _ => {
            issues.push(IssueData::new(
                "GATE_CASE",
                format!("{path}/operator"),
                "unsupported deterministic operator",
            ));
            return;
        }
    };
    validate_properties(case, path, allowed, required, issues);
    validate_identifier_field(case.get("target"), &format!("{path}/target"), true, issues);
    match operator {
        "eq" => {
            if let Some(value) = case.get("value") {
                if matches!(value, JsonValue::Object(_) | JsonValue::Array(_)) {
                    issues.push(IssueData::new(
                        "GATE_CASE",
                        format!("{path}/value"),
                        "eq requires a scalar value",
                    ));
                }
            }
        }
        "lt" | "le" | "ge" | "gt" => {
            if case.contains_key("value") && case.get("value").and_then(json_number).is_none() {
                issues.push(IssueData::new(
                    "GATE_CASE",
                    format!("{path}/value"),
                    "numeric operator requires a number",
                ));
            }
        }
        "in_range" => {
            let lower = case.get("lower").and_then(json_number);
            let upper = case.get("upper").and_then(json_number);
            if lower.is_none() && case.contains_key("lower") {
                issues.push(IssueData::new(
                    "GATE_CASE",
                    format!("{path}/lower"),
                    "range lower bound must be a number",
                ));
            }
            if upper.is_none() && case.contains_key("upper") {
                issues.push(IssueData::new(
                    "GATE_CASE",
                    format!("{path}/upper"),
                    "range upper bound must be a number",
                ));
            }
            if let (Some(lower), Some(upper)) = (lower, upper) {
                if compare_numbers(lower, upper) == Ordering::Greater {
                    issues.push(IssueData::new(
                        "GATE_CASE",
                        format!("{path}/lower"),
                        "range lower bound exceeds upper bound",
                    ));
                }
            }
        }
        "in_set" => match case.get("values") {
            Some(JsonValue::Array(values)) => {
                if values.is_empty() || values.iter().any(|value| value.string().is_none()) {
                    issues.push(IssueData::new(
                        "GATE_CASE",
                        format!("{path}/values"),
                        "in_set requires a non-empty string array",
                    ));
                } else {
                    let mut unique = HashSet::new();
                    if values
                        .iter()
                        .filter_map(JsonValue::string)
                        .any(|value| !unique.insert(value))
                    {
                        issues.push(IssueData::new(
                            "GATE_CASE",
                            format!("{path}/values"),
                            "in_set values must be unique",
                        ));
                    }
                }
            }
            Some(_) => issues.push(IssueData::new(
                "GATE_CASE",
                format!("{path}/values"),
                "in_set values must be an array",
            )),
            None => {}
        },
        _ => {}
    }
}

pub(super) fn validate_otherwise(cases: &[JsonValue], path: &str, issues: &mut Vec<IssueData>) {
    let positions: Vec<usize> = cases
        .iter()
        .enumerate()
        .filter_map(|(index, case)| {
            (case.object()?.get("operator")?.string()? == "otherwise").then_some(index)
        })
        .collect();
    if positions.len() != 1 {
        issues.push(IssueData::new(
            "GATE_OTHERWISE",
            path,
            "exactly one otherwise case is required",
        ));
    } else if positions[0] + 1 != cases.len() {
        issues.push(IssueData::new(
            "GATE_OTHERWISE",
            format!("{path}/{}", positions[0]),
            "otherwise must be the final case",
        ));
    }
}
