//! Opt-in: runs only when RUGTRIP_API_URL is set. Keeps traffic light.
use rugtrip::*;
use std::time::Duration;

fn url() -> Option<String> {
    std::env::var("RUGTRIP_API_URL").ok().filter(|s| !s.is_empty())
}

#[tokio::test]
async fn live_rest() {
    let Some(u) = url() else {
        eprintln!("skipped: set RUGTRIP_API_URL to run");
        return;
    };
    let c = Client::new(u).unwrap();
    let _ = c.health().await.unwrap();
    let feed = c
        .feed_recent(&FeedQuery {
            limit: Some(3),
            ..Default::default()
        })
        .await
        .unwrap();
    if let Some(p) = feed.first() {
        let t = c.check(&p.token).await.unwrap();
        assert!(t.best.rating.risk_rank().is_some());
        assert_eq!(c.pool(&p.pool).await.unwrap().pool, p.pool);
    }
    assert_eq!(c.check("0x1234").await.unwrap_err().kind, ErrorKind::BadAddress);
    assert_eq!(
        c.check("0x0000000000000000000000000000000000000001").await.unwrap_err().kind,
        ErrorKind::UnknownToken
    );
}

#[tokio::test]
async fn live_stream_connects() {
    let Some(u) = url() else {
        eprintln!("skipped: set RUGTRIP_API_URL to run");
        return;
    };
    let mut s = Client::new(u)
        .unwrap()
        .stream(StreamOptions {
            max_reconnects: Some(0),
            ..Default::default()
        })
        .unwrap();
    let ev = tokio::time::timeout(Duration::from_secs(10), s.next()).await.unwrap().unwrap();
    assert!(matches!(ev, Some(StreamEvent::Connected { .. })));
    s.close().await;
}
