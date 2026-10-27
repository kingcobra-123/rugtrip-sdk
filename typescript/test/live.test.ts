// Opt-in: runs only when RUGTRIP_API_URL is set. Keeps traffic light (a handful of GETs, one short stream).
import { test } from "node:test";
import assert from "node:assert/strict";
import { RATINGS, RugTripClient, RugTripError } from "../src/index.js";

const url = process.env.RUGTRIP_API_URL;

test("live API", { skip: url ? false : "set RUGTRIP_API_URL to run" }, async () => {
  const c = new RugTripClient({ baseUrl: url });
  const h = await c.health();
  assert.equal(typeof h.ok, "boolean");
  const feed = await c.feedRecent({ limit: 3 });
  assert.ok(Array.isArray(feed.pools));
  if (feed.pools.length > 0) {
    const t = await c.check(feed.pools[0].token);
    assert.ok(RATINGS.includes(t.best.rating));
    const v = await c.pool(feed.pools[0].pool);
    assert.equal(v.pool, feed.pools[0].pool);
  }
  await assert.rejects(c.check("0x1234"), (e: unknown) => e instanceof RugTripError && e.code === "bad_address");
  await assert.rejects(c.check("0x0000000000000000000000000000000000000001"), (e: unknown) => e instanceof RugTripError && e.code === "unknown_token");
  const st = await c.stats();
  assert.equal(typeof st.pools_24h, "number");
});

test("live stream opens", { skip: url ? false : "set RUGTRIP_API_URL to run" }, async () => {
  const c = new RugTripClient({ baseUrl: url });
  const s = c.stream({ maxReconnects: 0 });
  await new Promise<void>((resolve, reject) => {
    const t = setTimeout(() => reject(new Error("no open within 10 s")), 10_000);
    s.on("open", () => {
      clearTimeout(t);
      resolve();
    });
    s.start();
  });
  s.close();
});
