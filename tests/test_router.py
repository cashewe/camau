import asyncio
import math
from collections.abc import Iterator, Mapping

import pytest

from camau import ConfigurationError, Router, TaskResultError
from tests.support.specifications import task_spec


class StatefulTasks(Mapping[str, object]):
    def __init__(self, entries):
        self.entries = entries
        self.lookups = []

    def __getitem__(self, key):
        self.lookups.append(key)
        values = self.entries[key]
        return values[min(self.lookups.count(key) - 1, len(values) - 1)]

    def __iter__(self) -> Iterator[str]:
        return iter(self.entries)

    def __len__(self) -> int:
        return len(self.entries)


@pytest.mark.asyncio
async def test_one_task_isolated_and_input_contract():
    seen = None

    async def echo(payload):
        nonlocal seen
        seen = payload
        payload["nested"]["value"] = 2
        return payload

    source = {"nested": {"value": 1}, "integer": 2**63 - 1, "float": 1.25}
    router = Router(task_spec(), {"echo": echo, "unused": object()})
    result = await router.run(source)
    assert result == {"nested": {"value": 2}, "integer": 2**63 - 1, "float": 1.25}
    assert source["nested"]["value"] == 1
    assert seen is not source and result is not seen

    with pytest.raises(TypeError):
        await router.run([])
    with pytest.raises(ValueError):
        await router.run({"too_large": 2**63})
    with pytest.raises(ValueError):
        await router.run({"nan": math.nan})


def test_task_binding_is_eager_but_does_not_invoke():
    called = False

    def task(_payload):
        nonlocal called
        called = True

    Router(task_spec(), {"echo": task})
    assert not called

    with pytest.raises(ConfigurationError) as missing:
        Router(task_spec(), {})
    assert missing.value.assessment.issues[0].code == "TASK_MISSING"

    with pytest.raises(ConfigurationError) as invalid:
        Router(task_spec(), {"echo": 1})
    assert invalid.value.assessment.issues[0].code == "TASK_NOT_CALLABLE"


def test_router_state_cannot_be_reassigned_deleted_or_reinitialized():
    async def echo(payload):
        return payload

    router = Router(task_spec(), {"echo": echo})

    with pytest.raises(AttributeError):
        router._inner = router._inner
    with pytest.raises(AttributeError):
        del router._inner
    with pytest.raises(AttributeError):
        router.__init__(task_spec(), {"echo": echo})


def test_failed_construction_does_not_initialize_router_state():
    router = Router.__new__(Router)

    with pytest.raises(ConfigurationError):
        router.__init__(task_spec(), {})

    assert not hasattr(router, "_inner")


@pytest.mark.asyncio
async def test_each_unique_task_is_resolved_once_and_reused_for_every_node():
    async def first(payload):
        return {"calls": [*payload.get("calls", []), "first"]}

    async def second(payload):
        return {"calls": [*payload.get("calls", []), "second"]}

    specification = {
        "entry": "first",
        "output": "second",
        "nodes": [
            {"id": "first", "type": "task", "task": "shared", "next": "second"},
            {"id": "second", "type": "task", "task": "shared"},
        ],
    }
    tasks = StatefulTasks({"shared": [first, second]})

    router = Router(specification, tasks)

    assert tasks.lookups == ["shared"]
    assert await router.run({}) == {"calls": ["first", "first"]}


def test_unique_invalid_task_entries_are_resolved_once_with_node_paths():
    specification = {
        "entry": "first",
        "output": "third",
        "nodes": [
            {"id": "first", "type": "task", "task": "missing", "next": "second"},
            {"id": "second", "type": "task", "task": "invalid", "next": "third"},
            {"id": "third", "type": "task", "task": "missing"},
        ],
    }
    tasks = StatefulTasks({"invalid": [object()]})

    with pytest.raises(ConfigurationError) as caught:
        Router(specification, tasks)

    assert tasks.lookups == ["missing", "invalid"]
    assert [(issue.code, issue.path) for issue in caught.value.assessment.issues] == [
        ("TASK_MISSING", "/nodes/0/task"),
        ("TASK_NOT_CALLABLE", "/nodes/1/task"),
        ("TASK_MISSING", "/nodes/2/task"),
    ]


@pytest.mark.asyncio
async def test_task_failures_and_contract_errors():
    failure = LookupError("original")

    async def fails(_payload):
        raise failure

    with pytest.raises(LookupError) as caught:
        await Router(task_spec(), {"echo": fails}).run({})
    assert caught.value is failure

    def synchronous(_payload):
        return {"secret": "not in the message"}

    with pytest.raises(TaskResultError) as nonawaitable:
        await Router(task_spec(), {"echo": synchronous}).run({})
    assert nonawaitable.value.reason == "non_awaitable"
    assert "secret" not in str(nonawaitable.value)

    async def nonobject(_payload):
        return ["secret"]

    with pytest.raises(TaskResultError) as nonobject_error:
        await Router(task_spec(), {"echo": nonobject}).run({})
    assert nonobject_error.value.reason == "non_object"
    assert nonobject_error.value.invalid_value == ["secret"]


@pytest.mark.asyncio
async def test_router_is_reentrant_and_frozen_from_source_mutation():
    specification = task_spec()
    registry = {"echo": None}

    async def echo(payload):
        await asyncio.sleep(0)
        return payload

    registry["echo"] = echo
    router = Router(specification, registry)
    specification["nodes"][0]["task"] = "changed"
    registry["echo"] = None
    assert await asyncio.gather(*(router.run({"value": index}) for index in range(20))) == [
        {"value": index} for index in range(20)
    ]
