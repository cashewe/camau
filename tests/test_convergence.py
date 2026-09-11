import asyncio

import pytest

from camau import Assessor, ConfigurationError, Router
from tests.support.specifications import fanout_spec


def test_missing_convergence_source_is_a_path_aware_assessment_error():
    specification = fanout_spec()
    specification["nodes"][3]["inputs"]["second"] = "missing"

    assessment = Assessor.assess(specification)

    issue = next(issue for issue in assessment.issues if issue.code == "REFERENCE_MISSING")
    assert issue.path == "/nodes/3/inputs/second"
    assert '"missing"' in issue.message
    with pytest.raises(ConfigurationError):
        Router(specification, {})


@pytest.mark.asyncio
async def test_fanout_schedules_isolates_and_orders_convergence():
    both_started = asyncio.Event()
    started = 0

    async def branch(payload, name):
        nonlocal started
        started += 1
        if started == 2:
            both_started.set()
        await asyncio.wait_for(both_started.wait(), 1)
        payload["branch"] = name
        return payload

    router = Router(
        fanout_spec(),
        {
            "left": lambda payload: branch(payload, "left"),
            "right": lambda payload: branch(payload, "right"),
        },
    )
    source = {"request": {"id": 1}}
    result = await router.run(source)
    assert list(result) == ["first", "second"]
    assert result["first"]["branch"] == "left"
    assert result["second"]["branch"] == "right"
    assert "branch" not in source


@pytest.mark.asyncio
@pytest.mark.asyncio
async def test_staged_convergence():
    specification = {
        "entry": "fan",
        "output": "final-map",
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
                "next": "adjudicate",
            },
            {"id": "adjudicate", "type": "task", "task": "adjudicate", "next": "final"},
            {"id": "c-task", "type": "task", "task": "c", "next": "final"},
            {
                "id": "final",
                "type": "converge",
                "inputs": {"adjudicated": "adjudicate", "c": "c-task"},
                "next": "final-map",
            },
            {
                "id": "final-map",
                "type": "map-schema",
                "mappings": [
                    {"target": "/decision", "path-to-source": "/adjudicated", "type": "object"},
                    {"target": "/independent", "path-to-source": "/c", "type": "object"},
                ],
            },
        ],
    }

    async def named(name):
        return {"name": name}

    async def adjudicate(payload):
        return {"names": [payload["a"]["name"], payload["b"]["name"]]}

    router = Router(
        specification,
        {
            "a": lambda _: named("a"),
            "b": lambda _: named("b"),
            "c": lambda _: named("c"),
            "adjudicate": adjudicate,
        },
    )
    result = await router.run({})
    assert result == {"decision": {"names": ["a", "b"]}, "independent": {"name": "c"}}


@pytest.mark.asyncio
async def test_nested_fanout_resumes_parent_flow():
    specification = {
        "entry": "outer",
        "output": "outer-join",
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
            {"id": "child-a-task", "type": "task", "task": "child-a", "next": "inner-join"},
            {"id": "child-b-task", "type": "task", "task": "child-b", "next": "inner-join"},
            {
                "id": "inner-join",
                "type": "converge",
                "inputs": {"child-a": "child-a-task", "child-b": "child-b-task"},
                "next": "parent-task",
            },
            {"id": "parent-task", "type": "task", "task": "parent", "next": "outer-join"},
            {"id": "peer-task", "type": "task", "task": "peer", "next": "outer-join"},
            {
                "id": "outer-join",
                "type": "converge",
                "inputs": {"parent": "parent-task", "peer": "peer-task"},
            },
        ],
    }

    async def identity(payload):
        return payload

    router = Router(
        specification, {name: identity for name in ["child-a", "child-b", "parent", "peer"]}
    )
    result = await router.run({"id": 1})
    assert list(result) == ["parent", "peer"]
    assert list(result["parent"]) == ["child-a", "child-b"]
