// Context assembly (spec §10) — the single most important function in the
// codebase. Segment order is driven by stability so provider-side prefix
// caching gets a stable head: rules → brief/slots/tags → decklist →
// computed state → pending proposals → decision log residue. The transcript
// follows as messages; the tail restate block rides in front of the user's
// message text.
//
// HARD RULE: the `owned` flag never appears anywhere in this file's output.

import type { DatabaseSync } from "node:sqlite";
import { getDeck } from "../deck/service.ts";
import { getBrief } from "../deck/brief.ts";
import { listProposals } from "../deck/proposals.ts";
import type { ChatMessage } from "./llm.ts";

// Segment 1 — agent rules and output contract (~600 tokens, static).
export const AGENT_RULES = `You are the resident deck-building agent for a Magic: The Gathering Commander deck. You collaborate with the deck's owner through proposals — you never edit the deck directly.

CARD KNOWLEDGE CONTRACT (highest priority):
- You may reference a card ONLY if it appears in the current decklist below or in a search_cards result from this conversation. Your memory of Magic cards is a query generator, not a fact source: if you have a hunch a card exists, search for it before saying anything about it.
- Prefer re-searching over trusting an old search result from many turns ago — oracle text in this context is authoritative; your recollection is not.
- Every card name you write in prose MUST use [[Card Name]] syntax, spelled exactly as the database returned it. Unresolvable names are bounced back to you for correction and never reach the user.
- When you argue a rules interaction, quote the specific oracle clause you are relying on, verbatim, from text visible in this context.
- Never count cards or compute totals yourself — the computed state below is authoritative; read it.

PROPOSALS:
- Deck changes go through the propose_changes tool. A proposal holds 3–5 items maximum — rank your best ideas; the cap is the point. Each item needs a rationale.
- Use the same group_id for items that must be accepted or rejected together (a swap justified by one line of reasoning). Leave unrelated items ungrouped.
- Slots are an optional organizational overlay, not a precondition. A deck may define none, and cards may sit unslotted indefinitely. Never withhold a proposal, or ask the owner to create a slot, because a card has nowhere to go — set slot_name only when a fitting slot already exists, and omit it otherwise.
- Brief changes go through propose_brief_edit / propose_engine_edit. The brief is the deck's durable intent — propose edits when the owner's rulings show the written brief is stale.
- Do not re-propose a card the owner rejected unless something material changed. When you do re-propose, cite the prior rejection (use get_card_history) and state what changed. If the owner pushes back, you get ONE counter-argument — with the oracle clause you are relying on — then defer.
- Never suggest a hard-filtered card. They are excluded from your searches; the filter list below is a reminder, not an invitation.

AUDIT REFERENCES:
- The owner can hand you a finding from a recorded audit run as a token like \`audit#12/reasoning:no-win-path\`. That token is a pointer, not the finding — call get_audit with its run_id and finding_key and read the finding before you respond. Never infer what it said from the slug.
- An audit run is a snapshot taken at a deck revision, not a description of the deck in front of you. Findings from §8.1 (counts, legality, slot deltas) are authoritative for the revision they were taken at; the computed state below is authoritative for now. Reasoning findings (§8.2) are another model's judgement — engage with them, agree or disagree from the oracle text, and say which.

STYLE:
- Be direct and specific. Lead with the recommendation, then the reasoning.
- The decklist below is grouped by slot where slots exist; read slot deltas from the computed state rather than inferring them.
- If the brief is empty, help the owner articulate a thesis before proposing card changes.`;

export interface AssembledContext {
  system: string;
  transcript: ChatMessage[];
  tailRestate: string;
  segments: ContextSegment[];
}

// One row of the segmented context meter (spec §11). Kept alongside the
// assembled text rather than recomputed elsewhere, so the meter always
// measures exactly what was sent.
export interface ContextSegment {
  key: string;
  label: string;
  text: string;
}

export interface DeckSections {
  // Segments 2–6 of spec §10, joined — everything about the deck, minus the
  // chat agent's rules. Reused by the audit reasoning pass.
  sections: string;
  tailRestate: string;
  segments: ContextSegment[];
}

