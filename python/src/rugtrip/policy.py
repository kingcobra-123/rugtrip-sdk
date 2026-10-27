"""Pre-trade gate: your own rules applied to one pool's Verdict.

These are client-side preferences; they say nothing about how RugTrip rates a pool. Passing a policy does not make
a token low-risk: ratings are risk opinions, not guarantees.
"""

from __future__ import annotations

import dataclasses
from dataclasses import dataclass, field
from typing import Literal, Optional, Sequence

from .models import RATINGS, Reason, Verdict


@dataclass(frozen=True)
class Policy:
    #: Most risky rating you accept. Default "CAUTION" (skips TRAP and HIGH_RISK).
    max_rating: str = "CAUTION"
    #: Skip when any reason has level "bad", whatever the rating.
    skip_on_bad_reason: bool = True
    #: Accepted LP custody values; None or () = don't check.
    require_lp_custody: Optional[Sequence[str]] = ("burned", "locked")
    #: Sell test couldn't run (no_route / error) or not run yet. That is "couldn't check", not a trap.
    #: A failed test sale (blocked) always skips.
    treat_unknown_sim_as: Literal["skip", "allow"] = "skip"
    #: Max buy / sell tax from the sell test, %. None = don't check.
    max_buy_tax_pct: Optional[float] = 10.0
    max_sell_tax_pct: Optional[float] = 10.0
    #: Max share of supply the creator still holds, %. None = don't check.
    max_creator_pct: Optional[float] = 10.0
    #: Max share of supply bought by insiders in the launch block, %. None = don't check.
    max_bundle_pct: Optional[float] = 25.0
    #: Creator / bundle numbers not measured yet. Default "allow" (listed in ``unknowns``).
    treat_unknown_holdings_as: Literal["skip", "allow"] = "allow"
    #: Min pool liquidity in USD; None = don't check. When set, unknown liquidity skips.
    min_liq_usd: Optional[float] = None

    def replace(self, **changes: object) -> "Policy":
        return dataclasses.replace(self, **changes)  # type: ignore[arg-type]


DEFAULT_POLICY = Policy()


@dataclass(frozen=True)
class PolicyHit:
    #: which policy field caused the skip (or "rating_unrecognized" / "sell_blocked")
    rule: str
    text: str


@dataclass(frozen=True)
class GateResult:
    skip: bool
    #: "SKIP" or "BUY_CANDIDATE"
    decision: str
    hits: list[PolicyHit] = field(default_factory=list)
    #: facts that could not be checked (allowed by the policy, or the cause of a skip)
    unknowns: list[str] = field(default_factory=list)
    #: the verdict's own plain-word reasons, most serious first
    reasons: list[Reason] = field(default_factory=list)


def risk_rank(rating: str) -> int:
    """0 = TRAP ... 3 = LOWER_RISK; -1 for a rating this SDK version doesn't know."""
    try:
        return RATINGS.index(rating)
    except ValueError:
        return -1


_LEVEL_ORDER = {"bad": 0, "warn": 1, "unknown": 2, "ok": 3}


def _fmt(x: float) -> str:
    return str(int(x)) if float(x).is_integer() else f"{x:.1f}"


def gate(v: Verdict, policy: Optional[Policy] = None, **overrides: object) -> GateResult:
    """Apply a policy (default: DEFAULT_POLICY, with keyword overrides) to one pool's verdict."""
    p = (policy or DEFAULT_POLICY).replace(**overrides) if overrides else (policy or DEFAULT_POLICY)
    hits: list[PolicyHit] = []
    unknowns: list[str] = []
    d = v.details

    rank = risk_rank(v.rating)
    if rank < 0:
        hits.append(PolicyHit("rating_unrecognized", f"Unrecognized rating {v.rating}"))
    elif rank < risk_rank(p.max_rating):
        hits.append(PolicyHit("max_rating", f"Rated {v.rating} (policy accepts up to {p.max_rating})"))

    if p.skip_on_bad_reason:
        hits.extend(PolicyHit("skip_on_bad_reason", r.text) for r in v.reasons if r.level == "bad")

    if p.require_lp_custody:
        custody = d.lp.custody if d and d.lp else None
        if custody is None:
            unknowns.append("LP custody not measured yet")
            hits.append(PolicyHit("require_lp_custody", "LP custody not measured yet"))
        elif custody not in p.require_lp_custody:
            hits.append(PolicyHit("require_lp_custody", f"LP custody is {custody} (policy requires {' or '.join(p.require_lp_custody)})"))

    def sim_unknown(why: str) -> None:
        unknowns.append(why)
        if p.treat_unknown_sim_as == "skip":
            hits.append(PolicyHit("treat_unknown_sim_as", why))

    sim = d.sim if d else None
    if sim is None:
        sim_unknown("Sell test not run yet")
    elif sim.status == "blocked":
        hits.append(PolicyHit("sell_blocked", "A test sale failed"))
    elif sim.status == "no_route":
        sim_unknown("Sell test couldn't run (no route): sellability not checked")
    elif sim.status == "error":
        sim_unknown("Sell test couldn't run (error): sellability not checked")
    elif sim.status != "ok":
        sim_unknown(f"Sell test status {sim.status}: sellability not checked")
    else:
        if p.max_buy_tax_pct is not None:
            if sim.buy_tax_pct is None:
                sim_unknown("Buy tax not measured")
            elif sim.buy_tax_pct > p.max_buy_tax_pct:
                hits.append(PolicyHit("max_buy_tax_pct", f"Buy tax {_fmt(sim.buy_tax_pct)}% (policy max {_fmt(p.max_buy_tax_pct)}%)"))
        if p.max_sell_tax_pct is not None:
            if sim.sell_tax_pct is None:
                sim_unknown("Sell tax not measured")
            elif sim.sell_tax_pct > p.max_sell_tax_pct:
                hits.append(PolicyHit("max_sell_tax_pct", f"Sell tax {_fmt(sim.sell_tax_pct)}% (policy max {_fmt(p.max_sell_tax_pct)}%)"))

    def holding(val: Optional[float], mx: Optional[float], rule: str, label: str) -> None:
        if mx is None:
            return
        if val is None:
            unknowns.append(f"{label} not measured yet")
            if p.treat_unknown_holdings_as == "skip":
                hits.append(PolicyHit("treat_unknown_holdings_as", f"{label} not measured yet"))
        elif val > mx:
            hits.append(PolicyHit(rule, f"{label} {_fmt(val)}% (policy max {_fmt(mx)}%)"))

    holding(d.creator_pct if d else None, p.max_creator_pct, "max_creator_pct", "Creator holds")
    holding(d.bundle_pct if d else None, p.max_bundle_pct, "max_bundle_pct", "Launch-block insider buys")

    if p.min_liq_usd is not None:
        if v.liq_usd is None:
            hits.append(PolicyHit("min_liq_usd", "Liquidity unknown"))
        elif v.liq_usd < p.min_liq_usd:
            hits.append(PolicyHit("min_liq_usd", f"Liquidity ${_fmt(v.liq_usd)} (policy min ${_fmt(p.min_liq_usd)})"))

    reasons = sorted(v.reasons, key=lambda r: _LEVEL_ORDER.get(r.level, 2))
    skip = bool(hits)
    return GateResult(skip=skip, decision="SKIP" if skip else "BUY_CANDIDATE", hits=hits, unknowns=unknowns, reasons=reasons)


def should_skip(v: Verdict, policy: Optional[Policy] = None, **overrides: object) -> bool:
    """True when the policy says skip this pool."""
    return gate(v, policy, **overrides).skip
