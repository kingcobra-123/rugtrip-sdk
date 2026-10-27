// Gate every new pool from the RugTrip stream with your policy and print BUY-CANDIDATE / SKIP with plain-word
// reasons. No trading code, no keys: plug your own buy logic in where it says so.
//
//   RUGTRIP_API_URL=http://127.0.0.1:8742 node sniper-gate.ts [--max-rating CAUTION] [--allow-unknown-sim] [--min-liq 1000]
//
// Unauthenticated stream connections receive events 60 s after they happen; real-time comes with API plans.
import { RugTripClient, gate, type Policy, type Rating } from "@rugtrip/sdk";

function flag(name: string): string | undefined {
  const i = process.argv.indexOf(name);
  return i >= 0 ? process.argv[i + 1] : undefined;
}

const policy: Partial<Policy> = {};
if (flag("--max-rating")) policy.maxRating = flag("--max-rating") as Rating;
if (process.argv.includes("--allow-unknown-sim")) policy.treatUnknownSimAs = "allow";
if (flag("--min-liq")) policy.minLiqUsd = Number(flag("--min-liq"));

const rt = new RugTripClient(); // baseUrl from RUGTRIP_API_URL; there is no default endpoint
const stream = rt.stream();

stream
  .on("open", ({ url }) => console.error(`connected to ${url} (unauthenticated: events arrive 60 s late)`))
  .on("verdict", (v) => {
    if (v.version !== 1) return; // only new pools; re-ratings have version > 1
    const g = gate(v, policy);
    const liq = v.liq_usd === null ? "liq ?" : `$${Math.round(v.liq_usd)}`;
    console.log(`${g.decision === "BUY_CANDIDATE" ? "BUY-CANDIDATE" : "SKIP         "} ${(v.symbol || "?").padEnd(12)} ${v.venue} ${v.rating.padEnd(10)} ${liq.padEnd(8)} ${v.pool}`);
    if (g.skip) {
      for (const h of g.hits) console.log(`    - ${h.text}`);
    } else {
      for (const r of g.reasons.filter((r) => r.level !== "ok")) console.log(`    ~ ${r.text}`);
      for (const u of g.unknowns) console.log(`    ? ${u}`);
      // your buy logic goes here (size, slippage, keys: all yours)
    }
  })
  .on("pull", (p) => console.log(`PULLED        ${(p.symbol || "?").padEnd(12)} ${p.venue} after ${p.life_s} s ${p.pool}`))
  .on("reconnecting", ({ attempt, delayMs, reason }) => console.error(`reconnecting #${attempt} in ${delayMs} ms (${reason})`))
  .on("error", (e) => console.error(`stream error: ${e.message}`))
  .start();

process.on("SIGINT", () => {
  stream.close();
  process.exit(0);
});
