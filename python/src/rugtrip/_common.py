from __future__ import annotations

import os
import random
from typing import Any, Mapping, Optional
from urllib.parse import urlsplit

import httpx

from ._version import __version__
from .errors import ConfigError, RugTripError, error_from_response, parse_retry_after

USER_AGENT = f"rugtrip-sdk-python/{__version__}"


def resolve_base_url(base_url: Optional[str]) -> str:
    """The base URL from the argument, else env RUGTRIP_API_URL. There is no default endpoint."""
    raw = (base_url or os.environ.get("RUGTRIP_API_URL") or "").strip()
    if not raw:
        raise ConfigError("base_url is required: pass base_url=... or set RUGTRIP_API_URL. The SDK has no default endpoint.")
    u = urlsplit(raw)
    if u.scheme not in ("http", "https") or not u.netloc:
        raise ConfigError(f"base_url must be an http:// or https:// URL, got {raw!r}")
    return raw.rstrip("/")


def backoff_s(attempt: int, base_s: float, cap_s: float, rand: Optional[float] = None) -> float:
    """Backoff with jitter: a random wait in [d/2, d], d = min(cap, base * 2**attempt)."""
    d = min(cap_s, base_s * (2**attempt))
    r = random.random() if rand is None else rand
    return d / 2 + r * d / 2


def clean_query(q: Mapping[str, Any]) -> dict[str, str]:
    out: dict[str, str] = {}
    for k, v in q.items():
        if v is None or v == "" or v is False:
            continue
        out[k] = "1" if v is True else str(v)
    return out


def handle_response(r: httpx.Response) -> Any:
    body: Any = None
    if r.content:
        try:
            body = r.json()
        except ValueError:
            body = None
    if r.status_code >= 400:
        raise error_from_response(r.status_code, body, parse_retry_after(r.headers.get("retry-after")))
    if body is None:
        raise RugTripError("bad_response", f"Expected JSON from {r.request.url}", status=r.status_code)
    return body


def map_transport_error(e: Exception, timeout: float) -> RugTripError:
    if isinstance(e, httpx.TimeoutException):
        return RugTripError("timeout", f"Request timed out after {timeout} s")
    return RugTripError("network", f"Network error: {e}")
