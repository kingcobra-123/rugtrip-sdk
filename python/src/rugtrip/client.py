"""HTTP clients for the public RugTrip API: ``Client`` (sync) and ``AsyncClient``."""

from __future__ import annotations

import asyncio
import time
from typing import Any, Mapping, Optional

import httpx

from ._common import USER_AGENT, backoff_s, clean_query, handle_response, map_transport_error, resolve_base_url
from .address import to_target
from .errors import RugTripError
from .models import Health, RugsToday, Scorecard, Stats, TokenCheck, Verdict
from .stream import Stream

__all__ = ["Client", "AsyncClient"]


class _Base:
    def __init__(self, base_url: Optional[str], timeout: float, max_retries: int, retry_base_s: float, headers: Optional[Mapping[str, str]]):
        self.base_url = resolve_base_url(base_url)
        self.timeout = timeout
        self.max_retries = max(0, max_retries)
        self.retry_base_s = retry_base_s
        self.user_agent = USER_AGENT
        self.headers = {"User-Agent": USER_AGENT, "Accept": "application/json", **(headers or {})}

    def _wait(self, err: RugTripError, attempt: int) -> float:
        return err.retry_after_s if err.retry_after_s is not None else backoff_s(attempt, self.retry_base_s, 30.0)

    @staticmethod
    def _feed_q(rating: Optional[str], venue: Optional[str], limit: Optional[int], before: Optional[float], after: Optional[float], include_late: bool) -> dict[str, str]:
        return clean_query({"rating": rating, "venue": venue, "limit": limit, "before": before, "after": after, "include_late": include_late})


class Client(_Base):
    """Sync client.

    >>> rt = Client(base_url=YOUR_API_URL)   # or set RUGTRIP_API_URL
    >>> t = rt.check("0x...")                           # token, pool, v4 pool id or chart link
    """

    def __init__(self, base_url: Optional[str] = None, *, timeout: float = 10.0, max_retries: int = 2, retry_base_s: float = 1.0, headers: Optional[Mapping[str, str]] = None, transport: Optional[httpx.BaseTransport] = None):
        super().__init__(base_url, timeout, max_retries, retry_base_s, headers)
        self._http = httpx.Client(base_url=self.base_url, timeout=timeout, headers=self.headers, transport=transport)

    def close(self) -> None:
        self._http.close()

    def __enter__(self) -> "Client":
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()

    def _get(self, path: str, params: Optional[Mapping[str, str]] = None) -> Any:
        attempt = 0
        while True:
            try:
                try:
                    r = self._http.get(path, params=params)
                except httpx.HTTPError as e:
                    raise map_transport_error(e, self.timeout) from e
                return handle_response(r)
            except RugTripError as err:
                if not err.retryable or attempt >= self.max_retries:
                    raise
                time.sleep(self._wait(err, attempt))
                attempt += 1

    def health(self) -> Health:
        return Health.from_dict(self._get("/healthz"))

    def check(self, target: str) -> TokenCheck:
        """Token address, pool address, v4 pool id or DexScreener / GeckoTerminal link -> the token's verdicts.
        ``best`` is the most liquid pool; gate on the pool you will actually trade through."""
        return TokenCheck.from_dict(self._get(f"/v1/token/{to_target(target)}"))

    def pool(self, addr: str) -> Verdict:
        return Verdict.from_dict(self._get(f"/v1/pool/{to_target(addr)}"))

    def feed_recent(self, *, rating: Optional[str] = None, venue: Optional[str] = None, limit: Optional[int] = None, before: Optional[float] = None, after: Optional[float] = None, include_late: bool = False) -> list[Verdict]:
        """Newest pools with verdicts (unauthenticated: 60 s delayed)."""
        d = self._get("/v1/feed/recent", self._feed_q(rating, venue, limit, before, after, include_late))
        return [Verdict.from_dict(p) for p in d.get("pools", [])]

    def rugs_today(self, *, since: Optional[float] = None, before: Optional[float] = None, limit: Optional[int] = None) -> RugsToday:
        return RugsToday.from_dict(self._get("/v1/rugs/today", clean_query({"since": since, "before": before, "limit": limit})))

    def scorecard(self) -> Scorecard:
        return Scorecard.from_dict(self._get("/v1/scorecard"))

    def stats(self) -> Stats:
        return Stats.from_dict(self._get("/v1/stats"))


class AsyncClient(_Base):
    """Async client; also provides ``stream()``.

    >>> async with AsyncClient(base_url=YOUR_API_URL) as rt:
    ...     t = await rt.check("0x...")
    ...     async for ev in rt.stream():
    ...         ...
    """

    def __init__(self, base_url: Optional[str] = None, *, timeout: float = 10.0, max_retries: int = 2, retry_base_s: float = 1.0, headers: Optional[Mapping[str, str]] = None, transport: Optional[httpx.AsyncBaseTransport] = None):
        super().__init__(base_url, timeout, max_retries, retry_base_s, headers)
        self._http = httpx.AsyncClient(base_url=self.base_url, timeout=timeout, headers=self.headers, transport=transport)

    async def aclose(self) -> None:
        await self._http.aclose()

    async def __aenter__(self) -> "AsyncClient":
        return self

    async def __aexit__(self, *exc: object) -> None:
        await self.aclose()

    async def _get(self, path: str, params: Optional[Mapping[str, str]] = None) -> Any:
        attempt = 0
        while True:
            try:
                try:
                    r = await self._http.get(path, params=params)
                except httpx.HTTPError as e:
                    raise map_transport_error(e, self.timeout) from e
                return handle_response(r)
            except RugTripError as err:
                if not err.retryable or attempt >= self.max_retries:
                    raise
                await asyncio.sleep(self._wait(err, attempt))
                attempt += 1

    async def health(self) -> Health:
        return Health.from_dict(await self._get("/healthz"))

    async def check(self, target: str) -> TokenCheck:
        """Token address, pool address, v4 pool id or DexScreener / GeckoTerminal link -> the token's verdicts."""
        return TokenCheck.from_dict(await self._get(f"/v1/token/{to_target(target)}"))

    async def pool(self, addr: str) -> Verdict:
        return Verdict.from_dict(await self._get(f"/v1/pool/{to_target(addr)}"))

    async def feed_recent(self, *, rating: Optional[str] = None, venue: Optional[str] = None, limit: Optional[int] = None, before: Optional[float] = None, after: Optional[float] = None, include_late: bool = False) -> list[Verdict]:
        d = await self._get("/v1/feed/recent", self._feed_q(rating, venue, limit, before, after, include_late))
        return [Verdict.from_dict(p) for p in d.get("pools", [])]

    async def rugs_today(self, *, since: Optional[float] = None, before: Optional[float] = None, limit: Optional[int] = None) -> RugsToday:
        return RugsToday.from_dict(await self._get("/v1/rugs/today", clean_query({"since": since, "before": before, "limit": limit})))

    async def scorecard(self) -> Scorecard:
        return Scorecard.from_dict(await self._get("/v1/scorecard"))

    async def stats(self) -> Stats:
        return Stats.from_dict(await self._get("/v1/stats"))

    def stream(self, **kwargs: Any) -> Stream:
        """Live verdict / pull stream with auto-reconnect. See :class:`rugtrip.stream.Stream`."""
        headers = {k: v for k, v in self.headers.items() if k.lower() != "accept"}
        return Stream(self.base_url, headers=headers, **kwargs)

