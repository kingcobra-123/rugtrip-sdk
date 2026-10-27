from __future__ import annotations

import re
from urllib.parse import quote, unquote

_HEX = re.compile(r"^0[xX][0-9a-fA-F]+$")


def _exact(s: str) -> str | None:
    if _HEX.match(s) and len(s) - 2 in (40, 64):
        return s
    return None


def to_target(value: str) -> str:
    """Token address, pool address, v4 pool id or chart link -> the path segment the API expects.

    Links are reduced to the last 40- or 64-hex segment of their path; anything else is sent URL-encoded (the server
    answers ``bad_address``). Case is kept so the server can check EIP-55 checksums.
    """
    s = value.strip()
    hit = _exact(s)
    if hit:
        return hit
    if "/" in s:
        path = re.split(r"[?#]", s, maxsplit=1)[0]
        for seg in reversed(path.split("/")):
            hit = _exact(unquote(seg))
            if hit:
                return hit
    return quote(s, safe="")
