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
