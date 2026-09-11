use std::cmp::Ordering;
use std::collections::HashSet;

use crate::diagnostics::IssueData;
use crate::json_parsing::JsonValue;
use indexmap::IndexMap;

use super::super::{JsonNumber, compare_numbers, json_number, text};

pub(super) fn validate_gate_overlaps(
    node: &IndexMap<String, JsonValue>,
    node_index: usize,
    issues: &mut Vec<IssueData>,
) {
    let Some(cases) = node.get("cases").and_then(JsonValue::array) else {
        return;
    };
    for right in 0..cases.len() {
        for left in 0..right {
            if cases_overlap(&cases[left], &cases[right]) {
                issues.push(IssueData::new(
                    "GATE_OVERLAP",
                    format!("/nodes/{node_index}/cases/{right}"),
                    format!("case overlaps earlier case at index {left}"),
                ));
                break;
            }
        }
    }
}

fn cases_overlap(left: &JsonValue, right: &JsonValue) -> bool {
    let Some(left) = left.object() else {
        return false;
    };
    let Some(right) = right.object() else {
        return false;
    };
    let Some(left_op) = text(left, "operator") else {
        return false;
    };
    let Some(right_op) = text(right, "operator") else {
        return false;
    };
    if left_op == "otherwise" || right_op == "otherwise" {
        return false;
    }
    if let (Some(left_interval), Some(right_interval)) =
        (numeric_interval(left), numeric_interval(right))
    {
        return intervals_overlap(left_interval, right_interval);
    }
    match (left_op, right_op) {
        ("eq", "eq") => {
            left.get("value") == right.get("value")
                || numeric_equal_values(left.get("value"), right.get("value"))
        }
        ("eq", "in_set") => left
            .get("value")
            .and_then(JsonValue::string)
            .is_some_and(|value| {
                right
                    .get("values")
                    .and_then(JsonValue::array)
                    .is_some_and(|values| values.iter().any(|item| item.string() == Some(value)))
            }),
        ("in_set", "eq") => right
            .get("value")
            .and_then(JsonValue::string)
            .is_some_and(|value| {
                left.get("values")
                    .and_then(JsonValue::array)
                    .is_some_and(|values| values.iter().any(|item| item.string() == Some(value)))
            }),
        ("in_set", "in_set") => {
            let left_values: HashSet<_> = left
                .get("values")
                .and_then(JsonValue::array)
                .unwrap_or(&[])
                .iter()
                .filter_map(JsonValue::string)
                .collect();
            right
                .get("values")
                .and_then(JsonValue::array)
                .unwrap_or(&[])
                .iter()
                .filter_map(JsonValue::string)
                .any(|value| left_values.contains(value))
        }
        _ => false,
    }
}

fn numeric_equal_values(left: Option<&JsonValue>, right: Option<&JsonValue>) -> bool {
    match (left.and_then(json_number), right.and_then(json_number)) {
        (Some(left), Some(right)) => compare_numbers(left, right) == Ordering::Equal,
        _ => false,
    }
}

#[derive(Clone, Copy)]
struct Interval {
    lower: Option<(JsonNumber, bool)>,
    upper: Option<(JsonNumber, bool)>,
}

fn numeric_interval(case: &IndexMap<String, JsonValue>) -> Option<Interval> {
    match text(case, "operator")? {
        "eq" => {
            let value = json_number(case.get("value")?)?;
            Some(Interval {
                lower: Some((value, true)),
                upper: Some((value, true)),
            })
        }
        "lt" => Some(Interval {
            lower: None,
            upper: Some((json_number(case.get("value")?)?, false)),
        }),
        "le" => Some(Interval {
            lower: None,
            upper: Some((json_number(case.get("value")?)?, true)),
        }),
        "ge" => Some(Interval {
            lower: Some((json_number(case.get("value")?)?, true)),
            upper: None,
        }),
        "gt" => Some(Interval {
            lower: Some((json_number(case.get("value")?)?, false)),
            upper: None,
        }),
        "in_range" => Some(Interval {
            lower: Some((json_number(case.get("lower")?)?, true)),
            upper: Some((json_number(case.get("upper")?)?, true)),
        }),
        _ => None,
    }
}

fn intervals_overlap(left: Interval, right: Interval) -> bool {
    let lower = match (left.lower, right.lower) {
        (Some(left), Some(right)) => match compare_numbers(left.0, right.0) {
            Ordering::Greater => left,
            Ordering::Less => right,
            Ordering::Equal => (left.0, left.1 && right.1),
        },
        (Some(value), None) | (None, Some(value)) => value,
        (None, None) => return true,
    };
    let upper = match (left.upper, right.upper) {
        (Some(left), Some(right)) => match compare_numbers(left.0, right.0) {
            Ordering::Less => left,
            Ordering::Greater => right,
            Ordering::Equal => (left.0, left.1 && right.1),
        },
        (Some(value), None) | (None, Some(value)) => value,
        (None, None) => return true,
    };
    match compare_numbers(lower.0, upper.0) {
        Ordering::Less => true,
        Ordering::Equal => lower.1 && upper.1,
        Ordering::Greater => false,
    }
}
