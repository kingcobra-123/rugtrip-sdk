"""Wire types for the RugTrip public API (JSON, snake_case). Timestamps are unix seconds (float).

Every model has ``from_dict`` (tolerant of extra fields, so newer servers don't break older clients) and keeps the
server's field names.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Literal, Mapping, Optional, Union

Rating = Literal["TRAP", "HIGH_RISK", "CAUTION", "LOWER_RISK"]
#: All ratings, most risky first. A rating is a risk opinion, not a guarantee.
RATINGS: tuple[str, ...] = ("TRAP", "HIGH_RISK", "CAUTION", "LOWER_RISK")
Level = Literal["bad", "warn", "ok", "unknown"]
Check = Literal["sell_sim", "lp", "code", "bundle", "creator", "wallets", "template"]
Venue = Literal["v2", "v3", "v4"]
#: Who controls the pool's liquidity. ``unknown`` = not measured yet or no single dominant holder.
LpCustody = Literal["burned", "locked", "one_wallet", "contract", "unknown"]
#: Buy-and-sell test result. ``blocked`` = a test sale failed. ``no_route`` / ``error`` = the test couldn't run:
#: "couldn't check", never evidence of a trap.
SimStatus = Literal["ok", "blocked", "no_route", "error"]


def _opt_float(v: Any) -> Optional[float]:
    return None if v is None else float(v)


@dataclass(frozen=True)
class Reason:
    level: str
    check: str
    text: str

    @classmethod
    def from_dict(cls, d: Mapping[str, Any]) -> "Reason":
        return cls(level=d["level"], check=d["check"], text=d["text"])


@dataclass(frozen=True)
class LpDetails:
    custody: str
    #: share of the LP held by the largest single wallet, %
    held_pct: Optional[float] = None
    #: share of the LP burned or in a lock, %
    locked_pct: Optional[float] = None
    lock_until: Optional[float] = None

    @classmethod
    def from_dict(cls, d: Mapping[str, Any]) -> "LpDetails":
        return cls(
            custody=d.get("custody") or "unknown",
            held_pct=_opt_float(d.get("held_pct")),
            locked_pct=_opt_float(d.get("locked_pct")),
            lock_until=_opt_float(d.get("lock_until")),
        )


@dataclass(frozen=True)
class SimDetails:
    status: str
    buy_tax_pct: Optional[float] = None
    sell_tax_pct: Optional[float] = None

    @classmethod
    def from_dict(cls, d: Mapping[str, Any]) -> "SimDetails":
        return cls(status=d.get("status") or "error", buy_tax_pct=_opt_float(d.get("buy_tax_pct")), sell_tax_pct=_opt_float(d.get("sell_tax_pct")))


@dataclass(frozen=True)
class VerdictDetails:
    """Public facts behind the reasons. Fields are None when unknown; ``lp`` / ``sim`` are None while pending."""

    lp: Optional[LpDetails] = None
    sim: Optional[SimDetails] = None
    #: share of supply bought in the launch block by insiders, %
    bundle_pct: Optional[float] = None
    #: share of supply the creator still holds, %
    creator_pct: Optional[float] = None
    deployer: Optional[str] = None
    lp_holder: Optional[str] = None
    creator: Optional[str] = None

    @classmethod
    def from_dict(cls, d: Mapping[str, Any]) -> "VerdictDetails":
        return cls(
            lp=LpDetails.from_dict(d["lp"]) if d.get("lp") else None,
            sim=SimDetails.from_dict(d["sim"]) if d.get("sim") else None,
            bundle_pct=_opt_float(d.get("bundle_pct")),
            creator_pct=_opt_float(d.get("creator_pct")),
            deployer=d.get("deployer"),
            lp_holder=d.get("lp_holder"),
            creator=d.get("creator"),
        )


@dataclass(frozen=True)
class Verdict:
    """One pool's verdict. ``version`` 1 = first verdict; higher = re-rated."""

    pool: str
    token: str
    symbol: str
    venue: str
    pool_age_s: int
    liq_usd: Optional[float]
    rating: str
    reasons: list[Reason]
    issued_at: float
    updated_at: float
    version: int
    details: Optional[VerdictDetails] = None

    @classmethod
    def from_dict(cls, d: Mapping[str, Any]) -> "Verdict":
        return cls(
            pool=d["pool"],
            token=d["token"],
            symbol=d.get("symbol", ""),
            venue=d["venue"],
            pool_age_s=int(d.get("pool_age_s", 0)),
            liq_usd=_opt_float(d.get("liq_usd")),
            rating=d["rating"],
            reasons=[Reason.from_dict(r) for r in d.get("reasons", [])],
            issued_at=float(d["issued_at"]),
            updated_at=float(d.get("updated_at", d["issued_at"])),
            version=int(d.get("version", 1)),
            details=VerdictDetails.from_dict(d["details"]) if d.get("details") else None,
        )


