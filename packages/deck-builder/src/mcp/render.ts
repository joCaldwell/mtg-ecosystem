import type { Actor } from "../actor.ts";
// Text for agents (spec §10.2): compact, meant to be read, grouped the way
// the deck is organized. No JSON blobs and no ids the agent will not need.
// Counts are always the app's, stated outright, so the reader never counts.

import type { Database } from "../sqlite.ts";
import type { CardRow, ResolutionFailure } from "../cards.ts";
import type { ActivityEntry } from "../activity.ts";
import { getPower, profileLines, targetText } from "../deck/power.ts";
import { getBrief } from "../deck/brief.ts";
import { type Finding, checkDeck } from "../deck/checks.ts";
import type { DeckComparison } from "../deck/fork.ts";
import { type CardRuling, type LogEntry, listLog } from "../deck/log.ts";
import { type ProposalView, listProposals } from "../deck/proposals.ts";
import {
  type ComputedState,
  type DeckCardView,
  type DeckSummary,
  type DeckView,
  getDeck,
} from "../deck/service.ts";
import { type DocumentEditView, listDocumentEdits } from "../documents.ts";
import { type LoadedScope, type MemoryView, loadScope } from "../memory.ts";
import { type CostEstimate, formatUsd } from "../prices.ts";
import type { PlayerStats } from "../stats.ts";

// The corner stat as the card prints it: power/toughness, or loyalty.
function statline(c: { power: string | null; toughness: string | null; loyalty: string | null }): string {
  return c.power != null && c.toughness != null
    ? ` ${c.power}/${c.toughness}`
    : c.loyalty != null
      ? ` [${c.loyalty}]`
      : "";
}

const indent = (text: string, by = "    ") =>
  text
    .split("\n")
    .map((l) => `${by}${l}`)
    .join("\n");

// Spec §4.4: a card's price is the cheapest printing's, and is part of how
// the card is shown everywhere. A card with no price shows none.
function priceTag(usdCents: number | null, quantity = 1): string {
  if (usdCents == null) return "";
  return ` · ${formatUsd(usdCents)}${quantity > 1 ? " each" : ""}`;
}

/** "$412.37 at the cheapest printing of each card (Scryfall prices of 2026-10-11); 2 cards have no price and are not counted". */
function costPhrase(cost: CostEstimate & { as_of: string | null }): string {
  const missing = cost.unpriced
    ? `; ${cost.unpriced} card${cost.unpriced === 1 ? " has" : "s have"} no price and ${cost.unpriced === 1 ? "is" : "are"} not counted`
    : "";
  return `${formatUsd(cost.usd_cents)} at the cheapest printing of each card (Scryfall prices of ${cost.as_of!.slice(0, 10)})${missing}`;
}

/** One card: name, cost, type, stats, price; oracle text beneath when asked. */
export function cardLine(
  c: Pick<
    CardRow,
    "name" | "mana_cost" | "type_line" | "oracle_text" | "power" | "toughness" | "loyalty" | "usd_cents"
  >,
  opts: { text?: boolean; quantity?: number; suffix?: string } = {},
): string {
  const qty = opts.quantity && opts.quantity > 1 ? `${opts.quantity}x ` : "";
  const cost = c.mana_cost ? ` ${c.mana_cost}` : "";
  const head = `- ${qty}${c.name}${cost} — ${c.type_line}${statline(c)}${priceTag(c.usd_cents, opts.quantity)}${opts.suffix ?? ""}`;
  return opts.text === false || !c.oracle_text ? head : `${head}\n${indent(c.oracle_text)}`;
}

function range(min: number | null, max: number | null): string {
  if (min == null && max == null) return "no target";
  if (min != null && max != null) return min === max ? `target ${min}` : `target ${min}–${max}`;
  return min != null ? `target ${min}+` : `target up to ${max}`;
}

// ---------- memories and rulings, as they attach to other things ----------

