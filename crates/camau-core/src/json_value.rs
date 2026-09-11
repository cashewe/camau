use indexmap::IndexMap;

use crate::json_pointer::JsonPointer;

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

    pub fn pointer(&self, pointer: &JsonPointer) -> Option<&JsonValue> {
        let mut current = self;
        for token in pointer.tokens() {
            match current {
                Self::Object(object) => current = object.get(token.key())?,
                Self::Array(array) => current = array.get(token.index()?)?,
                _ => return None,
            }
        }
        Some(current)
    }
}

#[cfg(test)]
mod tests {
    use super::JsonValue;
    use crate::json_pointer::JsonPointer;
    use indexmap::IndexMap;

    #[test]
    fn pointer_indexes_are_canonical_only_for_arrays() {
        let array = JsonValue::Array(vec![JsonValue::String("zero".to_owned()), JsonValue::Null]);
        assert_eq!(
            array.pointer(&JsonPointer::parse("/0").unwrap()),
            Some(&JsonValue::String("zero".to_owned()))
        );
        for token in ["", "+0", "+1", "01", "-0", "-1", "18446744073709551616"] {
            assert_eq!(
                array.pointer(&JsonPointer::parse(&format!("/{token}")).unwrap()),
                None
            );
        }

        let object = JsonValue::Object(
            ["", "+0", "+1", "01", "-0", "-1", "18446744073709551616"]
                .into_iter()
                .map(|key| (key.to_owned(), JsonValue::Null))
                .collect::<IndexMap<_, _>>(),
        );
        for token in ["", "+0", "+1", "01", "-0", "-1", "18446744073709551616"] {
            assert_eq!(
                object.pointer(&JsonPointer::parse(&format!("/{token}")).unwrap()),
                Some(&JsonValue::Null)
            );
        }
    }
}
