import { toTarget } from "./address.js";
import { errorFromResponse, parseRetryAfter, RugTripError } from "./errors.js";
import { RugTripStream, type StreamOptions } from "./stream.js";
import { backoffMs, resolveBaseUrl } from "./util.js";
import type { FeedQuery, FeedResponse, Health, RugsQuery, RugsToday, Scorecard, Stats, TokenCheck, Verdict } from "./types.js";
import { VERSION } from "./version.js";

export interface ClientOptions {
  /** API base URL, e.g. "http://127.0.0.1:8742". Required here or via env RUGTRIP_API_URL. There is no default. */
  baseUrl?: string;
  /** per-request timeout, ms (default 10000) */
  timeoutMs?: number;
  /** retries for rate limits, timeouts, network errors and 502/503/504 (default 2; 0 disables) */
  maxRetries?: number;
  /** first retry wait when the server gives no Retry-After, ms (default 1000; doubles each retry, capped at 30 s, jittered) */
  retryBaseMs?: number;
  /** extra headers sent with every request */
  headers?: Record<string, string>;
  /** custom fetch (tests, proxies); default globalThis.fetch */
  fetch?: typeof fetch;
}

export interface RequestOptions {
  signal?: AbortSignal;
}

const sleep = (ms: number, signal?: AbortSignal) =>
  new Promise<void>((resolve, reject) => {
    if (signal?.aborted) return reject(signal.reason);
    const t = setTimeout(resolve, ms);
    signal?.addEventListener("abort", () => {
      clearTimeout(t);
      reject(signal.reason);
    }, { once: true });
  });

export class RugTripClient {
  readonly baseUrl: string;
  readonly userAgent = `rugtrip-sdk-ts/${VERSION}`;
  private readonly timeoutMs: number;
  private readonly maxRetries: number;
  private readonly retryBaseMs: number;
  private readonly headers: Record<string, string>;
  private readonly fetchImpl: typeof fetch;

  constructor(opts: ClientOptions = {}) {
    this.baseUrl = resolveBaseUrl(opts.baseUrl);
    this.timeoutMs = opts.timeoutMs ?? 10_000;
    this.maxRetries = Math.max(0, opts.maxRetries ?? 2);
    this.retryBaseMs = opts.retryBaseMs ?? 1000;
    this.headers = { ...(opts.headers ?? {}) };
    const f = opts.fetch ?? (globalThis.fetch as typeof fetch | undefined);
    if (!f) throw new RugTripError("config", "No fetch available: use Node >= 18 or pass { fetch }");
    this.fetchImpl = f;
  }

  /** `GET /healthz` */
  health(o?: RequestOptions): Promise<Health> {
    return this.get<Health>("/healthz", undefined, o);
  }

  /**
   * Check a token before buying. Accepts a token address, a pool address, a v4 pool id or a DexScreener /
   * GeckoTerminal link. Returns the token's `best` (most liquid) pool verdict plus every known pool.
   * Gate on the pool you will actually trade through.
   */
  check(target: string, o?: RequestOptions): Promise<TokenCheck> {
    return this.get<TokenCheck>(`/v1/token/${toTarget(target)}`, undefined, o);
  }

  /** `GET /v1/pool/:addr`: one pool's verdict (20-byte pool address or 32-byte v4 pool id). */
  pool(addr: string, o?: RequestOptions): Promise<Verdict> {
    return this.get<Verdict>(`/v1/pool/${toTarget(addr)}`, undefined, o);
  }

  /** `GET /v1/feed/recent`: newest pools with verdicts (unauthenticated: 60 s delayed). */
  feedRecent(q: FeedQuery = {}, o?: RequestOptions): Promise<FeedResponse> {
    return this.get<FeedResponse>(
      "/v1/feed/recent",
      { rating: q.rating, venue: q.venue, limit: q.limit, before: q.before, after: q.after, include_late: q.includeLate ? "1" : undefined },
      o,
    );
  }

  /** `GET /v1/rugs/today`: liquidity pulls in the last 24 h, newest first. */
  rugsToday(q: RugsQuery = {}, o?: RequestOptions): Promise<RugsToday> {
    return this.get<RugsToday>("/v1/rugs/today", { since: q.since, before: q.before, limit: q.limit }, o);
  }

  /** `GET /v1/scorecard`: per day and rating, how many pools were later pulled. */
  scorecard(o?: RequestOptions): Promise<Scorecard> {
    return this.get<Scorecard>("/v1/scorecard", undefined, o);
  }

  /** `GET /v1/stats` */
  stats(o?: RequestOptions): Promise<Stats> {
    return this.get<Stats>("/v1/stats", undefined, o);
  }

  /** A `/v1/stream` WebSocket client with auto-reconnect. Call `.start()` after attaching handlers. */
  stream(opts: Omit<StreamOptions, "baseUrl"> = {}): RugTripStream {
    return new RugTripStream({ ...opts, baseUrl: this.baseUrl, headers: { "User-Agent": this.userAgent, ...this.headers, ...(opts.headers ?? {}) } });
  }

  private async get<T>(path: string, query?: Record<string, string | number | undefined>, o: RequestOptions = {}): Promise<T> {
    let url = this.baseUrl + path;
    if (query) {
      const qs = new URLSearchParams();
      for (const [k, v] of Object.entries(query)) if (v !== undefined && v !== null && v !== "") qs.set(k, String(v));
      const s = qs.toString();
      if (s) url += `?${s}`;
    }
    for (let attempt = 0; ; attempt++) {
      try {
        return await this.once<T>(url, o.signal);
      } catch (e) {
        const err = e instanceof RugTripError ? e : new RugTripError("network", String(e), { cause: e });
        if (o.signal?.aborted || !err.retryable || attempt >= this.maxRetries) throw err;
        const wait = err.retryAfterMs ?? backoffMs(attempt, this.retryBaseMs, 30_000);
        await sleep(wait, o.signal);
      }
    }
  }

  private async once<T>(url: string, signal?: AbortSignal): Promise<T> {
    const ctl = new AbortController();
    const onAbort = () => ctl.abort(signal?.reason);
    signal?.addEventListener("abort", onAbort, { once: true });
    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      ctl.abort();
    }, this.timeoutMs);
    try {
      let res: Response;
      try {
        res = await this.fetchImpl(url, { method: "GET", headers: { Accept: "application/json", "User-Agent": this.userAgent, ...this.headers }, signal: ctl.signal });
      } catch (e) {
        if (timedOut) throw new RugTripError("timeout", `Request timed out after ${this.timeoutMs} ms`, { cause: e });
        if (signal?.aborted) throw e;
        throw new RugTripError("network", `Network error: ${(e as Error)?.message ?? e}`, { cause: e });
      }
      const text = await res.text().catch((e) => {
        if (timedOut) throw new RugTripError("timeout", `Request timed out after ${this.timeoutMs} ms`, { cause: e });
        throw new RugTripError("network", `Network error reading body: ${(e as Error)?.message ?? e}`, { cause: e });
      });
      let body: unknown = undefined;
      try {
        body = text ? JSON.parse(text) : undefined;
      } catch {
        /* non-JSON */
      }
      if (!res.ok) throw errorFromResponse(res.status, body, parseRetryAfter(res.headers.get("retry-after")));
      if (body === undefined) throw new RugTripError("bad_response", `Expected JSON from ${url}`, { status: res.status });
      return body as T;
    } finally {
      clearTimeout(timer);
      signal?.removeEventListener("abort", onAbort);
    }
  }
}
