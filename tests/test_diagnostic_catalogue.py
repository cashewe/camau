import pytest

from camau import Assessor, ConfigurationError, Router


def codes(specification):
    return {issue.code for issue in Assessor.assess(specification).issues}


def base_task():
    return {
        "entry": "out",
        "output": "out",
        "nodes": [{"id": "out", "type": "task", "task": "task"}],
    }


def fanout_without_convergence():
    return {
        "entry": "fan",
        "output": "shared",
        "nodes": [
            {
                "id": "fan",
                "type": "fan-out",
                "branches": [
                    {"id": "a", "target": "left"},
                    {"id": "b", "target": "right"},
                ],
            },
            {"id": "left", "type": "task", "task": "left", "next": "shared"},
            {"id": "right", "type": "task", "task": "right", "next": "shared"},
            {"id": "shared", "type": "task", "task": "shared"},
        ],
    }


def nested_early_parent_consumption():
    return {
        "entry": "outer",
        "output": "bad-join",
        "nodes": [
            {
                "id": "outer",
                "type": "fan-out",
                "branches": [
                    {"id": "parent", "target": "inner"},
                    {"id": "peer", "target": "peer-task"},
                ],
            },
            {
                "id": "inner",
                "type": "fan-out",
                "branches": [
                    {"id": "child-a", "target": "child-a-task"},
                    {"id": "child-b", "target": "child-b-task"},
                ],
            },
            {"id": "child-a-task", "type": "task", "task": "child-a", "next": "bad-join"},
            {"id": "child-b-task", "type": "task", "task": "child-b", "next": "bad-join"},
            {"id": "peer-task", "type": "task", "task": "peer", "next": "bad-join"},
            {
                "id": "bad-join",
                "type": "converge",
                "inputs": {
                    "parent": "child-a-task",
                    "child-b": "child-b-task",
                    "peer": "peer-task",
                },
            },
        ],
    }


def reused_flows():
    return {
        "entry": "fan",
        "output": "first",
        "nodes": [
            {
                "id": "fan",
                "type": "fan-out",
                "branches": [
                    {"id": "a", "target": "a-task"},
                    {"id": "b", "target": "b-task"},
                ],
            },
            {"id": "a-task", "type": "task", "task": "a", "next": "first"},
            {"id": "b-task", "type": "task", "task": "b", "next": "first"},
            {"id": "first", "type": "converge", "inputs": {"a": "a-task", "b": "b-task"}},
            {"id": "second", "type": "converge", "inputs": {"a": "a-task", "b": "b-task"}},
        ],
    }


def mismatched_convergence():
    return {
        "entry": "fan",
        "output": "join",
        "nodes": [
            {
                "id": "fan",
                "type": "fan-out",
                "branches": [
                    {"id": "a", "target": "a-task"},
                    {"id": "b", "target": "b-task"},
                ],
            },
            {"id": "a-task", "type": "task", "task": "a", "next": "join"},
            {"id": "b-task", "type": "task", "task": "b", "next": "other"},
            {"id": "other", "type": "task", "task": "other"},
            {"id": "join", "type": "converge", "inputs": {"a": "a-task", "b": "b-task"}},
        ],
    }


def exclusive_convergence():
    return {
        "entry": "gate",
        "output": "join",
        "nodes": [
            {
                "id": "gate",
                "type": "deterministic-gate",
                "select": "/kind",
                "cases": [
                    {"operator": "eq", "value": "a", "target": "a-task"},
                    {"operator": "otherwise", "target": "b-task"},
                ],
            },
            {"id": "a-task", "type": "task", "task": "a", "next": "join"},
            {"id": "b-task", "type": "task", "task": "b", "next": "join"},
            {
                "id": "join",
                "type": "converge",
                "inputs": {"a-result": "a-task", "b-result": "b-task"},
            },
        ],
    }


