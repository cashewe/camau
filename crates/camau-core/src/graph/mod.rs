mod algorithms;
mod compiler;
mod model;
mod numbers;
mod semantics;
mod structure;

use indexmap::IndexMap;

use crate::diagnostics::IssueData;
use crate::json_parsing::JsonValue;

pub use compiler::compile;
pub use model::{
    Branch, CompiledGraph, ConvergeInput, GateCase, JsonNumber, Mapping, Node, NodeKind, ValueType,
    WeightedRoute,
};
pub use numbers::{compare_numbers, json_number};

pub fn assess(value: &JsonValue) -> Vec<IssueData> {
    let mut issues = Vec::new();
    structure::validate_structure(value, &mut issues);
    semantics::validate_semantics(value, &mut issues);
    issues
}

fn text<'a>(object: &'a IndexMap<String, JsonValue>, key: &str) -> Option<&'a str> {
    object.get(key)?.string()
}

#[cfg(test)]
mod tests;