@dataclass(frozen=True)
class TokenCheck:
    """``GET /v1/token/:x``: ``best`` is the most liquid pool; ``pools`` lists every known pool, most liquid first."""

    token: str
    symbol: str
    chain_id: int
    best: Verdict
    pools: list[Verdict]

    @classmethod
    def from_dict(cls, d: Mapping[str, Any]) -> "TokenCheck":
        return cls(
            token=d["token"],
            symbol=d.get("symbol", ""),
            chain_id=int(d["chain_id"]),
            best=Verdict.from_dict(d["best"]),
            pools=[Verdict.from_dict(p) for p in d.get("pools", [])],
        )


@dataclass(frozen=True)
class Pull:
    """A liquidity pull (rug) seen on chain."""

    pool: str
    token: str
    symbol: str
    venue: str
    launched_at: float
    pulled_at: float
    life_s: int
    liq_removed_usd: Optional[float]

    @classmethod
    def from_dict(cls, d: Mapping[str, Any]) -> "Pull":
        return cls(
            pool=d["pool"],
            token=d["token"],
            symbol=d.get("symbol", ""),
            venue=d["venue"],
            launched_at=float(d["launched_at"]),
            pulled_at=float(d["pulled_at"]),
            life_s=int(d["life_s"]),
            liq_removed_usd=_opt_float(d.get("liq_removed_usd")),
        )


@dataclass(frozen=True)
class RugsToday:
    since: float
    pulls: list[Pull]

    @classmethod
    def from_dict(cls, d: Mapping[str, Any]) -> "RugsToday":
        return cls(since=float(d["since"]), pulls=[Pull.from_dict(p) for p in d.get("pulls", [])])


@dataclass(frozen=True)
class ScorecardDay:
    day: str
    rating: str
    pools: int
    pulled_1h: int
    pulled_6h: int
    pulled_24h: int

    @classmethod
    def from_dict(cls, d: Mapping[str, Any]) -> "ScorecardDay":
        return cls(day=d["day"], rating=d["rating"], pools=int(d["pools"]), pulled_1h=int(d["pulled_1h"]), pulled_6h=int(d["pulled_6h"]), pulled_24h=int(d["pulled_24h"]))


@dataclass(frozen=True)
class ScorecardTotal:
    pools: int
    pulled_1h: int
    pulled_6h: int
    pulled_24h: int

    @classmethod
    def from_dict(cls, d: Mapping[str, Any]) -> "ScorecardTotal":
        return cls(pools=int(d["pools"]), pulled_1h=int(d["pulled_1h"]), pulled_6h=int(d["pulled_6h"]), pulled_24h=int(d["pulled_24h"]))


@dataclass(frozen=True)
class Scorecard:
    days: list[ScorecardDay]
    #: keyed by rating, summed over ``days``
    totals: dict[str, ScorecardTotal]

    @classmethod
    def from_dict(cls, d: Mapping[str, Any]) -> "Scorecard":
        return cls(
            days=[ScorecardDay.from_dict(x) for x in d.get("days", [])],
            totals={k: ScorecardTotal.from_dict(v) for k, v in (d.get("totals") or {}).items()},
        )


