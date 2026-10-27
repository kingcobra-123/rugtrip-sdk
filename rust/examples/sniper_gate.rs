//! Gate every new pool from the live stream with the default policy and print BUY-CANDIDATE / SKIP.
//! No trading code, no keys. `RUGTRIP_API_URL=http://127.0.0.1:8742 cargo run --example sniper_gate`
use rugtrip::{gate, Client, Decision, Policy, StreamEvent, StreamOptions};

#[tokio::main]
async fn main() {
    let rt = Client::from_env().unwrap_or_else(|e| {
        eprintln!("{}", e.message);
        std::process::exit(2)
    });
    let policy = Policy::default();
    let mut stream = rt.stream(StreamOptions::default()).expect("stream");
    eprintln!("listening on {} (unauthenticated: events arrive 60 s late)", stream.url());
    loop {
        match stream.next().await {
            Ok(Some(StreamEvent::Verdict(v))) if v.version == 1 => {
                let g = gate(&v, &policy);
                let label = if g.decision == Decision::BuyCandidate { "BUY-CANDIDATE" } else { "SKIP" };
                println!("{label:13} {:<12} {} {} {}", v.symbol, v.venue.as_str(), v.rating, v.pool);
                for h in &g.hits {
                    println!("              - {}", h.text);
                }
            }
            Ok(Some(StreamEvent::Pull(p))) => println!("PULLED        {:<12} after {} s  {}", p.symbol, p.life_s, p.pool),
            Ok(Some(StreamEvent::Connected { .. })) => eprintln!("connected"),
            Ok(Some(StreamEvent::Reconnecting { attempt, delay, reason })) => eprintln!("reconnecting #{attempt} in {delay:?}: {reason}"),
            Ok(Some(_)) => {}
            Ok(None) => break,
            Err(e) => {
                eprintln!("{}", e.message);
                break;
            }
        }
    }
}
