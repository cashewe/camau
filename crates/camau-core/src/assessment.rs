use crate::diagnostics::{AssessmentData, IssueData};
use crate::graph;
use crate::json_parsing::{JsonValue, parse_json};

pub fn assess_json(source: &str) -> (Option<JsonValue>, AssessmentData) {
    let (value, problems) = parse_json(source);
    let mut issues = problems
        .into_iter()
        .map(|problem| IssueData::new(problem.code, problem.path, problem.message))
        .collect::<Vec<_>>();

    if let Some(value) = &value {
        issues.extend(graph::assess(value));
    }

    (value, AssessmentData::new(issues))
}
