"""Errors. API codes come from the ``{"error": code, "message": text}`` body; the rest are raised by the SDK."""

from __future__ import annotations

import email.utils
import time
from typing import Any, Optional

API_CODES = frozenset({"bad_address", "unknown_token", "rate_limited", "bad_request", "not_found", "unauthorized", "pro_required", "internal"})
#: SDK-side codes
SDK_CODES = frozenset({"http_error", "bad_response", "timeout", "network", "config"})


class RugTripError(Exception):
    """``code`` is one of API_CODES or SDK_CODES. ``api_code`` keeps the raw server string."""

    def __init__(self, code: str, message: str, *, status: Optional[int] = None, retry_after_s: Optional[float] = None, api_code: Optional[str] = None):
        super().__init__(message)
        self.code = code
        self.message = message
        self.status = status
        self.retry_after_s = retry_after_s
        self.api_code = api_code

    @property
    def retryable(self) -> bool:
        return self.code in ("rate_limited", "timeout", "network") or (self.status is not None and 502 <= self.status <= 504)

    def __repr__(self) -> str:
        return f"RugTripError(code={self.code!r}, status={self.status!r}, message={self.message!r})"


class ConfigError(RugTripError):
    def __init__(self, message: str):
        super().__init__("config", message)


def error_from_response(status: int, body: Any, retry_after_s: Optional[float] = None) -> RugTripError:
    if isinstance(body, dict) and isinstance(body.get("error"), str):
        api_code = body["error"]
        message = body.get("message") if isinstance(body.get("message"), str) else api_code
        code = api_code if api_code in API_CODES else ("rate_limited" if status == 429 else "http_error")
        return RugTripError(code, message, status=status, retry_after_s=retry_after_s, api_code=api_code)
    return RugTripError("rate_limited" if status == 429 else "http_error", f"HTTP {status}", status=status, retry_after_s=retry_after_s)


def parse_retry_after(v: Optional[str], now: Optional[float] = None) -> Optional[float]:
    """Retry-After (seconds or HTTP date) -> seconds."""
    if not v:
        return None
    s = v.strip()
    try:
        return max(0.0, float(s))
    except ValueError:
        pass
    try:
        t = email.utils.parsedate_to_datetime(s).timestamp()
    except (TypeError, ValueError):
        return None
    return max(0.0, t - (time.time() if now is None else now))
