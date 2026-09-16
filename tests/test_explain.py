import asyncio

import pytest

from camau import MappingError, Router, RoutingSelectionError


@pytest.mark.asyncio
async def test_explain_records_task_transformation_and_deterministic_route():
    specification = {
        "entry": "enrich",
        "output": "format",
        "nodes": [
            {"id": "enrich", "type": "task", "task": "enrich", "next": "route"},
            {
                "id": "route",
                "type": "deterministic-gate",
                "select": "/category",
                "cases": [
                    {"operator": "eq", "value": "premium", "target": "format"},
                    {"operator": "otherwise", "target": "unsupported"},
                ],
            },
            {
                "id": "format",
                "type": "map-schema",
                "mappings": [
                    {"target": "/category", "path-to-source": "/category", "type": "string"},
                    {"target": "/handled", "default": True, "type": "boolean"},
                ],
            },
            {"id": "unsupported", "type": "raise-error", "message": "unsupported"},
        ],
    }
    calls = 0

    async def enrich(payload):
        nonlocal calls
        calls += 1
        return {**payload, "category": "premium"}

    explanation = await Router(specification, {"enrich": enrich}).explain({"request": 1})

    assert calls == 1
    assert explanation["objects"] == [
        {"id": 0, "value": {"request": 1}},
        {"id": 1, "value": {"request": 1, "category": "premium"}},
        {"id": 2, "value": {"category": "premium", "handled": True}},
    ]
    assert explanation["visits"] == [
        {
            "node_id": "enrich",
            "node_type": "task",
            "reason": {"kind": "entry"},
            "input_object_ids": [0],
            "output_object_id": 1,
            "status": "completed",
        },
        {
            "node_id": "route",
            "node_type": "deterministic-gate",
            "reason": {"kind": "next", "from_node": "enrich"},
            "input_object_ids": [1],
            "output_object_id": 1,
            "status": "completed",
        },
        {
            "node_id": "format",
            "node_type": "map-schema",
            "reason": {
                "kind": "deterministic-gate",
                "from_node": "route",
                "selected_path": "/category",
                "value_found": True,
                "selected_value": "premium",
                "matched": {"operator": "eq", "value": "premium"},
            },
            "input_object_ids": [1],
            "output_object_id": 2,
            "status": "completed",
        },
    ]
    assert explanation["outcome"] == {"kind": "success", "output_object_id": 2}


@pytest.mark.asyncio
async def test_explain_returns_configured_raise_error_as_terminal_outcome():
    specification = {
        "entry": "route",
        "output": "supported",
        "nodes": [
            {
                "id": "route",
                "type": "deterministic-gate",
                "select": "/category",
                "cases": [
                    {"operator": "eq", "value": "supported", "target": "supported"},
                    {"operator": "otherwise", "target": "unsupported"},
                ],
            },
            {"id": "supported", "type": "task", "task": "supported"},
            {"id": "unsupported", "type": "raise-error", "message": "unsupported category"},
        ],
    }

    async def supported(payload):
        return payload

    explanation = await Router(specification, {"supported": supported}).explain(
        {"category": "unknown"}
    )

    assert explanation["visits"][-1] == {
        "node_id": "unsupported",
        "node_type": "raise-error",
        "reason": {
            "kind": "deterministic-gate",
            "from_node": "route",
            "selected_path": "/category",
            "value_found": True,
            "selected_value": "unknown",
            "matched": {"operator": "otherwise"},
        },
        "input_object_ids": [0],
        "status": "terminal",
    }
    assert explanation["outcome"] == {
        "kind": "raise-error",
        "code": "GWALL",
        "node_id": "unsupported",
        "message": "unsupported category",
        "current_object_id": 0,
    }


@pytest.mark.asyncio
async def test_explain_propagates_task_exceptions_unchanged():
    failure = LookupError("original")

    async def fails(_payload):
        raise failure

    specification = {
        "entry": "call",
        "output": "call",
        "nodes": [{"id": "call", "type": "task", "task": "fails"}],
    }

    with pytest.raises(LookupError) as caught:
        await Router(specification, {"fails": fails}).explain({})
    assert caught.value is failure

    mapping_specification = {
        "entry": "map",
        "output": "map",
        "nodes": [
            {
                "id": "map",
                "type": "map-schema",
                "mappings": [{"target": "/value", "path-to-source": "/missing", "type": "string"}],
            }
        ],
    }
    with pytest.raises(MappingError):
        await Router(mapping_specification, {}).explain({})


