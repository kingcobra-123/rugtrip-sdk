import { test } from "node:test";
import assert from "node:assert/strict";
import { RugTripStream, streamUrl, type Pull, type Verdict, type WebSocketLike } from "../src/index.js";
import { fixtureText } from "./helpers.js";

class FakeWS implements WebSocketLike {
  static instances: FakeWS[] = [];
  readyState = 0;
  onopen: ((ev: unknown) => void) | null = null;
  onmessage: ((ev: { data: unknown }) => void) | null = null;
  onclose: ((ev: { code?: number; reason?: string }) => void) | null = null;
  onerror: ((ev: unknown) => void) | null = null;
  constructor(public url: string, public opts?: unknown) {
    FakeWS.instances.push(this);
  }
  open() {
    this.readyState = 1;
    this.onopen?.({});
  }
  send(data: string) {
    this.onmessage?.({ data });
  }
  close(code = 1006, reason = "") {
    if (this.readyState === 3) return;
    this.readyState = 3;
    this.onclose?.({ code, reason });
  }
}

const tick = (ms = 0) => new Promise((r) => setTimeout(r, ms));

test("stream URL is derived from baseUrl", () => {
  assert.equal(streamUrl("http://127.0.0.1:8742"), "ws://127.0.0.1:8742/v1/stream");
  assert.equal(streamUrl("https://api.example.test/base/"), "wss://api.example.test/base/v1/stream");
});

test("typed events, unknown frames, reconnect with backoff, close stops", async () => {
  FakeWS.instances = [];
  const s = new RugTripStream({ baseUrl: "http://127.0.0.1:8742", WebSocket: FakeWS, initialBackoffMs: 5, maxBackoffMs: 20, idleTimeoutMs: 0, headers: { "User-Agent": "t" } });
  const verdicts: Verdict[] = [];
  const pulls: Pull[] = [];
  const unknown: string[] = [];
  const reconnects: number[] = [];
  let opens = 0;
  s.on("verdict", (v) => verdicts.push(v))
    .on("pull", (p) => pulls.push(p))
    .on("unknown", (u) => unknown.push(u.type))
    .on("reconnecting", (r) => reconnects.push(r.delayMs))
    .on("open", () => opens++)
    .on("error", () => {});
  s.start();
  await tick();
  const ws1 = FakeWS.instances[0];
  assert.equal(ws1.url, "ws://127.0.0.1:8742/v1/stream");
  ws1.open();
  assert.equal(s.connected, true);
  for (const line of fixtureText("stream.jsonl").split("\n").filter(Boolean)) ws1.send(line);
  ws1.send("garbage");
  assert.ok(verdicts.length > 0 && pulls.length > 0);
  assert.deepEqual(unknown, ["hello_from_the_future"]);
  assert.equal(typeof verdicts[0].rating, "string");

  ws1.close(1006);
  assert.equal(reconnects.length, 1);
  assert.ok(reconnects[0] >= 2 && reconnects[0] <= 5, `first delay ${reconnects[0]}`);
  await tick(30);
  assert.equal(FakeWS.instances.length, 2);
  const ws2 = FakeWS.instances[1];
  ws2.close(1006); // failed again before any frame: backoff grows
  assert.equal(reconnects.length, 2);
  assert.ok(reconnects[1] >= 5 && reconnects[1] <= 10, `second delay ${reconnects[1]}`);
  await tick(30);
  FakeWS.instances[2].open();
  assert.equal(opens, 2);

  s.close();
  await tick(30);
  assert.equal(FakeWS.instances.length, 3, "no reconnect after close()");
  assert.equal(s.connected, false);
});

test("idle watchdog reconnects a silent connection", async () => {
  FakeWS.instances = [];
  const s = new RugTripStream({ baseUrl: "http://127.0.0.1:8742", WebSocket: FakeWS, initialBackoffMs: 1, maxBackoffMs: 2, idleTimeoutMs: 15 });
  const reasons: string[] = [];
  s.on("reconnecting", (r) => reasons.push(r.reason));
  s.start();
  await tick();
  FakeWS.instances[0].open();
  await tick(40);
  assert.ok(reasons.some((r) => r.startsWith("no frames")), JSON.stringify(reasons));
  assert.ok(FakeWS.instances.length >= 2);
  s.close();
});

test("maxReconnects gives up with an error", async () => {
  FakeWS.instances = [];
  const s = new RugTripStream({ baseUrl: "http://127.0.0.1:8742", WebSocket: FakeWS, initialBackoffMs: 1, maxBackoffMs: 1, maxReconnects: 1, idleTimeoutMs: 0 });
  const errors: string[] = [];
  s.on("error", (e) => errors.push(e.message));
  s.start();
  await tick();
  FakeWS.instances[0].close();
  await tick(10);
  FakeWS.instances[1].close();
  await tick(10);
  assert.equal(FakeWS.instances.length, 2);
  assert.ok(errors.some((m) => m.startsWith("Gave up")));
});
