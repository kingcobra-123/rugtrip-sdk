from conftest import load, load_text

from rugtrip import RATINGS, Health, PullEvent, RugsToday, Scorecard, Stats, TokenCheck, UnknownEvent, Verdict, VerdictEvent, parse_frame


def test_token_fixture():
    t = TokenCheck.from_dict(load("token.json"))
    assert t.chain_id == 4663
    assert t.best == t.pools[0]
    assert all(p.rating in RATINGS for p in t.pools)
    assert t.best.details is not None and t.best.details.lp is not None


def test_other_fixtures():
    v = Verdict.from_dict(load("pool.json"))
    assert v.venue == "v4" and len(v.pool) == 66
    feed = [Verdict.from_dict(p) for p in load("feed_recent.json")["pools"]]
    sims = {p.details.sim.status for p in feed if p.details and p.details.sim}
    assert {"ok", "blocked", "no_route", "error"} <= sims
    r = RugsToday.from_dict(load("rugs_today.json"))
    assert r.pulls and r.pulls[0].life_s >= 0
    sc = Scorecard.from_dict(load("scorecard.json"))
    assert sc.days and "TRAP" in sc.totals
    assert isinstance(Stats.from_dict(load("stats.json")).pools_24h, int)
    assert isinstance(Health.from_dict(load("healthz.json")).ok, bool)


def test_extra_fields_are_ignored():
    d = load("pool.json")
    d["some_future_field"] = 1
    d["details"]["sim"]["future"] = "x"
    assert Verdict.from_dict(d).pool == d["pool"]


def test_stream_frames():
    evs = [parse_frame(line) for line in load_text("stream.jsonl").splitlines() if line.strip()]
    assert any(isinstance(e, VerdictEvent) for e in evs)
    assert any(isinstance(e, PullEvent) for e in evs)
    unknown = [e for e in evs if isinstance(e, UnknownEvent)]
    assert [u.type for u in unknown] == ["hello_from_the_future"]
    assert parse_frame("garbage") is None
    assert parse_frame('{"data": 1}') is None
    assert parse_frame('{"type": "verdict", "data": {}}') is None
