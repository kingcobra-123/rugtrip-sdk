import type { LpCustody, Rating, Reason, Verdict } from "./types.js";
import { RATINGS } from "./types.js";

/**
 * Your own pre-trade rules, applied to one pool's Verdict. These are client-side preferences; they say nothing
 * about how RugTrip rates a pool. Passing a policy does not make a token low-risk: ratings are risk opinions.
 */
export interface Policy {
  /** Most risky rating you accept. Default "CAUTION" (skips TRAP and HIGH_RISK). */
  maxRating: Rating;
  /** Skip when any reason has level "bad", whatever the rating. Default true. */
  skipOnBadReason: boolean;
  /** Accepted LP custody values; null or [] = don't check. Default ["burned", "locked"]. */
  requireLpCustody: LpCustody[] | null;
  /**
   * What to do when the sell test couldn't run (status no_route / error, or not run yet).
   * That is "couldn't check", not a trap. Default "skip". A failed test sale (blocked) always skips.
   */
  treatUnknownSimAs: "skip" | "allow";
  /** Max buy / sell tax from the sell test, %. null = don't check. Default 10 / 10. */
  maxBuyTaxPct: number | null;
  maxSellTaxPct: number | null;
  /** Max share of supply the creator still holds, %. null = don't check. Default 10. */
  maxCreatorPct: number | null;
  /** Max share of supply bought by insiders in the launch block, %. null = don't check. Default 25. */
  maxBundlePct: number | null;
  /** What to do when creator / bundle numbers are not measured yet. Default "allow" (listed in `unknowns`). */
  treatUnknownHoldingsAs: "skip" | "allow";
  /** Min pool liquidity in USD; null = don't check. When set, unknown liquidity skips. Default null. */
  minLiqUsd: number | null;
}

export const DEFAULT_POLICY: Readonly<Policy> = Object.freeze<Policy>({
  maxRating: "CAUTION",
  skipOnBadReason: true,
  requireLpCustody: ["burned", "locked"],
  treatUnknownSimAs: "skip",
  maxBuyTaxPct: 10,
  maxSellTaxPct: 10,
  maxCreatorPct: 10,
  maxBundlePct: 25,
  treatUnknownHoldingsAs: "allow",
  minLiqUsd: null,
});

export interface PolicyHit {
  /** which policy field caused the skip */
  rule: keyof Policy | "rating_unrecognized" | "sell_blocked";
  text: string;
}

export interface GateResult {
  skip: boolean;
  decision: "SKIP" | "BUY_CANDIDATE";
  /** policy rules that failed (empty when not skipped) */
  hits: PolicyHit[];
  /** facts that could not be checked (allowed by the policy, or the cause of a skip) */
  unknowns: string[];
  /** the verdict's own plain-word reasons, most serious first */
  reasons: Reason[];
}

/** 0 = TRAP ... 3 = LOWER_RISK; -1 for a rating this SDK version doesn't know. */
export function riskRank(r: string): number {
  return RATINGS.indexOf(r as Rating);
}

const LEVEL_ORDER: Record<string, number> = { bad: 0, warn: 1, unknown: 2, ok: 3 };

function fmt(x: number): string {
  return Number.isInteger(x) ? String(x) : x.toFixed(1);
}

