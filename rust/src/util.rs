use crate::error::{Error, ErrorKind, Result};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Resolve the base URL (argument, else env RUGTRIP_API_URL). There is no default endpoint.
pub fn resolve_base_url(base_url: Option<&str>) -> Result<String> {
    let raw = match base_url.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => s.to_string(),
        None => std::env::var("RUGTRIP_API_URL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Config,
                    "base URL is required: pass it or set RUGTRIP_API_URL. The SDK has no default endpoint.",
                )
            })?,
    };
    let u = reqwest::Url::parse(&raw).map_err(|e| Error::new(ErrorKind::Config, format!("base URL is not a valid URL ({e}): {raw}")))?;
    if u.scheme() != "http" && u.scheme() != "https" {
        return Err(Error::new(ErrorKind::Config, "base URL must start with http:// or https://"));
    }
    Ok(raw.trim_end_matches('/').to_string())
}

static SEED: AtomicU64 = AtomicU64::new(0);

/// A pseudo-random number in [0, 1) for jitter (not for anything security related).
fn jitter_unit() -> f64 {
    let mut x = SEED.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed)
        ^ std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1);
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

/// Backoff with jitter: a random wait in [d/2, d], d = min(cap, base * 2^attempt).
pub fn backoff(attempt: u32, base: Duration, cap: Duration) -> Duration {
    backoff_with(attempt, base, cap, jitter_unit())
}

/// `backoff` with an explicit jitter value in [0, 1].
pub fn backoff_with(attempt: u32, base: Duration, cap: Duration, unit: f64) -> Duration {
    let d = base.as_secs_f64() * 2f64.powi(attempt.min(30) as i32);
    let d = d.min(cap.as_secs_f64());
    Duration::from_secs_f64(d / 2.0 + unit.clamp(0.0, 1.0) * d / 2.0)
}
