//! `/v1/stream` WebSocket client with auto-reconnect.
//!
//! Unauthenticated connections receive `verdict` and `pull` events 60 s after they happen; real-time delivery comes
//! with API plans. Event-driven: call [`EventStream::next`] in a loop and react to each event as it arrives.
use crate::error::{Error, ErrorKind, Result};
use crate::types::{Alert, Pull, Verdict};
use crate::util::{backoff, resolve_base_url};
use futures_util::{SinkExt, StreamExt};
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::{HeaderName, HeaderValue};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Debug, Clone)]
pub struct StreamOptions {
    /// first reconnect wait (default 500 ms)
    pub initial_backoff: Duration,
    /// reconnect wait cap (default 30 s)
    pub max_backoff: Duration,
    /// give up after this many consecutive failed attempts (default: never)
    pub max_reconnects: Option<u32>,
    /// connect + handshake timeout (default 10 s)
    pub open_timeout: Duration,
    /// send a ping after this long without frames (default 20 s; None disables pings and the idle check)
    pub ping_interval: Option<Duration>,
    /// reconnect when nothing (frames or pongs) arrives for this long (default 60 s)
    pub idle_timeout: Duration,
}

impl Default for StreamOptions {
    fn default() -> Self {
        StreamOptions {
            initial_backoff: Duration::from_millis(500),
            max_backoff: Duration::from_secs(30),
            max_reconnects: None,
            open_timeout: Duration::from_secs(10),
            ping_interval: Some(Duration::from_secs(20)),
            idle_timeout: Duration::from_secs(60),
        }
    }
}

/// One stream event.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent {
    /// connected (`attempt` = consecutive failures before this connection)
    Connected {
        attempt: u32,
    },
    Verdict(Verdict),
    Pull(Pull),
    /// only sent to real-time (paid plan) connections
    Alert(Alert),
    /// a frame type this SDK version doesn't know (forward compatibility)
    Unknown {
        kind: String,
        data: serde_json::Value,
    },
    /// the connection dropped; the next call to `next()` waits `delay` and reconnects
    Reconnecting {
        attempt: u32,
        delay: Duration,
        reason: String,
    },
}

/// Parse one text frame. None for malformed frames.
pub fn parse_frame(raw: &str) -> Option<StreamEvent> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    let kind = v.get("type")?.as_str()?.to_string();
    let data = v.get("data").cloned().unwrap_or(serde_json::Value::Null);
    match kind.as_str() {
        "verdict" => serde_json::from_value(data).ok().map(StreamEvent::Verdict),
        "pull" => serde_json::from_value(data).ok().map(StreamEvent::Pull),
        "alert" => serde_json::from_value(data).ok().map(StreamEvent::Alert),
        _ => Some(StreamEvent::Unknown { kind, data }),
    }
}

/// http(s)://host[/prefix] -> ws(s)://host[/prefix]/v1/stream
pub fn stream_url(base_url: &str) -> Result<String> {
    let base = resolve_base_url(Some(base_url))?;
    let rest = base.split_once("://").map(|x| x.1).unwrap_or(&base);
    let scheme = if base.starts_with("https://") { "wss" } else { "ws" };
    Ok(format!("{scheme}://{rest}/v1/stream"))
}

/// Auto-reconnecting stream. Create it with [`crate::Client::stream`] or [`EventStream::new`].
pub struct EventStream {
    url: String,
    headers: Vec<(String, String)>,
    opts: StreamOptions,
    ws: Option<Ws>,
    attempt: u32,
    delay: Option<Duration>,
    got_frame: bool,
    last_seen: Instant,
    closed: bool,
}

