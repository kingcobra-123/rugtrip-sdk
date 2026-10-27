use crate::address::to_target;
use crate::error::{parse_retry_after, Error, ErrorKind, Result};
use crate::stream::{EventStream, StreamOptions};
use crate::types::*;
use crate::util::{backoff, resolve_base_url};
use serde::de::DeserializeOwned;
use std::time::Duration;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// `rugtrip-sdk-rust/<version>`
pub fn user_agent() -> String {
    format!("rugtrip-sdk-rust/{VERSION}")
}

/// Builder for [`Client`].
#[derive(Debug, Clone)]
pub struct ClientBuilder {
    base_url: Option<String>,
    timeout: Duration,
    max_retries: u32,
    retry_base: Duration,
    headers: Vec<(String, String)>,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        ClientBuilder {
            base_url: None,
            timeout: Duration::from_secs(10),
            max_retries: 2,
            retry_base: Duration::from_secs(1),
            headers: vec![],
        }
    }
}

impl ClientBuilder {
    /// API base URL (http or https). Required (or env RUGTRIP_API_URL). There is no default.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }
    /// per-request timeout (default 10 s)
    pub fn timeout(mut self, t: Duration) -> Self {
        self.timeout = t;
        self
    }
    /// retries for rate limits, timeouts, network errors and 502/503/504 (default 2)
    pub fn max_retries(mut self, n: u32) -> Self {
        self.max_retries = n;
        self
    }
    /// first retry wait when the server gives no Retry-After (default 1 s; doubles, capped at 30 s, jittered)
    pub fn retry_base(mut self, d: Duration) -> Self {
        self.retry_base = d;
        self
    }
    /// an extra header sent with every request
    pub fn header(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.headers.push((k.into(), v.into()));
        self
    }
    pub fn build(self) -> Result<Client> {
        let base = resolve_base_url(self.base_url.as_deref())?;
        let mut hm = reqwest::header::HeaderMap::new();
        hm.insert(reqwest::header::ACCEPT, reqwest::header::HeaderValue::from_static("application/json"));
        for (k, v) in &self.headers {
            let name = reqwest::header::HeaderName::from_bytes(k.as_bytes()).map_err(|e| Error::new(ErrorKind::Config, format!("bad header name {k}: {e}")))?;
            let val = reqwest::header::HeaderValue::from_str(v).map_err(|e| Error::new(ErrorKind::Config, format!("bad header value for {k}: {e}")))?;
            hm.insert(name, val);
        }
        let http = reqwest::Client::builder()
            .user_agent(user_agent())
            .timeout(self.timeout)
            .default_headers(hm)
            .build()
            .map_err(|e| Error::new(ErrorKind::Config, format!("http client: {e}")))?;
        Ok(Client {
            http,
            base,
            max_retries: self.max_retries,
            retry_base: self.retry_base,
            headers: self.headers,
        })
    }
}

/// Async client for the public RugTrip API.
#[derive(Debug, Clone)]
pub struct Client {
    http: reqwest::Client,
    base: String,
    max_retries: u32,
    retry_base: Duration,
    headers: Vec<(String, String)>,
}

