import { RugTripError } from "./errors.js";

function envBaseUrl(): string | undefined {
  const proc = (globalThis as { process?: { env?: Record<string, string | undefined> } }).process;
  const v = proc?.env?.RUGTRIP_API_URL;
  return v && v.trim() ? v.trim() : undefined;
}

/** Resolve and validate the base URL (option, else env RUGTRIP_API_URL). Throws `config` when missing. */
export function resolveBaseUrl(baseUrl?: string): string {
  const raw = (baseUrl ?? envBaseUrl())?.trim();
  if (!raw) throw new RugTripError("config", "baseUrl is required: pass { baseUrl } or set RUGTRIP_API_URL. The SDK has no default endpoint.");
  let u: URL;
  try {
    u = new URL(raw);
  } catch {
    throw new RugTripError("config", `baseUrl is not a valid URL: ${raw}`);
  }
  if (u.protocol !== "http:" && u.protocol !== "https:") throw new RugTripError("config", "baseUrl must start with http:// or https://");
  return u.toString().replace(/\/+$/, "");
}

/** Backoff with jitter: a random wait in [d/2, d], d = min(cap, base * 2^attempt). */
export function backoffMs(attempt: number, baseMs: number, capMs: number, rand: () => number = Math.random): number {
  const d = Math.min(capMs, baseMs * 2 ** attempt);
  return Math.round(d / 2 + rand() * (d / 2));
}
