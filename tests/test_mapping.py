import pytest

from camau import Assessor, MappingError, Router


@pytest.mark.asyncio
async def test_mapping_nested_defaults_and_errors():
    specification = {
        "entry": "map",
        "output": "map",
        "nodes": [
            {
                "id": "map",
                "type": "map-schema",
                "mappings": [
                    {"target": "/user/name", "path-to-source": "/profile/name", "type": "string"},
                    {"target": "/user/tags", "path-to-source": "/profile/tags", "type": "array"},
                    {
                        "target": "/source",
                        "path-to-source": "/missing",
                        "default": "gateway",
                        "type": "string",
                    },
                ],
            }
        ],
    }
    router = Router(specification, {})
    assert await router.run({"profile": {"name": "Ada", "tags": ["a"]}}) == {
        "user": {"name": "Ada", "tags": ["a"]},
        "source": "gateway",
    }
    with pytest.raises(MappingError) as mismatch:
        await router.run({"profile": {"name": 3, "tags": []}})
    assert mismatch.value.reason == "type_mismatch"
    assert mismatch.value.target == "/user/name"

    invalid = specification.copy()
    invalid["nodes"] = [dict(specification["nodes"][0])]
    invalid["nodes"][0]["mappings"] = [
        {"target": "/user", "default": {}, "type": "object"},
        {"target": "/user/name", "default": "x", "type": "string"},
    ]
    assert "MAPPING_TARGET" in {issue.code for issue in Assessor.assess(invalid).issues}


@pytest.mark.asyncio
@pytest.mark.parametrize("token", ["", "+0", "+1", "01", "-0", "-1", "18446744073709551616"])
async def test_array_mapping_indexes_require_canonical_decimal_tokens(token):
    specification = {
        "entry": "map",
        "output": "map",
        "nodes": [
            {
                "id": "map",
                "type": "map-schema",
                "mappings": [
                    {"target": "/value", "path-to-source": f"/values/{token}", "type": "string"}
                ],
            }
        ],
    }
    router = Router(specification, {})

    with pytest.raises(MappingError) as caught:
        await router.run({"values": ["zero", "one"]})
    assert caught.value.reason == "source_missing"
    assert await router.run({"values": {token: "matched"}}) == {"value": "matched"}


@pytest.mark.asyncio
async def test_canonical_array_mapping_indexes_resolve():
    specification = {
        "entry": "map",
        "output": "map",
        "nodes": [
            {
                "id": "map",
                "type": "map-schema",
                "mappings": [
                    {"target": "/zero", "path-to-source": "/values/0", "type": "string"},
                    {"target": "/one", "path-to-source": "/values/1", "type": "string"},
                    {
                        "target": "/escaped~1key",
                        "path-to-source": "/source~0key",
                        "type": "string",
                    },
                ],
            }
        ],
    }

    assert await Router(specification, {}).run(
        {"values": ["zero", "one"], "source~key": "escaped"}
    ) == {
        "zero": "zero",
        "one": "one",
        "escaped/key": "escaped",
    }
