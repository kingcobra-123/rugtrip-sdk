mod common;
use common::{fixture, json};
use rugtrip::*;

#[test]
fn token_fixture() {
    let t: TokenCheck = json("token.json");
    assert_eq!(t.chain_id, 4663);
    assert_eq!(t.best, t.pools[0]);
    assert!(t.pools.iter().all(|p| p.rating.risk_rank().is_some()));
    assert!(t.best.details.as_ref().unwrap().lp.is_some());
}

#[test]
fn other_fixtures() {
    let v: Verdict = json("pool.json");
    assert_eq!(v.venue, Venue::V4);
    assert_eq!(v.pool.len(), 66);
    let f: FeedResponse = json("feed_recent.json");
    let sims: std::collections::HashSet<_> = f.pools.iter().filter_map(|p| p.details.as_ref()?.sim.as_ref().map(|s| s.status)).collect();
    for s in [SimStatus::Ok, SimStatus::Blocked, SimStatus::NoRoute, SimStatus::Error] {
        assert!(sims.contains(&s), "fixture covers {s:?}");
    }
    let r: RugsToday = json("rugs_today.json");
    assert!(!r.pulls.is_empty());
    let sc: Scorecard = json("scorecard.json");
    assert!(!sc.days.is_empty() && sc.totals.contains_key("TRAP"));
    let st: Stats = json("stats.json");
    assert!(st.pools_24h >= 0);
    let h: Health = json("healthz.json");
    let _ = h.ok;
    let e: ErrorBody = json("error_bad_address.json");
    assert_eq!(e.error, "bad_address");
}

#[test]
fn unknown_enum_values_and_extra_fields_are_tolerated() {
    let mut v: serde_json::Value = json("pool.json");
    v["rating"] = "SOMETHING_NEW".into();
    v["venue"] = "v9".into();
    v["details"]["lp"]["custody"] = "timelocked_by_dao".into();
    v["details"]["sim"]["status"] = "partial".into();
    v["reasons"][0]["check"] = "new_check".into();
    v["future_field"] = 1.into();
    let p: Verdict = serde_json::from_value(v).unwrap();
    assert_eq!(p.rating, Rating::Unrecognized);
    assert_eq!(p.venue, Venue::Unrecognized);
    let d = p.details.unwrap();
    assert_eq!(d.lp.unwrap().custody, LpCustody::Unknown);
    assert_eq!(d.sim.unwrap().status, SimStatus::Unrecognized);
    assert_eq!(p.reasons[0].check, Check::Unrecognized);
}

#[test]
fn stream_frames() {
    let evs: Vec<_> = fixture("stream.jsonl")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(parse_frame)
        .collect();
    assert!(evs.iter().any(|e| matches!(e, StreamEvent::Verdict(_))));
    assert!(evs.iter().any(|e| matches!(e, StreamEvent::Pull(_))));
    let unknown: Vec<_> = evs
        .iter()
        .filter_map(|e| {
            if let StreamEvent::Unknown { kind, .. } = e {
                Some(kind.as_str())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(unknown, vec!["hello_from_the_future"]);
    assert!(parse_frame("garbage").is_none());
    assert!(parse_frame(r#"{"type":"verdict","data":{}}"#).is_none());
}

#[test]
fn targets_and_urls() {
    let id = "0x3e767ad60e19334d08952bed1b1a88c722d55eddfeaaab50b695330c260a4322";
    assert_eq!(to_target(&format!("https://dexscreener.com/robinhood/{id}?x=1")), id);
    assert_eq!(to_target(&format!(" {id} ")), id);
    assert_eq!(to_target("hello world"), "hello%20world");
    assert_eq!(stream_url("http://127.0.0.1:8742").unwrap(), "ws://127.0.0.1:8742/v1/stream");
    assert_eq!(stream_url("https://api.example.test/base/").unwrap(), "wss://api.example.test/base/v1/stream");
}

#[test]
fn backoff_is_capped_and_jittered() {
    use std::time::Duration;
    let s = Duration::from_secs(1);
    let cap = Duration::from_secs(30);
    assert_eq!(backoff_with(0, s, cap, 0.0), Duration::from_millis(500));
    assert_eq!(backoff_with(0, s, cap, 1.0), s);
    assert_eq!(backoff_with(40, s, cap, 1.0), cap);
    for i in 0..10 {
        let b = backoff(i, s, cap);
        assert!(b <= cap && b >= Duration::from_millis(500));
    }
}

#[test]
fn base_url_required() {
    if std::env::var("RUGTRIP_API_URL").is_err() {
        assert_eq!(Client::from_env().unwrap_err().kind, ErrorKind::Config);
    }
    assert_eq!(Client::new("ftp://x").unwrap_err().kind, ErrorKind::Config);
    assert_eq!(Client::new("").unwrap_err().kind, ErrorKind::Config);
    assert_eq!(Client::new("http://127.0.0.1:8742/").unwrap().base_url(), "http://127.0.0.1:8742");
    assert!(user_agent().starts_with("rugtrip-sdk-rust/"));
}