function fmtCardLine(c: {
  name: string;
  mana_cost: string | null;
  type_line: string;
  power: string | null;
  toughness: string | null;
  loyalty: string | null;
  quantity: number;
  oracle_text: string;
  tagNames: string[];
}): string {
  const qty = c.quantity > 1 ? `${c.quantity}x ` : "";
  const cost = c.mana_cost ? ` ${c.mana_cost}` : "";
  const pt =
    c.power != null && c.toughness != null
      ? ` ${c.power}/${c.toughness}`
      : c.loyalty != null
        ? ` [${c.loyalty}]`
        : "";
  const tags = c.tagNames.length ? ` (tags: ${c.tagNames.join(", ")})` : "";
  const text = c.oracle_text
    ? "\n" + c.oracle_text.split("\n").map((l) => `    ${l}`).join("\n")
    : "";
  return `- ${qty}[[${c.name}]]${cost} — ${c.type_line}${pt}${tags}${text}`;
}

export function assembleDeckSections(
  db: DatabaseSync,
  deckId: number,
  retentionN: number,
): DeckSections {
  const state = getDeck(db, deckId);
  const brief = getBrief(db, deckId);
  const { deck, slots, tags, cards, computed } = state;

  // ---- Segments 2–3: brief, then slots + tag vocabulary (seldom change) ----
  const engineLines = brief.engines.map((e) => {
    const pieces = e.pieces
      .map((p) => `[[${p.name}]]${p.in_deck ? "" : " (NOT in deck)"}`)
      .join(", ");
    return `- ${e.name}: ${e.description || "(no description)"}\n  pieces: ${pieces || "(none listed)"}`;
  });
  const slotLines = slots.map((s) => {
    const target =
      s.target_min != null || s.target_max != null
        ? ` (target ${s.target_min ?? 0}–${s.target_max ?? "∞"})`
        : " (no target)";
    return `- ${s.name}${target}`;
  });

  const segBrief = `# Deck brief — ${deck.name}

## Thesis
${brief.thesis || "(not written yet)"}

## Constraints
${brief.constraints_md || "(none recorded)"}

## Named engines
${engineLines.join("\n") || "(none defined)"}`;

  const segSlots = `# Slots (optional — a deck may define none; unslotted is a valid resting place)
${slotLines.join("\n") || "(none defined — every card is unslotted, which is fine; propose changes without a slot)"}

# Tag vocabulary (controlled — propose additions, never invent)
${tags.map((t) => t.name).join(", ") || "(empty)"}`;

  // ---- Segment 4: decklist grouped by slot (changes per accepted proposal) ----
  const tagName = new Map(tags.map((t) => [t.id, t.name]));
  const withTags = (c: (typeof cards)[0]) => ({
    ...c,
    tagNames: c.tag_ids.map((id) => tagName.get(id)).filter((n): n is string => !!n),
  });

  const commanders = cards.filter((c) => c.role === "commander").map(withTags);
  const companion = cards.filter((c) => c.role === "companion").map(withTags);
  const main = cards.filter((c) => c.role === "card").map(withTags);

  const groups: string[] = [];
  groups.push(
    `## Command Zone (${commanders.length})\n${commanders.map(fmtCardLine).join("\n") || "(empty — no commander chosen)"}`,
  );
  for (const s of slots) {
    const inSlot = main.filter((c) => c.slot_id === s.id);
    const delta = computed.slot_deltas.find((d) => d.slot_id === s.id);
    const status = delta && delta.status !== "untargeted" && delta.status !== "ok"
      ? ` — ${delta.status} by ${Math.abs(delta.delta)}`
      : "";
    groups.push(
      `## ${s.name} (${delta?.count ?? 0}${status})\n${inSlot.map(fmtCardLine).join("\n") || "(empty)"}`,
    );
  }
  const unslotted = main.filter((c) => c.slot_id == null);
  if (unslotted.length)
    groups.push(`## Unslotted (${computed.unslotted_count})\n${unslotted.map(fmtCardLine).join("\n")}`);
  if (companion.length)
    groups.push(`## Companion (outside the 100)\n${companion.map(fmtCardLine).join("\n")}`);

  const segDecklist = `# Decklist — ${computed.card_count}/100 cards, identity ${deck.color_identity || "(none)"}\n\n${groups.join("\n\n")}`;

  // ---- Segment 5: computed state (given, never derived) ----
  const slotDeltaLines = computed.slot_deltas.map((d) => {
    const target = d.target_min != null || d.target_max != null
      ? `${d.target_min ?? 0}–${d.target_max ?? "∞"}`
      : "none";
    return `- ${d.name}: ${d.count} (target ${target}, ${d.status}${d.delta !== 0 ? ` ${d.delta > 0 ? "+" : ""}${d.delta}` : ""})`;
  });
  const violations = [
    ...computed.identity_violations.map((v) => `- identity: [[${v.name}]] (${v.color_identity}) outside ${deck.color_identity}`),
    ...computed.singleton_violations.map((v) => `- copies: ${v.quantity}x [[${v.name}]] exceeds limit ${v.limit ?? "∞"}`),
    ...computed.legality_violations.map((v) => `- legality: [[${v.name}]] is ${v.legality}`),
  ];
  const curveLine = Object.entries(computed.curve).map(([b, n]) => `${b}:${n}`).join(" ");
  const pipLine = Object.entries(computed.pips).filter(([, n]) => n > 0).map(([c, n]) => `${c}${n}`).join(" ");

  const segComputed = `# Computed state (authoritative — never recount)
- Cards: ${computed.card_count}/100 (${computed.delta_to_100 === 0 ? "exact" : computed.delta_to_100 > 0 ? `${computed.delta_to_100} over` : `${-computed.delta_to_100} short`})
- Pending proposals: +${computed.pending.adds}/−${computed.pending.cuts} → would be ${computed.pending.projected_count}
- Lands: ${computed.land_count} | Curve (MV:count): ${curveLine} | Pips: ${pipLine || "—"}
- Unslotted: ${computed.unslotted_count}
${slotDeltaLines.length ? `- Slot deltas:\n${slotDeltaLines.map((l) => `  ${l}`).join("\n")}` : ""}
${violations.length ? `- Violations:\n${violations.map((l) => `  ${l}`).join("\n")}` : "- Violations: none"}`;

  // ---- Segment 6: pending proposals ----
  const open = listProposals(db, deckId, "open");
  const pendingLines = open.flatMap((p) =>
    p.items
      .filter((i) => i.status === "pending")
      .map(
        (i) =>
          `- [#${p.id}/${i.id}] ${i.action} [[${i.card_name}]]${i.group_id ? ` (group ${i.group_id})` : ""}: ${i.rationale}`,
      ),
  );
  const pendingBriefEdits = db
    .prepare("SELECT kind, rationale FROM brief_edits WHERE deck_id = ? AND status = 'pending'")
    .all(deckId) as unknown as Array<{ kind: string; rationale: string }>;
  const segPending = `# Awaiting the owner's ruling
${pendingLines.join("\n") || "(no pending proposal items)"}
${pendingBriefEdits.length ? pendingBriefEdits.map((e) => `- [brief/${e.kind}] ${e.rationale}`).join("\n") : ""}`;

  // ---- Segment 7: decision log resident portion (spec §12) ----
  const hardFilters = db
    .prepare("SELECT card_name, reason FROM hard_filters WHERE deck_id = ? ORDER BY card_name")
    .all(deckId) as unknown as Array<{ card_name: string; reason: string }>;
  const playtestNotes = db
    .prepare("SELECT card_name, note FROM card_notes WHERE deck_id = ? ORDER BY id DESC")
    .all(deckId) as unknown as Array<{ card_name: string; note: string }>;
  // Deck-level playtest notes from goldfishing in Archidekt (spec §9). Kept
  // forever alongside the card-specific ones (spec §12).
  const deckNotes = db
    .prepare("SELECT revision, note FROM playtest_notes WHERE deck_id = ? ORDER BY id DESC")
    .all(deckId) as unknown as Array<{ revision: number; note: string }>;
  const recent = db
    .prepare(
      `SELECT kind, action, card_name, rationale, rejection_type, rejection_reason, brief_flag
       FROM decision_log WHERE deck_id = ? ORDER BY id DESC LIMIT ?`,
    )
    .all(deckId, retentionN) as unknown as Array<{
    kind: string;
    action: string | null;
    card_name: string | null;
    rationale: string | null;
    rejection_type: string | null;
    rejection_reason: string | null;
    brief_flag: number;
  }>;

  const recentLines = recent.map((e) => {
    const card = e.card_name ? ` [[${e.card_name}]]` : "";
    if (e.kind === "reject")
      return `- rejected${card} (${e.rejection_type}): "${e.rejection_reason}"${e.brief_flag ? " ← flagged for brief review" : ""}`;
    if (e.kind === "undo")
      return e.action === "import" ? `- undid an import${card}` : `- undid ${e.action}${card}`;
    // A whole pasted list, applied at once — one line, not one per card.
    if (e.action === "import") return `- ${e.rationale ?? "imported a list"}`;
    if (e.kind === "filter_removed") return `- hard filter removed${card}`;
    return `- accepted ${e.action ?? "edit"}${card}${e.rationale ? `: ${e.rationale}` : ""}`;
  });

  const segLog = `# Decision log (owner rulings — this is what you learn from)

## Hard filters (never suggest; excluded from your searches)
${hardFilters.map((f) => `- [[${f.card_name}]]: "${f.reason}"`).join("\n") || "(none)"}

## Playtest findings (strongest evidence — from real games)
${playtestNotes.map((n) => `- [[${n.card_name}]]: "${n.note}"`).join("\n") || "(none)"}

## Playtest notes from goldfishing (whole-deck, tagged with the deck revision)
${deckNotes.map((n) => `- (rev ${n.revision}) "${n.note}"`).join("\n") || "(none)"}

## Recent decisions (newest first, last ${retentionN})
${recentLines.join("\n") || "(none yet)"}`;

  // ---- Tail restate block (~75 tokens, deliberately redundant) ----
  const under = computed.slot_deltas.filter((d) => d.status === "under").map((d) => `${d.name} ${d.delta}`);
  const over = computed.slot_deltas.filter((d) => d.status === "over").map((d) => `${d.name} +${d.delta}`);
  const tailRestate = [
    `Cards to 100: ${-computed.delta_to_100 === 0 ? "at 100" : computed.delta_to_100 > 0 ? `${computed.delta_to_100} over` : `${-computed.delta_to_100} needed`}`,
    under.length ? `Under target: ${under.join(", ")}` : null,
    over.length ? `Over target: ${over.join(", ")}` : null,
    hardFilters.length ? `Hard-filtered (never suggest): ${hardFilters.map((f) => f.card_name).join(", ")}` : null,
  ]
    .filter(Boolean)
    .join(" | ");

  const segments: ContextSegment[] = [
    { key: "brief", label: "Deck brief", text: segBrief },
    { key: "slots", label: "Slots & tag vocabulary", text: segSlots },
    { key: "decklist", label: "Decklist with oracle text", text: segDecklist },
    { key: "computed", label: "Computed state", text: segComputed },
    { key: "pending", label: "Pending proposals", text: segPending },
    { key: "log", label: "Decision log (resident portion)", text: segLog },
  ];

  return {
    sections: segments.map((s) => s.text).join("\n\n---\n\n"),
    tailRestate,
    segments,
  };
}

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