export function memoryLine(m: MemoryView): string {
  const where = m.scope === "global" ? "global" : `deck "${m.deck_name}"`;
  const flags = [where, m.basis === "inferred" ? "inferred — a hypothesis" : "stated"];
  if (m.owner_touched) flags.push("the owner's");
  if (m.status !== "active") flags.push(m.status);
  return `- memory #${m.id} (${flags.join(", ")}): ${m.text}`;
}

export function rulingLine(r: CardRuling): string {
  const what =
    r.kind === "reject"
      ? `the owner rejected ${r.action === "add" ? "adding" : "cutting"} it`
      : r.kind === "accept"
        ? `the owner accepted ${r.action === "add" ? "adding" : "cutting"} it`
        : r.kind === "undo"
          ? `undone (${r.action})`
          : r.kind === "import"
            ? `${r.action === "add" ? "added" : "cut"} by an import`
            : `${r.action === "add" ? "added" : r.action === "cut" ? "cut" : "role changed"} directly by ${r.author}`;
  const where = r.from_deck ? `in "${r.from_deck}", which this deck was forked from` : `at revision ${r.revision}`;
  const parts = [`- ruling #${r.log_id} ${where}: ${what}`];
  if (r.ruling_reason) parts.push(`the owner's reason: "${r.ruling_reason}"`);
  if (r.reason && r.kind !== "edit") parts.push(`it was proposed because: "${r.reason}"`);
  if (r.undone) parts.push("(later undone)");
  return parts.join(" — ");
}

/** The context the app attaches to a card wherever it appears (spec §7.2, §8.3). */
export interface CardContext {
  memories?: MemoryView[];
  rulings?: CardRuling[];
  /** Other decks holding the card; only present when the call asked for it (§4.2). */
  myDecks?: string[];
  /** "in the deck" / "on the maybe list", when a deck was named. */
  membership?: string;
}

export function cardWithContext(card: CardRow, ctx: CardContext = {}, text = true): string {
  const lines = [cardLine(card, { text, suffix: ctx.membership ? `  (${ctx.membership})` : "" })];
  const notes: string[] = [];
  if (card.commander_legality !== "legal")
    notes.push(`- ${card.commander_legality.replace("_", " ")} in Commander`);
  for (const r of ctx.rulings ?? []) notes.push(rulingLine(r));
  for (const m of ctx.memories ?? []) notes.push(memoryLine(m));
  if (ctx.myDecks?.length) notes.push(`- in your decks: ${ctx.myDecks.join(", ")}`);
  if (notes.length) lines.push(indent(notes.join("\n"), "  "));
  return lines.join("\n");
}

export function resolutionFailureLine(f: ResolutionFailure): string {
  if (f.problem === "ambiguous")
    return (
      `- "${f.ref}" is the name of more than one card. Use an oracle_id: ` +
      f.candidates.map((c) => `${c.name} (${c.oracle_id})`).join("; ")
    );
  return f.suggestions.length
    ? `- "${f.ref}" is NOT a card name. Closest names: ${f.suggestions.join("; ")}`
    : `- "${f.ref}" is NOT a card name, and nothing similar exists.`;
}

// ---------- the deck ----------

export const DECK_SECTIONS = [
  "brief",
  "power",
  "list",
  "maybe",
  "state",
  "checks",
  "memories",
  "proposals",
  "rulings",
] as const;
export type DeckSection = (typeof DECK_SECTIONS)[number];

function header(view: DeckView): string {
  const d = view.deck;
  const bits = [
    `lifecycle ${d.category}`,
    `revision ${d.revision}`,
    `identity ${d.color_identity || (d.ci_mask === 0 ? "colourless" : "none yet (no commander)")}`,
    `${view.computed.card_count}/100 cards`,
  ];
  if (d.parent) bits.push(`forked from deck ${d.parent.id} "${d.parent.name}"`);
  const rule = d.category === "legacy"
    ? "Legacy: read-only. View, export or fork it; only the owner can restore it."
    : (d.category === "paper"
      ? "Paper: change its cards only with propose_changes."
      : "Theorycrafted: edit its cards directly with edit_cards.") + " Brief and power target changes always require proposals.";
  return `# Deck ${d.id} "${d.name}" — ${bits.join(" · ")}\n${rule}`;
}

