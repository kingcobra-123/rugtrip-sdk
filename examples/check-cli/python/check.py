"""One-shot check of a token, pool, v4 pool id or DexScreener / GeckoTerminal link.

    RUGTRIP_API_URL=http://127.0.0.1:8742 python check.py <token | pool | chart link>
"""

import sys

from rugtrip import Client, RugTripError, gate


def main() -> int:
    if len(sys.argv) < 2:
        print("usage: python check.py <token | pool | v4 pool id | chart link>", file=sys.stderr)
        return 2
    try:
        with Client() as rt:
            t = rt.check(sys.argv[1])
    except RugTripError as e:
        print(f"{e.code}: {e.message}", file=sys.stderr)
        return 2 if e.code == "config" else 1
    print(f"{t.symbol} {t.token} ({len(t.pools)} pool{'' if len(t.pools) == 1 else 's'})")
    for v in t.pools:
        g = gate(v)
        liq = "liq ?" if v.liq_usd is None else f"${v.liq_usd:,.0f}"
        print(f"\n  {v.venue} {v.pool} {v.rating} {liq} -> {g.decision}")
        for r in g.reasons:
            print(f"    [{r.level}] {r.text}")
        for h in g.hits:
            print(f"    policy: {h.text}")
        for u in g.unknowns:
            print(f"    not checked: {u}")
    print("\nRatings are risk opinions, not guarantees.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
