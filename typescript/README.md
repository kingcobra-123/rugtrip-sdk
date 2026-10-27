# @rugtrip/sdk

TypeScript / JavaScript client for the RugTrip API: rug-risk verdicts for Robinhood Chain pools, a pre-trade policy
gate and a live verdict/pull stream. Zero runtime dependencies: global `fetch` (Node >= 18) and global `WebSocket`
(Node >= 22, browsers), or the optional `ws` package for the stream on older Node.

```ts
import { RugTripClient, gate } from "@rugtrip/sdk";

const rt = new RugTripClient({ baseUrl: "http://127.0.0.1:8742" }); // or RUGTRIP_API_URL; no default endpoint
const t = await rt.check("0x…");                                     // token, pool, v4 pool id or chart link
const g = gate(t.best);                                              // conservative default policy
console.log(g.decision, g.hits.map((h) => h.text));
```

Ratings are risk opinions, not guarantees. Never treat LOWER_RISK as safe.
See the repository README for the policy helper, the stream client, rate limits and error codes.
