// Manual consolidate command (spec §11).
//
// The chat is an append-only session log; the deck, brief, and decision log
// live in the database and are the source of truth. Consolidation therefore
// discards conversational texture, not data — which is what makes it safe to
// do aggressively.
//
// HARD BOUNDARY — enforced here in code, not in the prompt (spec §11):
// applying a consolidation performs exactly three writes, all listed in
// applyConsolidation(). It can never touch the decklist, computed state,
// pending proposals, hard filters, playtest findings, or the brief. Brief
// changes it wants exist only as pending brief_edits rows, which the owner
// rules on separately through the normal gate. Worst case for a bad run is
// losing some conversational context and re-explaining something once.

import type { DatabaseSync } from "node:sqlite";
import { ServiceError } from "../errors.ts";
import { withTransaction } from "../db.ts";
import { getBrief, proposeBriefEdit } from "../deck/brief.ts";
import { unresolvedRefs } from "./lint.ts";
import { markCompacted, residentMessages } from "./chatStore.ts";
import { callJson, type ChatMessage, type ChatTransport } from "./llm.ts";

// Never compact the most recent exchanges — the chat has to keep reading as a
// conversation, and the tail is where the live thread of work is.
const KEEP_RECENT_MESSAGES = 10;

export interface RescuedFact {
  fact: string;
  should_have_been: string;
  why: string;
}

export interface ConsolidationView {
  id: number;
  status: "pending" | "accepted" | "rejected";
  summary: string;
  discarded: string[];
  rescued: RescuedFact[];
  through_message_id: number;
  message_count: number;
  brief_edit_ids: number[];
  created_at: string;
  resolved_at: string | null;
}

const CONSOLIDATION_RULES = `You are compacting the session transcript of one long-running Magic: The Gathering deck-building chat.

WHAT YOU ARE DOING: the messages below are leaving the model's context window. The deck, the brief, and the decision log are stored separately in a database and are NOT at risk — they stay exactly as they are. You are condensing conversation, not data.

Produce three things:
1. A SUMMARY that replaces those messages in future context. Preserve decisions, constraints the owner stated, open threads, and things you would otherwise ask about again. Discard pleasantries, superseded plans, search results (searchable again), and reasoning that led nowhere. Aim for under 400 words.
2. BRIEF EDITS for high-impact durable information that belongs in the deck brief rather than in a chat summary. These go to the owner for approval — propose one only when the transcript shows the written brief is genuinely stale or missing something the owner stated.
3. A RESCUED report: facts you had to lift out of the transcript because nothing structured was holding them. This is a diagnostic. If the same kind of fact keeps getting rescued, a tool is missing — that fact should have been written to a record when it happened. Only report a fact here if it is genuinely absent from the structured records shown below; a fact already recorded there is not rescued. Be honest and specific; an empty list is a good result.

STRICT RULES:
- Every card name you write MUST use [[Card Name]] syntax, spelled exactly as it appears in the transcript or decklist. Names that do not resolve will be bounced back to you.
- Do not invent facts. If the transcript is thin, the summary is short.
- You cannot change the deck. Do not describe changes as if they happened; only accepted proposals change the deck, and those are already recorded in the decision log.
- Respond with ONLY a JSON object, no markdown fences, in this shape:
{
  "summary": "prose that replaces the compacted messages",
  "discarded": ["short description of something dropped as outdated or superseded"],
  "brief_edits": [
    { "section": "thesis" | "constraints", "content": "full replacement text for that section", "rationale": "why the current text is stale" }
  ],
  "rescued": [
    { "fact": "the durable fact", "should_have_been": "brief | playtest note | hard filter | slot target | card note", "why": "why it was only in chat" }
  ]
}`;

// The active compaction summary, if the owner has accepted one. It stands in
// for the chat messages it replaced and rides ahead of the resident
// transcript (spec §11).
export function activeSummary(
  db: DatabaseSync,
  deckId: number,
): { id: number; summary: string; through_message_id: number; message_count: number } | null {
  return (
    (db
      .prepare(
        `SELECT id, summary, through_message_id, message_count FROM consolidations
         WHERE deck_id = ? AND status = 'accepted' AND superseded_by IS NULL
         ORDER BY id DESC LIMIT 1`,
      )
      .get(deckId) as
      | { id: number; summary: string; through_message_id: number; message_count: number }
      | undefined) ?? null
  );
}

