import asyncio

import pytest

from camau import Router, Task


def task_spec(task_name: str = "echo"):
    return {
        "entry": "first",
        "output": "second",
        "nodes": [
            {"id": "first", "type": "task", "task": task_name, "next": "second"},
            {"id": "second", "type": "task", "task": task_name},
        ],
    }


async def echo(payload):
    return payload


@pytest.mark.asyncio
async def test_direct_callable_and_missing_healthcheck_default_to_true():
    direct = Router(task_spec(), {"echo": echo})
    registered = Router(task_spec(), {"echo": Task(echo)})

    assert await direct.healthcheck() == {"echo": True}
    assert await registered.healthcheck() == {"echo": True}


@pytest.mark.asyncio
async def test_unique_referenced_healthchecks_run_concurrently():
    started = set()
    both_started = asyncio.Event()

    async def check(name, result):
        started.add(name)
        if len(started) == 2:
            both_started.set()
        await asyncio.wait_for(both_started.wait(), timeout=1)
        return result

    specification = {
        "entry": "fan",
        "output": "join",
        "nodes": [
            {
                "id": "fan",
                "type": "fan-out",
                "branches": [
                    {"id": "left-flow", "target": "left"},
                    {"id": "right-flow", "target": "right"},
                ],
            },
            {"id": "left", "type": "task", "task": "left", "next": "join"},
            {"id": "right", "type": "task", "task": "right", "next": "join"},
            {
                "id": "join",
                "type": "converge",
                "inputs": {"left-flow": "left", "right-flow": "right"},
            },
        ],
    }
    router = Router(
        specification,
        {
            "left": Task(echo, healthcheck=lambda: check("left", True)),
            "right": Task(echo, healthcheck=lambda: check("right", False)),
            "unused": Task(echo, healthcheck=lambda: check("unused", False)),
        },
    )

    assert await router.healthcheck() == {"left": True, "right": False}
    assert started == {"left", "right"}


@pytest.mark.asyncio
async def test_healthcheck_contract_and_exceptions_are_not_hidden():
    def synchronous():
        return True

    async def non_boolean():
        return 1

    failure = LookupError("backend probe failed")

    async def fails():
        raise failure

    with pytest.raises(TypeError, match="awaitable"):
        await Router(task_spec(), {"echo": Task(echo, healthcheck=synchronous)}).healthcheck()
    with pytest.raises(TypeError, match="boolean"):
        await Router(task_spec(), {"echo": Task(echo, healthcheck=non_boolean)}).healthcheck()
    with pytest.raises(LookupError) as caught:
        await Router(task_spec(), {"echo": Task(echo, healthcheck=fails)}).healthcheck()
    assert caught.value is failure


def test_task_is_immutable():
    task = Task(echo)

    with pytest.raises(AttributeError):
        task._task = echo
    with pytest.raises(AttributeError):
        del task._task
