import asyncio

from conftest import load_text
from websockets.asyncio.server import serve

from rugtrip import Connected, PullEvent, Reconnecting, Stream, UnknownEvent, VerdictEvent, stream_url


def test_stream_url():
    assert stream_url("http://127.0.0.1:8742") == "ws://127.0.0.1:8742/v1/stream"
    assert stream_url("https://api.example.test/base/") == "wss://api.example.test/base/v1/stream"


def test_stream_events_and_reconnect():
    frames = [line for line in load_text("stream.jsonl").splitlines() if line.strip()]
    conns = []

    async def handler(ws):
        conns.append(ws.request.headers.get("User-Agent"))
        if len(conns) == 1:
            for f in frames:
                await ws.send(f)
            await ws.send("garbage")
            await ws.close()  # server drops: client must reconnect
        else:
            await ws.send(frames[0])
            await ws.wait_closed()

    async def main():
        async with serve(handler, "127.0.0.1", 0) as server:
            port = server.sockets[0].getsockname()[1]
            s = Stream(f"http://127.0.0.1:{port}", initial_backoff_s=0.01, max_backoff_s=0.05)
            seen = []
            async with asyncio.timeout(10):
                async for ev in s:
                    seen.append(ev)
                    if sum(isinstance(e, Connected) for e in seen) == 2 and isinstance(ev, (VerdictEvent, PullEvent)):
                        await s.close()
            return seen

    seen = asyncio.run(main())
    kinds = [type(e).__name__ for e in seen]
    assert kinds[0] == "Connected"
    assert any(isinstance(e, VerdictEvent) for e in seen)
    assert any(isinstance(e, PullEvent) for e in seen)
    assert [e.type for e in seen if isinstance(e, UnknownEvent)] == ["hello_from_the_future"]
    rec = [e for e in seen if isinstance(e, Reconnecting)]
    assert len(rec) == 1 and rec[0].attempt == 1 and 0.005 <= rec[0].delay_s <= 0.01
    assert conns[0].startswith("rugtrip-sdk-python/")


def test_stream_gives_up():
    async def main():
        # nothing listens on this port (bind then close to get a free one)
        import socket

        sock = socket.socket()
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
        sock.close()
        s = Stream(f"http://127.0.0.1:{port}", initial_backoff_s=0.001, max_backoff_s=0.002, max_reconnects=2)
        seen = []
        try:
            async for ev in s:
                seen.append(ev)
        except Exception as e:  # noqa: BLE001
            return seen, e
        return seen, None

    seen, err = asyncio.run(main())
    assert err is not None and getattr(err, "code", None) == "network"
    assert [e.attempt for e in seen if isinstance(e, Reconnecting)] == [1, 2]
