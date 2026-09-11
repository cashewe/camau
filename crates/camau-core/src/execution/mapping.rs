use indexmap::IndexMap;

use crate::graph::Mapping;
use crate::json_parsing::{JsonPointer, JsonValue};

use super::MappingFailure;

pub(super) fn apply_mappings(
    node_id: &str,
    input: &JsonValue,
    mappings: &[Mapping],
) -> Result<JsonValue, MappingFailure> {
    let mut output = IndexMap::new();
    for mapping in mappings {
        let value = match mapping
            .source
            .as_ref()
            .and_then(|source| input.pointer(source))
        {
            Some(value) => value.clone(),
            None => match &mapping.default {
                Some(default) => default.clone(),
                None => {
                    return Err(MappingFailure {
                        node_id: node_id.to_owned(),
                        target: mapping.target.as_str().to_owned(),
                        source_path: mapping
                            .source
                            .as_ref()
                            .map(|source| source.as_str().to_owned()),
                        expected_type: mapping.value_type.name(),
                        reason: "source_missing",
                    });
                }
            },
        };
        if !mapping.value_type.matches(&value) {
            return Err(MappingFailure {
                node_id: node_id.to_owned(),
                target: mapping.target.as_str().to_owned(),
                source_path: mapping
                    .source
                    .as_ref()
                    .map(|source| source.as_str().to_owned()),
                expected_type: mapping.value_type.name(),
                reason: "type_mismatch",
            });
        }
        insert_target(&mut output, &mapping.target, value);
    }
    Ok(JsonValue::Object(output))
}

fn insert_target(root: &mut IndexMap<String, JsonValue>, pointer: &JsonPointer, value: JsonValue) {
    let tokens = pointer.tokens();
    let mut current = root;
    for token in &tokens[..tokens.len() - 1] {
        let entry = current
            .entry(token.key().to_owned())
            .or_insert_with(|| JsonValue::Object(IndexMap::new()));
        current = match entry {
            JsonValue::Object(object) => object,
            _ => unreachable!("mapping target conflicts are assessed"),
        };
    }
    current.insert(tokens.last().unwrap().key().to_owned(), value);
}
