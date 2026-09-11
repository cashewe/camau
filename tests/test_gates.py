import pytest

from camau import Router, RoutingSelectionError
from tests.support.specifications import gated_spec


@pytest.mark.asyncio
async def test_deterministic_gate_and_gwall_context():
    calls = 0

    async def supported(payload):
        nonlocal calls
        calls += 1
        return payload

    router = Router(gated_spec(), {"supported": supported})
    assert await router.run({"score": 10}) == {"score": 10, "model": "supported"}
    assert calls == 1
    with pytest.raises(RoutingSelectionError) as caught:
        await router.run({"score": "bad"})
    error = caught.value
    assert error.code == "GWALL"
    assert error.gate_node_id == "gate"
    assert error.selected_path == "/score"
    assert error.value_found is True
    assert error.selected_value == "bad"
    assert error.matched_operator == "otherwise"


@pytest.mark.asyncio
async def test_randomised_gate_seeded_serial_repeatability_and_reachability():
    specification = {
        "entry": "gate",
        "output": "done",
        "nodes": [
            {
                "id": "gate",
                "type": "randomised-gate",
                "routes": [
                    {"weight": 3, "target": "left"},
                    {"weight": 1, "target": "right"},
                ],
            },
            {"id": "left", "type": "task", "task": "left", "next": "done"},
            {"id": "right", "type": "task", "task": "right", "next": "done"},
            {
                "id": "done",
                "type": "map-schema",
                "mappings": [{"target": "/route", "path-to-source": "/route", "type": "string"}],
            },
        ],
    }

    async def left(_payload):
        return {"route": "left"}

    async def right(_payload):
        return {"route": "right"}

    async def sequence(seed):
        router = Router(specification, {"left": left, "right": right}, seed=seed)
        return [(await router.run({}))["route"] for _ in range(100)]

    first = await sequence(42)
    second = await sequence(42)
    assert first == second
    assert set(first) == {"left", "right"}

    router = Router(specification, {"left": left, "right": right}, seed=9)
    samples = [(await router.run({}))["route"] for _ in range(5_000)]
    left_share = samples.count("left") / len(samples)
    assert 0.70 < left_share < 0.80
    with pytest.raises(TypeError):
        Router(specification, {"left": left, "right": right}, seed=True)


@pytest.mark.asyncio
async def test_randomised_gate_handles_extreme_and_zero_weights():
    async def left(_payload):
        return {"route": "left"}

    async def right(_payload):
        return {"route": "right"}

    async def routes(weights, samples):
        specification = {
            "entry": "gate",
            "output": "done",
            "nodes": [
                {
                    "id": "gate",
                    "type": "randomised-gate",
                    "routes": [
                        {"weight": weights[0], "target": "left"},
                        {"weight": weights[1], "target": "right"},
                    ],
                },
                {"id": "left", "type": "task", "task": "left", "next": "done"},
                {"id": "right", "type": "task", "task": "right", "next": "done"},
                {
                    "id": "done",
                    "type": "map-schema",
                    "mappings": [
                        {"target": "/route", "path-to-source": "/route", "type": "string"}
                    ],
                },
            ],
        }
        router = Router(specification, {"left": left, "right": right}, seed=4)
        return [(await router.run({}))["route"] for _ in range(samples)]

    assert set(await routes((1e308, 1e308), 100)) == {"left", "right"}
    assert await routes((0, 1e308), 20) == ["right"] * 20
    assert await routes((1e-300, 1e308), 20) == ["right"] * 20
