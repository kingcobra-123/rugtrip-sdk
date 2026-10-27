"""Opt-in: runs only when RUGTRIP_API_URL is set. Keeps traffic light."""

import asyncio
import os

import pytest

from rugtrip import RATINGS, AsyncClient, Client, Connected, RugTripError

URL = os.environ.get("RUGTRIP_API_URL")
pytestmark = pytest.mark.skipif(not URL, reason="set RUGTRIP_API_URL to run")


def test_live_rest():
    with Client(URL) as c:
        assert isinstance(c.health().ok, bool)
        feed = c.feed_recent(limit=3)
        if feed:
            t = c.check(feed[0].token)
            assert t.best.rating in RATINGS
            assert c.pool(feed[0].pool).pool == feed[0].pool
        with pytest.raises(RugTripError) as e:
            c.check("0x1234")
        assert e.value.code == "bad_address"
        with pytest.raises(RugTripError) as e:
            c.check("0x0000000000000000000000000000000000000001")
        assert e.value.code == "unknown_token"


def test_live_stream_connects():
    async def main():
        async with AsyncClient(URL) as c:
            s = c.stream(max_reconnects=0)

            async def first():
                async for ev in s:
                    assert isinstance(ev, Connected)
                    await s.close()

            await asyncio.wait_for(first(), 10)

    asyncio.run(main())