@dataclass(frozen=True)
class Stats:
    pools_24h: int
    pulls_24h: int
    median_life_s: Optional[float] = None
    #: alias of ``e2e_p50_ms`` kept for v1 clients
    decision_p50_ms: Optional[float] = None
    #: first sight of a pool -> first verdict, ms, p50
    e2e_p50_ms: Optional[float] = None
    #: verdict compute time, microseconds, p50
    engine_p50_us: Optional[float] = None

    @classmethod
    def from_dict(cls, d: Mapping[str, Any]) -> "Stats":
        return cls(
            pools_24h=int(d["pools_24h"]),
            pulls_24h=int(d["pulls_24h"]),
            median_life_s=_opt_float(d.get("median_life_s")),
            decision_p50_ms=_opt_float(d.get("decision_p50_ms")),
            e2e_p50_ms=_opt_float(d.get("e2e_p50_ms")),
            engine_p50_us=_opt_float(d.get("engine_p50_us")),
        )


@dataclass(frozen=True)
class Health:
    ok: bool
    head_block: Optional[int] = None
    lag_s: Optional[float] = None

    @classmethod
    def from_dict(cls, d: Mapping[str, Any]) -> "Health":
        hb = d.get("head_block")
        return cls(ok=bool(d["ok"]), head_block=None if hb is None else int(hb), lag_s=_opt_float(d.get("lag_s")))


@dataclass(frozen=True)
class Alert:
    """Alert frames are only sent to real-time (paid plan) connections."""

    id: int
    token: str
    symbol: str
    kind: str
    text: str
    numbers: dict[str, str]
    block: int
    at: float
    latency_ms: Optional[float] = None
    rating: Optional[str] = None
    pool_age_s: Optional[int] = None

    @classmethod
    def from_dict(cls, d: Mapping[str, Any]) -> "Alert":
        pa = d.get("pool_age_s")
        return cls(
            id=int(d["id"]),
            token=d["token"],
            symbol=d.get("symbol", ""),
            kind=d["kind"],
            text=d["text"],
            numbers={str(k): str(v) for k, v in (d.get("numbers") or {}).items()},
            block=int(d["block"]),
            at=float(d["at"]),
            latency_ms=_opt_float(d.get("latency_ms")),
            rating=d.get("rating"),
            pool_age_s=None if pa is None else int(pa),
        )


# ---------------------------------------------------------------------------------------------------------- stream


@dataclass(frozen=True)
class VerdictEvent:
    data: Verdict
    type: Literal["verdict"] = "verdict"


@dataclass(frozen=True)
class PullEvent:
    data: Pull
    type: Literal["pull"] = "pull"


@dataclass(frozen=True)
class AlertEvent:
    data: Alert
    type: Literal["alert"] = "alert"


@dataclass(frozen=True)
class UnknownEvent:
    """A frame type this SDK version doesn't know (forward compatibility)."""

    type: str
    data: Any
    raw: str = field(repr=False, default="")


@dataclass(frozen=True)
class Connected:
    url: str
    attempt: int
    type: Literal["connected"] = "connected"


@dataclass(frozen=True)
class Reconnecting:
    attempt: int
    delay_s: float
    reason: str
    type: Literal["reconnecting"] = "reconnecting"


StreamEvent = Union[VerdictEvent, PullEvent, AlertEvent, UnknownEvent, Connected, Reconnecting]


def parse_frame(raw: str) -> Union[VerdictEvent, PullEvent, AlertEvent, UnknownEvent, None]:
    """Parse one ``/v1/stream`` text frame. Returns None for malformed frames."""
    import json

    try:
        m = json.loads(raw)
    except ValueError:
        return None
    if not isinstance(m, dict) or not isinstance(m.get("type"), str):
        return None
    t, data = m["type"], m.get("data")
    try:
        if t == "verdict":
            return VerdictEvent(Verdict.from_dict(data))
        if t == "pull":
            return PullEvent(Pull.from_dict(data))
        if t == "alert":
            return AlertEvent(Alert.from_dict(data))
    except (KeyError, TypeError, ValueError):
        return None
    return UnknownEvent(type=t, data=data, raw=raw)
