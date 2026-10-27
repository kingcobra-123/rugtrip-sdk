//! Pre-trade gate: your own rules applied to one pool's Verdict.
//!
//! These are client-side preferences; they say nothing about how RugTrip rates a pool. Passing a policy does not
//! make a token low-risk: ratings are risk opinions, not guarantees.
use crate::types::{Level, LpCustody, Rating, Reason, SimStatus, Verdict};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unknowns {
    Skip,
    Allow,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Policy {
    /// Most risky rating you accept. Default `Caution` (skips TRAP and HIGH_RISK).
    pub max_rating: Rating,
    /// Skip when any reason has level "bad", whatever the rating. Default true.
    pub skip_on_bad_reason: bool,
    /// Accepted LP custody values; empty = don't check. Default burned, locked.
    pub require_lp_custody: Vec<LpCustody>,
    /// Sell test couldn't run (no_route / error) or not run yet: "couldn't check", not a trap. Default `Skip`.
    /// A failed test sale (blocked) always skips.
    pub treat_unknown_sim_as: Unknowns,
    /// Max buy / sell tax from the sell test, %. None = don't check. Default 10 / 10.
    pub max_buy_tax_pct: Option<f64>,
    pub max_sell_tax_pct: Option<f64>,
    /// Max share of supply the creator still holds, %. None = don't check. Default 10.
    pub max_creator_pct: Option<f64>,
    /// Max share of supply bought by insiders in the launch block, %. None = don't check. Default 25.
    pub max_bundle_pct: Option<f64>,
    /// Creator / bundle numbers not measured yet. Default `Allow` (listed in `unknowns`).
    pub treat_unknown_holdings_as: Unknowns,
    /// Min pool liquidity in USD; None = don't check. When set, unknown liquidity skips. Default None.
    pub min_liq_usd: Option<f64>,
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            max_rating: Rating::Caution,
            skip_on_bad_reason: true,
            require_lp_custody: vec![LpCustody::Burned, LpCustody::Locked],
            treat_unknown_sim_as: Unknowns::Skip,
            max_buy_tax_pct: Some(10.0),
            max_sell_tax_pct: Some(10.0),
            max_creator_pct: Some(10.0),
            max_bundle_pct: Some(25.0),
            treat_unknown_holdings_as: Unknowns::Allow,
            min_liq_usd: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Skip,
    BuyCandidate,
}

impl Decision {
    pub fn as_str(&self) -> &'static str {
        match self {
            Decision::Skip => "SKIP",
            Decision::BuyCandidate => "BUY_CANDIDATE",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PolicyHit {
    /// which policy field caused the skip (or "rating_unrecognized" / "sell_blocked")
    pub rule: &'static str,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GateResult {
    pub skip: bool,
    pub decision: Decision,
    /// policy rules that failed (empty when not skipped)
    pub hits: Vec<PolicyHit>,
    /// facts that could not be checked (allowed by the policy, or the cause of a skip)
    pub unknowns: Vec<String>,
    /// the verdict's own plain-word reasons, most serious first
    pub reasons: Vec<Reason>,
}

fn fmt(x: f64) -> String {
    if x.fract() == 0.0 {
        format!("{x:.0}")
    } else {
        format!("{x:.1}")
    }
}

fn level_order(l: Level) -> u8 {
    match l {
        Level::Bad => 0,
        Level::Warn => 1,
        Level::Unknown => 2,
        Level::Ok => 3,
    }
}

/// Apply a policy to one pool's verdict.
pub fn gate(v: &Verdict, p: &Policy) -> GateResult {
    let mut hits: Vec<PolicyHit> = vec![];
    let mut unknowns: Vec<String> = vec![];
    let d = v.details.as_ref();
    let hit = |hits: &mut Vec<PolicyHit>, rule: &'static str, text: String| hits.push(PolicyHit { rule, text });

    match (v.rating.risk_rank(), p.max_rating.risk_rank()) {
        (None, _) => hit(&mut hits, "rating_unrecognized", "Unrecognized rating".into()),
        (Some(r), Some(m)) if r < m => hit(&mut hits, "max_rating", format!("Rated {} (policy accepts up to {})", v.rating, p.max_rating)),
        _ => {}
    }

    if p.skip_on_bad_reason {
        for r in v.reasons.iter().filter(|r| r.level == Level::Bad) {
            hit(&mut hits, "skip_on_bad_reason", r.text.clone());
        }
    }

    if !p.require_lp_custody.is_empty() {
        match d.and_then(|d| d.lp.as_ref()).map(|lp| lp.custody) {
            None => {
                unknowns.push("LP custody not measured yet".into());
                hit(&mut hits, "require_lp_custody", "LP custody not measured yet".into());
            }
            Some(c) if !p.require_lp_custody.contains(&c) => {
                let want: Vec<&str> = p.require_lp_custody.iter().map(|c| c.as_str()).collect();
                hit(
                    &mut hits,
                    "require_lp_custody",
                    format!("LP custody is {} (policy requires {})", c.as_str(), want.join(" or ")),
                );
            }
            _ => {}
        }
    }

    let sim_unknown = |hits: &mut Vec<PolicyHit>, unknowns: &mut Vec<String>, why: String| {
        if p.treat_unknown_sim_as == Unknowns::Skip {
            hits.push(PolicyHit {
                rule: "treat_unknown_sim_as",
                text: why.clone(),
            });
        }
        unknowns.push(why);
    };
    match d.and_then(|d| d.sim.as_ref()) {
        None => sim_unknown(&mut hits, &mut unknowns, "Sell test not run yet".into()),
        Some(s) => match s.status {
            SimStatus::Blocked => hit(&mut hits, "sell_blocked", "A test sale failed".into()),
            SimStatus::NoRoute => sim_unknown(&mut hits, &mut unknowns, "Sell test couldn't run (no route): sellability not checked".into()),
            SimStatus::Error => sim_unknown(&mut hits, &mut unknowns, "Sell test couldn't run (error): sellability not checked".into()),
            SimStatus::Unrecognized => sim_unknown(&mut hits, &mut unknowns, "Sell test status unrecognized: sellability not checked".into()),
            SimStatus::Ok => {
                if let Some(max) = p.max_buy_tax_pct {
                    match s.buy_tax_pct {
                        None => sim_unknown(&mut hits, &mut unknowns, "Buy tax not measured".into()),
                        Some(t) if t > max => hit(&mut hits, "max_buy_tax_pct", format!("Buy tax {}% (policy max {}%)", fmt(t), fmt(max))),
                        _ => {}
                    }
                }
                if let Some(max) = p.max_sell_tax_pct {
                    match s.sell_tax_pct {
                        None => sim_unknown(&mut hits, &mut unknowns, "Sell tax not measured".into()),
                        Some(t) if t > max => hit(&mut hits, "max_sell_tax_pct", format!("Sell tax {}% (policy max {}%)", fmt(t), fmt(max))),
                        _ => {}
                    }
                }
            }
        },
    }

    let mut holding = |val: Option<f64>, max: Option<f64>, rule: &'static str, label: &str| {
        let Some(max) = max else { return };
        match val {
            None => {
                unknowns.push(format!("{label} not measured yet"));
                if p.treat_unknown_holdings_as == Unknowns::Skip {
                    hits.push(PolicyHit {
                        rule: "treat_unknown_holdings_as",
                        text: format!("{label} not measured yet"),
                    });
                }
            }
            Some(x) if x > max => hits.push(PolicyHit {
                rule,
                text: format!("{label} {}% (policy max {}%)", fmt(x), fmt(max)),
            }),
            _ => {}
        }
    };
    holding(d.and_then(|d| d.creator_pct), p.max_creator_pct, "max_creator_pct", "Creator holds");
    holding(d.and_then(|d| d.bundle_pct), p.max_bundle_pct, "max_bundle_pct", "Launch-block insider buys");

    if let Some(min) = p.min_liq_usd {
        match v.liq_usd {
            None => hit(&mut hits, "min_liq_usd", "Liquidity unknown".into()),
            Some(l) if l < min => hit(&mut hits, "min_liq_usd", format!("Liquidity ${} (policy min ${})", fmt(l), fmt(min))),
            _ => {}
        }
    }

    let mut reasons = v.reasons.clone();
    reasons.sort_by_key(|r| level_order(r.level));
    let skip = !hits.is_empty();
    GateResult {
        skip,
        decision: if skip { Decision::Skip } else { Decision::BuyCandidate },
        hits,
        unknowns,
        reasons,
    }
}

/// true when the policy says skip this pool.
pub fn should_skip(v: &Verdict, p: &Policy) -> bool {
    gate(v, p).skip
}