export function assembleContext(
  db: DatabaseSync,
  deckId: number,
  retentionN: number,
): AssembledContext {
  const { sections, tailRestate, segments } = assembleDeckSections(db, deckId, retentionN);

  // Compacted messages stay on disk but leave context (spec §11).
  const rows = db
    .prepare(
      "SELECT role, content_json FROM chat_messages WHERE deck_id = ? AND compacted_at IS NULL ORDER BY id",
    )
    .all(deckId) as unknown as Array<{ role: string; content_json: string }>;
  const resident = rows.map((r) => JSON.parse(r.content_json) as ChatMessage);

  const summary = activeSummary(db, deckId);
  const transcript: ChatMessage[] = summary
    ? [
        {
          role: "system",
          content: `<compacted_history messages="${summary.message_count}">\nEarlier in this chat, condensed and approved by the owner. Conversational texture only — the deck, brief, and decision log above are the source of truth.\n\n${summary.summary}\n</compacted_history>`,
        },
        ...resident,
      ]
    : resident;

  return {
    system: [AGENT_RULES, sections].join("\n\n---\n\n"),
    transcript,
    tailRestate,
    segments: [
      { key: "rules", label: "Agent rules & output contract", text: AGENT_RULES },
      ...segments,
      {
        key: "transcript",
        label: "Session transcript",
        text: transcript.map((m) => JSON.stringify(m)).join("\n"),
      },
      { key: "tail_restate", label: "Tail restate block", text: tailRestate },
    ],
  };
}
