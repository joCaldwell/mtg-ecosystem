// Output linting (spec §6.4): every [[Card Name]] must resolve EXACTLY
// against the card_names table (which includes face names). Never fuzzy —
// a near miss becomes a "did you mean" bounced back to the agent; it never
// reaches the screen.

import type { DatabaseSync } from "node:sqlite";
import { resolveExactName, suggestNames } from "../search/index.ts";

export interface LintFailure {
  name: string;
  suggestions: string[];
}

export interface LintResult {
  ok: boolean;
  failures: LintFailure[];
}

export function extractCardRefs(text: string): string[] {
  const refs: string[] = [];
  for (const m of text.matchAll(/\[\[([^\[\]]+)\]\]/g)) {
    refs.push(m[1].trim());
  }
  return [...new Set(refs)];
}

export function lintOutput(db: DatabaseSync, text: string): LintResult {
  const failures: LintFailure[] = [];
  for (const name of extractCardRefs(text)) {
    if (resolveExactName(db, name).length === 0) {
      failures.push({ name, suggestions: suggestNames(db, name, 5) });
    }
  }
  return { ok: failures.length === 0, failures };
}

export function lintCorrectionMessage(failures: LintFailure[]): string {
  const lines = failures.map((f) => {
    const hint = f.suggestions.length
      ? ` Did you mean: ${f.suggestions.map((s) => `[[${s}]]`).join(", ")}? Only use one of these if you have verified it in this conversation's search results or the decklist — otherwise search first.`
      : ` No similar card exists — search_cards before mentioning it, or drop the claim.`;
    return `- [[${f.name}]] does not resolve to any card.${hint}`;
  });
  return `SYSTEM LINT: your reply referenced card names that do not exist in the database. It was NOT shown to the owner. Correct the following and re-send your full reply:\n${lines.join("\n")}`;
}
