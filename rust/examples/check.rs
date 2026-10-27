//! One-shot check: `RUGTRIP_API_URL=http://127.0.0.1:8742 cargo run --example check -- <token|pool|chart link>`
use rugtrip::{gate, Client, Policy};

#[tokio::main]
async fn main() {
    let Some(target) = std::env::args().nth(1) else {
        eprintln!("usage: check <token | pool | v4 pool id | chart link>");
        std::process::exit(2);
    };
    let rt = match Client::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e.message);
            std::process::exit(2);
        }
    };
    match rt.check(&target).await {
        Ok(t) => {
            println!("{} {} ({} pool{})", t.symbol, t.token, t.pools.len(), if t.pools.len() == 1 { "" } else { "s" });
            for v in &t.pools {
                let g = gate(v, &Policy::default());
                let liq = v.liq_usd.map(|l| format!("${l:.0}")).unwrap_or_else(|| "liq ?".into());
                println!("\n  {} {} {} {} -> {}", v.venue.as_str(), v.pool, v.rating, liq, g.decision.as_str());
                for r in &g.reasons {
                    println!("    [{}] {}", r.level.as_str(), r.text);
                }
                for h in &g.hits {
                    println!("    policy: {}", h.text);
                }
                for u in &g.unknowns {
                    println!("    not checked: {u}");
                }
            }
            println!("\nRatings are risk opinions, not guarantees.");
        }
        Err(e) => {
            eprintln!("{}: {}", e.code(), e.message);
            std::process::exit(1);
        }
    }
}
