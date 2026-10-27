//! Wire types for the RugTrip public API (JSON, snake_case). Timestamps are unix seconds (f64).
//! Unknown enum strings from newer servers deserialize to an `Unrecognized` / `Unknown` variant instead of failing.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Risk rating, most risky first. A rating is a risk opinion, not a guarantee.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Rating {
    Trap,
    HighRisk,
    Caution,
    LowerRisk,
    /// a rating this SDK version doesn't know
    #[serde(other)]
    Unrecognized,
}

impl Rating {
    pub const ALL: [Rating; 4] = [Rating::Trap, Rating::HighRisk, Rating::Caution, Rating::LowerRisk];
    /// 0 = TRAP ... 3 = LOWER_RISK; None for `Unrecognized`.
    pub fn risk_rank(&self) -> Option<u8> {
        match self {
            Rating::Trap => Some(0),
            Rating::HighRisk => Some(1),
            Rating::Caution => Some(2),
            Rating::LowerRisk => Some(3),
            Rating::Unrecognized => None,
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Rating::Trap => "TRAP",
            Rating::HighRisk => "HIGH_RISK",
            Rating::Caution => "CAUTION",
            Rating::LowerRisk => "LOWER_RISK",
            Rating::Unrecognized => "UNRECOGNIZED",
        }
    }
}

impl std::fmt::Display for Rating {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Bad,
    Warn,
    Ok,
    #[serde(other)]
    Unknown,
}

impl Level {
    pub fn as_str(&self) -> &'static str {
        match self {
            Level::Bad => "bad",
            Level::Warn => "warn",
            Level::Ok => "ok",
            Level::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Check {
    SellSim,
    Lp,
    Code,
    Bundle,
    Creator,
    Wallets,
    Template,
    #[serde(other)]
    Unrecognized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Venue {
    V2,
    V3,
    V4,
    #[serde(other)]
    Unrecognized,
}

impl Venue {
    pub fn as_str(&self) -> &'static str {
        match self {
            Venue::V2 => "v2",
            Venue::V3 => "v3",
            Venue::V4 => "v4",
            Venue::Unrecognized => "unrecognized",
        }
    }
}

/// Who controls the pool's liquidity. `Unknown` = not measured yet or no single dominant holder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LpCustody {
    Burned,
    Locked,
    OneWallet,
    Contract,
    #[serde(other)]
    Unknown,
}

impl LpCustody {
    pub fn as_str(&self) -> &'static str {
        match self {
            LpCustody::Burned => "burned",
            LpCustody::Locked => "locked",
            LpCustody::OneWallet => "one_wallet",
            LpCustody::Contract => "contract",
            LpCustody::Unknown => "unknown",
        }
    }
}

/// Buy-and-sell test result. `Blocked` = a test sale failed. `NoRoute` / `Error` = the test couldn't run:
/// "couldn't check", never evidence of a trap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimStatus {
    Ok,
    Blocked,
    NoRoute,
    Error,
    #[serde(other)]
    Unrecognized,
}

/// One plain-words finding behind a rating.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reason {
    pub level: Level,
    pub check: Check,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LpDetails {
    pub custody: LpCustody,
    /// share of the LP held by the largest single wallet, %
    #[serde(default)]
    pub held_pct: Option<f64>,
    /// share of the LP burned or in a lock, %
    #[serde(default)]
    pub locked_pct: Option<f64>,
    #[serde(default)]
    pub lock_until: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimDetails {
    pub status: SimStatus,
    #[serde(default)]
    pub buy_tax_pct: Option<f64>,
    #[serde(default)]
    pub sell_tax_pct: Option<f64>,
}

/// Public facts behind the reasons. Fields are None when unknown; `lp` / `sim` are None while pending.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VerdictDetails {
    #[serde(default)]
    pub lp: Option<LpDetails>,
    #[serde(default)]
    pub sim: Option<SimDetails>,
    /// share of supply bought in the launch block by insiders, %
    #[serde(default)]
    pub bundle_pct: Option<f64>,
    /// share of supply the creator still holds, %
    #[serde(default)]
    pub creator_pct: Option<f64>,
    #[serde(default)]
    pub deployer: Option<String>,
    #[serde(default)]
    pub lp_holder: Option<String>,
    #[serde(default)]
    pub creator: Option<String>,
}

