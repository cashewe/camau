import asyncio

import pytest

from camau import Router
from tests.support.specifications import fanout_spec


@pytest.mark.asyncio
async def test_failure_cancels_and_awaits_sibling_cleanup():
    cleanup = asyncio.Event()
    failure = RuntimeError("boom")

    async def fails(_payload):
        await asyncio.sleep(0)
        raise failure

    async def waits(_payload):
        try:
            await asyncio.sleep(60)
        finally:
            await asyncio.sleep(0)
            cleanup.set()

    with pytest.raises(RuntimeError) as caught:
        await Router(fanout_spec(), {"left": fails, "right": waits}).run({})
    assert caught.value is failure
    assert cleanup.is_set()


@pytest.mark.asyncio
async def test_caller_cancellation_cleans_up_all_tasks():
    started = asyncio.Event()
    cleaned = 0

    async def waits(payload):
        nonlocal cleaned
        started.set()
        try:
            await asyncio.sleep(60)
        finally:
            cleaned += 1
        return payload

    run = asyncio.create_task(Router(fanout_spec(), {"left": waits, "right": waits}).run({}))
    await started.wait()
    run.cancel()
    with pytest.raises(asyncio.CancelledError):
        await run
    assert cleaned == 2


@pytest.mark.asyncio
async def test_task_cancellation_cleans_siblings_and_propagates_unchanged():
    cleaned = asyncio.Event()

    async def cancels(_payload):
        await asyncio.sleep(0)
        raise asyncio.CancelledError("task requested cancellation")

    async def waits(_payload):
        try:
            await asyncio.sleep(60)
        finally:
            cleaned.set()

    with pytest.raises(asyncio.CancelledError) as caught:
        await Router(fanout_spec(), {"left": cancels, "right": waits}).run({})
    assert caught.value.args == ("task requested cancellation",)
    assert cleaned.is_set()


@pytest.mark.asyncio
async def test_task_suppressing_cancellation_delays_failure_until_it_finishes():
    finished = asyncio.Event()
    failure = RuntimeError("primary failure")

    async def fails(_payload):
        await asyncio.sleep(0)
        raise failure

    async def suppresses(_payload):
        try:
            await asyncio.sleep(60)
        except asyncio.CancelledError:
            await asyncio.sleep(0.01)
            finished.set()
            return {}

    with pytest.raises(RuntimeError) as caught:
        await Router(fanout_spec(), {"left": fails, "right": suppresses}).run({})
    assert caught.value is failure
    assert finished.is_set()


@pytest.mark.asyncio
async def test_shared_future_resumes_every_ticket():
    async def resolve():
        await asyncio.sleep(0)
        return {"shared": True}

    shared = resolve()

    def returns_shared(_payload):
        return shared

    result = await Router(
        fanout_spec(),
        {"left": returns_shared, "right": returns_shared},
    ).run({})

    assert result == {
        "first": {"shared": True},
        "second": {"shared": True},
    }


@pytest.mark.asyncio
async def test_shared_future_failure_propagates_once_without_lost_cleanup():
    failure = RuntimeError("shared failure")
    shared = asyncio.get_running_loop().create_future()
    shared.set_exception(failure)

    def returns_shared(_payload):
        return shared

    with pytest.raises(RuntimeError) as caught:
        await Router(
            fanout_spec(),
            {"left": returns_shared, "right": returns_shared},
        ).run({})

    assert caught.value is failure


@pytest.mark.asyncio
async def test_cancelling_shared_future_consumers_cancels_the_future_once():
    class CountingFuture(asyncio.Future):
        def __init__(self):
            super().__init__()
            self.cancel_calls = 0

        def cancel(self, msg=None):
            self.cancel_calls += 1
            return super().cancel(msg)

    shared = CountingFuture()

    def returns_shared(_payload):
        return shared

    run = asyncio.create_task(
        Router(
            fanout_spec(),
            {"left": returns_shared, "right": returns_shared},
        ).run({})
    )
    await asyncio.sleep(0)
    run.cancel()

    with pytest.raises(asyncio.CancelledError):
        await run
    assert shared.cancelled()
    assert shared.cancel_calls == 1


@pytest.mark.asyncio
async def test_shared_future_supports_three_consumers():
    specification = {
        "entry": "fan",
        "output": "join",
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
            {"id": "a-task", "type": "task", "task": "shared", "next": "join"},
            {"id": "b-task", "type": "task", "task": "shared", "next": "join"},
            {"id": "c-task", "type": "task", "task": "shared", "next": "join"},
            {
                "id": "join",
                "type": "converge",
                "inputs": {"a": "a-task", "b": "b-task", "c": "c-task"},
            },
        ],
    }
    shared = asyncio.get_running_loop().create_future()
    shared.set_result({"value": 1})

    def returns_shared(_payload):
        return shared

    result = await Router(specification, {"shared": returns_shared}).run({})

    assert result == {
        "a": {"value": 1},
        "b": {"value": 1},
        "c": {"value": 1},
    }
