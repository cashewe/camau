import pytest

from camau import Assessor, Router, TaskResultError
from tests.support.specifications import task_spec


def cyclic_payload():
    payload = {}
    payload["self"] = payload
    return payload


@pytest.mark.asyncio
async def test_router_input_rejects_direct_dictionary_and_list_cycles_with_paths():
    async def echo(payload):
        return payload

    router = Router(task_spec(), {"echo": echo})
    with pytest.raises(ValueError, match=r"reference cycle at /self"):
        await router.run(cyclic_payload())

    items = []
    items.append(items)
    with pytest.raises(ValueError, match=r"reference cycle at /items/0"):
        await router.run({"items": items})


@pytest.mark.asyncio
async def test_router_input_rejects_indirect_mixed_cycle():
    async def echo(payload):
        return payload

    payload = {}
    items = [payload]
    payload["items"] = items
    with pytest.raises(ValueError, match=r"reference cycle at /items/0"):
        await Router(task_spec(), {"echo": echo}).run(payload)


def test_dictionary_specification_reports_cycles_as_json_value_issues():
    specification = cyclic_payload()
    issue = Assessor.assess(specification).issues[0]
    assert (issue.code, issue.path) == ("JSON_VALUE", "/self")
    assert "reference cycle" in issue.message


@pytest.mark.asyncio
async def test_task_result_cycle_uses_the_task_result_error_contract():
    async def cyclic(_payload):
        return cyclic_payload()

    with pytest.raises(TaskResultError) as caught:
        await Router(task_spec(), {"echo": cyclic}).run({})
    assert caught.value.reason == "invalid_json"


@pytest.mark.asyncio
async def test_deep_acyclic_input_fails_at_the_documented_depth():
    async def echo(payload):
        return payload

    payload = {}
    current = payload
    for _ in range(128):
        child = {}
        current["child"] = child
        current = child

    with pytest.raises(ValueError, match="maximum depth of 128"):
        await Router(task_spec(), {"echo": echo}).run(payload)