export function stateLines(view: DeckView): string {
  const c = view.computed;
  const lines = [
    `- Cards: ${c.card_count}/100 (${countPhrase(c)})`,
    `- Lands: ${c.land_count} · average mana value of nonlands: ${
      c.average_mana_value == null ? "n/a" : c.average_mana_value.toFixed(2)
    }`,
    `- Curve (mana value: cards): ${Object.entries(c.curve).map(([b, n]) => `${b}: ${n}`).join(", ")}`,
    `- Coloured pips in costs: ${
      Object.entries(c.pips).filter(([, n]) => n > 0).map(([k, n]) => `${k} ${n}`).join(", ") || "none"
    }`,
  ];
  // Nothing to say until prices have been loaded once.
  if (c.cost.as_of) lines.push(`- Estimated cost: ${costPhrase(c.cost)}`);
  if (c.pending.adds || c.pending.cuts)
    lines.push(
      `- Awaiting the owner's ruling: +${c.pending.adds} / −${c.pending.cuts}; if all were accepted the deck would be ${c.pending.projected_count}/100`,
    );
  if (view.slots.length) {
    lines.push(`- Slots:`);
    for (const s of c.slot_deltas) {
      const status =
        s.status === "under"
          ? ` — UNDER by ${-s.delta}`
          : s.status === "over"
            ? ` — OVER by ${s.delta}`
            : "";
      lines.push(`  - ${s.name}: ${s.count} primary (${range(s.target_min, s.target_max)})${status}; ${s.secondary_count} secondary; ${s.coverage_count} total coverage`);
    }
    lines.push(`  - unslotted: ${c.unslotted_count}`);
  }
  if (c.maybe_count) lines.push(`- On the maybe list (outside the 100): ${c.maybe_count}`);
  return lines.join("\n");
}

function countPhrase(c: ComputedState): string {
  return c.delta_to_100 === 0
    ? "exactly 100"
    : c.delta_to_100 > 0
      ? `${c.delta_to_100} over`
      : `${-c.delta_to_100} short`;
}

export function findingLines(findings: Finding[]): string {
  if (!findings.length) return "No problems found by the deterministic checks.";
  return findings
    .map((f) => `- ${f.severity === "error" ? "PROBLEM" : "note"}: ${f.title}. ${f.detail}`)
    .join("\n");
}

/** One line after a write: the numbers an agent would otherwise go and count. */
export function afterWrite(db: Database, actor: Actor, deckId: number): string {
  const view = getDeck(db, actor, deckId);
  const c = view.computed;
  const problems = checkDeck(db, actor, deckId);
  const off = c.slot_deltas
    .filter((s) => s.status === "under" || s.status === "over")
    .map((s) => `${s.name} ${s.status} by ${Math.abs(s.delta)}`);
  return [
    `Deck ${view.deck.id} "${view.deck.name}" is now at revision ${view.deck.revision}: ${c.card_count}/100 cards (${countPhrase(c)}), ${c.land_count} lands.`,
    c.cost.as_of ? `Estimated cost: ${costPhrase(c.cost)}.` : null,
    off.length ? `Slots off target: ${off.join("; ")}.` : null,
    problems.length
      ? `Checks:\n${findingLines(problems)}`
      : "Checks: no problems.",
  ]
    .filter(Boolean)
    .join("\n");
}

