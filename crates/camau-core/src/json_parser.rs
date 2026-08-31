use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt;

use indexmap::IndexMap;
use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};

use crate::json_pointer::join_pointer;
use crate::json_value::JsonValue;

#[derive(Clone, Debug, thiserror::Error)]
#[error("{message} at {path}")]
pub struct JsonProblem {
    pub code: &'static str,
    pub path: String,
    pub message: String,
}

pub fn parse_json(source: &str) -> (Option<JsonValue>, Vec<JsonProblem>) {
    let duplicates = RefCell::new(Vec::new());
    let seed = ValueSeed {
        path: String::new(),
        duplicates: &duplicates,
    };
    let mut deserializer = serde_json::Deserializer::from_str(source);
    match seed.deserialize(&mut deserializer) {
        Ok(value) => {
            if let Err(error) = deserializer.end() {
                return (
                    None,
                    vec![JsonProblem {
                        code: "JSON_PARSE",
                        path: String::new(),
                        message: error.to_string(),
                    }],
                );
            }
            (Some(value), duplicates.into_inner())
        }
        Err(error) => {
            let mut problems = duplicates.into_inner();
            let message = error.to_string();
            problems.push(JsonProblem {
                code: if message.contains("number out of range")
                    || contains_nonfinite_literal(source)
                {
                    "JSON_VALUE"
                } else {
                    "JSON_PARSE"
                },
                path: String::new(),
                message,
            });
            (None, problems)
        }
    }
}

fn contains_nonfinite_literal(source: &str) -> bool {
    let bytes = source.as_bytes();
    let mut in_string = false;
    let mut escaped = false;
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if byte == b'"' {
            in_string = true;
            index += 1;
            continue;
        }
        let remaining = &bytes[index..];
        for token in [
            b"NaN".as_slice(),
            b"Infinity".as_slice(),
            b"-Infinity".as_slice(),
        ] {
            if remaining.starts_with(token) {
                let before_valid = index == 0 || !is_identifier_byte(bytes[index - 1]);
                let after = index + token.len();
                let after_valid = after >= bytes.len() || !is_identifier_byte(bytes[after]);
                if before_valid && after_valid {
                    return true;
                }
            }
        }
        index += 1;
    }
    false
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

struct ValueSeed<'a> {
    path: String,
    duplicates: &'a RefCell<Vec<JsonProblem>>,
}

impl<'de> DeserializeSeed<'de> for ValueSeed<'_> {
    type Value = JsonValue;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(ValueVisitor(self))
    }
}

struct ValueVisitor<'a>(ValueSeed<'a>);

impl<'de> Visitor<'de> for ValueVisitor<'_> {
    type Value = JsonValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a supported JSON value")
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(JsonValue::Null)
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(JsonValue::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(JsonValue::Integer(value))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        i64::try_from(value)
            .map(JsonValue::Integer)
            .map_err(|_| E::custom("number out of range for signed 64-bit integer"))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if value.is_finite() {
            Ok(JsonValue::Number(value))
        } else {
            Err(E::custom("number out of range for finite binary64"))
        }
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(JsonValue::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(JsonValue::String(value))
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(ValueSeed {
            path: format!("{}/{}", self.0.path, values.len()),
            duplicates: self.0.duplicates,
        })? {
            values.push(value);
        }
        Ok(JsonValue::Array(values))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = IndexMap::new();
        let mut seen = HashSet::new();
        while let Some(key) = map.next_key::<String>()? {
            let path = join_pointer(&self.0.path, &key);
            let value = map.next_value_seed(ValueSeed {
                path: path.clone(),
                duplicates: self.0.duplicates,
            })?;
            if !seen.insert(key.clone()) {
                self.0.duplicates.borrow_mut().push(JsonProblem {
                    code: "JSON_DUPLICATE_KEY",
                    path,
                    message: format!("object key {key:?} is repeated"),
                });
            }
            values.insert(key, value);
        }
        Ok(JsonValue::Object(values))
    }
}
