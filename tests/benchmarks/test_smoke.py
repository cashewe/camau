import pytest

from .runner import fanout_router, linear_router, measure, soak


@pytest.mark.asyncio
async def test_benchmark_scenarios_complete_at_smoke_scale() -> None:
    payload = {"blob": "x" * 128}

    linear = await measure(linear_router(), payload, warmup=1, samples=2)
    fanout = await measure(fanout_router(), payload, warmup=1, samples=2)
    soaked = await soak(linear_router(), payload, runs=2)

    assert linear["samples"] == 2
    assert fanout["samples"] == 2
    assert soaked["runs"] == 2
