use std::time::Duration;

/// Error kinds. The first group comes from the API's `{"error": code}` body; the rest are raised by the SDK.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    BadAddress,
    UnknownToken,
    RateLimited,
    BadRequest,
    NotFound,
    Unauthorized,
    ProRequired,
    Internal,
    /// an HTTP error without a known API code
    Http,
    BadResponse,
    Timeout,
    Network,
    Config,
}

impl ErrorKind {
    pub fn code(&self) -> &'static str {
        match self {
            ErrorKind::BadAddress => "bad_address",
            ErrorKind::UnknownToken => "unknown_token",
            ErrorKind::RateLimited => "rate_limited",
            ErrorKind::BadRequest => "bad_request",
            ErrorKind::NotFound => "not_found",
            ErrorKind::Unauthorized => "unauthorized",
            ErrorKind::ProRequired => "pro_required",
            ErrorKind::Internal => "internal",
            ErrorKind::Http => "http_error",
            ErrorKind::BadResponse => "bad_response",
            ErrorKind::Timeout => "timeout",
            ErrorKind::Network => "network",
            ErrorKind::Config => "config",
        }
    }
    fn from_api(code: &str) -> Option<ErrorKind> {
        Some(match code {
            "bad_address" => ErrorKind::BadAddress,
            "unknown_token" => ErrorKind::UnknownToken,
            "rate_limited" => ErrorKind::RateLimited,
            "bad_request" => ErrorKind::BadRequest,
            "not_found" => ErrorKind::NotFound,
            "unauthorized" => ErrorKind::Unauthorized,
            "pro_required" => ErrorKind::ProRequired,
            "internal" => ErrorKind::Internal,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("{} ({}): {message}", kind.code(), status.map(|s| s.to_string()).unwrap_or_else(|| "-".into()))]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
    /// HTTP status, when the error came from a response
    pub status: Option<u16>,
    /// server-suggested wait (Retry-After)
    pub retry_after: Option<Duration>,
    /// the raw API code string (may be one this SDK version doesn't know)
    pub api_code: Option<String>,
}

impl Error {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Error {
        Error {
            kind,
            message: message.into(),
            status: None,
            retry_after: None,
            api_code: None,
        }
    }
    pub fn code(&self) -> &'static str {
        self.kind.code()
    }
    /// true for errors that may succeed when retried later
    pub fn retryable(&self) -> bool {
        matches!(self.kind, ErrorKind::RateLimited | ErrorKind::Timeout | ErrorKind::Network) || matches!(self.status, Some(502..=504))
    }
    pub(crate) fn from_response(status: u16, body: &[u8], retry_after: Option<Duration>) -> Error {
        if let Ok(b) = serde_json::from_slice::<crate::types::ErrorBody>(body) {
            let kind = ErrorKind::from_api(&b.error).unwrap_or(if status == 429 { ErrorKind::RateLimited } else { ErrorKind::Http });
            let message = if b.message.is_empty() { b.error.clone() } else { b.message };
            return Error {
                kind,
                message,
                status: Some(status),
                retry_after,
                api_code: Some(b.error),
            };
        }
        let kind = if status == 429 { ErrorKind::RateLimited } else { ErrorKind::Http };
        Error {
            kind,
            message: format!("HTTP {status}"),
            status: Some(status),
            retry_after,
            api_code: None,
        }
    }
}

/// Retry-After in seconds (the HTTP-date form is not parsed).
pub(crate) fn parse_retry_after(v: Option<&str>) -> Option<Duration> {
    let s = v?.trim();
    s.parse::<f64>().ok().filter(|x| x.is_finite() && *x >= 0.0).map(Duration::from_secs_f64)
}

pub type Result<T> = std::result::Result<T, Error>;
