mod common;
use common::json;
use rugtrip::*;

fn lower() -> Verdict {
    json::<TokenCheck>("token_lower_risk.json").best
}

fn feed() -> Vec<Verdict> {
    json::<FeedResponse>("feed_recent.json").pools
}

#[test]
fn defaults_are_conservative() {
    let p = Policy::default();
    assert_eq!(p.max_rating, Rating::Caution);
    assert_eq!(p.require_lp_custody, vec![LpCustody::Burned, LpCustody::Locked]);
    assert_eq!(p.treat_unknown_sim_as, Unknowns::Skip);
}

#[test]
fn candidate() {
    let g = gate(&lower(), &Policy::default());
    assert_eq!(g.decision, Decision::BuyCandidate, "{:?}", g.hits);
    assert!(g.hits.is_empty());
}

#[test]
fn trap_and_high_risk_skip() {
    for v in feed().iter().filter(|v| matches!(v.rating, Rating::Trap | Rating::HighRisk)) {
        let g = gate(v, &Policy::default());
        assert!(g.skip && g.hits.iter().any(|h| h.rule == "max_rating"));
    }
}

#[test]
fn blocked_always_skips() {
    let f = feed();
    let v = f
        .iter()
        .find(|v| v.details.as_ref().and_then(|d| d.sim.as_ref()).map(|s| s.status) == Some(SimStatus::Blocked))
        .unwrap();
    let p = Policy {
        max_rating: Rating::Trap,
        treat_unknown_sim_as: Unknowns::Allow,
        skip_on_bad_reason: false,
        require_lp_custody: vec![],
        ..Policy::default()
    };
    assert!(gate(v, &p).hits.iter().any(|h| h.rule == "sell_blocked"));
}

#[test]
fn no_route_and_error_are_unknown_not_trap() {
    for status in [SimStatus::NoRoute, SimStatus::Error] {
        let mut v = lower();
        v.details.as_mut().unwrap().sim = Some(SimDetails {
            status,
            buy_tax_pct: None,
            sell_tax_pct: None,
        });
        let strict = gate(&v, &Policy::default());
        assert!(strict.skip && strict.hits.iter().all(|h| h.rule == "treat_unknown_sim_as"), "{:?}", strict.hits);
        assert!(strict.unknowns.iter().any(|u| u.contains("couldn't run")));
        let lenient = gate(
            &v,
            &Policy {
                treat_unknown_sim_as: Unknowns::Allow,
                ..Policy::default()
            },
        );
        assert!(!lenient.skip && !lenient.unknowns.is_empty());
    }
}

#[test]
fn pending_and_custody() {
    let mut v = lower();
    v.details.as_mut().unwrap().sim = None;
    assert!(should_skip(&v, &Policy::default()));
    assert!(!should_skip(
        &v,
        &Policy {
            treat_unknown_sim_as: Unknowns::Allow,
            ..Policy::default()
        }
    ));
    let mut v = lower();
    v.details.as_mut().unwrap().lp.as_mut().unwrap().custody = LpCustody::OneWallet;
    assert!(gate(&v, &Policy::default()).hits.iter().any(|h| h.rule == "require_lp_custody"));
    assert!(!should_skip(
        &v,
        &Policy {
            require_lp_custody: vec![],
            ..Policy::default()
        }
    ));
    v.details.as_mut().unwrap().lp = None;
    assert!(should_skip(&v, &Policy::default()));
}

#[test]
fn limits() {
    let mut v = lower();
    v.details.as_mut().unwrap().sim.as_mut().unwrap().sell_tax_pct = Some(12.0);
    assert!(gate(&v, &Policy::default()).hits.iter().any(|h| h.rule == "max_sell_tax_pct"));
    let mut c = lower();
    let d = c.details.as_mut().unwrap();
    d.creator_pct = Some(30.0);
    d.bundle_pct = Some(40.0);
    let rules: Vec<_> = gate(&c, &Policy::default()).hits.iter().map(|h| h.rule).collect();
    assert!(rules.contains(&"max_creator_pct") && rules.contains(&"max_bundle_pct"));
    let mut u = lower();
    u.details.as_mut().unwrap().creator_pct = None;
    assert!(!should_skip(&u, &Policy::default()));
    assert_eq!(gate(&u, &Policy::default()).unknowns.len(), 1);
    assert!(should_skip(
        &u,
        &Policy {
            treat_unknown_holdings_as: Unknowns::Skip,
            ..Policy::default()
        }
    ));
    assert!(should_skip(
        &lower(),
        &Policy {
            min_liq_usd: Some(1000.0),
            ..Policy::default()
        }
    ));
    let mut n = lower();
    n.liq_usd = None;
    assert!(should_skip(
        &n,
        &Policy {
            min_liq_usd: Some(1.0),
            ..Policy::default()
        }
    ));
    let mut r = lower();
    r.rating = Rating::Unrecognized;
    assert!(gate(&r, &Policy::default()).hits.iter().any(|h| h.rule == "rating_unrecognized"));
}

#[test]
fn bad_reason_and_order() {
    let mut v = lower();
    v.reasons.push(Reason {
        level: Level::Bad,
        check: Check::Code,
        text: "Something bad".into(),
    });
    let g = gate(&v, &Policy::default());
    assert!(g.hits.iter().any(|h| h.text == "Something bad"));
    assert_eq!(g.reasons[0].level, Level::Bad);
}