function listSection(view: DeckView, withText: boolean): string {
  const tagName = new Map(view.tags.map((t) => [t.id, t.name]));
  const slotName = new Map(view.slots.map(s => [s.id, s.name]));
  const line = (c: DeckCardView) => {
    const tags = c.tag_ids.map((id) => tagName.get(id)).filter(Boolean);
    return cardLine(c, {
      text: withText,
      quantity: c.quantity,
      suffix: [c.role !== "card" && c.slot_id != null ? `  [primary slot: ${slotName.get(c.slot_id)}]` : "",
        c.secondary_slot_ids.length ? `  [secondary slots: ${c.secondary_slot_ids.map(id => slotName.get(id)).join(", ")}]` : "",
        tags.length ? `  [tags: ${tags.join(", ")}]` : ""].join(""),
    });
  };
  const main = view.cards.filter((c) => !c.maybe);
  const groups: string[] = [];
  const commanders = main.filter((c) => c.role === "commander");
  groups.push(
    `### Command zone (${commanders.length})\n${commanders.map(line).join("\n") || "(empty — no commander chosen)"}`,
  );
  const cards = main.filter((c) => c.role === "card");
  for (const s of view.slots) {
    const inSlot = cards.filter((c) => c.slot_id === s.id);
    const delta = view.computed.slot_deltas.find((d) => d.slot_id === s.id)!;
    groups.push(
      `### ${s.name} (${delta.count} primary; ${range(s.target_min, s.target_max)}; ${delta.secondary_count} secondary)\n${inSlot.map(line).join("\n") || "(no ordinary cards primarily filed here)"}` +
      (delta.secondary_count ? `\nSecondary contributors: ${main.filter(c => c.role !== "companion" && c.secondary_slot_ids.includes(s.id)).map(c => `${c.quantity > 1 ? `${c.quantity}x ` : ""}${c.name}`).join("; ")}` : ""),
    );
  }
  const unslotted = cards.filter((c) => c.slot_id == null);
  if (unslotted.length)
    groups.push(
      `### ${view.slots.length ? "Unslotted" : "Cards"} (${view.computed.unslotted_count})\n${unslotted.map(line).join("\n")}`,
    );
  const companion = main.filter((c) => c.role === "companion");
  if (companion.length)
    groups.push(`### Companion (outside the 100)\n${companion.map(line).join("\n")}`);

  const vocab = [
    view.slots.length
      ? null
      : "This deck defines no slots; that is allowed. Define some with define_slots if filing would help.",
    view.slots.length ? "List grouped by primary purpose. Targets count only primary assignments; secondary purposes are shown separately." : null,
    view.tags.length ? `Tags in use: ${view.tags.map((t) => t.name).join(", ")}` : null,
  ].filter(Boolean);
  return [`## List`, ...vocab, ...groups].join("\n\n");
}

export function renderScope(loaded: LoadedScope, label: string): string {
  if (!loaded.memories.length && !loaded.omitted) return `(no ${label} memories yet)`;
  const lines = loaded.memories.map(memoryLine);
  if (loaded.over)
    lines.push(
      `- OVER BUDGET: ${loaded.omitted} more ${label} memor${loaded.omitted === 1 ? "y" : "ies"} did not fit ` +
        `(${loaded.used}/${loaded.budget} estimated tokens loaded). Consolidate this scope: ` +
        `search_memories to read the rest, then merge, supersede or retire with revise_memory.`,
    );
  return lines.join("\n");
}

export function proposalLines(proposals: ProposalView[], edits: DocumentEditView[]): string {
  const lines: string[] = [];
  for (const p of proposals) {
    const pending = p.items.filter((i) => i.status === "pending");
    if (!pending.length) continue;
    lines.push(`- proposal #${p.id} by ${p.author}${p.summary ? `: ${p.summary}` : ""} [current; based on r${p.base_revision ?? "unknown"}${p.stale ? "; stale" : ""}]`);
    for (const i of pending)
      lines.push(
        `  - ${i.action} ${i.card_name}${i.to_maybe ? ", moving it to the maybe list" : ""}${i.secondary_slot_names.length ? ` [secondary slots: ${i.secondary_slot_names.join(", ")}]` : ""}${i.group_id ? ` (group "${i.group_id}")` : ""}: ${i.reason}`,
      );
  }
  for (const e of edits)
    lines.push(
      `- proposed ${e.kind === "profile" ? "profile" : `brief (${e.kind.replace("_", " ")})`} edit #${e.id} by ${e.author}: ${e.reason} [current${e.group_id ? `; approval group #${e.group_id}` : ""}; based on r${e.base_revision ?? "unknown"}${e.stale ? "; stale" : ""}]`,
    );
  return lines.join("\n") || "(nothing is awaiting the owner's ruling)";
}

