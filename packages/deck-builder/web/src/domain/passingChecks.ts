import type { Computed, Finding } from "../api/client.ts";

// The server reports only failing checks (src/deck/checks.ts). The checks
// that always run and read straight off `computed`, or off the absence of
// their finding key, can be named as passing without recounting anything.
// Conditional checks (pairs, engines, companions, power) are left out: their
// absence does not mean they ran.
export function passingChecks(checks: Finding[], computed: Computed): string[] {
  const failed = (...prefixes: string[]) => checks.some((f) => prefixes.some((p) => f.key === p || f.key.startsWith(`${p}:`)));
  return [
    computed.card_count === 100 && "100 cards",
    !failed("no_commander", "commander_ineligible") && "Eligible commander",
    computed.identity_violations.length === 0 && "Colour identity",
    computed.legality_violations.length === 0 && "Commander legality",
    computed.singleton_violations.length === 0 && "Copy limits",
  ].filter((label): label is string => Boolean(label));
}