@pytest.mark.parametrize(
    ("expected", "specification"),
    [
        ("JSON_PARSE", "{"),
        ("JSON_DUPLICATE_KEY", '{"entry":"out","entry":"out","output":"out","nodes":[]}'),
        ("JSON_VALUE", {"entry": "out", "output": "out", "nodes": {1}}),
        ("JSON_VALUE", '{"entry":"out","output":"out","nodes":NaN}'),
        ("SCHEMA_REQUIRED", {}),
        ("SCHEMA_UNKNOWN", {**base_task(), "unknown": True}),
        ("SCHEMA_TYPE", {"entry": "out", "output": "out", "nodes": "wrong"}),
        ("SCHEMA_VALUE", {"entry": "out", "output": "out", "nodes": []}),
        (
            "ID_INVALID",
            {
                "entry": "1bad",
                "output": "1bad",
                "nodes": [{"id": "1bad", "type": "task", "task": "task"}],
            },
        ),
        (
            "ID_RESERVED",
            {
                "entry": "GWALL",
                "output": "GWALL",
                "nodes": [{"id": "GWALL", "type": "task", "task": "task"}],
            },
        ),
        (
            "ID_COLLISION",
            {
                "entry": "same",
                "output": "same",
                "nodes": [
                    {"id": "same", "type": "task", "task": "a"},
                    {"id": "same", "type": "task", "task": "b"},
                ],
            },
        ),
        (
            "REFERENCE_MISSING",
            {
                "entry": "a",
                "output": "a",
                "nodes": [{"id": "a", "type": "task", "task": "a", "next": "missing"}],
            },
        ),
        (
            "ENTRY_INVALID",
            {
                "entry": "stop",
                "output": "out",
                "nodes": [
                    {"id": "stop", "type": "raise-error", "message": "stop"},
                    {"id": "out", "type": "task", "task": "out"},
                ],
            },
        ),
        (
            "OUTPUT_INVALID",
            {
                "entry": "gate",
                "output": "gate",
                "nodes": [
                    {
                        "id": "gate",
                        "type": "randomised-gate",
                        "routes": [{"weight": 1, "target": "a"}, {"weight": 1, "target": "b"}],
                    },
                    {"id": "a", "type": "raise-error", "message": "a"},
                    {"id": "b", "type": "raise-error", "message": "b"},
                ],
            },
        ),
        (
            "GRAPH_CYCLE",
            {
                "entry": "a",
                "output": "b",
                "nodes": [
                    {"id": "a", "type": "task", "task": "a", "next": "b"},
                    {"id": "b", "type": "task", "task": "b", "next": "a"},
                ],
            },
        ),
        (
            "GRAPH_UNREACHABLE",
            {
                "entry": "out",
                "output": "out",
                "nodes": [
                    {"id": "out", "type": "task", "task": "out"},
                    {"id": "orphan", "type": "raise-error", "message": "orphan"},
                ],
            },
        ),
        (
            "GRAPH_DEAD_END",
            {
                "entry": "start",
                "output": "out",
                "nodes": [
                    {"id": "start", "type": "task", "task": "start"},
                    {"id": "out", "type": "task", "task": "out"},
                ],
            },
        ),
        ("GRAPH_IMPLICIT_JOIN", fanout_without_convergence()),
        ("FLOW_INVALID", nested_early_parent_consumption()),
        ("FLOW_REUSED", reused_flows()),
        ("FLOW_UNCONSUMED", fanout_without_convergence()),
        ("CONVERGE_LINK", mismatched_convergence()),
        ("CONVERGE_ACTIVITY", exclusive_convergence()),
        (
            "MAPPING_TARGET",
            {
                "entry": "map",
                "output": "map",
                "nodes": [
                    {
                        "id": "map",
                        "type": "map-schema",
                        "mappings": [
                            {"target": "/x", "default": {}, "type": "object"},
                            {"target": "/x/y", "default": 1, "type": "integer"},
                        ],
                    }
                ],
            },
        ),
        (
            "MAPPING_DEFAULT",
            {
                "entry": "map",
                "output": "map",
                "nodes": [
                    {
                        "id": "map",
                        "type": "map-schema",
                        "mappings": [{"target": "/x", "default": 1, "type": "string"}],
                    }
                ],
            },
        ),
        (
            "GATE_CASE",
            {
                "entry": "gate",
                "output": "out",
                "nodes": [
                    {
                        "id": "gate",
                        "type": "deterministic-gate",
                        "select": "/x",
                        "cases": [
                            {"operator": "in_range", "lower": 2, "upper": 1, "target": "out"},
                            {"operator": "otherwise", "target": "out"},
                        ],
                    },
                    {"id": "out", "type": "task", "task": "out"},
                ],
            },
        ),
        (
            "GATE_OTHERWISE",
            {
                "entry": "gate",
                "output": "out",
                "nodes": [
                    {
                        "id": "gate",
                        "type": "deterministic-gate",
                        "select": "/x",
                        "cases": [
                            {"operator": "eq", "value": 1, "target": "out"},
                            {"operator": "eq", "value": 2, "target": "out"},
                        ],
                    },
                    {"id": "out", "type": "task", "task": "out"},
                ],
            },
        ),
        (
            "GATE_OVERLAP",
            {
                "entry": "gate",
                "output": "out",
                "nodes": [
                    {
                        "id": "gate",
                        "type": "deterministic-gate",
                        "select": "/x",
                        "cases": [
                            {"operator": "eq", "value": 1, "target": "out"},
                            {"operator": "in_range", "lower": 0, "upper": 2, "target": "out"},
                            {"operator": "otherwise", "target": "stop"},
                        ],
                    },
                    {"id": "out", "type": "task", "task": "out"},
                    {"id": "stop", "type": "raise-error", "message": "stop"},
                ],
            },
        ),
        (
            "RANDOM_WEIGHT",
            {
                "entry": "gate",
                "output": "out",
                "nodes": [
                    {
                        "id": "gate",
                        "type": "randomised-gate",
                        "routes": [{"weight": 0, "target": "out"}, {"weight": 1, "target": "stop"}],
                    },
                    {"id": "out", "type": "task", "task": "out"},
                    {"id": "stop", "type": "raise-error", "message": "stop"},
                ],
            },
        ),
        (
            "RANDOM_TARGET",
            {
                "entry": "gate",
                "output": "out",
                "nodes": [
                    {
                        "id": "gate",
                        "type": "randomised-gate",
                        "routes": [{"weight": 1, "target": "out"}, {"weight": 1, "target": "out"}],
                    },
                    {"id": "out", "type": "task", "task": "out"},
                ],
            },
        ),
    ],
)
def test_each_assessment_diagnostic_has_a_public_fixture(expected, specification):
    assert expected in codes(specification)


