import asyncio

import httpx
import pytest
from conftest import load

from rugtrip import AsyncClient, Client, ConfigError, RugTripError, to_target
from rugtrip._common import backoff_s
from rugtrip.errors import parse_retry_after

BASE = "http://127.0.0.1:8742"
POOL_ID = "0x3e767ad60e19334d08952bed1b1a88c722d55eddfeaaab50b695330c260a4322"


def scripted(responses, calls):
    def handler(request: httpx.Request):
        calls.append(request)
        r = responses.pop(0)
        if isinstance(r, Exception):
            raise r
        return r

    return handler


def test_base_url_required(monkeypatch):
    with pytest.raises(ConfigError):
        Client()
    with pytest.raises(ConfigError):
        Client("ftp://x")
    monkeypatch.setenv("RUGTRIP_API_URL", BASE + "/")
    assert Client().base_url == BASE


def test_check_sends_ua_and_parses():
    calls = []
    body = load("token.json")
    c = Client(BASE, transport=httpx.MockTransport(scripted([httpx.Response(200, json=body)], calls)))
    t = c.check(body["token"])
    assert t.best.pool == body["best"]["pool"]
    assert str(calls[0].url) == f"{BASE}/v1/token/{body['token']}"
    assert calls[0].headers["user-agent"].startswith("rugtrip-sdk-python/")


def test_targets_and_feed_query():
    assert to_target(f"https://dexscreener.com/robinhood/{POOL_ID}?x=1") == POOL_ID
    assert to_target("hello world") == "hello%20world"
    calls = []
    c = Client(BASE, transport=httpx.MockTransport(scripted([httpx.Response(200, json={"pools": []})], calls)))
    assert c.feed_recent(rating="CAUTION", venue="v4", limit=10, after=5, include_late=True) == []
    assert calls[0].url.query.decode() == "rating=CAUTION&venue=v4&limit=10&after=5&include_late=1"


def test_error_codes():
    c = Client(
        BASE,
        max_retries=0,
        transport=httpx.MockTransport(
            scripted([httpx.Response(400, json=load("error_bad_address.json")), httpx.Response(404, json=load("error_unknown_token.json")), httpx.Response(500, text="oops")], [])
        ),
    )
    with pytest.raises(RugTripError) as e:
        c.check("0x1234")
    assert e.value.code == "bad_address" and e.value.status == 400
    with pytest.raises(RugTripError) as e:
        c.pool(POOL_ID)
    assert e.value.code == "unknown_token" and "haven't seen" in e.value.message
    with pytest.raises(RugTripError) as e:
        c.stats()
    assert e.value.code == "http_error" and e.value.status == 500


def test_rate_limit_retry_then_success():
    calls = []
    rl = load("error_rate_limited.json")
    c = Client(BASE, max_retries=2, retry_base_s=0.001, transport=httpx.MockTransport(scripted([httpx.Response(429, json=rl, headers={"retry-after": "0"}), httpx.Response(429, json=rl), httpx.Response(200, json=load("stats.json"))], calls)))
    assert c.stats().pools_24h >= 0
    assert len(calls) == 3


def test_rate_limit_surfaces():
    rl = load("error_rate_limited.json")
    c = Client(BASE, max_retries=1, retry_base_s=0.001, transport=httpx.MockTransport(scripted([httpx.Response(429, json=rl), httpx.Response(429, json=rl, headers={"retry-after": "7"})], [])))
    with pytest.raises(RugTripError) as e:
        c.stats()
    assert e.value.code == "rate_limited" and e.value.retryable and e.value.retry_after_s == 7


def test_network_and_timeout():
    c = Client(BASE, max_retries=1, retry_base_s=0.001, transport=httpx.MockTransport(scripted([httpx.ConnectError("boom"), httpx.Response(200, json=load("healthz.json"))], [])))
    assert isinstance(c.health().ok, bool)
    t = Client(BASE, max_retries=0, transport=httpx.MockTransport(scripted([httpx.ReadTimeout("slow")], [])))
    with pytest.raises(RugTripError) as e:
        t.health()
    assert e.value.code == "timeout"


def test_async_client():
    async def main():
        calls = []
        rl = load("error_rate_limited.json")
        handler = scripted([httpx.Response(429, json=rl, headers={"retry-after": "0"}), httpx.Response(200, json=load("token_lower_risk.json"))], calls)
        async with AsyncClient(BASE, retry_base_s=0.001, transport=httpx.MockTransport(handler)) as c:
            t = await c.check(f"https://www.geckoterminal.com/robinhood/pools/{POOL_ID}")
            assert t.best.rating == "LOWER_RISK"
        assert str(calls[0].url).endswith(POOL_ID)

    asyncio.run(main())


def test_backoff_and_retry_after():
    assert backoff_s(0, 1, 30, rand=0) == 0.5
    assert backoff_s(0, 1, 30, rand=1) == 1
    assert backoff_s(20, 1, 30, rand=1) == 30
    assert parse_retry_after("3") == 3
    assert parse_retry_after("Thu, 01 Jan 1970 00:00:10 GMT", now=0) == 10
    assert parse_retry_after("soon") is None