function renderMessage(m: ChatMessage & { id: number }): string | null {
  if (m.role === "user")
    return `USER: ${(m.content ?? "").replace(/^<state_summary>.*?<\/state_summary>\n\n/s, "")}`;
  if (m.role === "assistant") {
    if (m.content) return `AGENT: ${m.content}`;
    if (m.tool_calls?.length)
      return `AGENT (tool): ${m.tool_calls.map((c) => c.function.name).join(", ")}`;
    return null;
  }
  if (m.role === "system") return `SYSTEM NOTE: ${m.content ?? ""}`;
  // Tool results are re-derivable (searches) or already reflected in the
  // decklist — never worth spending summary tokens on.
  return null;
}

/**
 * Run the consolidation pass. Produces a PENDING consolidation plus pending
 * brief edits; nothing is applied until the owner rules (spec §11).
 */
export async function runConsolidation(
  db: DatabaseSync,
  deckId: number,
  transport: ChatTransport,
): Promise<ConsolidationView> {
  if (!db.prepare("SELECT 1 FROM decks WHERE id = ?").get(deckId))
    throw new ServiceError(`Deck ${deckId} not found`, 404);

  const existing = db
    .prepare("SELECT id FROM consolidations WHERE deck_id = ? AND status = 'pending'")
    .get(deckId) as { id: number } | undefined;
  if (existing)
    throw new ServiceError(
      `Consolidation #${existing.id} is already awaiting your ruling — accept or reject it first.`,
    );

  const resident = residentMessages(db, deckId);

  const zone = resident.slice(0, Math.max(0, resident.length - KEEP_RECENT_MESSAGES));
  if (!zone.length)
    throw new ServiceError(
      `Nothing to consolidate — the last ${KEEP_RECENT_MESSAGES} messages are always kept resident.`,
    );

  const rendered = zone
    .map((r) => renderMessage({ id: r.id, ...r.message }))
    .filter((l): l is string => !!l);
  if (!rendered.length)
    throw new ServiceError("Nothing to consolidate — the compaction zone holds no prose messages.");

  const prior = activeSummary(db, deckId);
  const brief = getBrief(db, deckId);
  // Slots and engines ship alongside the brief so the rescued report stays a
  // useful diagnostic: without them the model "rescues" facts that are
  // already held in a structured record, which is exactly the false positive
  // the report exists to distinguish from a real missing tool.
  const slots = db
    .prepare(
      "SELECT name, target_min, target_max FROM slots WHERE deck_id = ? ORDER BY position",
    )
    .all(deckId) as unknown as Array<{
    name: string;
    target_min: number | null;
    target_max: number | null;
  }>;

  const userPrompt = [
    `# What is already held in a structured record (do NOT "rescue" these — they are not at risk)

## Thesis
${brief.thesis || "(not written yet)"}

## Constraints
${brief.constraints_md || "(none recorded)"}

## Named engines
${brief.engines.map((e) => `- ${e.name}: ${e.description || "(no description)"}`).join("\n") || "(none defined)"}

## Slots and their targets
${slots.map((s) => `- ${s.name} (target ${s.target_min ?? 0}–${s.target_max ?? "∞"})`).join("\n") || "(none defined)"}

Hard filters, playtest notes and the full decision log are also stored and are not at risk.`,
    prior
      ? `# Existing compaction summary (covers everything before the messages below — fold it into your new summary)\n\n${prior.summary}`
      : null,
    `# Messages leaving context (${rendered.length} of them)\n\n${rendered.join("\n\n")}`,
    "Consolidate now. JSON only.",
  ]
    .filter(Boolean)
    .join("\n\n---\n\n");

  const messages: ChatMessage[] = [
    { role: "system", content: CONSOLIDATION_RULES },
    { role: "user", content: userPrompt },
  ];

  // One correction round-trip, shared by both failure modes: unparseable JSON
  // and a summary naming cards that do not exist. A summary with a
  // hallucinated card in it would poison every later turn, so it is never
  // stored uncorrected.
  const result = await callJson(transport, messages, (candidate) => {
    const bad = unresolvedRefs(db, String(candidate.summary ?? ""));
    if (!bad.length) return null;
    return {
      correction: `These card names do not resolve: ${bad.join(", ")}. Rewrite the summary using only names that appear verbatim in the transcript, and respond with ONLY the JSON object.`,
      failure: `The summary referenced cards that do not exist (${bad.join(", ")}) and was discarded rather than stored.`,
    };
  });
  if (result.error) throw new ServiceError(result.error, 502);
  const parsed: any = result.parsed;

  const summary = String(parsed.summary ?? "").trim();
  if (!summary) throw new ServiceError("The model returned an empty summary; nothing was changed.", 502);

  const discarded = (Array.isArray(parsed.discarded) ? parsed.discarded : [])
    .map((d: unknown) => String(d).trim())
    .filter(Boolean)
    .slice(0, 20);
  const rescued: RescuedFact[] = (Array.isArray(parsed.rescued) ? parsed.rescued : [])
    .map((r: any) => ({
      fact: String(r?.fact ?? "").trim(),
      should_have_been: String(r?.should_have_been ?? "").trim(),
      why: String(r?.why ?? "").trim(),
    }))
    .filter((r: RescuedFact) => r.fact)
    .slice(0, 20);

  // Brief edits with unresolvable card refs are dropped, not corrected — the
  // summary is the part that must be right for context to stay clean.
  const briefEditIds: number[] = [];
  for (const edit of Array.isArray(parsed.brief_edits) ? parsed.brief_edits.slice(0, 4) : []) {
    const section = edit?.section === "constraints" ? "constraints" : "thesis";
    const content = String(edit?.content ?? "").trim();
    const rationale = String(edit?.rationale ?? "").trim();
    if (!content || !rationale) continue;
    if (unresolvedRefs(db, `${content} ${rationale}`).length) continue;
    briefEditIds.push(
      proposeBriefEdit(db, deckId, section, { content }, rationale, "consolidation"),
    );
  }

  const r = db
    .prepare(
      `INSERT INTO consolidations
       (deck_id, summary, discarded_json, rescued_json, through_message_id, message_count, brief_edit_ids)
       VALUES (?, ?, ?, ?, ?, ?, ?)`,
    )
    .run(
      deckId,
      summary,
      JSON.stringify(discarded),
      JSON.stringify(rescued),
      zone[zone.length - 1].id,
      zone.length,
      JSON.stringify(briefEditIds),
    );

  return getConsolidation(db, deckId, Number(r.lastInsertRowid));
}

