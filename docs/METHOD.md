# How RugTrip ratings are made

RugTrip rates every new pool on Robinhood Chain (chain id 4663) with one of four ratings and 3 to 6 plain-word reasons. Every first rating is frozen the moment it is issued, then scored against what the pool actually did.

Live version, with numbers that refresh every few minutes: the **Method** page on the RugTrip site. The numbers below are a dated snapshot.

## The four ratings

| Rating | Meaning | What it means for a buyer |
|---|---|---|
| **TRAP** | You likely can't get out. | A test sale failed, the code matches a known trap, or the liquidity is already gone. Expect not to get out. |
| **HIGH RISK** | One wallet can take the money. | You can probably sell today. Someone else decides whether there is anything left to sell into. |
| **CAUTION** | Mixed signals. | Something is off, or a check hasn't come back yet. Read the reasons before you buy. |
| **LOWER RISK** | No trap found yet. | Sales clear, the liquidity is locked or burned, nothing else flagged. Still a reading, not a promise. |

Lower risk is the highest rating we give, and it means no trap found yet. Ratings keep updating as the chain moves, and rugs can still happen at any rating.

## What we check

Each check writes its own reason line, tagged with where it came from. The rating follows the worst thing found.

| Check | What it looks at | Why it matters |
|---|---|---|
| **Sell simulation** (`fork sim`) | A test buy and a test sale against current chain state. Does the sale land, and what do the buy and sell taxes really cost? | A contract can claim anything. A sale that fails in simulation is the clearest sign holders can't get out. |
| **Liquidity custody and locks** (`custody`) | Who controls the pool's liquidity: burned, held by a lock, parked in a contract, or sitting in a single wallet. | Whoever holds the liquidity can remove it in one transaction. It is the strongest single signal we have. |
| **Contract and hook code** (`fingerprint`) | The token contract and any Uniswap v4 hook, matched by code family against traps caught before. Plus owner powers: minting, pausing, blocking wallets, changing the tax. | A new name costs nothing. New trap code does. The code is a better tell than the ticker. |
| **Bundled launches** (`launch`) | How much of the supply insiders bought inside the launch block, before anyone outside could. | Supply bought before the market opens is supply waiting to be sold into you. |
| **Creator holdings** (`forensics`) | How much of the supply the creator still holds, and whether they are selling it. | Creator dumps are the largest single way outside buyers lose money on this chain. |
| **Linked wallets** (`forensics`) | The on-chain history of the creator and the liquidity holder: earlier launches, and earlier pools whose liquidity was pulled. | Scammers reuse wallets. A fresh token from a wallet with a pull history is not a fresh start. |
| **Known templates** (`launch`) | Whether a launch copies a setup that earlier pulled pools used. | Scripted launches look scripted. A setup with a pull history is flagged on the launch block. |

## When we can't check

- **Couldn't check is not the same as failed.** A test sale can fail to run because there is no trade route yet or the network hiccupped. The API reports that as `sim.status = "no_route"` or `"error"` and a reason line with `level = "unknown"`. It is never rated TRAP. Only a sale that runs and fails (`"blocked"`) counts against a token.
- **Unknown liquidity custody** is shown the same way (`lp.custody = "unknown"`, an `unknown` reason line).
- A pool with open questions can't be rated LOWER RISK, and the rating updates as answers come in.
- **Coverage is not even.** Custody reads cleanest on Uniswap v4 pools and is hardest on older v2-style pairs, so expect more "couldn't confirm" lines there.

## How we measure ourselves

The first rating of every pool is written once. A database trigger rejects any edit, and later re-ratings are added beside it, never over it. Then we check one thing: was the pool's liquidity pulled within 1, 6 or 24 hours?

Snapshot, complete UTC days 2026-10-11 to 2026-10-26 (162,277 pools rated), as returned by `GET /v1/scorecard` on 2026-10-27:

| Rating | Pools | Pulled within 24 h |
|---|---|---|
| TRAP | 4,120 | 65.5% |
| HIGH RISK | 44,046 | 69.8% |
| CAUTION | 106,877 | 6.2% |
| LOWER RISK | 7,234 | 0.03% (2 pools) |

Misses stay on the board: the 2 pools rated LOWER RISK that were pulled are counted, not hidden. Days before 2026-10-27 were rated by replaying recorded chain history through the same engine, with a few inputs read after the fact, so read them as an estimate; since then every rating is issued live.

**The strongest signal.** Custody at the first rating, against pulls in the next six hours, pools launched 2026-10-19 to 10-23: **0 of 1,769** pools with locked or burned liquidity were pulled, against **88%** when one wallet held it.

## Speed, honestly

Two different clocks, both medians from `GET /v1/stats`:

- **Verdict compute** (`engine_p50_us`): the engine turning what it already knows about a pool into a rating and its reasons. 24.5 µs on 2026-10-27.
- **End to end** (`e2e_p50_ms`): from the moment we first see a pool to its first verdict being written. 941 ms on 2026-10-27. Most of it is waiting for the test sale and the custody read to land.

## What we don't publish

Exact cut-offs, the wallet and code lists behind them, and how we link wallets together stay server-side. Launch rings test their setups against public checkers; publishing the lines would show them exactly where to stand. What we publish instead is the outcome: every first rating, frozen and scored in the open.

## Limits

- Ratings are risk opinions read from on-chain data, not financial advice.
- A rating is a reading of the chain right now. It keeps updating, and it can be wrong.
- Rugs can happen at any rating, LOWER RISK included. The scorecard shows the misses.
- RugTrip is not affiliated with Robinhood Markets, Inc.

## For builders

RugTrip SDK — TypeScript, Python, Rust — open source: <https://github.com/kingcobra-123/rugtrip-sdk>

```ts
import { RugTrip } from "@rugtrip/sdk";
const rt = new RugTrip();
const v = await rt.check(token);
if (v.rating === "TRAP") return skip(v);
```

---

RugTrip shows risk signals, not financial advice. Ratings can be wrong. Not affiliated with Robinhood Markets, Inc.
