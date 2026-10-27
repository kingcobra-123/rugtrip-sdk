import { test } from "node:test";
import assert from "node:assert/strict";
import { RugTripClient, RugTripError, backoffMs, parseRetryAfter, resolveBaseUrl, toTarget, type TokenCheck } from "../src/index.js";
import { fixture, jsonResponse } from "./helpers.js";

const BASE = "http://127.0.0.1:8742";
const POOL_ID = "0x3e767ad60e19334d08952bed1b1a88c722d55eddfeaaab50b695330c260a4322";

type Call = { url: string; headers: Record<string, string> };
function fakeFetch(responses: Array<Response | Error>, calls: Call[] = []): typeof fetch {
  return (async (input: string | URL | Request, init?: RequestInit) => {
    calls.push({ url: String(input), headers: (init?.headers ?? {}) as Record<string, string> });
    const r = responses.shift();
    if (!r) throw new Error("no more responses");
    if (r instanceof Error) throw r;
    return r;
  }) as typeof fetch;
}

test("baseUrl is required, no default", () => {
  const saved = process.env.RUGTRIP_API_URL;
  delete process.env.RUGTRIP_API_URL;
  try {
    assert.throws(() => new RugTripClient(), (e: unknown) => e instanceof RugTripError && e.code === "config");
    assert.throws(() => resolveBaseUrl("ftp://x"), (e: unknown) => e instanceof RugTripError && e.code === "config");
    process.env.RUGTRIP_API_URL = BASE + "/";
    assert.equal(new RugTripClient({ fetch: fakeFetch([]) }).baseUrl, BASE);
  } finally {
    if (saved === undefined) delete process.env.RUGTRIP_API_URL;
    else process.env.RUGTRIP_API_URL = saved;
  }
});

test("check() hits /v1/token with user agent and returns the body", async () => {
  const calls: Call[] = [];
  const body = fixture<TokenCheck>("token.json");
  const c = new RugTripClient({ baseUrl: BASE, fetch: fakeFetch([jsonResponse(200, body)], calls) });
  const t = await c.check(body.token);
  assert.equal(t.best.pool, body.best.pool);
  assert.equal(calls[0].url, `${BASE}/v1/token/${body.token}`);
  assert.match(calls[0].headers["User-Agent"], /^rugtrip-sdk-ts\/\d+\.\d+\.\d+$/);
});

test("chart links are reduced to the address", () => {
  assert.equal(toTarget(`https://dexscreener.com/robinhood/${POOL_ID}?maker=1`), POOL_ID);
  assert.equal(toTarget(`  ${POOL_ID} `), POOL_ID);
  assert.equal(toTarget("hello world"), "hello%20world");
});

test("feedRecent builds the query string", async () => {
  const calls: Call[] = [];
  const c = new RugTripClient({ baseUrl: BASE, fetch: fakeFetch([jsonResponse(200, { pools: [] })], calls) });
  await c.feedRecent({ rating: "CAUTION", venue: "v4", limit: 10, after: 5, includeLate: true });
  assert.equal(calls[0].url, `${BASE}/v1/feed/recent?rating=CAUTION&venue=v4&limit=10&after=5&include_late=1`);
});

test("API error codes map to RugTripError", async () => {
  const c = new RugTripClient({ baseUrl: BASE, maxRetries: 0, fetch: fakeFetch([jsonResponse(400, fixture("error_bad_address.json")), jsonResponse(404, fixture("error_unknown_token.json")), jsonResponse(500, "oops")]) });
  await assert.rejects(c.check("0x1234"), (e: unknown) => e instanceof RugTripError && e.code === "bad_address" && e.status === 400);
  await assert.rejects(c.pool(POOL_ID), (e: unknown) => e instanceof RugTripError && e.code === "unknown_token" && e.message.includes("haven't seen"));
  await assert.rejects(c.stats(), (e: unknown) => e instanceof RugTripError && e.code === "http_error" && e.status === 500);
});

test("rate_limited is retried with Retry-After, then succeeds", async () => {
  const calls: Call[] = [];
  const c = new RugTripClient({ baseUrl: BASE, maxRetries: 2, retryBaseMs: 1, fetch: fakeFetch([jsonResponse(429, fixture("error_rate_limited.json"), { "retry-after": "0" }), jsonResponse(429, fixture("error_rate_limited.json")), jsonResponse(200, fixture("stats.json"))], calls) });
  const s = await c.stats();
  assert.equal(typeof s.pools_24h, "number");
  assert.equal(calls.length, 3);
});

test("rate_limited surfaces after retries are used up", async () => {
  const c = new RugTripClient({ baseUrl: BASE, maxRetries: 1, retryBaseMs: 1, fetch: fakeFetch([jsonResponse(429, fixture("error_rate_limited.json")), jsonResponse(429, fixture("error_rate_limited.json"), { "retry-after": "7" })]) });
  await assert.rejects(c.stats(), (e: unknown) => e instanceof RugTripError && e.code === "rate_limited" && e.retryable && e.retryAfterMs === 7000);
});

test("network errors retry; timeouts map to timeout", async () => {
  const c = new RugTripClient({ baseUrl: BASE, maxRetries: 1, retryBaseMs: 1, fetch: fakeFetch([new TypeError("fetch failed"), jsonResponse(200, fixture("healthz.json"))]) });
  assert.equal(typeof (await c.health()).ok, "boolean");
  const slow = ((_u: unknown, init?: RequestInit) =>
    new Promise((_, rej) => init?.signal?.addEventListener("abort", () => rej(new Error("aborted"))))) as typeof fetch;
  const t = new RugTripClient({ baseUrl: BASE, timeoutMs: 20, maxRetries: 0, fetch: slow });
  await assert.rejects(t.health(), (e: unknown) => e instanceof RugTripError && e.code === "timeout");
});

test("backoff is capped and jittered; Retry-After parses", () => {
  assert.equal(backoffMs(0, 1000, 30000, () => 0), 500);
  assert.equal(backoffMs(0, 1000, 30000, () => 1), 1000);
  assert.equal(backoffMs(20, 1000, 30000, () => 1), 30000);
  assert.equal(parseRetryAfter("3"), 3000);
  assert.equal(parseRetryAfter(new Date(10_000).toUTCString(), 0), 10_000);
  assert.equal(parseRetryAfter("soon"), undefined);
});
