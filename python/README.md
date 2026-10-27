# rugtrip (Python)

Python client for the RugTrip API: rug-risk verdicts for Robinhood Chain pools, a pre-trade policy gate and a live
verdict/pull stream. Sync (`Client`) and async (`AsyncClient`) over httpx; stream over `websockets`.

```python
from rugtrip import Client, gate

rt = Client(base_url="http://127.0.0.1:8742")  # or set RUGTRIP_API_URL; there is no default endpoint
t = rt.check("0x...")                          # token, pool, v4 pool id or chart link
g = gate(t.best)                               # conservative default policy
print(g.decision, [h.text for h in g.hits])
```

Ratings are risk opinions, not guarantees. Never treat LOWER_RISK as safe.
See the repository README for the policy helper, the stream client, rate limits and error codes.