impl EventStream {
    pub fn new(base_url: &str, headers: Vec<(String, String)>, opts: StreamOptions) -> Result<EventStream> {
        Ok(EventStream {
            url: stream_url(base_url)?,
            headers,
            opts,
            ws: None,
            attempt: 0,
            delay: None,
            got_frame: false,
            last_seen: Instant::now(),
            closed: false,
        })
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn is_connected(&self) -> bool {
        self.ws.is_some()
    }

    /// Close the connection; `next()` returns `Ok(None)` afterwards.
    pub async fn close(&mut self) {
        self.closed = true;
        if let Some(mut ws) = self.ws.take() {
            let _ = ws.close(None).await;
        }
    }

    /// The next event. `Ok(None)` after `close()`; `Err` only when `max_reconnects` is used up.
    pub async fn next(&mut self) -> Result<Option<StreamEvent>> {
        loop {
            if self.closed {
                return Ok(None);
            }
            if let Some(d) = self.delay.take() {
                tokio::time::sleep(d).await;
            }
            if self.ws.is_none() {
                return match self.connect().await {
                    Ok(ws) => {
                        self.ws = Some(ws);
                        self.got_frame = false;
                        self.last_seen = Instant::now();
                        Ok(Some(StreamEvent::Connected { attempt: self.attempt }))
                    }
                    Err(reason) => self.reconnect(reason),
                };
            }
            let ws = self.ws.as_mut().expect("connected");
            let msg = match self.opts.ping_interval {
                Some(every) => match tokio::time::timeout(every, ws.next()).await {
                    Ok(m) => m,
                    Err(_) => {
                        if self.last_seen.elapsed() >= self.opts.idle_timeout {
                            return self.reconnect(format!("no frames for {:?}", self.opts.idle_timeout));
                        }
                        if ws.send(Message::Ping(Default::default())).await.is_err() {
                            return self.reconnect("ping failed".into());
                        }
                        continue;
                    }
                },
                None => ws.next().await,
            };
            let text = match msg {
                Some(Ok(Message::Text(t))) => t.as_str().to_string(),
                Some(Ok(Message::Binary(b))) => String::from_utf8_lossy(&b).into_owned(),
                Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => {
                    self.last_seen = Instant::now();
                    continue;
                }
                Some(Ok(Message::Close(f))) => {
                    return self.reconnect(format!(
                        "closed ({})",
                        f.map(|f| u16::from(f.code).to_string()).unwrap_or_else(|| "no code".into())
                    ))
                }
                Some(Err(e)) => return self.reconnect(format!("websocket error: {e}")),
                None => return self.reconnect("closed".into()),
            };
            self.last_seen = Instant::now();
            if !self.got_frame {
                self.got_frame = true;
                self.attempt = 0; // healthy connection: reset backoff
            }
            if let Some(ev) = parse_frame(&text) {
                return Ok(Some(ev));
            }
        }
    }

    async fn connect(&self) -> std::result::Result<Ws, String> {
        let mut req = self.url.as_str().into_client_request().map_err(|e| format!("bad stream URL: {e}"))?;
        for (k, v) in &self.headers {
            if let (Ok(n), Ok(v)) = (HeaderName::from_bytes(k.as_bytes()), HeaderValue::from_str(v)) {
                req.headers_mut().insert(n, v);
            }
        }
        match tokio::time::timeout(self.opts.open_timeout, tokio_tungstenite::connect_async(req)).await {
            Ok(Ok((ws, _))) => Ok(ws),
            Ok(Err(e)) => Err(format!("connect: {e}")),
            Err(_) => Err("connect timed out".into()),
        }
    }

    fn reconnect(&mut self, reason: String) -> Result<Option<StreamEvent>> {
        self.ws = None;
        if let Some(max) = self.opts.max_reconnects {
            if self.attempt >= max {
                self.closed = true;
                return Err(Error::new(
                    ErrorKind::Network,
                    format!("gave up after {} reconnect attempts ({reason})", self.attempt),
                ));
            }
        }
        let delay = backoff(self.attempt, self.opts.initial_backoff, self.opts.max_backoff);
        self.attempt += 1;
        self.delay = Some(delay);
        Ok(Some(StreamEvent::Reconnecting {
            attempt: self.attempt,
            delay,
            reason,
        }))
    }
}
