import copy

from conftest import load

from rugtrip import DEFAULT_POLICY, Policy, TokenCheck, Verdict, gate, risk_rank, should_skip
from rugtrip.models import Reason, SimDetails

FEED = [Verdict.from_dict(p) for p in load("feed_recent.json")["pools"]]
LOWER_RAW = load("token_lower_risk.json")["best"]


def lower(**mut) -> Verdict:
    d = copy.deepcopy(LOWER_RAW)
    for path, val in mut.items():
        obj = d
        keys = path.split("__")
        for k in keys[:-1]:
            obj = obj[k]
        obj[keys[-1]] = val
    return Verdict.from_dict(d)


def test_rank_and_defaults():
    assert [risk_rank(r) for r in ("TRAP", "HIGH_RISK", "CAUTION", "LOWER_RISK")] == [0, 1, 2, 3]
    assert risk_rank("NEW") == -1
    assert DEFAULT_POLICY.max_rating == "CAUTION"
    assert tuple(DEFAULT_POLICY.require_lp_custody) == ("burned", "locked")
    assert DEFAULT_POLICY.treat_unknown_sim_as == "skip"


def test_candidate():
    g = gate(lower())
    assert g.decision == "BUY_CANDIDATE", g.hits
    assert not g.skip and g.hits == []


def test_trap_and_high_risk_skip():
    for v in FEED:
        if v.rating in ("TRAP", "HIGH_RISK"):
            g = gate(v)
            assert g.skip and any(h.rule == "max_rating" for h in g.hits)


def test_blocked_always_skips():
    v = next(p for p in FEED if p.details and p.details.sim and p.details.sim.status == "blocked")
    g = gate(v, max_rating="TRAP", treat_unknown_sim_as="allow", skip_on_bad_reason=False, require_lp_custody=None)
    assert any(h.rule == "sell_blocked" for h in g.hits)


def test_no_route_and_error_are_unknown_not_trap():
    for status in ("no_route", "error"):
        v = lower(details__sim={"status": status, "buy_tax_pct": None, "sell_tax_pct": None})
        strict = gate(v)
        assert strict.skip and all(h.rule == "treat_unknown_sim_as" for h in strict.hits), strict.hits
        assert any("couldn't run" in u for u in strict.unknowns)
        lenient = gate(v, Policy(treat_unknown_sim_as="allow"))
        assert not lenient.skip and lenient.unknowns


def test_pending_sim_and_lp():
    assert should_skip(lower(details__sim=None))
    assert not should_skip(lower(details__sim=None), treat_unknown_sim_as="allow")
    assert should_skip(lower(details__lp=None))
    assert any(h.rule == "require_lp_custody" for h in gate(lower(details__lp__custody="one_wallet")).hits)
    assert not should_skip(lower(details__lp__custody="one_wallet"), require_lp_custody=None)


def test_limits():
    assert any(h.rule == "max_sell_tax_pct" for h in gate(lower(details__sim__sell_tax_pct=12)).hits)
    rules = {h.rule for h in gate(lower(details__creator_pct=30, details__bundle_pct=40)).hits}
    assert {"max_creator_pct", "max_bundle_pct"} <= rules
    u = lower(details__creator_pct=None)
    assert not should_skip(u) and len(gate(u).unknowns) == 1
    assert should_skip(u, treat_unknown_holdings_as="skip")
    assert should_skip(lower(), min_liq_usd=1000)
    assert should_skip(lower(liq_usd=None), min_liq_usd=1)
    assert any(h.rule == "rating_unrecognized" for h in gate(lower(rating="NEW_RATING")).hits)


def test_bad_reason_and_order():
    d = copy.deepcopy(LOWER_RAW)
    d["reasons"].append({"level": "bad", "check": "code", "text": "Something bad"})
    g = gate(Verdict.from_dict(d))
    assert any(h.text == "Something bad" for h in g.hits)
    assert g.reasons[0].level == "bad"
    assert TokenCheck.from_dict(load("token_lower_risk.json")).best.rating == "LOWER_RISK"
