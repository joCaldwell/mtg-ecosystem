// Where a review handed one proposal starts (the inbox's "Review in the
// list", app/routes.ts). Apart from review.ts and routes.ts, which the public
// chunk carries, so shared pages do not pay for an editor-only rule.

import type { ReviewUnit } from "./review.ts";

/** That proposal's first open decision, or the first decision when it has none left. */
export function firstUnitOf(units: ReviewUnit[], proposalId: number | undefined): number {
  return Math.max(0, units.findIndex((u) => u.proposalId === proposalId));
}

/** The proposal a `/deck/:id/review?proposal=…` link names, if it names one. */
export function proposalParam(search: string): number | undefined {
  const id = Number(new URLSearchParams(search).get("proposal"));
  return Number.isInteger(id) && id > 0 ? id : undefined;
}
