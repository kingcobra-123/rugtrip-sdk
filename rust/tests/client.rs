//! HTTP client tests against a tiny scripted HTTP/1.1 server on a loopback port.
mod common;
use common::fixture;
use rugtrip::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

struct Reply {
    status: u16,
    body: String,
    headers: Vec<(&'static str, &'static str)>,
    delay: Option<Duration>,
}

fn reply(status: u16, body: impl Into<String>) -> Reply {
    Reply {
        status,
        body: body.into(),
        headers: vec![],
        delay: None,
    }
}

/// Serves the replies in order, one per connection; returns the base URL and the captured request heads.
async fn server(replies: Vec<Reply>) -> (String, Arc<Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(vec![]));
    let seen2 = seen.clone();
    tokio::spawn(async move {
        for r in replies {
            let (mut s, _) = l.accept().await.unwrap();
            let mut buf = vec![0u8; 8192];
            let mut n = 0;
            while !buf[..n].windows(4).any(|w| w == b"\r\n\r\n") {
                let k = s.read(&mut buf[n..]).await.unwrap();
                if k == 0 {
                    break;
                }
                n += k;
            }
            seen2.lock().unwrap().push(String::from_utf8_lossy(&buf[..n]).to_string());
            if let Some(d) = r.delay {
                tokio::time::sleep(d).await;
            }
            let mut head = format!(
                "HTTP/1.1 {} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n",
                r.status,
                r.body.len()
            );
            for (k, v) in r.headers {
                head.push_str(&format!("{k}: {v}\r\n"));
            }
            head.push_str("\r\n");
            let _ = s.write_all(head.as_bytes()).await;
            let _ = s.write_all(r.body.as_bytes()).await;
            let _ = s.shutdown().await;
        }
    });
    (base, seen)
}

fn client(base: &str, retries: u32) -> Client {
    Client::builder()
        .base_url(base)
        .max_retries(retries)
        .retry_base(Duration::from_millis(1))
        .timeout(Duration::from_millis(500))
        .build()
        .unwrap()
}

#[tokio::test]
async fn check_sends_user_agent_and_parses() {
    let body = fixture("token.json");
    let token = serde_json::from_str::<TokenCheck>(&body).unwrap().token;
    let (base, seen) = server(vec![reply(200, body)]).await;
    let t = client(&base, 0).check(&token).await.unwrap();
    assert_eq!(t.token, token);
    let head = seen.lock().unwrap()[0].to_lowercase();
    assert!(head.starts_with(&format!("get /v1/token/{token} http/1.1")), "{head}");
    assert!(head.contains("user-agent: rugtrip-sdk-rust/"), "{head}");
}

#[tokio::test]
async fn feed_query_string() {
    let (base, seen) = server(vec![reply(200, r#"{"pools":[]}"#)]).await;
    let q = FeedQuery {
        rating: Some(Rating::Caution),
        venue: Some(Venue::V4),
        limit: Some(10),
        after: Some(5.0),
        include_late: true,
        ..Default::default()
    };
    assert!(client(&base, 0).feed_recent(&q).await.unwrap().is_empty());
    let head = seen.lock().unwrap()[0].clone();
    assert!(
        head.starts_with("GET /v1/feed/recent?rating=CAUTION&venue=v4&limit=10&after=5&include_late=1 "),
        "{head}"
    );
}

#[tokio::test]
async fn api_errors_map() {
    let (base, _) = server(vec![
        reply(400, fixture("error_bad_address.json")),
        reply(404, fixture("error_unknown_token.json")),
        reply(500, "oops"),
    ])
    .await;
    let c = client(&base, 0);
    let e = c.check("0x1234").await.unwrap_err();
    assert_eq!((e.kind, e.status), (ErrorKind::BadAddress, Some(400)));
    let e = c.pool("0x3e767ad60e19334d08952bed1b1a88c722d55eddfeaaab50b695330c260a4322").await.unwrap_err();
    assert_eq!(e.kind, ErrorKind::UnknownToken);
    assert!(e.message.contains("haven't seen"));
    let e = c.stats().await.unwrap_err();
    assert_eq!((e.kind, e.status), (ErrorKind::Http, Some(500)));
}

#[tokio::test]
async fn rate_limited_retries_then_succeeds() {
    let rl = fixture("error_rate_limited.json");
    let mut first = reply(429, rl.clone());
    first.headers.push(("retry-after", "0"));
    let (base, seen) = server(vec![first, reply(429, rl), reply(200, fixture("stats.json"))]).await;
    assert!(client(&base, 2).stats().await.unwrap().pools_24h >= 0);
    assert_eq!(seen.lock().unwrap().len(), 3);
}

#[tokio::test]
async fn rate_limited_surfaces_with_retry_after() {
    let rl = fixture("error_rate_limited.json");
    let mut second = reply(429, rl.clone());
    second.headers.push(("retry-after", "7"));
    let (base, _) = server(vec![reply(429, rl), second]).await;
    let e = client(&base, 1).stats().await.unwrap_err();
    assert_eq!(e.kind, ErrorKind::RateLimited);
    assert!(e.retryable());
    assert_eq!(e.retry_after, Some(Duration::from_secs(7)));
}

#[tokio::test]
async fn timeout_maps() {
    let mut slow = reply(200, fixture("healthz.json"));
    slow.delay = Some(Duration::from_secs(2));
    let (base, _) = server(vec![slow]).await;
    let e = client(&base, 0).health().await.unwrap_err();
    assert_eq!(e.kind, ErrorKind::Timeout);
}

#[tokio::test]
async fn network_error_maps() {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    drop(l);
    let e = client(&base, 0).health().await.unwrap_err();
    assert_eq!(e.kind, ErrorKind::Network);
}
