"""``/v1/stream`` WebSocket client with auto-reconnect.

Unauthenticated connections receive ``verdict`` and ``pull`` events 60 s after they happen; real-time delivery
comes with API plans. Event-driven: iterate the stream and react to each event as it arrives.
"""

from __future__ import annotations

import asyncio
from typing import AsyncIterator, Mapping, Optional
from urllib.parse import urlsplit, urlunsplit

from websockets.asyncio.client import connect
from websockets.exceptions import WebSocketException

from ._common import USER_AGENT, backoff_s, resolve_base_url
from .errors import RugTripError
from .models import Connected, Reconnecting, StreamEvent, parse_frame

__all__ = ["Stream", "stream_url"]


def stream_url(base_url: str) -> str:
    """http(s)://host[/prefix] -> ws(s)://host[/prefix]/v1/stream"""
    u = urlsplit(resolve_base_url(base_url))
    scheme = "wss" if u.scheme == "https" else "ws"
    return urlunsplit((scheme, u.netloc, u.path.rstrip("/") + "/v1/stream", "", ""))


class Stream:
    """Async iterator of stream events.

    Yields ``VerdictEvent`` / ``PullEvent`` / ``AlertEvent`` / ``UnknownEvent`` as frames arrive, plus
    ``Connected`` and ``Reconnecting`` markers so a bot can log or backfill after a gap. Reconnects with
    exponential backoff and jitter (capped) until ``close()`` or ``max_reconnects`` consecutive failures.
    Pings are handled by the ``websockets`` library; a dead connection is detected and replaced.
    """

    def __init__(
        self,
        base_url: Optional[str] = None,
        *,
        headers: Optional[Mapping[str, str]] = None,
        initial_backoff_s: float = 0.5,
        max_backoff_s: float = 30.0,
        max_reconnects: Optional[int] = None,
        open_timeout_s: float = 10.0,
        ping_interval_s: Optional[float] = 20.0,
    ):
        self.url = stream_url(resolve_base_url(base_url))
        self.headers = dict(headers or {"User-Agent": USER_AGENT})
        self.initial_backoff_s = initial_backoff_s
        self.max_backoff_s = max_backoff_s
        self.max_reconnects = max_reconnects
        self.open_timeout_s = open_timeout_s
        self.ping_interval_s = ping_interval_s
        self._closed = False
        self._ws = None

    async def close(self) -> None:
        self._closed = True
        ws = self._ws
        if ws is not None:
            await ws.close()

    def __aiter__(self) -> AsyncIterator[StreamEvent]:
        return self._run()

    async def _run(self) -> AsyncIterator[StreamEvent]:
        attempt = 0
        ua = self.headers.get("User-Agent", USER_AGENT)
        extra = {k: v for k, v in self.headers.items() if k.lower() != "user-agent"}
        while not self._closed:
            reason = "closed"
            try:
                async with connect(
                    self.url,
                    user_agent_header=ua,
                    additional_headers=extra or None,
                    open_timeout=self.open_timeout_s,
                    ping_interval=self.ping_interval_s,
                    max_size=2**22,
                ) as ws:
                    self._ws = ws
                    yield Connected(url=self.url, attempt=attempt)
                    got_frame = False
                    async for raw in ws:
                        if not got_frame:
                            got_frame = True
                            attempt = 0  # healthy connection: reset backoff
                        text = raw if isinstance(raw, str) else raw.decode("utf-8", "replace")
                        ev = parse_frame(text)
                        if ev is not None:
                            yield ev
                    reason = f"closed ({ws.close_code})" if ws.close_code else "closed"
            except (OSError, asyncio.TimeoutError, TimeoutError, WebSocketException) as e:
                reason = f"{type(e).__name__}: {e}"
            finally:
                self._ws = None
            if self._closed:
                return
            if self.max_reconnects is not None and attempt >= self.max_reconnects:
                raise RugTripError("network", f"Gave up after {attempt} reconnect attempts ({reason})")
            delay = backoff_s(attempt, self.initial_backoff_s, self.max_backoff_s)
            attempt += 1
            yield Reconnecting(attempt=attempt, delay_s=delay, reason=reason)
            await asyncio.sleep(delay)
