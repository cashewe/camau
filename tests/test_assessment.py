import pytest

from camau import Assessor


def test_assessment_schema_immutability_and_renderers():
    schema = Assessor.schema()
    schema["title"] = "changed"
    assert Assessor.schema()["title"] == "Camau routing specification"

    assessment = Assessor.assess('{"entry":"x","entry":"y","output":"x","nodes":[]}')
    assert not assessment.valid
    assert isinstance(assessment.issues, tuple)
    assert any(issue.code == "JSON_DUPLICATE_KEY" for issue in assessment.issues)
    assert "<testsuite" in assessment.to_junit_xml()
    annotations = assessment.to_github_annotations("route%,\n.json")
    assert "%25" in annotations and "%0A" in annotations and "::error" in annotations

    with pytest.raises(AttributeError):
        assessment.valid = True
    with pytest.raises(AttributeError):
        assessment.issues[0].code = "changed"
