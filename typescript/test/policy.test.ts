import { test } from "node:test";
import assert from "node:assert/strict";
import { DEFAULT_POLICY, gate, riskRank, shouldSkip, type FeedResponse, type TokenCheck, type Verdict } from "../src/index.js";
import { fixture } from "./helpers.js";

const feed = fixture<FeedResponse>("feed_recent.json").pools;
const lower = fixture<TokenCheck>("token_lower_risk.json").best;
const clone = (v: Verdict): Verdict => JSON.parse(JSON.stringify(v)) as Verdict;

test("risk rank orders TRAP first", () => {
  assert.deepEqual(["TRAP", "HIGH_RISK", "CAUTION", "LOWER_RISK"].map(riskRank), [0, 1, 2, 3]);
  assert.equal(riskRank("SOMETHING_NEW"), -1);
});

test("defaults are conservative", () => {
  assert.equal(DEFAULT_POLICY.maxRating, "CAUTION");
  assert.deepEqual(DEFAULT_POLICY.requireLpCustody, ["burned", "locked"]);
  assert.equal(DEFAULT_POLICY.treatUnknownSimAs, "skip");
});

test("lower-risk pool with locked LP and an ok sell test is a candidate", () => {
  const g = gate(lower);
  assert.equal(g.decision, "BUY_CANDIDATE", JSON.stringify(g.hits));
  assert.equal(g.skip, false);
  assert.deepEqual(g.hits, []);
});

test("TRAP / HIGH_RISK always skip under defaults", () => {
  for (const v of feed.filter((p) => p.rating === "TRAP" || p.rating === "HIGH_RISK")) {
    const g = gate(v);
    assert.equal(g.skip, true);
    assert.ok(g.hits.some((h) => h.rule === "maxRating"));
  }
});

test("blocked sell test skips even when policy allows unknown sims", () => {
  const v = feed.find((p) => p.details?.sim?.status === "blocked")!;
  const g = gate(v, { maxRating: "TRAP", treatUnknownSimAs: "allow", skipOnBadReason: false, requireLpCustody: null });
  assert.ok(g.hits.some((h) => h.rule === "sell_blocked"));
});

test("no_route / error mean couldn't check, not trap", () => {
  for (const status of ["no_route", "error"] as const) {
    const v = clone(lower);
    v.details!.sim = { status, buy_tax_pct: null, sell_tax_pct: null };
    const strict = gate(v);
    assert.equal(strict.skip, true);
    assert.ok(strict.hits.every((h) => h.rule === "treatUnknownSimAs"), JSON.stringify(strict.hits));
    assert.ok(strict.unknowns.some((u) => u.includes("couldn't run")));
    const lenient = gate(v, { treatUnknownSimAs: "allow" });
    assert.equal(lenient.skip, false);
    assert.ok(lenient.unknowns.length > 0);
  }
});

test("pending sim (null) counts as unknown", () => {
  const v = clone(lower);
  v.details!.sim = null;
  assert.equal(shouldSkip(v), true);
  assert.equal(shouldSkip(v, { treatUnknownSimAs: "allow" }), false);
});

test("LP custody rule", () => {
  const v = clone(lower);
  v.details!.lp!.custody = "one_wallet";
  assert.ok(gate(v).hits.some((h) => h.rule === "requireLpCustody"));
  assert.equal(shouldSkip(v, { requireLpCustody: null }), false);
  v.details!.lp = null;
  assert.equal(shouldSkip(v), true);
  v.details = null;
  assert.equal(shouldSkip(v, { requireLpCustody: [], treatUnknownSimAs: "allow" }), false);
});

test("taxes, creator and bundle limits", () => {
  const v = clone(lower);
  v.details!.sim!.sell_tax_pct = 12;
  assert.ok(gate(v).hits.some((h) => h.rule === "maxSellTaxPct"));
  assert.equal(shouldSkip(v, { maxSellTaxPct: null }), false);
  const c = clone(lower);
  c.details!.creator_pct = 30;
  c.details!.bundle_pct = 40;
  const rules = gate(c).hits.map((h) => h.rule);
  assert.ok(rules.includes("maxCreatorPct") && rules.includes("maxBundlePct"));
  const u = clone(lower);
  u.details!.creator_pct = null;
  assert.equal(shouldSkip(u), false);
  assert.ok(gate(u).unknowns.length === 1);
  assert.equal(shouldSkip(u, { treatUnknownHoldingsAs: "skip" }), true);
});

test("bad reasons skip; min liquidity; unknown rating skips", () => {
  const v = clone(lower);
  v.reasons.push({ level: "bad", check: "code", text: "Something bad" });
  assert.ok(gate(v).hits.some((h) => h.text === "Something bad"));
  assert.equal(shouldSkip(lower, { minLiqUsd: 1000 }), true);
  const n = clone(lower);
  n.liq_usd = null;
  assert.equal(shouldSkip(n, { minLiqUsd: 1 }), true);
  const r = clone(lower);
  (r as { rating: string }).rating = "NEW_RATING";
  assert.ok(gate(r).hits.some((h) => h.rule === "rating_unrecognized"));
});

test("reasons come back most serious first", () => {
  const v = feed.find((p) => p.reasons.some((r) => r.level === "bad") && p.reasons.some((r) => r.level !== "bad"))!;
  const g = gate(v);
  assert.equal(g.reasons[0].level, "bad");
});
