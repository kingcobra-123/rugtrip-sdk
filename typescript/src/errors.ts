/**
 * Error codes. The first group comes from the API's `{"error": code}` body; the rest are raised by the SDK.
 */
export type ErrorCode =
  | "bad_address"
  | "unknown_token"
  | "rate_limited"
  | "bad_request"
  | "not_found"
  | "unauthorized"
  | "pro_required"
  | "internal"
  | "http_error"
  | "bad_response"
  | "timeout"
  | "network"
  | "config";

const API_CODES = new Set<string>([
  "bad_address",
  "unknown_token",
  "rate_limited",
  "bad_request",
  "not_found",
  "unauthorized",
  "pro_required",
  "internal",
]);

export class RugTripError extends Error {
  readonly code: ErrorCode;
  /** HTTP status, when the error came from a response */
  readonly status?: number;
  /** server-suggested wait before retrying (from Retry-After), ms */
  readonly retryAfterMs?: number;
  /** the raw error code string from the API (may be a code this SDK version does not know) */
  readonly apiCode?: string;

  constructor(code: ErrorCode, message: string, opts: { status?: number; retryAfterMs?: number; apiCode?: string; cause?: unknown } = {}) {
    super(message, opts.cause === undefined ? undefined : { cause: opts.cause });
    this.name = "RugTripError";
    this.code = code;
    this.status = opts.status;
    this.retryAfterMs = opts.retryAfterMs;
    this.apiCode = opts.apiCode;
  }

  /** true for errors that may succeed when retried later */
  get retryable(): boolean {
    return this.code === "rate_limited" || this.code === "timeout" || this.code === "network" || (this.status !== undefined && this.status >= 502 && this.status <= 504);
  }
}

/** Map an HTTP error response body to a RugTripError. */
export function errorFromResponse(status: number, body: unknown, retryAfterMs?: number): RugTripError {
  if (body && typeof body === "object" && typeof (body as { error?: unknown }).error === "string") {
    const apiCode = (body as { error: string }).error;
    const message = typeof (body as { message?: unknown }).message === "string" ? (body as { message: string }).message : apiCode;
    const code: ErrorCode = API_CODES.has(apiCode) ? (apiCode as ErrorCode) : status === 429 ? "rate_limited" : "http_error";
    return new RugTripError(code, message, { status, retryAfterMs, apiCode });
  }
  const code: ErrorCode = status === 429 ? "rate_limited" : "http_error";
  return new RugTripError(code, `HTTP ${status}`, { status, retryAfterMs });
}

/** Parse a Retry-After header (seconds or HTTP date) to ms. */
export function parseRetryAfter(v: string | null | undefined, now: number = Date.now()): number | undefined {
  if (!v) return undefined;
  const s = v.trim();
  if (/^\d+(\.\d+)?$/.test(s)) return Math.round(parseFloat(s) * 1000);
  const t = Date.parse(s);
  return Number.isNaN(t) ? undefined : Math.max(0, t - now);
}
