use crate::diagnostics::IssueData;
use crate::json_value::JsonValue;
use indexmap::IndexMap;

pub(super) fn validate_mapping_targets(
    node: &IndexMap<String, JsonValue>,
    node_index: usize,
    issues: &mut Vec<IssueData>,
) {
    let Some(mappings) = node.get("mappings").and_then(JsonValue::array) else {
        return;
    };
    let mut targets: Vec<&str> = Vec::new();
    for (mapping_index, mapping) in mappings.iter().enumerate() {
        let Some(target) = mapping
            .object()
            .and_then(|mapping| mapping.get("target"))
            .and_then(JsonValue::string)
        else {
            continue;
        };
        for previous in &targets {
            let parent = format!("{previous}/");
            let child = format!("{target}/");
            if target == *previous || target.starts_with(&parent) || previous.starts_with(&child) {
                issues.push(IssueData::new(
                    "MAPPING_TARGET",
                    format!("/nodes/{node_index}/mappings/{mapping_index}/target"),
                    "mapping target duplicates or overlaps an earlier target",
                ));
                break;
            }
        }
        targets.push(target);
    }
}
