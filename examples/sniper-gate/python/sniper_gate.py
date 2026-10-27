"""Gate every new pool from the RugTrip stream with your policy and print BUY-CANDIDATE / SKIP with plain-word
reasons. No trading code, no keys: plug your own buy logic in where it says so.

    RUGTRIP_API_URL=http://127.0.0.1:8742 python sniper_gate.py [--max-rating CAUTION] [--allow-unknown-sim] [--min-liq 1000]

Unauthenticated stream connections receive events 60 s after they happen; real-time comes with API plans.
"""

import argparse
import asyncio
import sys

from rugtrip import AsyncClient, Connected, Policy, PullEvent, Reconnecting, VerdictEvent, gate


async def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--max-rating", default="CAUTION")
    ap.add_argument("--allow-unknown-sim", action="store_true")
    ap.add_argument("--min-liq", type=float, default=None)
    a = ap.parse_args()
    policy = Policy(max_rating=a.max_rating, treat_unknown_sim_as="allow" if a.allow_unknown_sim else "skip", min_liq_usd=a.min_liq)

    async with AsyncClient() as rt:  # base URL from RUGTRIP_API_URL; there is no default endpoint
        async for ev in rt.stream():
            if isinstance(ev, VerdictEvent):
                v = ev.data
                if v.version != 1:  # only new pools; re-ratings have version > 1
                    continue
                g = gate(v, policy)
                liq = "liq ?" if v.liq_usd is None else f"${v.liq_usd:,.0f}"
                label = "BUY-CANDIDATE" if g.decision == "BUY_CANDIDATE" else "SKIP"
                print(f"{label:13} {v.symbol or '?':<12} {v.venue} {v.rating:<10} {liq:<8} {v.pool}", flush=True)
                if g.skip:
                    for h in g.hits:
                        print(f"    - {h.text}")
                else:
                    for r in g.reasons:
                        if r.level != "ok":
                            print(f"    ~ {r.text}")
                    for u in g.unknowns:
                        print(f"    ? {u}")
                    # your buy logic goes here (size, slippage, keys: all yours)
            elif isinstance(ev, PullEvent):
                p = ev.data
                print(f"PULLED        {p.symbol or '?':<12} {p.venue} after {p.life_s} s {p.pool}", flush=True)
            elif isinstance(ev, Connected):
                print(f"connected to {ev.url} (unauthenticated: events arrive 60 s late)", file=sys.stderr)
            elif isinstance(ev, Reconnecting):
                print(f"reconnecting #{ev.attempt} in {ev.delay_s:.1f} s ({ev.reason})", file=sys.stderr)


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        pass