/** Apply a policy (merged over DEFAULT_POLICY) to one pool's verdict. */
export function gate(v: Verdict, policy: Partial<Policy> = {}): GateResult {
  const p: Policy = { ...DEFAULT_POLICY, ...policy };
  const hits: PolicyHit[] = [];
  const unknowns: string[] = [];
  const d = v.details;

  const rank = riskRank(v.rating);
  if (rank < 0) hits.push({ rule: "rating_unrecognized", text: `Unrecognized rating ${v.rating}` });
  else if (rank < riskRank(p.maxRating)) hits.push({ rule: "maxRating", text: `Rated ${v.rating} (policy accepts up to ${p.maxRating})` });

  if (p.skipOnBadReason) {
    for (const r of v.reasons) if (r.level === "bad") hits.push({ rule: "skipOnBadReason", text: r.text });
  }

  if (p.requireLpCustody && p.requireLpCustody.length > 0) {
    const custody = d?.lp?.custody;
    if (!custody) {
      unknowns.push("LP custody not measured yet");
      hits.push({ rule: "requireLpCustody", text: "LP custody not measured yet" });
    } else if (!p.requireLpCustody.includes(custody)) {
      hits.push({ rule: "requireLpCustody", text: `LP custody is ${custody} (policy requires ${p.requireLpCustody.join(" or ")})` });
    }
  }

  const sim = d?.sim;
  const simUnknown = (why: string) => {
    unknowns.push(why);
    if (p.treatUnknownSimAs === "skip") hits.push({ rule: "treatUnknownSimAs", text: why });
  };
  if (!sim) simUnknown("Sell test not run yet");
  else if (sim.status === "blocked") hits.push({ rule: "sell_blocked", text: "A test sale failed" });
  else if (sim.status === "no_route") simUnknown("Sell test couldn't run (no route): sellability not checked");
  else if (sim.status === "error") simUnknown("Sell test couldn't run (error): sellability not checked");
  else if (sim.status !== "ok") simUnknown(`Sell test status ${sim.status as string}: sellability not checked`);
  else {
    if (p.maxBuyTaxPct !== null) {
      if (sim.buy_tax_pct === null) simUnknown("Buy tax not measured");
      else if (sim.buy_tax_pct > p.maxBuyTaxPct) hits.push({ rule: "maxBuyTaxPct", text: `Buy tax ${fmt(sim.buy_tax_pct)}% (policy max ${fmt(p.maxBuyTaxPct)}%)` });
    }
    if (p.maxSellTaxPct !== null) {
      if (sim.sell_tax_pct === null) simUnknown("Sell tax not measured");
      else if (sim.sell_tax_pct > p.maxSellTaxPct) hits.push({ rule: "maxSellTaxPct", text: `Sell tax ${fmt(sim.sell_tax_pct)}% (policy max ${fmt(p.maxSellTaxPct)}%)` });
    }
  }

  const holding = (val: number | null | undefined, max: number | null, rule: "maxCreatorPct" | "maxBundlePct", label: string) => {
    if (max === null) return;
    if (val === null || val === undefined) {
      unknowns.push(`${label} not measured yet`);
      if (p.treatUnknownHoldingsAs === "skip") hits.push({ rule: "treatUnknownHoldingsAs", text: `${label} not measured yet` });
    } else if (val > max) {
      hits.push({ rule, text: `${label} ${fmt(val)}% (policy max ${fmt(max)}%)` });
    }
  };
  holding(d?.creator_pct, p.maxCreatorPct, "maxCreatorPct", "Creator holds");
  holding(d?.bundle_pct, p.maxBundlePct, "maxBundlePct", "Launch-block insider buys");

  if (p.minLiqUsd !== null) {
    if (v.liq_usd === null) hits.push({ rule: "minLiqUsd", text: "Liquidity unknown" });
    else if (v.liq_usd < p.minLiqUsd) hits.push({ rule: "minLiqUsd", text: `Liquidity $${fmt(v.liq_usd)} (policy min $${fmt(p.minLiqUsd)})` });
  }

  const reasons = [...v.reasons].sort((a, b) => (LEVEL_ORDER[a.level] ?? 2) - (LEVEL_ORDER[b.level] ?? 2));
  const skip = hits.length > 0;
  return { skip, decision: skip ? "SKIP" : "BUY_CANDIDATE", hits, unknowns, reasons };
}

/** true when the policy says skip this pool. */
export function shouldSkip(v: Verdict, policy: Partial<Policy> = {}): boolean {
  return gate(v, policy).skip;
}
