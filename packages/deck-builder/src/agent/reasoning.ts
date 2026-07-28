// Audit reasoning pass (spec §8.2) — the model judges what SQL cannot:
// win-path, engine completeness, symmetry, anti-synergies, survival. The
// deterministic results are SUPPLIED as context; the model never re-derives
// them. Every card reference in its findings is validated before display —
// findings that name nonexistent or out-of-deck cards are dropped.

import type { DatabaseSync } from "node:sqlite";
import { deckCardIndex } from "../deck/service.ts";
import { normalizeName } from "../db.ts";
import { assembleDeckSections } from "./context.ts";
import { unresolvedRefs } from "./lint.ts";
import { callJson, type ChatMessage, type ChatTransport } from "./llm.ts";
import type { Finding } from "../deck/audit.ts";

export interface ReasoningFinding extends Finding {
  oracle_id?: string; // resolved during validation for action:"cut" findings
}

export interface ReasoningResult {
  summary: string;
  findings: ReasoningFinding[];
  dismissed: Array<ReasoningFinding & { dismissal: { type: string; reason: string } }>;
  dropped: number;
  error?: string;
}

const REASONING_RULES = `You are auditing a Magic: The Gathering Commander deck. The deck's brief, full decklist with oracle text, computed state, and the owner's decision history are below. A deterministic checker has already verified counts, legality, and slot targets — its results are listed and are AUTHORITATIVE. Never recount or re-derive them; reason from them.

Answer these questions, grounded in the oracle text you can see:
1. Win path: does the deck have a realistic route to its stated win condition? Where does it stall?
2. Engines: is every named engine's piece set present, and is there redundancy if a piece is removed?
3. Symmetry: which cards break symmetry in the owner's favor, and which quietly give opponents as much as they give the owner?
4. Anti-synergies: which cards work against the thesis or a named engine?
5. Survival: can the deck live long enough to assemble its engine, given its interaction and defenses?

STRICT RULES:
- Reference ONLY cards that appear in the decklist below. Use [[Card Name]] syntax with names spelled exactly as shown. Findings that reference other cards will be discarded.
- When you claim an interaction, quote the oracle clause you rely on.
- Do NOT report anything the deterministic checker already covers (counts, legality, slot deltas).
- If the owner's dismissed-findings list contains an issue you would raise again, reuse its exact slug so it stays suppressed.
- Respond with ONLY a JSON object, no markdown fences, in this shape:
{
  "summary": "2-4 sentence overall assessment",
  "findings": [
    {
      "slug": "stable-kebab-case-id-for-this-issue",
      "severity": "error" | "warn",
      "title": "short statement",
      "detail": "explanation with [[Card Name]] refs and quoted oracle clauses",
      "action": "cut" | null,
      "card_name": "Exact Card Name" | null
    }
  ]
}
- "action": "cut" with "card_name" only when removing that specific in-deck card is the fix. Suggestions to ADD cards belong in "detail" as advice (the chat agent handles searches), with action null.
- 0 findings is a valid answer for a healthy deck. Cap at 8 findings, most important first.`;

function slugify(s: string): string {
  return (
    s
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 60) || "finding"
  );
}

export async function runReasoningPass(
  db: DatabaseSync,
  deckId: number,
  instructions: string,
  transport: ChatTransport,
  retentionN: number,
  activeDismissals: Map<string, { type: string; reason: string }>,
): Promise<ReasoningResult> {
  const { sections } = assembleDeckSections(db, deckId, retentionN);

  const priorReasoningDismissals = [...activeDismissals.entries()]
    .filter(([key]) => key.startsWith("reasoning:"))
    .map(([key, d]) => `- slug "${key.slice("reasoning:".length)}": dismissed (${d.type}) — "${d.reason}"`);

  const userPrompt = [
    priorReasoningDismissals.length
      ? `Previously dismissed reasoning findings (reuse these slugs if raising the same issue):\n${priorReasoningDismissals.join("\n")}`
      : null,
    instructions.trim() ? `Owner's focus for this audit: ${instructions.trim()}` : null,
    "Run the reasoning audit now. JSON only.",
  ]
    .filter(Boolean)
    .join("\n\n");

  const messages: ChatMessage[] = [
    { role: "system", content: `${REASONING_RULES}\n\n---\n\n${sections}` },
    { role: "user", content: userPrompt },
  ];

  // One retry on unparseable output; a second failure degrades to an errored
  // run rather than throwing — the deterministic half stands on its own.
  const result = await callJson(transport, messages);
  if (result.error)
    return { summary: "", findings: [], dismissed: [], dropped: 0, error: result.error };
  const parsed: any = result.parsed;

  // Validate every finding: all [[refs]] and card_name must resolve to real
  // cards; action:"cut" targets must actually be in the deck.
  const { byName: inDeck } = deckCardIndex(db, deckId);

  let dropped = 0;
  const seen = new Set<string>();
  const valid: ReasoningFinding[] = [];
  for (const raw of Array.isArray(parsed?.findings) ? parsed.findings : []) {
    const slug = slugify(String(raw.slug ?? raw.title ?? "finding"));
    if (seen.has(slug)) {
      dropped++;
      continue;
    }
    const title = String(raw.title ?? "").trim();
    const detail = String(raw.detail ?? "").trim();
    if (!title) {
      dropped++;
      continue;
    }
    if (unresolvedRefs(db, `${title} ${detail}`).length) {
      dropped++;
      continue;
    }
    const finding: ReasoningFinding = {
      key: `reasoning:${slug}`,
      severity: raw.severity === "error" ? "error" : "warn",
      title,
      detail,
    };
    if (raw.action === "cut" && raw.card_name) {
      const card = inDeck.get(normalizeName(String(raw.card_name)));
      if (card) {
        finding.action = "cut";
        finding.oracle_id = card.oracle_id;
        finding.card_name = String(raw.card_name);
      }
      // cut target not in deck → keep as informational finding
    }
    seen.add(slug);
    valid.push(finding);
  }

  const findings = valid.filter((f) => !activeDismissals.has(f.key));
  const dismissed = valid
    .filter((f) => activeDismissals.has(f.key))
    .map((f) => ({ ...f, dismissal: activeDismissals.get(f.key)! }));

  return {
    summary: String(parsed?.summary ?? "").trim(),
    findings,
    dismissed,
    dropped,
  };
}