export function logLines(entries: LogEntry[]): string {
  if (!entries.length) return "(none)";
  return entries
    .map((e) => {
      const head = [`- #${e.id}`];
      if (e.deck_name) head.push(`deck ${e.deck_id} "${e.deck_name}" rev ${e.revision}`);
      else head.push("profile");
      head.push(e.ts.slice(0, 10), `by ${e.author}`);
      const verb =
        e.kind === "accept" ? "ACCEPTED" : e.kind === "reject" ? "REJECTED" : e.kind.toUpperCase();
      let line = `${head.join(" · ")} — ${verb}: ${e.summary}`;
      if (e.ruling_reason) line += ` — the owner's reason: "${e.ruling_reason}"`;
      if (e.undone_by) line += ` (undone by #${e.undone_by})`;
      if (e.reviewed_at == null) line += " [unreviewed]";
      // The cards of a ruling carry the proposer's reasons; a long batch is
      // already summarized and is not repeated card by card.
      const cards =
        e.kind === "accept" || e.kind === "reject" || e.cards.length <= 6
          ? e.cards
              .map(
                (c) =>
                  `  - ${c.action} ${c.quantity > 1 ? `${c.quantity}x ` : ""}${c.card_name}${c.reason && c.action !== "role" ? `: ${c.reason}` : ""}`,
              )
              .join("\n")
          : "";
      return cards ? `${line}\n${cards}` : line;
    })
    .join("\n");
}

export function renderDeck(
  db: Database, actor: Actor,
  deckId: number,
  sections: readonly DeckSection[] = DECK_SECTIONS,
  withText = true,
): string {
  const view = getDeck(db, actor, deckId);
  const want = new Set(sections);
  const out: string[] = [header(view)];

  if (want.has("brief")) {
    const brief = getBrief(db, actor, deckId);
    const engines = brief.engines.map((e) => {
      const pieces = e.pieces
        .map((p) => `${p.name}${p.in_deck ? "" : " (NOT in the deck)"}${p.note ? ` — ${p.note}` : ""}`)
        .join("; ");
      return `- ${e.name}: ${e.description || "(no description)"}\n  pieces: ${pieces || "(none listed)"}`;
    });
    out.push(
      `## Brief (the owner's document)\n### Thesis\n${brief.thesis || "(not written yet)"}\n### Constraints\n${
        brief.constraints_md || "(none recorded)"
      }\n### Engines\n${engines.join("\n") || "(none defined)"}`,
    );
  }
  if (want.has("power")) out.push(renderPower(getPower(db, actor, deckId)));
  if (want.has("list")) out.push(listSection(view, withText));
  if (want.has("maybe")) {
    const maybe = view.cards.filter((c) => c.maybe);
    if (maybe.length)
      out.push(
        `## Maybe list (${maybe.length}; outside the 100)\n` +
          maybe
            .map(c => cardLine(c, { text: withText, suffix: [
              c.slot_id == null ? "" : `  [primary slot: ${view.slots.find(s => s.id === c.slot_id)?.name}]`,
              c.secondary_slot_ids.length ? `  [secondary slots: ${c.secondary_slot_ids.map(id => view.slots.find(s => s.id === id)?.name).join(", ")}]` : "",
              c.maybe_note ? `  — ${c.maybe_note}` : "",
            ].join("") }))
            .join("\n"),
      );
  }
  if (want.has("state")) out.push(`## Computed state (the app's counts; do not recount)\n${stateLines(view)}`);
  if (want.has("checks")) out.push(`## Checks\n${findingLines(checkDeck(db, actor, deckId))}`);
  if (want.has("memories"))
    out.push(`## Memories for this deck\n${renderScope(loadScope(db, actor, "deck", deckId), "deck")}`);
  if (want.has("proposals"))
    out.push(
      `## Awaiting the owner's ruling\n${proposalLines(
        listProposals(db, actor, { deckId, status: "open" }),
        listDocumentEdits(db, actor, { deckId, status: "pending" }),
      )}`,
    );
  if (want.has("rulings"))
    out.push(`## Recent rulings and changes (newest first)\n${logLines(listLog(db, actor, { deckId, limit: 15 }))}`);
  return out.join("\n\n");
}

