// One-shot check of a token, pool, v4 pool id or DexScreener / GeckoTerminal link.
//
//   RUGTRIP_API_URL=http://127.0.0.1:8742 node check.ts <token | pool | chart link>
import { RugTripClient, RugTripError, gate } from "@rugtrip/sdk";

const target = process.argv[2];
if (!target) {
  console.error("usage: node check.ts <token | pool | v4 pool id | chart link>");
  process.exit(2);
}

try {
  const rt = new RugTripClient();
  const t = await rt.check(target);
  console.log(`${t.symbol} ${t.token} (${t.pools.length} pool${t.pools.length === 1 ? "" : "s"})`);
  for (const v of t.pools) {
    const g = gate(v);
    const liq = v.liq_usd === null ? "liq ?" : `$${Math.round(v.liq_usd)}`;
    console.log(`\n  ${v.venue} ${v.pool} ${v.rating} ${liq} -> ${g.decision}`);
    for (const r of g.reasons) console.log(`    [${r.level}] ${r.text}`);
    for (const h of g.hits) console.log(`    policy: ${h.text}`);
    for (const u of g.unknowns) console.log(`    not checked: ${u}`);
  }
  console.log("\nRatings are risk opinions, not guarantees.");
} catch (e) {
  if (e instanceof RugTripError) {
    console.error(`${e.code}: ${e.message}`);
    process.exit(e.code === "config" ? 2 : 1);
  }
  throw e;
}
