import asyncio
import copy

from hypothesis import given, settings
from hypothesis import strategies as st

from camau import Assessor, Router

json_scalars = (
    st.none()
    | st.booleans()
    | st.integers(-(2**63), 2**63 - 1)
    | st.floats(allow_nan=False, allow_infinity=False)
    | st.text()
)
json_values = st.recursive(
    json_scalars,
    lambda children: (
        st.lists(children, max_size=4) | st.dictionaries(st.text(max_size=12), children, max_size=4)
    ),
    max_leaves=20,
)
json_objects = st.dictionaries(st.text(max_size=12), json_values, max_size=5)


@given(json_objects)
@settings(max_examples=50, deadline=None)
def test_generated_objects_are_isolated_across_public_task_boundaries(payload):
    original = copy.deepcopy(payload)

    async def task(value):
        value["task-owned"] = True
        return value

    router = Router(
        {
            "entry": "task",
            "output": "task",
            "nodes": [{"id": "task", "type": "task", "task": "task"}],
        },
        {"task": task},
    )
    result = asyncio.run(router.run(payload))
    assert payload == original
    assert result == {**original, "task-owned": True}
    assert result is not payload


@given(st.integers(min_value=1, max_value=250))
@settings(max_examples=25, deadline=None)
def test_generated_deep_linear_graphs_assess_without_stack_recursion(length):
    nodes = []
    for index in range(length):
        node = {
            "id": f"n{index}",
            "type": "map-schema",
            "mappings": [{"target": "/value", "default": index, "type": "integer"}],
        }
        if index + 1 < length:
            node["next"] = f"n{index + 1}"
        nodes.append(node)
    assessment = Assessor.assess({"entry": "n0", "output": f"n{length - 1}", "nodes": nodes})
    assert assessment.valid, assessment.to_text()


@given(st.integers(min_value=2, max_value=100))
@settings(max_examples=25, deadline=None)
def test_generated_cycles_are_always_rejected(length):
    nodes = [
        {"id": f"n{index}", "type": "task", "task": "task", "next": f"n{(index + 1) % length}"}
        for index in range(length)
    ]
    assessment = Assessor.assess({"entry": "n0", "output": f"n{length - 1}", "nodes": nodes})
    assert "GRAPH_CYCLE" in {issue.code for issue in assessment.issues}


@given(st.integers(min_value=2, max_value=12))
@settings(max_examples=20, deadline=None)
def test_generated_fanout_flows_are_consumed_once(branch_count):
    branches = [{"id": f"flow-{index}", "target": f"task-{index}"} for index in range(branch_count)]
    tasks = [
        {"id": f"task-{index}", "type": "task", "task": f"task-{index}", "next": "join"}
        for index in range(branch_count)
    ]
    inputs = {f"flow-{index}": f"task-{index}" for index in range(branch_count)}
    specification = {
        "entry": "fan",
        "output": "join",
        "nodes": [
            {"id": "fan", "type": "fan-out", "branches": branches},
            *tasks,
            {"id": "join", "type": "converge", "inputs": inputs},
        ],
    }
    assessment = Assessor.assess(specification)
    assert assessment.valid, assessment.to_text()

    async def execute():
        async def identity(payload):
            return payload

        router = Router(specification, {f"task-{index}": identity for index in range(branch_count)})
        return await router.run({"value": 1})

    result = asyncio.run(execute())
    assert list(result) == list(inputs)
