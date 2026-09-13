from collections.abc import Awaitable, Callable
from typing import TypeAlias

JsonScalar: TypeAlias = str | int | float | bool | None
JsonValue: TypeAlias = JsonScalar | list["JsonValue"] | dict[str, "JsonValue"]
JsonObject: TypeAlias = dict[str, JsonValue]
AsyncTask: TypeAlias = Callable[[JsonObject], Awaitable[JsonObject]]
AsyncHealthcheck: TypeAlias = Callable[[], Awaitable[bool]]

__all__ = ["AsyncHealthcheck", "AsyncTask", "JsonObject", "JsonScalar", "JsonValue"]