def test_binding_diagnostics_are_retained_on_configuration_error():
    with pytest.raises(ConfigurationError) as missing:
        Router(base_task(), {})
    assert {issue.code for issue in missing.value.assessment.issues} == {"TASK_MISSING"}

    with pytest.raises(ConfigurationError) as not_callable:
        Router(base_task(), {"task": object()})
    assert {issue.code for issue in not_callable.value.assessment.issues} == {"TASK_NOT_CALLABLE"}


def test_missing_reference_suppresses_derived_graph_flow_and_convergence_symptoms():
    specification = {
        "entry": "fan",
        "output": "out",
        "nodes": [
            {
                "id": "fan",
                "type": "fan-out",
                "branches": [
                    {"id": "a", "target": "missing"},
                    {"id": "b", "target": "out"},
                ],
            },
            {"id": "out", "type": "task", "task": "out"},
        ],
    }
    observed = codes(specification)
    assert "REFERENCE_MISSING" in observed
    assert not observed.intersection(
        {
            "GRAPH_UNREACHABLE",
            "GRAPH_DEAD_END",
            "FLOW_UNCONSUMED",
            "CONVERGE_LINK",
            "CONVERGE_ACTIVITY",
        }
    )


def test_fatal_json_parse_suppresses_structural_and_semantic_diagnostics():
    assert codes('{"entry":') == {"JSON_PARSE"}
