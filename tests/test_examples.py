from pathlib import Path

import pytest

from camau import Assessor, Router

EXAMPLES = Path(__file__).parent / "examples"


def example(name: str) -> str:
    return (EXAMPLES / name).read_text(encoding="utf-8")


@pytest.mark.asyncio
async def test_scored_route_example_builds_and_runs() -> None:
    specification = example("scored-route.json")
    assert Assessor.assess(specification).valid

    async def priority_handler(payload):
        return {**payload, "handled-by": "priority"}

    async def standard_handler(payload):
        return {**payload, "handled-by": "standard"}

    router = Router(
        specification,
        {
            "priority-handler": priority_handler,
            "standard-handler": standard_handler,
        },
    )

    assert await router.run({"request": {"id": "request-42"}, "priority": "high"}) == {
        "request-id": "request-42",
        "handled-by": "priority",
    }


@pytest.mark.asyncio
async def test_parallel_checks_example_builds_and_runs() -> None:
    specification = example("parallel-checks.json")
    assert Assessor.assess(specification).valid

    async def fraud_checker(_payload):
        return {"status": "clear", "score": 0.02}

    async def policy_checker(_payload):
        return {"status": "eligible", "policy": "standard"}

    router = Router(
        specification,
        {
            "fraud-checker": fraud_checker,
            "policy-checker": policy_checker,
        },
    )

    assert await router.run({"customer-id": "customer-7"}) == {
        "fraud": {"status": "clear", "score": 0.02},
        "policy": {"status": "eligible", "policy": "standard"},
    }
