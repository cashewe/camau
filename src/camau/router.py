from __future__ import annotations

import asyncio
import inspect
from collections.abc import Mapping

from ._camau import _Router
from .types import AsyncTask, JsonObject, JsonValue


class Router:
    __slots__ = ("_inner",)
    _inner: _Router

    def __init__(
        self,
        specification: dict[str, JsonValue] | str,
        tasks: Mapping[str, AsyncTask],
        *,
        seed: int | None = None,
    ) -> None:
        if hasattr(self, "_inner"):
            raise AttributeError("Router instances are immutable")
        object.__setattr__(self, "_inner", _Router(specification, tasks, seed=seed))

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("Router instances are immutable")

    def __delattr__(self, name: str) -> None:
        raise AttributeError("Router instances are immutable")

    async def run(self, payload: JsonObject) -> JsonObject:
        execution = self._inner.new_run(payload)
        pending: dict[asyncio.Future[JsonObject], int] = {}

        def schedule(requests: list[tuple[int, AsyncTask, JsonObject]]) -> None:
            for ticket, task, task_payload in requests:
                result = task(task_payload)
                if not inspect.isawaitable(result):
                    execution.reject(ticket, result, "non_awaitable")
                pending[asyncio.ensure_future(result)] = ticket

        try:
            requests, output = execution.start()
            schedule(requests)
            while output is None:
                if not pending:
                    raise RuntimeError("compiled workflow stopped without an output")
                done, _ = await asyncio.wait(pending, return_when=asyncio.FIRST_COMPLETED)
                for completed in done:
                    ticket = pending.pop(completed)
                    result = completed.result()
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