export function renderPower(power: ReturnType<typeof getPower>): string {
  const mapping = (m: ReturnType<typeof getPower>["history"][number]["mapping"]) => [
    `Suggested bracket: ${m.bracket == null ? "Undetermined" : `${m.bracket} — ${m.name}`}${m.provisional ? " (provisional)" : ""}`,
    ...m.reasons.filter(s => !m.conflicts.includes(s)).map(s => `  - ${s}`),
    ...m.conflicts.map(s => `  - REVIEW: ${s}`),
    ...(m.unknowns.length ? [`  - Unknown: ${m.unknowns.join("; ")}`] : []),
  ].join("\n");
  const lines = ["## Power — target and current assessment", "### Target", power.target
    ? `${targetText(power.target)}\n${mapping(power.target.mapping)}` : "Not set. Use propose_power_target to help the owner set one."];
  lines.push("### Current assessment");
  const a = power.assessment;
  if (a) lines.push(`Assessment #${a.id} by ${a.author}, r${a.revision}, ${a.confidence}, evidence: ${a.evidence}${a.owner_touched ? "; the owner's assessment takes precedence" : ""}${a.stale ? " — STALE: cards changed; assess the current revision again" : ""}`,
    profileLines(a.profile).join("\n"), `Reasoning: ${a.notes}`, mapping(a.mapping));
  else lines.push("Not assessed. Read the list, then use assess_deck_power with its revision and your evidence.");
  const g = power.game_changers;
  lines.push(`Game Changers in the current list (app's count): ${g.count}${g.unknown_cards ? ` known; ${g.unknown_cards} card(s) have no metadata — refresh card data` : ""}${g.cards.length ? ` — ${g.cards.map(c => c.name).join("; ")}` : ""}`);
  if (power.differences.length) lines.push("### Differences from target", ...power.differences.map(d => `- ${d.field}: target ${d.target}; current ${d.current}`));
  lines.push("Field anchors: " + power.fields.map(f => `${f.label}: ${f.options.map(o => `${o.value} (${o.hint})`).join(" / ")}`).join("\n"));
  lines.push(`Bracket suggestions use Wizards guidance of ${power.guidance.version}; they guide pregame discussion, not rules certification. Unknowns and conflicts require review. Consistency, resilience and interaction are descriptive, not averaged into a score.`);
  return lines.join("\n");
}

// ---------- everything else ----------

export function rosterLines(decks: DeckSummary[]): string {
  if (!decks.length) return "(no decks yet)";
  return decks
    .map((d) => {
      const bits = [
          d.commanders.length ? d.commanders.join(" + ") : "no commander",
        d.color_identity || "—",
        `${d.card_count}/100`,
        `lifecycle ${d.category}`,
      ];
      if (d.parent_name) bits.push(`fork of "${d.parent_name}"`);
      const waiting = d.pending_items + d.pending_document_edits;
      if (waiting) bits.push(`${waiting} awaiting ruling`);
      return `- deck ${d.id} "${d.name}" — ${bits.join(" · ")}`;
    })
    .join("\n");
}

export function renderMemories(memories: MemoryView[]): string {
  if (!memories.length) return "No memories match.";
  return memories
    .map((m) => {
      const extra = [`written by ${m.author} ${m.created_at.slice(0, 10)}`];
      if (m.revised_at !== m.created_at) extra.push(`revised ${m.revised_at.slice(0, 10)}`);
      if (m.deck_revision != null) extra.push(`at deck revision ${m.deck_revision}`);
      if (m.cards.length) extra.push(`cards: ${m.cards.map((c) => c.name).join(", ")}`);
      if (m.superseded_by) extra.push(`superseded by #${m.superseded_by}`);
      return `${memoryLine(m)}\n  (${extra.join("; ")})`;
    })
    .join("\n");
}

