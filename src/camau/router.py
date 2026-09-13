from __future__ import annotations

import asyncio
import inspect
from collections.abc import Iterator, Mapping
from types import MappingProxyType
from typing import TypeAlias

from ._camau import _Router
from .task import Task
from .types import AsyncTask, JsonObject, JsonValue

TaskRegistration: TypeAlias = AsyncTask | Task


class _TaskRegistry(Mapping[str, AsyncTask]):
    def __init__(self, tasks: Mapping[str, TaskRegistration]) -> None:
        self._tasks = tasks
        self.resolved: dict[str, Task] = {}

    def __getitem__(self, name: str) -> AsyncTask:
        value = self._tasks[name]
        if isinstance(value, Task):
            self.resolved[name] = value
            return value.task
        if callable(value):
            self.resolved[name] = Task(value)
        return value

    def __iter__(self) -> Iterator[str]:
        return iter(self._tasks)

    def __len__(self) -> int:
        return len(self._tasks)


class Router:
    __slots__ = ("_inner", "_tasks")
    _inner: _Router
    _tasks: Mapping[str, Task]

    def __init__(
        self,
        specification: dict[str, JsonValue] | str,
        tasks: Mapping[str, TaskRegistration],
        *,
        seed: int | None = None,
    ) -> None:
        if hasattr(self, "_inner"):
            raise AttributeError("Router instances are immutable")
        registry = _TaskRegistry(tasks)
        inner = _Router(specification, registry, seed=seed)
        object.__setattr__(self, "_inner", inner)
        object.__setattr__(self, "_tasks", MappingProxyType(dict(registry.resolved)))

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("Router instances are immutable")

    def __delattr__(self, name: str) -> None:
        raise AttributeError("Router instances are immutable")

    async def healthcheck(self) -> dict[str, bool]:
        names = tuple(self._tasks)
        results = await asyncio.gather(*(self._tasks[name].healthcheck() for name in names))
        return dict(zip(names, results, strict=True))

    async def run(self, payload: JsonObject) -> JsonObject:
        execution = self._inner.new_run(payload)
        pending: dict[asyncio.Future[JsonObject], list[int]] = {}
        scheduled: dict[int, tuple[object, asyncio.Future[JsonObject]]] = {}

        def schedule(requests: list[tuple[int, AsyncTask, JsonObject]]) -> None:
            for ticket, task, task_payload in requests:
                result = task(task_payload)
                if not inspect.isawaitable(result):
                    execution.reject(ticket, result, "non_awaitable")
                identity = id(result)
                existing = scheduled.get(identity)
                future: asyncio.Future[JsonObject]
                if existing is None:
                    future = asyncio.ensure_future(result)
                    scheduled[identity] = (result, future)
                else:
                    future = existing[1]
                pending.setdefault(future, []).append(ticket)

        try:
            requests, output = execution.start()
            schedule(requests)
            while output is None:
                if not pending:
                    raise RuntimeError("compiled workflow stopped without an output")
                done, _ = await asyncio.wait(pending, return_when=asyncio.FIRST_COMPLETED)
                for completed in done:
                    tickets = pending.pop(completed)
                    result = completed.result()
                    for ticket in tickets:
                        requests, candidate = execution.resume(ticket, result)
                        schedule(requests)
                        if candidate is not None:
                            output = candidate
            return output
        except BaseException:
            for task in pending:
                task.cancel()
            if pending:
                await asyncio.gather(*pending, return_exceptions=True)
            raise