export function getConsolidation(
  db: DatabaseSync,
  deckId: number,
  id: number,
): ConsolidationView {
  const row = db
    .prepare("SELECT * FROM consolidations WHERE id = ? AND deck_id = ?")
    .get(id, deckId) as any;
  if (!row) throw new ServiceError(`Consolidation ${id} not found`, 404);
  return {
    id: row.id,
    status: row.status,
    summary: row.summary,
    discarded: JSON.parse(row.discarded_json),
    rescued: JSON.parse(row.rescued_json),
    through_message_id: row.through_message_id,
    message_count: row.message_count,
    brief_edit_ids: JSON.parse(row.brief_edit_ids),
    created_at: row.created_at,
    resolved_at: row.resolved_at,
  };
}

export function listConsolidations(db: DatabaseSync, deckId: number, status?: string) {
  const rows = db
    .prepare(
      `SELECT id FROM consolidations WHERE deck_id = ? ${status ? "AND status = ?" : ""} ORDER BY id DESC`,
    )
    .all(...(status ? [deckId, status] : [deckId])) as unknown as Array<{ id: number }>;
  return rows.map((r) => getConsolidation(db, deckId, r.id));
}

/**
 * Apply an approved consolidation.
 *
 * These are the ONLY writes performed, and the only ones this module is
 * capable of performing (spec §11's hard boundary):
 *   1. chatStore.markCompacted — UPDATE chat_messages SET compacted_at,
 *      this deck, id <= through_message_id
 *   2. UPDATE consolidations SET status/superseded_by
 * No DELETEs: the raw transcript stays on disk, so compaction is
 * non-destructive and a bad run costs at most some conversational context.
 */
export function acceptConsolidation(db: DatabaseSync, deckId: number, id: number): ConsolidationView {
  const c = getConsolidation(db, deckId, id);
  if (c.status !== "pending") throw new ServiceError(`Consolidation ${id} is already ${c.status}`);

  withTransaction(db, () => {
    markCompacted(db, deckId, c.through_message_id);
    // At most one summary is ever resident; this one folds in its predecessor.
    db.prepare(
      "UPDATE consolidations SET superseded_by = ? WHERE deck_id = ? AND status = 'accepted' AND superseded_by IS NULL",
    ).run(id, deckId);
    db.prepare(
      "UPDATE consolidations SET status = 'accepted', resolved_at = datetime('now') WHERE id = ?",
    ).run(id);
  });
  return getConsolidation(db, deckId, id);
}

// Rejecting leaves the transcript untouched. Any brief edits it proposed stay
// pending and are ruled on separately — they are ordinary brief edits.
export function rejectConsolidation(
  db: DatabaseSync,
  deckId: number,
  id: number,
): ConsolidationView {
  const c = getConsolidation(db, deckId, id);
  if (c.status !== "pending") throw new ServiceError(`Consolidation ${id} is already ${c.status}`);
  db.prepare(
    "UPDATE consolidations SET status = 'rejected', resolved_at = datetime('now') WHERE id = ?",
  ).run(id);
  return getConsolidation(db, deckId, id);
}

export { KEEP_RECENT_MESSAGES };
