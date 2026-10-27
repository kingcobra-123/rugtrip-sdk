import { backoffMs, resolveBaseUrl } from "./util.js";
import { RugTripError } from "./errors.js";
import type { Alert, Pull, Verdict } from "./types.js";

/** Minimal WebSocket shape (browser / Node global WebSocket, or the `ws` package). */
export interface WebSocketLike {
  readyState: number;
  onopen: ((ev: unknown) => void) | null;
  onmessage: ((ev: { data: unknown }) => void) | null;
  onclose: ((ev: { code?: number; reason?: string }) => void) | null;
  onerror: ((ev: unknown) => void) | null;
  close(code?: number, reason?: string): void;
}
export type WebSocketCtor = new (url: string, protocolsOrOptions?: unknown) => WebSocketLike;

export interface StreamOptions {
  /** API base URL (http/https; converted to ws/wss). Required here or via env RUGTRIP_API_URL. */
  baseUrl?: string;
  /** WebSocket implementation; default globalThis.WebSocket, else the optional `ws` package. */
  WebSocket?: WebSocketCtor;
  /** headers for the upgrade request (Node only; browsers can't set them) */
  headers?: Record<string, string>;
  /** first reconnect wait, ms (default 500) */
  initialBackoffMs?: number;
  /** reconnect wait cap, ms (default 30000) */
  maxBackoffMs?: number;
  /** give up after this many consecutive failed attempts (default Infinity) */
  maxReconnects?: number;
  /** reconnect when no frame arrives for this long, ms (default 300000; 0 disables) */
  idleTimeoutMs?: number;
}

export interface StreamEvents {
  open: { url: string; attempt: number };
  verdict: Verdict;
  pull: Pull;
  /** only sent to real-time (paid plan) connections */
  alert: Alert;
  /** a frame type this SDK version doesn't know (forward compatibility) */
  unknown: { type: string; data: unknown; raw: string };
  reconnecting: { attempt: number; delayMs: number; reason: string };
  close: { code?: number; reason?: string };
  error: RugTripError;
}

type Handler<K extends keyof StreamEvents> = (ev: StreamEvents[K]) => void;

/** http(s)://host[/prefix] -> ws(s)://host[/prefix]/v1/stream */
export function streamUrl(baseUrl: string): string {
  const u = new URL(resolveBaseUrl(baseUrl));
  u.protocol = u.protocol === "https:" ? "wss:" : "ws:";
  u.pathname = `${u.pathname.replace(/\/+$/, "")}/v1/stream`;
  return u.toString();
}

/** Parse one text frame. Returns undefined for malformed frames. */
export function parseFrame(raw: string): { type: string; data: unknown } | undefined {
  try {
    const m = JSON.parse(raw) as { type?: unknown; data?: unknown };
    if (m && typeof m === "object" && typeof m.type === "string") return { type: m.type, data: m.data };
  } catch {
    /* fallthrough */
  }
  return undefined;
}

function decode(data: unknown): string {
  if (typeof data === "string") return data;
  if (data instanceof ArrayBuffer) return new TextDecoder().decode(data);
  if (ArrayBuffer.isView(data)) return new TextDecoder().decode(data as Uint8Array);
  return String(data);
}

/**
 * Live verdict / pull stream with auto-reconnect (exponential backoff with jitter, capped).
 *
 * Unauthenticated connections receive `verdict` and `pull` events 60 s after they happen; real-time delivery
 * comes with API plans. Event-driven: attach handlers with `on()`, then `start()`. Nothing is polled.
 */
export class RugTripStream {
  readonly url: string;
  private readonly opts: StreamOptions;
  private handlers: { [K in keyof StreamEvents]?: Set<Handler<K>> } = {};
  private ws?: WebSocketLike;
  private attempt = 0;
  private stopped = true;
  private retryTimer?: ReturnType<typeof setTimeout>;
  private idleTimer?: ReturnType<typeof setTimeout>;

  constructor(opts: StreamOptions = {}) {
    this.opts = opts;
    this.url = streamUrl(resolveBaseUrl(opts.baseUrl));
  }

  on<K extends keyof StreamEvents>(event: K, fn: Handler<K>): this {
    let set = this.handlers[event] as Set<Handler<K>> | undefined;
    if (!set) {
      set = new Set();
      (this.handlers as Record<string, unknown>)[event] = set;
    }
    set.add(fn);
    return this;
  }

  off<K extends keyof StreamEvents>(event: K, fn: Handler<K>): this {
    (this.handlers[event] as Set<Handler<K>> | undefined)?.delete(fn);
    return this;
  }

  get connected(): boolean {
    return this.ws?.readyState === 1;
  }

  /** Connect (and keep reconnecting until `close()`). */
  start(): this {
    if (!this.stopped) return this;
    this.stopped = false;
    void this.connect();
    return this;
  }

