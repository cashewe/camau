use std::sync::Arc;

use crate::graph::compile;
use crate::json_parser::parse_json;
use crate::json_value::JsonValue;

use super::{Execution, ExecutionFailure, seeded_rng};

fn execution(specification: &str, payload: JsonValue) -> Execution {
    let (value, problems) = parse_json(specification);
    assert!(problems.is_empty());
    Execution::new(
        Arc::new(compile(&value.unwrap()).unwrap()),
        seeded_rng(Some(1)),
        payload,
    )
}

#[test]
fn task_requests_round_trip_without_python_state() {
    let mut execution = execution(
        r#"{"entry":"call","output":"call","nodes":[{"id":"call","type":"task","task":"echo"}]}"#,
        JsonValue::Object(indexmap::indexmap! { "value".to_owned() => JsonValue::Integer(1) }),
    );
    let progress = execution.start().unwrap();
    assert_eq!(progress.requests.len(), 1);
    assert_eq!(progress.requests[0].task_name, "echo");
    let ticket = progress.requests[0].ticket;
    let completed = execution
        .resume(ticket, progress.requests[0].payload.clone())
        .unwrap();
    assert_eq!(
        completed.output,
        Some(JsonValue::Object(indexmap::indexmap! {
            "value".to_owned() => JsonValue::Integer(1)
        }))
    );
}

#[test]
fn mapping_failures_own_their_context() {
    let mut execution = execution(
        r#"{"entry":"map","output":"map","nodes":[{"id":"map","type":"map-schema","mappings":[{"target":"/name","path-to-source":"/missing","type":"string"}]}]}"#,
        JsonValue::Object(indexmap::IndexMap::new()),
    );
    let failure = execution.start().unwrap_err();
    let ExecutionFailure::Mapping(failure) = failure else {
        panic!("expected mapping failure");
    };
    assert_eq!(failure.node_id, "map");
    assert_eq!(failure.target, "/name");
    assert_eq!(failure.source_path.as_deref(), Some("/missing"));
    assert_eq!(failure.reason, "source_missing");
}
