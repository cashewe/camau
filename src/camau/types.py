from collections.abc import Awaitable, Callable
from typing import TypeAlias

JsonScalar: TypeAlias = str | int | float | bool | None
JsonValue: TypeAlias = JsonScalar | list["JsonValue"] | dict[str, "JsonValue"]
JsonObject: TypeAlias = dict[str, JsonValue]
AsyncTask: TypeAlias = Callable[[JsonObject], Awaitable[JsonObject]]

__all__ = ["AsyncTask", "JsonObject", "JsonScalar", "JsonValue"]
