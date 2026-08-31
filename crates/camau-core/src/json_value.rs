use indexmap::IndexMap;

use crate::json_pointer::unescape_pointer_token;

#[derive(Clone, Debug, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Integer(i64),
    Number(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(IndexMap<String, JsonValue>),
}

impl JsonValue {
    pub fn object(&self) -> Option<&IndexMap<String, JsonValue>> {
        match self {
            Self::Object(value) => Some(value),
            _ => None,
        }
    }

    pub fn array(&self) -> Option<&[JsonValue]> {
        match self {
            Self::Array(value) => Some(value),
            _ => None,
        }
    }

    pub fn string(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    pub fn pointer(&self, pointer: &str) -> Option<&JsonValue> {
        if pointer.is_empty() {
            return Some(self);
        }
        let mut current = self;
        for token in pointer.strip_prefix('/')?.split('/') {
            let token = unescape_pointer_token(token)?;
            match current {
                Self::Object(object) => current = object.get(token.as_ref())?,
                Self::Array(array) => {
                    if token == "-" || (token.len() > 1 && token.starts_with('0')) {
                        return None;
                    }
                    current = array.get(token.parse::<usize>().ok()?)?;
                }
                _ => return None,
            }
        }
        Some(current)
    }
}
