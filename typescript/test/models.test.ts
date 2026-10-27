import { test } from "node:test";
import assert from "node:assert/strict";
import { RATINGS, parseFrame, type FeedResponse, type Health, type RugsToday, type Scorecard, type Stats, type TokenCheck, type Verdict } from "../src/index.js";
import { fixture, fixtureText } from "./helpers.js";

test("token fixture has best + pools with details", () => {
  const t = fixture<TokenCheck>("token.json");
  assert.equal(t.chain_id, 4663);
  assert.ok(t.pools.length >= 1);
  assert.deepEqual(t.best, t.pools[0]);
  for (const p of t.pools) {
    assert.ok(RATINGS.includes(p.rating));
    assert.ok(Array.isArray(p.reasons));
    assert.equal(p.token, t.token);
  }
});

test("pool / feed / rugs / scorecard / stats / health fixtures parse", () => {
  const v = fixture<Verdict>("pool.json");
  assert.equal(v.venue, "v4");
  assert.equal(v.pool.length, 66);
  const f = fixture<FeedResponse>("feed_recent.json");
  assert.ok(f.pools.length > 5);
  const sims = new Set(f.pools.map((p) => p.details?.sim?.status));
  for (const s of ["ok", "blocked", "no_route", "error"]) assert.ok(sims.has(s as never), `fixture covers sim ${s}`);
  const r = fixture<RugsToday>("rugs_today.json");
  assert.ok(r.pulls.length > 0 && typeof r.since === "number");
  const sc = fixture<Scorecard>("scorecard.json");
  assert.ok(sc.days.length > 0);
  assert.ok(sc.totals.TRAP);
  const st = fixture<Stats>("stats.json");
  assert.equal(typeof st.pools_24h, "number");
  const h = fixture<Health>("healthz.json");
  assert.equal(typeof h.ok, "boolean");
});

test("stream frames parse, unknown types pass through", () => {
  const lines = fixtureText("stream.jsonl").split("\n").filter(Boolean);
  const types = lines.map((l) => parseFrame(l)?.type);
  assert.ok(types.includes("verdict"));
  assert.ok(types.includes("pull"));
  assert.ok(types.includes("hello_from_the_future"));
  assert.equal(parseFrame("not json"), undefined);
  assert.equal(parseFrame('{"data":1}'), undefined);
});