/// One pool's verdict. `version` 1 = first verdict; higher = re-rated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Verdict {
    /// 20-byte pool address (v2/v3) or 32-byte pool id (v4), lower-case hex
    pub pool: String,
    pub token: String,
    #[serde(default)]
    pub symbol: String,
    pub venue: Venue,
    pub pool_age_s: i64,
    #[serde(default)]
    pub liq_usd: Option<f64>,
    pub rating: Rating,
    #[serde(default)]
    pub reasons: Vec<Reason>,
    pub issued_at: f64,
    pub updated_at: f64,
    pub version: i64,
    #[serde(default)]
    pub details: Option<VerdictDetails>,
}

/// `GET /v1/token/:x`: `best` is the most liquid pool; `pools` lists every known pool, most liquid first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenCheck {
    pub token: String,
    #[serde(default)]
    pub symbol: String,
    pub chain_id: u64,
    pub best: Verdict,
    pub pools: Vec<Verdict>,
}

/// A liquidity pull (rug) seen on chain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pull {
    pub pool: String,
    pub token: String,
    #[serde(default)]
    pub symbol: String,
    pub venue: Venue,
    pub launched_at: f64,
    pub pulled_at: f64,
    pub life_s: i64,
    #[serde(default)]
    pub liq_removed_usd: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedResponse {
    pub pools: Vec<Verdict>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RugsToday {
    pub since: f64,
    pub pulls: Vec<Pull>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScorecardDay {
    pub day: String,
    pub rating: Rating,
    pub pools: i64,
    pub pulled_1h: i64,
    pub pulled_6h: i64,
    pub pulled_24h: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ScorecardTotal {
    pub pools: i64,
    pub pulled_1h: i64,
    pub pulled_6h: i64,
    pub pulled_24h: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scorecard {
    pub days: Vec<ScorecardDay>,
    /// keyed by rating string ("TRAP", ...), summed over `days`
    pub totals: BTreeMap<String, ScorecardTotal>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    pub pools_24h: i64,
    pub pulls_24h: i64,
    #[serde(default)]
    pub median_life_s: Option<f64>,
    /// alias of `e2e_p50_ms` kept for v1 clients
    #[serde(default)]
    pub decision_p50_ms: Option<f64>,
    /// first sight of a pool -> first verdict, ms, p50
    #[serde(default)]
    pub e2e_p50_ms: Option<f64>,
    /// verdict compute time, microseconds, p50
    #[serde(default)]
    pub engine_p50_us: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Health {
    pub ok: bool,
    #[serde(default)]
    pub head_block: Option<u64>,
    #[serde(default)]
    pub lag_s: Option<f64>,
}

/// Alert frames are only sent to real-time (paid plan) connections.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Alert {
    pub id: i64,
    pub token: String,
    #[serde(default)]
    pub symbol: String,
    /// creator_sell | lp_change | owner_action | locked_launch
    pub kind: String,
    pub text: String,
    #[serde(default)]
    pub numbers: BTreeMap<String, serde_json::Value>,
    pub block: u64,
    pub at: f64,
    #[serde(default)]
    pub latency_ms: Option<f64>,
    #[serde(default)]
    pub rating: Option<Rating>,
    #[serde(default)]
    pub pool_age_s: Option<i64>,
}

/// `{"error": code, "message": text}`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub error: String,
    #[serde(default)]
    pub message: String,
}

/// Query for `GET /v1/feed/recent`.
#[derive(Debug, Clone, Default)]
pub struct FeedQuery {
    pub rating: Option<Rating>,
    pub venue: Option<Venue>,
    /// 1..200, default 50
    pub limit: Option<u32>,
    /// only verdicts issued before this unix time
    pub before: Option<f64>,
    /// only verdicts issued after this unix time
    pub after: Option<f64>,
    /// include late verdicts of older pools (default: only pools first rated within 1 h of creation)
    pub include_late: bool,
}

/// Query for `GET /v1/rugs/today`.
#[derive(Debug, Clone, Default)]
pub struct RugsQuery {
    /// unix time, at most 7 days back (default: 24 h ago)
    pub since: Option<f64>,
    /// page back: the oldest `pulled_at` received (exclusive)
    pub before: Option<f64>,
    /// 1..5000, default 5000
    pub limit: Option<u32>,
}