  /** Close the connection and stop reconnecting. */
  close(): void {
    this.stopped = true;
    clearTimeout(this.retryTimer);
    clearTimeout(this.idleTimer);
    const ws = this.ws;
    this.ws = undefined;
    if (ws && ws.readyState <= 1) ws.close(1000, "client closed");
  }

  private emit<K extends keyof StreamEvents>(event: K, ev: StreamEvents[K]): void {
    const set = this.handlers[event] as Set<Handler<K>> | undefined;
    if (!set) return;
    for (const fn of set) {
      try {
        fn(ev);
      } catch (e) {
        if (event !== "error") this.emit("error", new RugTripError("internal", `handler for "${event}" threw: ${(e as Error)?.message ?? e}`, { cause: e }));
      }
    }
  }

  private async ctor(): Promise<WebSocketCtor> {
    if (this.opts.WebSocket) return this.opts.WebSocket;
    const g = (globalThis as { WebSocket?: WebSocketCtor }).WebSocket;
    if (g) return g;
    try {
      const modName = "ws";
      const mod = (await import(modName)) as { default?: WebSocketCtor; WebSocket?: WebSocketCtor };
      const c = mod.default ?? mod.WebSocket;
      if (c) return c;
    } catch {
      /* fallthrough */
    }
    throw new RugTripError("config", "No WebSocket available: use Node >= 22, install the optional `ws` package, or pass { WebSocket }");
  }

  private armIdle(): void {
    clearTimeout(this.idleTimer);
    const ms = this.opts.idleTimeoutMs ?? 300_000;
    if (ms > 0) {
      this.idleTimer = setTimeout(() => {
        const ws = this.ws;
        if (ws) {
          this.ws = undefined;
          try {
            ws.close(4000, "idle");
          } catch {
            /* ignore */
          }
          this.scheduleReconnect(`no frames for ${ms} ms`);
        }
      }, ms);
    }
  }

  private async connect(): Promise<void> {
    if (this.stopped) return;
    let Ctor: WebSocketCtor;
    try {
      Ctor = await this.ctor();
    } catch (e) {
      this.stopped = true;
      this.emit("error", e as RugTripError);
      return;
    }
    if (this.stopped) return;
    const isNode = typeof (globalThis as { process?: { versions?: { node?: string } } }).process?.versions?.node === "string";
    let ws: WebSocketLike;
    try {
      ws = isNode && this.opts.headers ? new Ctor(this.url, { headers: this.opts.headers }) : new Ctor(this.url);
    } catch (e) {
      this.emit("error", new RugTripError("network", `WebSocket connect failed: ${(e as Error)?.message ?? e}`, { cause: e }));
      this.scheduleReconnect("connect failed");
      return;
    }
    this.ws = ws;
    let gotFrame = false;
    ws.onopen = () => {
      if (this.ws !== ws) return;
      this.emit("open", { url: this.url, attempt: this.attempt });
      this.armIdle();
    };
    ws.onmessage = (ev) => {
      if (this.ws !== ws) return;
      if (!gotFrame) {
        gotFrame = true;
        this.attempt = 0; // healthy connection: reset backoff
      }
      this.armIdle();
      const raw = decode(ev.data);
      const m = parseFrame(raw);
      if (!m) {
        this.emit("error", new RugTripError("bad_response", `Malformed stream frame: ${raw.slice(0, 120)}`));
        return;
      }
      if (m.type === "verdict") this.emit("verdict", m.data as Verdict);
      else if (m.type === "pull") this.emit("pull", m.data as Pull);
      else if (m.type === "alert") this.emit("alert", m.data as Alert);
      else this.emit("unknown", { type: m.type, data: m.data, raw });
    };
    ws.onerror = (ev) => {
      if (this.ws !== ws) return;
      const msg = (ev as { message?: string })?.message;
      this.emit("error", new RugTripError("network", `WebSocket error${msg ? `: ${msg}` : ""}`));
    };
    ws.onclose = (ev) => {
      if (this.ws !== ws) return;
      this.ws = undefined;
      clearTimeout(this.idleTimer);
      this.emit("close", { code: ev?.code, reason: ev?.reason });
      this.scheduleReconnect(`closed${ev?.code ? ` (${ev.code})` : ""}`);
    };
  }

  private scheduleReconnect(reason: string): void {
    if (this.stopped) return;
    const max = this.opts.maxReconnects ?? Infinity;
    if (this.attempt >= max) {
      this.stopped = true;
      this.emit("error", new RugTripError("network", `Gave up after ${this.attempt} reconnect attempts`));
      return;
    }
    const delayMs = backoffMs(this.attempt, this.opts.initialBackoffMs ?? 500, this.opts.maxBackoffMs ?? 30_000);
    this.attempt++;
    this.emit("reconnecting", { attempt: this.attempt, delayMs, reason });
    clearTimeout(this.retryTimer);
    this.retryTimer = setTimeout(() => void this.connect(), delayMs);
  }
}
