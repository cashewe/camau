from collections.abc import Mapping
from typing import Any, TypeAlias

from .types import AsyncTask, JsonObject, JsonValue

class Issue:
    @property
    def code(self) -> str: ...
    @property
    def message(self) -> str: ...
    @property
    def path(self) -> str: ...

class Assessment:
    @property
    def valid(self) -> bool: ...
    @property
    def issues(self) -> tuple[Issue, ...]: ...
    def to_text(self) -> str: ...
    def to_junit_xml(self) -> str: ...
    def to_github_annotations(self, source: str | None = None) -> str: ...

class Assessor:
    @staticmethod
    def assess(specification: dict[str, JsonValue] | str) -> Assessment: ...
    @staticmethod
    def schema() -> dict[str, JsonValue]: ...

class CamauError(Exception): ...

class ConfigurationError(CamauError):
    assessment: Assessment

class MappingError(CamauError):
    node_id: str
    target: str
    source: str | None
    expected_type: str
    reason: str

class TaskResultError(CamauError):
    node_id: str
    task_name: str
    reason: str
    invalid_value: Any

class RoutingSelectionError(CamauError):
    code: str
    node_id: str
    message: str
    gate_node_id: str | None
    selected_path: str | None
    value_found: bool | None
    selected_value: JsonValue | None
    matched_operator: str | None

TaskRequest: TypeAlias = tuple[int, AsyncTask, JsonObject]
Progress: TypeAlias = tuple[list[TaskRequest], JsonObject | None]

class Execution:
    def start(self) -> Progress: ...
    def resume(self, ticket: int, result: JsonObject) -> Progress: ...
    def reject(self, ticket: int, invalid_value: Any, reason: str) -> None: ...

class _Router:
    def __init__(
        self,
        specification: dict[str, JsonValue] | str,
        tasks: Mapping[str, AsyncTask],
        *,
        seed: int | None = None,
    ) -> None: ...
    def new_run(self, payload: JsonObject) -> Execution: ...
