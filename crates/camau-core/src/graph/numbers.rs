use std::cmp::Ordering;

use super::{JsonNumber, ValueType};
use crate::json_parsing::JsonValue;

impl ValueType {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "object" => Some(Self::Object),
            "array" => Some(Self::Array),
            "string" => Some(Self::String),
            "number" => Some(Self::Number),
            "integer" => Some(Self::Integer),
            "boolean" => Some(Self::Boolean),
            "null" => Some(Self::Null),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Object => "object",
            Self::Array => "array",
            Self::String => "string",
            Self::Number => "number",
            Self::Integer => "integer",
            Self::Boolean => "boolean",
            Self::Null => "null",
        }
    }

    pub fn matches(self, value: &JsonValue) -> bool {
        match self {
            Self::Object => matches!(value, JsonValue::Object(_)),
            Self::Array => matches!(value, JsonValue::Array(_)),
            Self::String => matches!(value, JsonValue::String(_)),
            Self::Number => matches!(value, JsonValue::Integer(_) | JsonValue::Number(_)),
            Self::Integer => {
                matches!(value, JsonValue::Integer(_))
                    || matches!(value, JsonValue::Number(number) if number.fract() == 0.0)
            }
            Self::Boolean => matches!(value, JsonValue::Bool(_)),
            Self::Null => matches!(value, JsonValue::Null),
        }
    }
}

impl JsonNumber {
    pub fn as_f64(self) -> f64 {
        match self {
            Self::Integer(value) => value as f64,
            Self::Float(value) => value,
        }
    }
}

pub fn compare_numbers(left: JsonNumber, right: JsonNumber) -> Ordering {
    match (left, right) {
        (JsonNumber::Integer(left), JsonNumber::Integer(right)) => left.cmp(&right),
        (JsonNumber::Float(left), JsonNumber::Float(right)) => left.partial_cmp(&right).unwrap(),
        (JsonNumber::Integer(integer), JsonNumber::Float(float)) => {
            compare_integer_float(integer, float)
        }
        (JsonNumber::Float(float), JsonNumber::Integer(integer)) => {
            compare_integer_float(integer, float).reverse()
        }
    }
}

fn compare_integer_float(integer: i64, float: f64) -> Ordering {
    const TWO_63: f64 = 9_223_372_036_854_775_808.0;
    if float >= TWO_63 {
        return Ordering::Less;
    }
    if float < -TWO_63 {
        return Ordering::Greater;
    }
    let truncated = float.trunc() as i64;
    match integer.cmp(&truncated) {
        Ordering::Equal if float.fract() > 0.0 => Ordering::Less,
        Ordering::Equal if float.fract() < 0.0 => Ordering::Greater,
        other => other,
    }
}

pub fn json_number(value: &JsonValue) -> Option<JsonNumber> {
    match value {
        JsonValue::Integer(value) => Some(JsonNumber::Integer(*value)),
        JsonValue::Number(value) => Some(JsonNumber::Float(*value)),
        _ => None,
    }
}
