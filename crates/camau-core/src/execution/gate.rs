use crate::graph::{GateCase, JsonNumber, compare_numbers, json_number};
use crate::json_value::JsonValue;

pub(super) fn select_case(
    selected: Option<&JsonValue>,
    cases: &[GateCase],
) -> (usize, &'static str) {
    for case in cases {
        let matched = match case {
            GateCase::Eq { value, .. } => {
                selected.is_some_and(|selected| json_equal(selected, value))
            }
            GateCase::Lt { value, .. } => numeric_match(selected, *value, |order| order.is_lt()),
            GateCase::Le { value, .. } => numeric_match(selected, *value, |order| order.is_le()),
            GateCase::Ge { value, .. } => numeric_match(selected, *value, |order| order.is_ge()),
            GateCase::Gt { value, .. } => numeric_match(selected, *value, |order| order.is_gt()),
            GateCase::Range { lower, upper, .. } => {
                selected.and_then(json_number).is_some_and(|value| {
                    compare_numbers(value, *lower).is_ge() && compare_numbers(value, *upper).is_le()
                })
            }
            GateCase::Set { values, .. } => selected
                .and_then(JsonValue::string)
                .is_some_and(|value| values.iter().any(|candidate| candidate == value)),
            GateCase::Otherwise { .. } => true,
        };
        if matched {
            return match case {
                GateCase::Eq { target, .. } => (*target, "eq"),
                GateCase::Lt { target, .. } => (*target, "lt"),
                GateCase::Le { target, .. } => (*target, "le"),
                GateCase::Ge { target, .. } => (*target, "ge"),
                GateCase::Gt { target, .. } => (*target, "gt"),
                GateCase::Range { target, .. } => (*target, "in_range"),
                GateCase::Set { target, .. } => (*target, "in_set"),
                GateCase::Otherwise { target } => (*target, "otherwise"),
            };
        }
    }
    unreachable!("validated gate has final otherwise")
}

fn numeric_match(
    selected: Option<&JsonValue>,
    configured: JsonNumber,
    predicate: impl Fn(std::cmp::Ordering) -> bool,
) -> bool {
    selected
        .and_then(json_number)
        .is_some_and(|value| predicate(compare_numbers(value, configured)))
}

fn json_equal(left: &JsonValue, right: &JsonValue) -> bool {
    match (json_number(left), json_number(right)) {
        (Some(left), Some(right)) => compare_numbers(left, right).is_eq(),
        _ => left == right,
    }
}