export function renderCompare(d: DeckComparison): string {
  const cards = (list: DeckComparison["only_in_a"]) =>
    list
      .map((c) => `- ${c.quantity > 1 ? `${c.quantity}x ` : ""}${c.name}${c.mana_cost ? ` ${c.mana_cost}` : ""} — ${c.type_line}${c.role !== "card" ? ` (${c.role})` : ""}`)
      .join("\n") || "(none)";
  const out = [
    `# Deck ${d.a.id} "${d.a.name}" (rev ${d.a.revision}, ${d.a.card_count} cards) compared with ` +
      `deck ${d.b.id} "${d.b.name}" (rev ${d.b.revision}, ${d.b.card_count} cards)` +
      (d.b_is_parent ? " — its parent" : ""),
    `${d.shared_count} cards are in both. Maybe lists are not compared.`,
    `## Only in "${d.a.name}" (${d.only_in_a.length})\n${cards(d.only_in_a)}`,
    `## Only in "${d.b.name}" (${d.only_in_b.length})\n${cards(d.only_in_b)}`,
  ];
  if (d.quantity_differs.length)
    out.push(
      `## Different quantities\n${d.quantity_differs.map((q) => `- ${q.name}: ${q.in_a} vs ${q.in_b}`).join("\n")}`,
    );
  if (d.role_differs.length)
    out.push(
      `## Different roles\n${d.role_differs.map((q) => `- ${q.name}: ${q.in_a} vs ${q.in_b}`).join("\n")}`,
    );
  return out.join("\n\n");
}

export function renderStats(s: PlayerStats): string {
  const out = [
    `# What the owner builds — computed from ${s.deck_count} Paper deck(s) in ${s.families} famil${s.families === 1 ? "y" : "ies"}`,
    "These are the app's counts. A fork and its parent count as one family toward reuse.",
    `## Decks\n${
      s.decks
        .map(
          (d) =>
            `- deck ${d.id} "${d.name}" (Paper${d.forked_from ? `, fork of "${d.forked_from}"` : ""}): ` +
            `${d.commanders.join(" + ") || "no commander"} · ${d.color_identity || "—"} · ${d.card_count} cards · ` +
            `${d.land_count} lands · avg MV ${d.average_mana_value ?? "n/a"}`,
        )
        .join("\n") || "(none)"
    }`,
    `## Colours\nDecks including each colour: ${Object.entries(s.colors).map(([c, n]) => `${c} ${n}`).join(", ")}\n` +
      `By exact identity: ${s.identities.map((i) => `${i.identity} ×${i.decks}`).join(", ") || "(none)"}`,
    `## Commander creature types\n${s.commander_types.map((t) => `${t.type} ×${t.decks}`).join(", ") || "(none)"}`,
    `## Averages per deck\nMana value of nonlands: ${s.averages.mana_value ?? "n/a"} · lands: ${s.averages.lands ?? "n/a"}\n` +
      `Cards by type: ${Object.entries(s.averages.by_type).map(([k, n]) => `${k} ${n}`).join(", ") || "(none)"}\n` +
      `Curve: ${Object.entries(s.averages.curve).map(([k, n]) => `${k}: ${n}`).join(", ") || "(none)"}`,
  ];
  if (s.slots.length)
    out.push(
      `## Slots the owner uses\n${s.slots.map((x) => `- ${x.name}: in ${x.decks} deck(s), averaging ${x.average_count} cards`).join("\n")}`,
    );
  out.push(
    `## Nonbasic cards in more than one deck family (${s.reused_cards.length} shown)\n${
      s.reused_cards.map((c) => `- ${c.name} — ${c.type_line} — ${c.decks} families`).join("\n") || "(none)"
    }`,
  );
  return out.join("\n\n");
}

export function renderActivity(entries: ActivityEntry[]): string {
  return entries.map((e) => `- ${e.ts} ${e.actor} ${e.action}: ${e.summary}`).join("\n") || "(none)";
}
