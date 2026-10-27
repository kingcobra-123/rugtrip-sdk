/**
 * Wire types for the RugTrip public API (JSON, snake_case). Field names match the server exactly so a response can
 * be used as-is. Timestamps are unix seconds (float).
 */

/** Risk rating, from most to least risky. A rating is a risk opinion, not a guarantee. */
export type Rating = "TRAP" | "HIGH_RISK" | "CAUTION" | "LOWER_RISK";

/** All ratings, most risky first. */
export const RATINGS: readonly Rating[] = ["TRAP", "HIGH_RISK", "CAUTION", "LOWER_RISK"];

export type Level = "bad" | "warn" | "ok" | "unknown";
export type Check = "sell_sim" | "lp" | "code" | "bundle" | "creator" | "wallets" | "template";
export type Venue = "v2" | "v3" | "v4";

/** Who controls the pool's liquidity. `unknown` = not measured yet or no single dominant holder. */
export type LpCustody = "burned" | "locked" | "one_wallet" | "contract" | "unknown";

/**
 * Result of the buy-and-sell test. `blocked` = a test sale failed. `no_route` and `error` mean the test could not
 * be run: that is "couldn't check", never evidence of a trap.
 */
export type SimStatus = "ok" | "blocked" | "no_route" | "error";

/** One plain-words finding behind a rating. */
export interface Reason {
  level: Level;
  check: Check;
  text: string;
}

export interface LpDetails {
  custody: LpCustody;
  /** share of the LP held by the largest single wallet, % */
  held_pct: number | null;
  /** share of the LP burned or in a lock, % */
  locked_pct: number | null;
  lock_until: number | null;
}

export interface SimDetails {
  status: SimStatus;
  buy_tax_pct: number | null;
  sell_tax_pct: number | null;
}

/** Public facts behind the reasons. Each field is null when unknown; `lp` / `sim` are null while still pending. */
export interface VerdictDetails {
  lp: LpDetails | null;
  sim: SimDetails | null;
  /** share of supply bought in the launch block by insiders, % */
  bundle_pct: number | null;
  /** share of supply the creator still holds, % */
  creator_pct: number | null;
  deployer: string | null;
  lp_holder: string | null;
  creator: string | null;
}

/** One pool's verdict. */
export interface Verdict {
  /** 20-byte pool address (v2/v3) or 32-byte pool id (v4), lower-case hex */
  pool: string;
  token: string;
  symbol: string;
  venue: Venue;
  pool_age_s: number;
  liq_usd: number | null;
  rating: Rating;
  reasons: Reason[];
  /** when the pool's first verdict was issued */
  issued_at: number;
  /** when the version shown was issued */
  updated_at: number;
  /** 1 = first verdict; higher = re-rated */
  version: number;
  details: VerdictDetails | null;
}

/** `GET /v1/token/:x`: `best` is the token's most liquid pool; `pools` lists every known pool (most liquid first). */
export interface TokenCheck {
  token: string;
  symbol: string;
  chain_id: number;
  best: Verdict;
  pools: Verdict[];
}

/** A liquidity pull (rug) seen on chain. */
export interface Pull {
  pool: string;
  token: string;
  symbol: string;
  venue: Venue;
  launched_at: number;
  pulled_at: number;
  life_s: number;
  liq_removed_usd: number | null;
}

export interface FeedResponse {
  pools: Verdict[];
}

export interface RugsToday {
  since: number;
  pulls: Pull[];
}

export interface ScorecardDay {
  day: string;
  rating: Rating;
  pools: number;
  pulled_1h: number;
  pulled_6h: number;
  pulled_24h: number;
}

export interface ScorecardTotal {
  pools: number;
  pulled_1h: number;
  pulled_6h: number;
  pulled_24h: number;
}

export interface Scorecard {
  days: ScorecardDay[];
  /** keyed by rating, summed over `days` */
  totals: Partial<Record<Rating, ScorecardTotal>>;
}

export interface Stats {
  pools_24h: number;
  pulls_24h: number;
  median_life_s: number | null;
  /** alias of `e2e_p50_ms` kept for v1 clients */
  decision_p50_ms: number | null;
  /** first sight of a pool -> first verdict, ms, p50 */
  e2e_p50_ms: number | null;
  /** verdict compute time, microseconds, p50 */
  engine_p50_us: number | null;
}

export interface Health {
  ok: boolean;
  head_block: number | null;
  lag_s: number | null;
}

/** Alert frames are only sent to real-time (paid plan) connections; typed here for completeness. */
export interface Alert {
  id: number;
  token: string;
  symbol: string;
  kind: "creator_sell" | "lp_change" | "owner_action" | "locked_launch" | (string & {});
  text: string;
  numbers: Record<string, string>;
  block: number;
  at: number;
  latency_ms: number | null;
  rating?: Rating | null;
  pool_age_s?: number | null;
}

/** One `/v1/stream` frame. */
export type StreamMessage =
  | { type: "verdict"; data: Verdict }
  | { type: "pull"; data: Pull }
  | { type: "alert"; data: Alert };

/** Error body: `{"error": code, "message": text}`. */
export interface ErrorBody {
  error: string;
  message: string;
}

export interface FeedQuery {
  rating?: Rating;
  venue?: Venue;
  /** 1..200, default 50 */
  limit?: number;
  /** only verdicts issued before this unix time */
  before?: number;
  /** only verdicts issued after this unix time */
  after?: number;
  /** include late verdicts of older pools (default: only pools first rated within 1 h of creation) */
  includeLate?: boolean;
}

export interface RugsQuery {
  /** unix time, at most 7 days back (default: 24 h ago) */
  since?: number;
  /** page back: pass the oldest `pulled_at` you received (exclusive) */
  before?: number;
  /** 1..5000, default 5000 */
  limit?: number;
}
