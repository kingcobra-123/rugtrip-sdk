//! RugTrip SDK: rug-risk verdicts for Robinhood Chain pools, a pre-trade policy gate and a live verdict/pull stream.
//!
//! Ratings are risk opinions, not guarantees. Never treat `LOWER_RISK` as safe.
//!
//! ```no_run
//! # async fn demo() -> rugtrip::Result<()> {
//! let rt = rugtrip::Client::from_env()?; // RUGTRIP_API_URL, or Client::new(url)
//! let t = rt.check("0x...").await?;
//! let g = rugtrip::gate(&t.best, &rugtrip::Policy::default());
//! println!("{} {:?}", g.decision.as_str(), g.hits);
//! # Ok(()) }
//! ```
mod address;
mod client;
mod error;
mod policy;
mod stream;
mod types;
mod util;

pub use address::to_target;
pub use client::{user_agent, Client, ClientBuilder, VERSION};
pub use error::{Error, ErrorKind, Result};
pub use policy::{gate, should_skip, Decision, GateResult, Policy, PolicyHit, Unknowns};
pub use stream::{parse_frame, stream_url, EventStream, StreamEvent, StreamOptions};
pub use types::*;
pub use util::{backoff, backoff_with, resolve_base_url};
