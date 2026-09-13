from __future__ import annotations

import inspect

from .types import AsyncHealthcheck, AsyncTask


class Task:
    __slots__ = ("_healthcheck", "_task")
    _healthcheck: AsyncHealthcheck | None
    _task: AsyncTask

    def __init__(
        self,
        task: AsyncTask,
        *,
        healthcheck: AsyncHealthcheck | None = None,
    ) -> None:
        object.__setattr__(self, "_task", task)
        object.__setattr__(self, "_healthcheck", healthcheck)

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("Task instances are immutable")

    def __delattr__(self, name: str) -> None:
        raise AttributeError("Task instances are immutable")

    @property
    def task(self) -> AsyncTask:
        return self._task

    async def healthcheck(self) -> bool:
        if self._healthcheck is None:
            return True
        result = self._healthcheck()
        if not inspect.isawaitable(result):
            raise TypeError("task healthcheck must return an awaitable")
        healthy = await result
        if not isinstance(healthy, bool):
            raise TypeError("task healthcheck must resolve to a boolean")
        return healthy


__all__ = ["Task"]
