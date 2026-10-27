import { readFileSync } from "node:fs";

// compiled to dist-test/test/helpers.js; fixtures live at the repo root
const FIXTURES = new URL("../../../fixtures/", import.meta.url);

export function fixture<T = unknown>(name: string): T {
  return JSON.parse(readFileSync(new URL(name, FIXTURES), "utf8")) as T;
}

export function fixtureText(name: string): string {
  return readFileSync(new URL(name, FIXTURES), "utf8");
}

export function jsonResponse(status: number, body: unknown, headers: Record<string, string> = {}): Response {
  return new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json", ...headers } });
}
