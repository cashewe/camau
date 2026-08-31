import camau


def test_public_exports_are_stable():
    assert camau.__all__ == [
        "Assessment",
        "Assessor",
        "CamauError",
        "ConfigurationError",
        "Issue",
        "MappingError",
        "Router",
        "RoutingSelectionError",
        "TaskResultError",
    ]
    assert {name for name in camau.__all__ if hasattr(camau, name)} == set(camau.__all__)
