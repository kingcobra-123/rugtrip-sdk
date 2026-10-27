# RugTrip SDK

RugTrip is a rug-risk radar for Robinhood Chain (chain id 4663). It watches new liquidity pools as they launch,
runs a set of on-chain checks (a test buy and sale, who controls the liquidity, launch-block insider buys, creator
holdings, the creator's history) and publishes a plain-words verdict for each pool, plus a live record of liquidity
pulls. This repository holds the open clients for the public RugTrip API in TypeScript, Python and Rust, so trading
bots, sniper bots, copy-trade bots and AI agents can check a token before buying and react to the live verdict and
pull stream.

> **Ratings are risk opinions, not guarantees. Never treat `LOWER_RISK` as safe.** A pool can pass every check and
> still go to zero. Use RugTrip as one input to your own decision.

Method: [how ratings are made and how they hold up](docs/METHOD.md).

| | package | source |
|---|---|---|
| TypeScript / JavaScript | `@rugtrip/sdk` | [`typescript/`](typescript) |
| Python | `rugtrip` | [`python/`](python) |
| Rust | `rugtrip` | [`rust/`](rust) |

Full contract for the public endpoints: [`docs/API.md`](docs/API.md).

## Install

```sh
npm install @rugtrip/sdk          # Node >= 18 (stream: Node >= 22, or add the optional `ws` package)
pip install rugtrip               # Python >= 3.10 (httpx + websockets)
cargo add rugtrip                 # Rust, tokio
```

Until the packages are published, install from a clone: `npm install ./typescript`, `pip install ./python`, or
`rugtrip = { path = "rust" }` in your `Cargo.toml`.

## Configure the base URL

The base URL is **required**. Pass it to the client or set `RUGTRIP_API_URL`. The SDKs have no default endpoint on
purpose: none is hard-coded, so your traffic only goes where you point it. The examples use a local instance:

```sh
export RUGTRIP_API_URL=http://127.0.0.1:8742
```

## Quickstart

### TypeScript

```ts
import { RugTripClient, gate } from "@rugtrip/sdk";

const rt = new RugTripClient({ baseUrl: process.env.RUGTRIP_API_URL });
const t = await rt.check("0x…");            // token, pool, v4 pool id or DexScreener / GeckoTerminal link
const g = gate(t.best);                      // conservative default policy
console.log(t.symbol, t.best.rating, g.decision);
for (const h of g.hits) console.log(" -", h.text);
for (const r of t.best.reasons) console.log(` [${r.level}] ${r.text}`);
```

### Python

```python
from rugtrip import Client, gate

with Client() as rt:                         # base URL from RUGTRIP_API_URL (or Client("http://…"))
    t = rt.check("0x…")                      # token, pool, v4 pool id or chart link
    g = gate(t.best)                         # conservative default policy
    print(t.symbol, t.best.rating, g.decision)
    for h in g.hits:
        print(" -", h.text)
```

### Rust

```rust
use rugtrip::{gate, Client, Policy};

#[tokio::main]
async fn main() -> rugtrip::Result<()> {
    let rt = Client::from_env()?;            // or Client::new("http://…")
    let t = rt.check("0x…").await?;
    let g = gate(&t.best, &Policy::default());
    println!("{} {} {}", t.symbol, t.best.rating, g.decision.as_str());
    Ok(())
}
```

`check()` returns `best` (the token's **most liquid** pool) and `pools` (every known pool, most liquid first).
Gate on the pool you will actually trade through: another pool of the same token can carry a different rating.

## Policy helper (pre-trade gate)

`gate(verdict, policy)` applies **your** rules to one pool's verdict and returns `SKIP` or `BUY_CANDIDATE`, the rules
that failed (`hits`), what could not be checked (`unknowns`), and the verdict's own plain-word reasons, most serious
first. `shouldSkip` / `should_skip` return just the boolean. The defaults are conservative:

| TypeScript | Python / Rust | default | meaning |
|---|---|---|---|
| `maxRating` | `max_rating` | `CAUTION` | most risky rating accepted (TRAP and HIGH_RISK skip) |
| `skipOnBadReason` | `skip_on_bad_reason` | `true` | any reason with level `bad` skips |
| `requireLpCustody` | `require_lp_custody` | `burned`, `locked` | accepted LP custody; empty = don't check |
| `treatUnknownSimAs` | `treat_unknown_sim_as` | `skip` | sell test couldn't run (`no_route` / `error`) or not run yet |
| `maxBuyTaxPct` / `maxSellTaxPct` | `max_buy_tax_pct` / `max_sell_tax_pct` | 10 / 10 | max taxes from the sell test |
| `maxCreatorPct` | `max_creator_pct` | 10 | max % of supply the creator still holds |
| `maxBundlePct` | `max_bundle_pct` | 25 | max % of supply bought by insiders in the launch block |
| `treatUnknownHoldingsAs` | `treat_unknown_holdings_as` | `allow` | creator / bundle numbers not measured yet (listed in `unknowns`) |
| `minLiqUsd` | `min_liq_usd` | off | min pool liquidity in USD; unknown liquidity skips when set |

These numbers are SDK defaults for your own gate, not RugTrip's rating rules. Tune them to your risk appetite.

About the sell test: `blocked` means a test sale failed and always skips. `no_route` and `error` mean the test
**couldn't run**: that is "couldn't check", never evidence of a trap. The default policy skips them because an
unchecked pool is unchecked; set `treatUnknownSimAs: "allow"` to accept them and read `unknowns` instead.

```ts
const g = gate(verdict, { maxRating: "LOWER_RISK", minLiqUsd: 5_000 });
if (g.skip) console.log("SKIP", g.hits.map((h) => h.text));
```

## Stream

The stream pushes a `verdict` event for every new pool and every re-rating (`version` > 1), and a `pull` event when
liquidity is removed. **Unauthenticated connections receive events 60 s after they happen; real-time delivery comes
with API plans.** All three clients reconnect automatically with exponential backoff and jitter (capped at 30 s),
reset the backoff once a connection delivers a frame, and detect silent connections.

```ts
rt.stream()
  .on("verdict", (v) => { if (v.version === 1) console.log(gate(v).decision, v.symbol, v.pool); })
  .on("pull", (p) => console.log("PULLED", p.symbol, p.life_s, "s"))
  .on("reconnecting", (r) => console.error("reconnecting", r.attempt, r.delayMs))
  .start();
```

```python
async with AsyncClient() as rt:
    async for ev in rt.stream():
        if isinstance(ev, VerdictEvent) and ev.data.version == 1:
            print(gate(ev.data).decision, ev.data.symbol, ev.data.pool)
```

```rust
let mut s = rt.stream(StreamOptions::default())?;
while let Some(ev) = s.next().await? {
    if let StreamEvent::Verdict(v) = ev { println!("{} {}", gate(&v, &Policy::default()).decision.as_str(), v.pool); }
}
```

Delivery is at-least-once (de-duplicate pulls by `pool`), and there is no replay: after a reconnect, backfill the
gap once with `feedRecent({ after: lastIssuedAt })` / `feed_recent(after=…)` if you need it. Unknown frame types are
passed through as `unknown` events instead of failing.

## Rate limits

Per client IP (current values, may change): checks (`/v1/token`, `/v1/pool`) burst 30 then 1 per second; other
reads burst 120 then 4 per second. On `429 rate_limited` the clients retry up to 2 times with exponential backoff and
jitter (honouring `Retry-After` when the server sends it), then raise a `rate_limited` error. Use the stream rather
than polling the feed.

## Errors

Every client raises one error type (`RugTripError` / `rugtrip::Error`) with a `code`:

| code | from | meaning |
|---|---|---|
| `bad_address` | API 400 | not an address, pool id or supported link, or a bad EIP-55 checksum |
| `unknown_token` | API 404 | no pool seen for this token or pool yet |
| `bad_request` | API 400 | invalid query parameter |
| `rate_limited` | API 429 | too many requests (retryable) |
| `not_found` | API 404 | no such resource |
| `unauthorized`, `pro_required` | API 401 / 403 | reserved for plan-gated features |
| `internal` | API 500 | server-side problem |
| `http_error` | SDK | other HTTP error without a known code (502/503/504 are retried) |
| `timeout` | SDK | request timed out (default 10 s; retryable) |
| `network` | SDK | connection failed (retryable) |
| `bad_response` | SDK | the response was not the expected JSON |
| `config` | SDK | missing or invalid base URL, no fetch / WebSocket available |

Requests send `User-Agent: rugtrip-sdk-<lang>/<version>` (`ts`, `python`, `rust`).

## Examples

- [`examples/sniper-gate`](examples/sniper-gate): subscribe to the stream, gate each new pool with the policy, print
  `BUY-CANDIDATE` / `SKIP` with the plain-word reasons. No trading code, no keys. TypeScript and Python; Rust as
  `cargo run --example sniper_gate` in [`rust/`](rust).
- [`examples/check-cli`](examples/check-cli): one-shot check of a token, pool or chart link. TypeScript and Python;
  Rust as `cargo run --example check -- <target>`.

```sh
export RUGTRIP_API_URL=http://127.0.0.1:8742
(cd typescript && npm ci && npm run build)
(cd examples/sniper-gate/typescript && npm install && node sniper-gate.ts)       # Node >= 22.18
(cd examples/sniper-gate/python && uv run --project ../../../python python sniper_gate.py)
(cd examples/check-cli/python && uv run --project ../../../python python check.py 0x…)
```

## Development

```sh
(cd typescript && npm ci && npm test)
(cd python && uv sync && uv run pytest)
(cd rust && cargo test)
```

Unit tests run on recorded API responses in [`fixtures/`](fixtures) (wallet fields replaced with placeholder
addresses). Live tests are opt-in: set `RUGTRIP_API_URL` and they run against that instance with a handful of
requests.

## License

MIT. See [LICENSE](LICENSE).

RugTrip is an independent project and is not affiliated with Robinhood.
