mod common;
use common::fixture;
use futures_util::{SinkExt, StreamExt};
use rugtrip::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
#[allow(clippy::result_large_err)]
async fn events_reconnect_and_close() {
    let frames: Vec<String> = fixture("stream.jsonl").lines().filter(|l| !l.trim().is_empty()).map(String::from).collect();
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let agents = Arc::new(Mutex::new(vec![]));
    let agents2 = agents.clone();
    let frames2 = frames.clone();
    tokio::spawn(async move {
        for i in 0..2 {
            let (s, _) = l.accept().await.unwrap();
            let a = agents2.clone();
            let cb = move |req: &Request, resp: Response| {
                a.lock()
                    .unwrap()
                    .push(req.headers().get("user-agent").and_then(|v| v.to_str().ok()).unwrap_or("").to_string());
                Ok(resp)
            };
            let mut ws = tokio_tungstenite::accept_hdr_async(s, cb).await.unwrap();
            if i == 0 {
                for f in &frames2 {
                    ws.send(Message::text(f.clone())).await.unwrap();
                }
                ws.send(Message::text("garbage")).await.unwrap();
                ws.close(None).await.unwrap();
            } else {
                ws.send(Message::text(frames2[0].clone())).await.unwrap();
                while let Some(Ok(_)) = ws.next().await {}
            }
        }
    });
    let c = Client::new(&base).unwrap();
    let mut s = c
        .stream(StreamOptions {
            initial_backoff: Duration::from_millis(10),
            max_backoff: Duration::from_millis(50),
            ..Default::default()
        })
        .unwrap();
    let mut seen = vec![];
    let mut connects = 0;
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(ev) = s.next().await.unwrap() {
            if matches!(ev, StreamEvent::Connected { .. }) {
                connects += 1;
            }
            let data = matches!(ev, StreamEvent::Verdict(_) | StreamEvent::Pull(_));
            seen.push(ev);
            if connects == 2 && data {
                s.close().await;
            }
        }
    })
    .await
    .unwrap();
    assert!(matches!(seen[0], StreamEvent::Connected { attempt: 0 }));
    assert!(seen.iter().any(|e| matches!(e, StreamEvent::Verdict(_))));
    assert!(seen.iter().any(|e| matches!(e, StreamEvent::Pull(_))));
    assert!(seen
        .iter()
        .any(|e| matches!(e, StreamEvent::Unknown { kind, .. } if kind == "hello_from_the_future")));
    let rec: Vec<_> = seen
        .iter()
        .filter_map(|e| {
            if let StreamEvent::Reconnecting { attempt, delay, .. } = e {
                Some((*attempt, *delay))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(rec.len(), 1);
    assert_eq!(rec[0].0, 1);
    assert!(rec[0].1 >= Duration::from_millis(5) && rec[0].1 <= Duration::from_millis(10), "{:?}", rec[0].1);
    assert!(agents.lock().unwrap()[0].starts_with("rugtrip-sdk-rust/"));
    assert!(s.next().await.unwrap().is_none(), "closed stream yields None");
}

#[tokio::test]
async fn gives_up_after_max_reconnects() {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    drop(l);
    let mut s = Client::new(&base)
        .unwrap()
        .stream(StreamOptions {
            initial_backoff: Duration::from_millis(1),
            max_backoff: Duration::from_millis(2),
            max_reconnects: Some(2),
            ..Default::default()
        })
        .unwrap();
    let mut attempts = vec![];
    let err = loop {
        match s.next().await {
            Ok(Some(StreamEvent::Reconnecting { attempt, .. })) => attempts.push(attempt),
            Ok(Some(_)) => {}
            Ok(None) => panic!("unexpected close"),
            Err(e) => break e,
        }
    };
    assert_eq!(attempts, vec![1, 2]);
    assert_eq!(err.kind, ErrorKind::Network);
}
