# rugtrip (Rust)

Async Rust client for the RugTrip API: rug-risk verdicts for Robinhood Chain pools, a pre-trade policy gate and a
live verdict/pull stream. reqwest (rustls) + tokio-tungstenite, serde types.

```rust
let rt = rugtrip::Client::new("http://127.0.0.1:8742")?; // or Client::from_env() with RUGTRIP_API_URL
let t = rt.check("0x...").await?;                       // token, pool, v4 pool id or chart link
let g = rugtrip::gate(&t.best, &rugtrip::Policy::default());
println!("{:?} {:?}", g.decision, g.hits);
```

Ratings are risk opinions, not guarantees. Never treat LOWER_RISK as safe.
See the repository README for the policy helper, the stream client, rate limits and error codes.
