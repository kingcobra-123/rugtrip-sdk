# RugTrip public API (v1)

The public, unauthenticated part of the RugTrip API. The SDKs in this repository wrap exactly these endpoints.

- **Base URL:** you configure it. There is no default endpoint. Examples use a local instance at
  `http://127.0.0.1:8742`.
- **Format:** JSON, `snake_case`. Timestamps are unix seconds (float). Addresses are lower-case hex.
- **Chain:** Robinhood Chain (chain id `4663`).
- Responses may gain new fields at any time; clients should ignore fields they don't know.

Ratings are risk opinions, not guarantees. Never treat `LOWER_RISK` as safe.

## Types

```
Rating  = "TRAP" | "HIGH_RISK" | "CAUTION" | "LOWER_RISK"          (most risky first)
Level   = "bad" | "warn" | "ok" | "unknown"
Check   = "sell_sim" | "lp" | "code" | "bundle" | "creator" | "wallets" | "template"
Venue   = "v2" | "v3" | "v4"

Reason  = {"level": Level, "check": Check, "text": "plain words"}

Verdict = {
  "pool": "0x…",            20-byte pool address (v2/v3) or 32-byte pool id (v4)
  "token": "0x…",
  "symbol": "…",
  "venue": Venue,
  "pool_age_s": 123,
  "liq_usd": 12345.6 | null,
  "rating": Rating,
  "reasons": [Reason],
  "issued_at": ts,          when the pool's first verdict was issued
  "updated_at": ts,         when the version shown was issued
  "version": 1,             1 = first verdict; higher = re-rated
  "details": Details | null
}

Details = {                 every field is null when unknown
  "lp":  {"custody": "burned|locked|one_wallet|contract|unknown",
          "held_pct": x,     share of LP held by the largest single wallet, %
          "locked_pct": x,   share of LP burned or in a lock, %
          "lock_until": ts|null} | null       (null = not measured yet)
  "sim": {"status": "ok|blocked|no_route|error",
          "buy_tax_pct": x, "sell_tax_pct": x} | null   (null = not run yet)
  "bundle_pct": x,          share of supply bought by insiders in the launch block, %
  "creator_pct": x,         share of supply the creator still holds, %
  "deployer": "0x…", "lp_holder": "0x…", "creator": "0x…"    on-chain public facts
}

Pull    = {"pool","token","symbol","venue","launched_at": ts,"pulled_at": ts,"life_s": 36,"liq_removed_usd": x|null}
```

### Reading `details`

- `lp.custody`: `burned` (most LP is burned), `locked` (most LP sits in a lock), `one_wallet` (a single wallet holds
  most LP and can remove it), `contract` (a contract that is not a recognised lock holds most LP), `unknown` (not
  measured yet, no liquidity yet, or no dominant holder).
- `sim.status`: `ok` (a test buy and sale went through; taxes are filled in), `blocked` (a test sale failed),
  `no_route` / `error` (the test **couldn't run**). `no_route` and `error` mean "couldn't check"; they are not
  evidence of a trap and should not be treated as one.

## Endpoints

### `GET /healthz`
`{"ok": true, "head_block": n | null, "lag_s": x | null}`. `ok` is false when the verdict engine has not reported in recently.

### `GET /v1/token/:x`
`:x` is a token address, a pool address, a v4 pool id, or a DexScreener / GeckoTerminal / explorer link (the last
`0x` hex run of 40 or 64 digits in the link's path is used). Mixed-case addresses must have a valid EIP-55 checksum.

```
{"token": "0x…", "symbol": "…", "chain_id": 4663, "best": Verdict, "pools": [Verdict]}
```
`pools` lists the token's pools, most liquid first (up to 50); `best` is the first of them (the most liquid pool,
not the least risky one). Gate on the pool you will actually trade through.

### `GET /v1/pool/:addr`
One pool's `Verdict`. Accepts a 20-byte pool address or a 32-byte v4 pool id (or a link carrying one).

### `GET /v1/feed/recent`
`{"pools": [Verdict]}`, newest first. Unauthenticated callers see the feed 60 s late.

| query | meaning |
|---|---|
| `rating` | one of the ratings (case-insensitive) |
| `venue` | `v2`, `v3` or `v4` |
| `limit` | 1..200, default 50 (larger values are capped silently) |
| `before` / `after` | unix time, compared with `issued_at` |
| `include_late=1` | also return verdicts that were first issued more than 1 h after the pool was created (default: only pools rated at launch) |

### `GET /v1/rugs/today`
`{"since": ts, "pulls": [Pull]}`: liquidity pulls in the rolling last 24 h, newest first.

| query | meaning |
|---|---|
| `since` | unix time, at most 7 days back |
| `before` | page back: pass the oldest `pulled_at` you received (exclusive) |
| `limit` | 1..5000, default 5000 |

### `GET /v1/scorecard`
How many pools of each rating were later pulled, per day (last 30 days).
```
{"days": [{"day": "YYYY-MM-DD", "rating": Rating, "pools": n, "pulled_1h": n, "pulled_6h": n, "pulled_24h": n}],
 "totals": {"<RATING>": {"pools": n, "pulled_1h": n, "pulled_6h": n, "pulled_24h": n}}}
```

### `GET /v1/stats`
```
{"pools_24h": n, "pulls_24h": n, "median_life_s": x|null,
 "e2e_p50_ms": x|null,      first sight of a pool -> first verdict, ms, p50
 "engine_p50_us": x|null,   verdict compute time, microseconds, p50
 "decision_p50_ms": x|null} alias of e2e_p50_ms kept for v1 clients
```

### `GET /v1/stream` (WebSocket)
Text frames, one JSON object each:
```
{"type": "verdict", "data": Verdict}
{"type": "pull",    "data": Pull}
```
- Unauthenticated connections receive `verdict` and `pull` frames **60 s after they happen**. Real-time delivery
  (and `alert` frames for watched tokens) comes with API plans.
- `verdict` frames are sent for every live verdict version: first verdicts (`version` 1) and re-ratings.
- Delivery is at-least-once: the same pull can arrive more than once; de-duplicate by `pool`.
- The server sends no heartbeat frames; it answers WebSocket pings. There is no replay: after a reconnect, backfill
  with `GET /v1/feed/recent?after=<last issued_at seen>` if you need the gap.
- Clients should ignore frame types they don't know.

## Errors

Errors are `{"error": "<code>", "message": "<plain words>"}` with a matching HTTP status.

| code | status | meaning |
|---|---|---|
| `bad_address` | 400 | not a token / pool address, v4 pool id or supported link (or a bad EIP-55 checksum) |
| `unknown_token` | 404 | no pool seen for this token or pool yet |
| `bad_request` | 400 | invalid query parameter (e.g. an unknown `rating`) |
| `rate_limited` | 429 | too many requests; back off and retry |
| `not_found` | 404 | no such resource |
| `unauthorized` / `pro_required` | 401 / 403 | reserved for plan-gated features |
| `internal` | 500 | server-side problem; retry later |

## Rate limits

Per client IP, token buckets (current values, may change):

| route class | burst | sustained |
|---|---|---|
| checks: `/v1/token`, `/v1/pool` | 30 | 1 per second |
| reads: `/v1/feed/recent`, `/v1/rugs/today`, `/v1/scorecard`, `/v1/stats` | 120 | 4 per second |

`429 rate_limited` responses currently carry no `Retry-After` header; the SDKs back off exponentially with jitter
(and honour `Retry-After` when present). Prefer the stream over polling the feed.

---

RugTrip is an independent project and is not affiliated with Robinhood.