@pytest.mark.asyncio
async def test_explain_samples_and_consumes_one_randomised_route():
    specification = {
        "entry": "choose",
        "output": "left",
        "nodes": [
            {
                "id": "choose",
                "type": "randomised-gate",
                "routes": [
                    {"weight": 3, "target": "left"},
                    {"weight": 1, "target": "right"},
                ],
            },
            {"id": "left", "type": "task", "task": "left"},
            {"id": "right", "type": "raise-error", "message": "right"},
        ],
    }

    async def left(_payload):
        return {"route": "left"}

    explained_router = Router(specification, {"left": left}, seed=7)
    control_router = Router(specification, {"left": left}, seed=7)

    explanation = await explained_router.explain({})

    async def run_outcome(router):
        try:
            return "success", await router.run({})
        except RoutingSelectionError:
            return ("raise-error",)

    after_explanation = await run_outcome(explained_router)
    first_control = await run_outcome(control_router)
    second_control = await run_outcome(control_router)

    explained_outcome = explanation["outcome"]
    first_outcome = (
        (
            "success",
            explanation["objects"][explained_outcome["output_object_id"]]["value"],
        )
        if explained_outcome["kind"] == "success"
        else ("raise-error",)
    )
    assert first_outcome == first_control
    assert after_explanation == second_control
    selected_visit = explanation["visits"][1]
    assert selected_visit["reason"]["kind"] == "randomised-gate"
    assert selected_visit["reason"]["probability"] in {0.75, 0.25}


@pytest.mark.asyncio
async def test_explain_marks_reached_sibling_task_abandoned_on_raise_error():
    specification = {
        "entry": "fan",
        "output": "final",
        "nodes": [
            {
                "id": "fan",
                "type": "fan-out",
                "branches": [
                    {"id": "a", "target": "a-task"},
                    {"id": "b", "target": "b-task"},
                    {"id": "c", "target": "c-task"},
                ],
            },
            {"id": "a-task", "type": "task", "task": "a", "next": "early"},
            {"id": "b-task", "type": "task", "task": "b", "next": "early"},
            {
                "id": "early",
                "type": "converge",
                "inputs": {"a": "a-task", "b": "b-task"},
                "next": "route",
            },
            {
                "id": "route",
                "type": "deterministic-gate",
                "select": "/a/stop",
                "cases": [
                    {"operator": "eq", "value": True, "target": "stop"},
                    {"operator": "otherwise", "target": "continue"},
                ],
            },
            {"id": "continue", "type": "task", "task": "continue", "next": "final"},
            {"id": "c-task", "type": "task", "task": "c", "next": "final"},
            {
                "id": "final",
                "type": "converge",
                "inputs": {"combined": "continue", "c": "c-task"},
            },
            {"id": "stop", "type": "raise-error", "message": "stopped"},
        ],
    }
    c_started = asyncio.Event()

    async def early(_payload):
        await c_started.wait()
        return {"stop": True}

    async def c(_payload):
        c_started.set()
        await asyncio.sleep(60)

    async def continue_task(payload):
        return payload

    explanation = await Router(
        specification,
        {"a": early, "b": early, "c": c, "continue": continue_task},
    ).explain({})

    visits = {visit["node_id"]: visit for visit in explanation["visits"]}
    assert visits["early"]["reason"] == {
        "kind": "convergence",
        "inputs": [
            {"flow_id": "a", "from_node": "a-task"},
            {"flow_id": "b", "from_node": "b-task"},
        ],
    }
    assert len(visits["early"]["input_object_ids"]) == 2
    assert visits["c-task"]["status"] == "abandoned"
    assert visits["stop"]["status"] == "terminal"
    assert explanation["outcome"]["kind"] == "raise-error"