fn q<T: ToString>(out: &mut Vec<(&'static str, String)>, k: &'static str, v: Option<T>) {
    if let Some(v) = v {
        out.push((k, v.to_string()));
    }
}

impl Client {
    /// Client for `base_url` with default settings.
    pub fn new(base_url: impl Into<String>) -> Result<Client> {
        ClientBuilder::default().base_url(base_url).build()
    }
    /// Client for env RUGTRIP_API_URL.
    pub fn from_env() -> Result<Client> {
        ClientBuilder::default().build()
    }
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }
    pub fn base_url(&self) -> &str {
        &self.base
    }

    /// `GET /healthz`
    pub async fn health(&self) -> Result<Health> {
        self.get("/healthz", &[]).await
    }

    /// Check a token before buying: a token address, pool address, v4 pool id or DexScreener / GeckoTerminal
    /// link. `best` is the most liquid pool; gate on the pool you will actually trade through.
    pub async fn check(&self, target: &str) -> Result<TokenCheck> {
        self.get(&format!("/v1/token/{}", to_target(target)), &[]).await
    }

    /// `GET /v1/pool/:addr`: one pool's verdict (20-byte pool address or 32-byte v4 pool id).
    pub async fn pool(&self, addr: &str) -> Result<Verdict> {
        self.get(&format!("/v1/pool/{}", to_target(addr)), &[]).await
    }

    /// `GET /v1/feed/recent`: newest pools with verdicts (unauthenticated: 60 s delayed).
    pub async fn feed_recent(&self, fq: &FeedQuery) -> Result<Vec<Verdict>> {
        let mut p = vec![];
        q(&mut p, "rating", fq.rating.map(|r| r.as_str()));
        q(&mut p, "venue", fq.venue.map(|v| v.as_str()));
        q(&mut p, "limit", fq.limit);
        q(&mut p, "before", fq.before);
        q(&mut p, "after", fq.after);
        q(&mut p, "include_late", fq.include_late.then_some("1"));
        let r: FeedResponse = self.get("/v1/feed/recent", &p).await?;
        Ok(r.pools)
    }

    /// `GET /v1/rugs/today`: liquidity pulls in the last 24 h, newest first.
    pub async fn rugs_today(&self, rq: &RugsQuery) -> Result<RugsToday> {
        let mut p = vec![];
        q(&mut p, "since", rq.since);
        q(&mut p, "before", rq.before);
        q(&mut p, "limit", rq.limit);
        self.get("/v1/rugs/today", &p).await
    }

    /// `GET /v1/scorecard`
    pub async fn scorecard(&self) -> Result<Scorecard> {
        self.get("/v1/scorecard", &[]).await
    }

    /// `GET /v1/stats`
    pub async fn stats(&self) -> Result<Stats> {
        self.get("/v1/stats", &[]).await
    }

    /// A `/v1/stream` client with auto-reconnect. Unauthenticated connections receive events 60 s late.
    pub fn stream(&self, opts: StreamOptions) -> Result<EventStream> {
        let mut headers = vec![("User-Agent".to_string(), user_agent())];
        headers.extend(self.headers.iter().cloned());
        EventStream::new(&self.base, headers, opts)
    }

    async fn get<T: DeserializeOwned>(&self, path: &str, query: &[(&'static str, String)]) -> Result<T> {
        let url = format!("{}{}", self.base, path);
        let mut attempt = 0u32;
        loop {
            match self.once::<T>(&url, query).await {
                Ok(v) => return Ok(v),
                Err(e) if e.retryable() && attempt < self.max_retries => {
                    let wait = e.retry_after.unwrap_or_else(|| backoff(attempt, self.retry_base, Duration::from_secs(30)));
                    tokio::time::sleep(wait).await;
                    attempt += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }

    async fn once<T: DeserializeOwned>(&self, url: &str, query: &[(&'static str, String)]) -> Result<T> {
        let map_err = |e: reqwest::Error| {
            if e.is_timeout() {
                Error::new(ErrorKind::Timeout, "request timed out")
            } else {
                Error::new(ErrorKind::Network, format!("network error: {e}"))
            }
        };
        let mut rb = self.http.get(url);
        if !query.is_empty() {
            rb = rb.query(query);
        }
        let res = rb.send().await.map_err(map_err)?;
        let status = res.status().as_u16();
        let retry_after = parse_retry_after(res.headers().get(reqwest::header::RETRY_AFTER).and_then(|v| v.to_str().ok()));
        let body = res.bytes().await.map_err(map_err)?;
        if status >= 400 {
            return Err(Error::from_response(status, &body, retry_after));
        }
        serde_json::from_slice(&body).map_err(|e| {
            let mut err = Error::new(ErrorKind::BadResponse, format!("unexpected JSON from {url}: {e}"));
            err.status = Some(status);
            err
        })
    }
}
