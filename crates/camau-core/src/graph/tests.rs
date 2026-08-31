use std::cmp::Ordering;

use super::*;
use crate::json_parser::parse_json;
use proptest::prelude::*;

#[test]
fn integer_float_comparison_is_exact_at_binary64_boundary() {
    assert_eq!(
        compare_numbers(
            JsonNumber::Integer(9_007_199_254_740_993),
            JsonNumber::Float(9_007_199_254_740_992.0)
        ),
        Ordering::Greater
    );
    assert_eq!(
        compare_numbers(
            JsonNumber::Integer(i64::MAX),
            JsonNumber::Float(9_223_372_036_854_775_808.0)
        ),
        Ordering::Less
    );
}

proptest! {
    #[test]
    fn generated_cycles_are_rejected(length in 2usize..80) {
        let nodes = (0..length)
            .map(|index| format!(
                r#"{{"id":"n{index}","type":"task","task":"task","next":"n{}"}}"#,
                (index + 1) % length
            ))
            .collect::<Vec<_>>()
            .join(",");
        let source = format!(
            r#"{{"entry":"n0","output":"n{}","nodes":[{nodes}]}}"#,
            length - 1
        );
        let (value, problems) = parse_json(&source);
        prop_assert!(problems.is_empty());
        let issues = assess(&value.unwrap());
        prop_assert!(issues.iter().any(|issue| issue.code == "GRAPH_CYCLE"));
    }

    #[test]
    fn generated_fanout_flows_are_valid(branch_count in 2usize..24) {
        let branches = (0..branch_count)
            .map(|index| format!(r#"{{"id":"f{index}","target":"t{index}"}}"#))
            .collect::<Vec<_>>()
            .join(",");
        let tasks = (0..branch_count)
            .map(|index| format!(r#"{{"id":"t{index}","type":"task","task":"task-{index}","next":"join"}}"#))
            .collect::<Vec<_>>()
            .join(",");
        let inputs = (0..branch_count)
            .map(|index| format!(r#""f{index}":"t{index}""#))
            .collect::<Vec<_>>()
            .join(",");
        let source = format!(
            r#"{{"entry":"fan","output":"join","nodes":[{{"id":"fan","type":"fan-out","branches":[{branches}]}},{tasks},{{"id":"join","type":"converge","inputs":{{{inputs}}}}}]}}"#
        );
        let (value, problems) = parse_json(&source);
        prop_assert!(problems.is_empty());
        let issues = assess(&value.unwrap());
        prop_assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn positive_weights_compile_to_their_normalized_total(weights in prop::collection::vec(1u16..10_000, 2..16)) {
        let routes = weights
            .iter()
            .enumerate()
            .map(|(index, weight)| format!(r#"{{"weight":{weight},"target":"t{index}"}}"#))
            .collect::<Vec<_>>()
            .join(",");
        let tasks = weights
            .iter()
            .enumerate()
            .map(|(index, _)| format!(r#"{{"id":"t{index}","type":"task","task":"task-{index}","next":"done"}}"#))
            .collect::<Vec<_>>()
            .join(",");
        let source = format!(
            r#"{{"entry":"gate","output":"done","nodes":[{{"id":"gate","type":"randomised-gate","routes":[{routes}]}},{tasks},{{"id":"done","type":"map-schema","mappings":[{{"target":"/ok","default":true,"type":"boolean"}}]}}]}}"#
        );
        let (value, problems) = parse_json(&source);
        prop_assert!(problems.is_empty());
        let value = value.unwrap();
        prop_assert!(assess(&value).is_empty());
        let graph = compile(&value).unwrap();
        let NodeKind::Randomised { total, .. } = graph.nodes[graph.entry].kind else {
            prop_assert!(false, "entry did not compile as randomised gate");
            return Ok(());
        };
        let expected: f64 = weights.iter().map(|weight| f64::from(*weight)).sum();
        prop_assert_eq!(total, expected);
    }
}

#[test]
fn ten_thousand_node_graph_assesses_iteratively() {
    let length = 10_000;
    let nodes = (0..length)
        .map(|index| {
            if index + 1 == length {
                format!(r#"{{"id":"n{index}","type":"task","task":"task"}}"#)
            } else {
                format!(
                    r#"{{"id":"n{index}","type":"task","task":"task","next":"n{}"}}"#,
                    index + 1
                )
            }
        })
        .collect::<Vec<_>>()
        .join(",");
    let source = format!(
        r#"{{"entry":"n0","output":"n{}","nodes":[{nodes}]}}"#,
        length - 1
    );
    let (value, problems) = parse_json(&source);
    assert!(problems.is_empty());
    assert!(assess(&value.unwrap()).is_empty());
}
