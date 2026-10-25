import { requireDeckOwner } from "../actor.ts";
// Spec §5.7: power is a declared profile, not an algorithm that evaluates cards.
// The app maps the profile to bracket guidance and computes Game Changers.
import type { Database } from "../sqlite.ts";
import * as z from "zod/v4";
import { type Actor, type Attribution, displayAuthor, authorOf, PermissionError, requireUi } from "../actor.ts";
import { recordActivity } from "../activity.ts";
import { lintProse } from "../cards.ts";
import { withTransaction } from "../db.ts";
import { ServiceError } from "../errors.ts";
import { requireDeck, requireEditableDeck } from "./service.ts";

const turn = z.number().int().min(1).max(99);
export const PowerProfileSchema = z.strictObject({
  intent: z.enum(["theme", "social", "synergy", "optimized", "competitive"]).nullable().default(null),
  win_turn: z.strictObject({ min: turn, max: turn }).refine(v => v.min <= v.max, "The first turn must not exceed the last turn").nullable().default(null),
  early_win_turn: turn.nullable().default(null),
  consistency: z.enum(["variable", "reliable", "repeatable"]).nullable().default(null),
  resilience: z.enum(["fragile", "recovering", "persistent"]).nullable().default(null),
  interaction: z.enum(["light", "flexible", "efficient"]).nullable().default(null),
  win_style: z.enum(["incremental", "burst", "combo"]).nullable().default(null),
  combos: z.enum(["none", "multi_card", "late_two_card", "early_two_card"]).nullable().default(null),
  extra_turns: z.enum(["none", "single", "chained"]).nullable().default(null),
  mass_land_denial: z.boolean().nullable().default(null),
}).refine(v => v.early_win_turn == null || v.win_turn == null || v.early_win_turn <= v.win_turn.min,
  "An occasional early win must be at or before the usual win range");

export const PowerTargetSchema = z.strictObject({
  profile: PowerProfileSchema,
  game_changers_max: z.number().int().min(0).max(100).nullable().default(null),
  notes: z.string().max(6000).default(""),
});
export const PowerAssessmentSchema = z.strictObject({
  profile: PowerProfileSchema,
  revision: z.number().int().min(0),
  evidence: z.enum(["list", "goldfish", "games"]),
  confidence: z.enum(["tentative", "supported"]),
  notes: z.string().trim().min(1, "Explain the assessment and its evidence").max(6000),
});
export type PowerProfile = z.output<typeof PowerProfileSchema>;
export type PowerTarget = z.output<typeof PowerTargetSchema>;
export type PowerAssessmentInput = z.input<typeof PowerAssessmentSchema>;
export type PowerTargetInput = z.input<typeof PowerTargetSchema>;

// Labels and anchors are returned to both front doors, rather than separately
// defining what "reliable" means in the UI and in the agent playbook.
export const POWER_FIELDS = [
  { key: "intent", label: "Intent", options: [
    { value: "theme", label: "Theme first", hint: "Showcase a constrained theme; power is secondary." },
    { value: "social", label: "Social", hint: "Low pressure, straightforward plans, room for every deck to develop." },
    { value: "synergy", label: "Strong synergy", hint: "Powered up casual play with deliberate engines and high card quality." },
    { value: "optimized", label: "Optimized", hint: "Pursue efficient, fast wins within this deck's chosen strategy." },
    { value: "competitive", label: "cEDH metagame", hint: "Built specifically to compete in the cEDH metagame." },
  ] },
  { key: "consistency", label: "Consistency", options: [
    { value: "variable", label: "Variable", hint: "The plan depends on the draw and games differ substantially." },
    { value: "reliable", label: "Reliable", hint: "Usually develops its plan, with several routes to get there." },
    { value: "repeatable", label: "Repeatable", hint: "Redundancy or tutors reliably assemble the same key lines." },
  ] },
  { key: "resilience", label: "Resilience", options: [
    { value: "fragile", label: "Fragile", hint: "One disrupted engine or board wipe often stalls the deck." },
    { value: "recovering", label: "Can rebuild", hint: "Can recover from a setback or pivot to another plan." },
    { value: "persistent", label: "Persistent", hint: "Repeated disruption rarely exhausts its engines or win attempts." },
  ] },
  { key: "interaction", label: "Interaction", options: [
    { value: "light", label: "Light", hint: "A few answers; mostly advances its own plan." },
    { value: "flexible", label: "Flexible", hint: "Answers varied threats and protects key plays." },
    { value: "efficient", label: "Efficient", hint: "Frequent cheap or free answers; can contest decisive turns." },
  ] },
  { key: "win_style", label: "Win conditions", options: [
    { value: "incremental", label: "Gradual", hint: "Visible board development gives opponents time to respond." },
    { value: "burst", label: "Explosive finish", hint: "Accrued resources enable a decisive turn from hand." },
    { value: "combo", label: "Combo", hint: "Assembles a specific game-ending combination." },
  ] },
  { key: "combos", label: "Infinite combos", options: [
    { value: "none", label: "None", hint: "No intentional infinite wins." },
    { value: "multi_card", label: "Three or more pieces", hint: "An intentional infinite needs at least three pieces; speed still matters." },
    { value: "late_two_card", label: "Late two-card combo", hint: "A two-card infinite is not frequently accessible in the first six turns." },
    { value: "early_two_card", label: "Early two-card combo", hint: "A two-card infinite is frequently accessible in the first six turns." },
  ] },
  { key: "extra_turns", label: "Extra turns", options: [
    { value: "none", label: "None", hint: "No extra-turn cards." },
    { value: "single", label: "Occasional single turns", hint: "Few extra-turn cards, without chaining or looping them." },
    { value: "chained", label: "Chained or looped", hint: "Takes successive extra turns or loops them." },
  ] },
] as const;

// These are expectations, not a sanctioned classifier. Tutor limits were
// removed in October; no tutor-count rule or average of the profile is used.
// https://magic.wizards.com/en/news/announcements/introducing-commander-brackets-beta
// https://magic.wizards.com/en/news/announcements/commander-brackets-beta-update-october-21-2025
// https://magic.wizards.com/en/news/announcements/commander-brackets-beta-update-february-9-2026
export const BRACKET_GUIDANCE = {
  version: "2026-02-09",
  url: "https://magic.wizards.com/en/news/announcements/commander-brackets-beta-update-february-9-2026",
  expectations_url: "https://magic.wizards.com/en/news/announcements/commander-brackets-beta-update-october-21-2025",
};
export interface PowerField {
  key: (typeof POWER_FIELDS)[number]["key"]; label: string;
  options: readonly { value: string; label: string; hint: string }[];
}
const BRACKET_NAMES = ["", "Exhibition", "Core", "Upgraded", "Optimized", "cEDH"];
export interface BracketMapping {
  bracket: number | null;
  name: string | null;
  provisional: boolean;
  reasons: string[];
  conflicts: string[];
  unknowns: string[];
}

export function mapBracket(profile: PowerProfile, gameChangers: number | null,
  opts: { incompleteGameChangers?: boolean; target?: boolean } = {}): BracketMapping {
  const base = profile.intent == null ? null : ({ theme: 1, social: 2, synergy: 3, optimized: 4, competitive: 5 } as const)[profile.intent];
  let bracket: number | null = base;
  const reasons: string[] = [];
  const conflicts: string[] = [];
  const unknowns: string[] = [];
  if (base != null) reasons.push(`${POWER_FIELDS[0].options.find(o => o.value === profile.intent)!.label} intent suggests bracket ${base}.`);
  const floor = (n: number, reason: string) => {
    reasons.push(reason);
    if (base != null && n > base) conflicts.push(reason);
    bracket = Math.max(bracket ?? n, n);
  };
  if (profile.win_turn) {
    const min = profile.win_turn.min;
    if (min <= 6) floor(4, "Usual wins before turn 7 exceed Upgraded's expectation of six turns to develop.");
    else if (min <= 8) floor(3, "Usual wins on turns 7–8 exceed Core's expectation of eight turns to develop.");
    else if (min === 9) floor(2, "Usual turn-9 wins exceed Exhibition's expectation of nine turns to develop.");
    else reasons.push("The usual win range leaves at least nine turns to develop; speed alone does not imply Exhibition.");
  } else unknowns.push("Usual win range");
  if (profile.early_win_turn != null)
    reasons.push(`An occasional turn-${profile.early_win_turn} win is recorded separately; an outlier alone does not raise the bracket. Reassess if it becomes frequent.`);
  if (gameChangers == null || opts.incompleteGameChangers) unknowns.push("Game Changer count or target limit");
  if (gameChangers != null && gameChangers > 3)
    floor(4, `${opts.target ? `Allowing up to ${gameChangers}` : `${gameChangers} known`} Game Changers exceeds Upgraded's limit of three.`);
  else if (gameChangers != null && gameChangers > 0)
    floor(3, `${opts.target ? `Allowing up to ${gameChangers}` : `${gameChangers} known`} Game Changer(s) fits Upgraded or higher under ordinary bracket restrictions.`);
  if (profile.combos == null) unknowns.push("Infinite combo availability");
  else if (profile.combos === "early_two_card") floor(4, "Frequently accessible early two-card infinites belong in Optimized or cEDH.");
  else if (profile.combos === "late_two_card") floor(3, "Intentional late two-card infinites belong in Upgraded or higher.");
  if (profile.extra_turns == null) unknowns.push("Extra-turn patterns");
  else if (profile.extra_turns === "chained") floor(4, "Chained or looped extra turns exceed brackets 1–3.");
  else if (profile.extra_turns === "single") floor(2, "Occasional single extra turns fit Core or higher under ordinary restrictions.");
  if (profile.mass_land_denial == null) unknowns.push("Mass land denial");
  else if (profile.mass_land_denial) floor(4, "Mass land denial exceeds brackets 1–3.");
  for (const key of ["consistency", "resilience", "interaction"] as const)
    if (profile[key] == null) unknowns.push(POWER_FIELDS.find(f => f.key === key)!.label);
  if (profile.intent == null) unknowns.push("Deck-building intent (required to distinguish cEDH)");
  if (profile.win_style == null) unknowns.push("Win conditions");
  else if (profile.win_style === "burst") floor(3, "A decisive finish from hand fits Upgraded or higher rather than Core's incremental, telegraphed wins.");
  else if (profile.win_style === "combo" && profile.combos === "none") {
    reasons.push("Non-infinite combo wins are judged by their speed, consistency and setup.");
  }
  // Rich descriptors do not each impose a floor. Efficient answers alone do
  // not make a slow casual deck Optimized. Flag tensions for human review.
  if (base != null && base <= 2 && profile.consistency === "repeatable" && profile.interaction === "efficient")
    conflicts.push("Highly repeatable lines with efficient disruption may exceed the chosen low-pressure intent; review the gameplay evidence.");
  if (profile.intent === "theme" && conflicts.length)
    conflicts.push("Exhibition can allow thematic exceptions by pod agreement. This suggestion uses ordinary restrictions; discuss exceptions before playing.");
  if (profile.intent === "competitive" && profile.win_turn && profile.win_turn.min >= 9)
    conflicts.push("The declared cEDH intent and slow win range need supporting evidence about how this deck competes in that metagame.");
  return { bracket, name: bracket == null ? null : BRACKET_NAMES[bracket], provisional: unknowns.length > 0 || conflicts.length > 0, reasons, conflicts, unknowns };
}

export function parseTarget(input: unknown): PowerTarget {
  const r = PowerTargetSchema.safeParse(input);
  if (!r.success) throw new ServiceError(`Invalid power target: ${z.prettifyError(r.error)}`, 422);
  return r.data;
}
function parseAssessment(input: unknown) {
  const r = PowerAssessmentSchema.safeParse(input);
  if (!r.success) throw new ServiceError(`Invalid power assessment: ${z.prettifyError(r.error)}`, 422);
  return r.data;
}

export function getPowerTarget(db: Database, actor: Actor, deckId: number): PowerTarget | null {
  requireDeck(db, actor, deckId);
  const row = db.prepare("SELECT target_json FROM power_targets WHERE deck_id = ?").get(deckId) as { target_json: string } | undefined;
  return row ? parseTarget(JSON.parse(row.target_json)) : null;
}

/** Internal document write; agents reach it through proposeDocumentEdit. */
export function writePowerTarget(db: Database, actor: Actor, deckId: number, input: unknown, author = authorOf(actor)) {
  requireEditableDeck(db, actor, deckId);
  requireUi(actor, "apply a power target directly (use propose_power_target)");
  const target = input === null ? null : parseTarget(input);
  if (target) lintProse(db, target.notes);
  if (target == null) db.prepare("DELETE FROM power_targets WHERE deck_id = ?").run(deckId);
  else db.prepare(`INSERT INTO power_targets (deck_id, target_json, author, author_user_id, author_via, author_label) VALUES (?, ?, ?, ?, ?, ?)
    ON CONFLICT(deck_id) DO UPDATE SET target_json = excluded.target_json, author = excluded.author, author_user_id = excluded.author_user_id, author_via = excluded.author_via, author_label = excluded.author_label, updated_at = datetime('now')`)
    .run(deckId, JSON.stringify(target), author, actor.user_id, actor.via, actor.label);
}

export function setPowerTarget(db: Database, actor: Actor, deckId: number, input: unknown) {
  requireDeckOwner(db, actor, deckId);
  requireUi(actor, "set the power target directly");
  writePowerTarget(db, actor, deckId, input);
}

export function profileLines(profile: PowerProfile): string[] {
  const lines = POWER_FIELDS.map(field => `${field.label}: ${field.options.find(o => o.value === profile[field.key])?.label ?? "Unknown"}`);
  lines.splice(1, 0, `Usual win range: ${profile.win_turn ? `turns ${profile.win_turn.min}–${profile.win_turn.max}` : "Unknown"}`,
    `Occasional early win: ${profile.early_win_turn == null ? "Unknown / not recorded" : `turn ${profile.early_win_turn}`}`);
  lines.push(`Mass land denial: ${profile.mass_land_denial == null ? "Unknown" : profile.mass_land_denial ? "Present / allowed" : "None"}`);
  return lines;
}

export function targetText(target: PowerTarget | null): string {
  if (!target) return "No power target set.";
  return [...profileLines(target.profile), `Game Changers: ${target.game_changers_max == null ? "No limit set" : `at most ${target.game_changers_max}`}`, target.notes].filter(Boolean).join("\n");
}

interface AssessmentRow extends Attribution {
  id: number; profile_json: string; revision: number; evidence: "list" | "goldfish" | "games";
  confidence: "tentative" | "supported"; game_changers_json: string; notes: string; author: string; owner_touched: number; created_at: string;
}
function assessmentRows(db: Database, actor: Actor, deckId: number) {
  return db.prepare("SELECT * FROM power_assessments WHERE deck_id = ? ORDER BY id DESC LIMIT 20").all(deckId) as unknown as AssessmentRow[];
}
export function recordPowerAssessment(db: Database, actor: Actor, deckId: number, input: unknown): number {
  requireEditableDeck(db, actor, deckId);
  const deck = requireDeck(db, actor, deckId);
  const value = parseAssessment(input);
  if (value.revision !== deck.revision)
    throw new ServiceError(`The deck is now at revision ${deck.revision}; re-read it and assess that revision. Nothing was recorded.`, 409);
  const current = assessmentRows(db, actor, deckId)[0];
  if (actor.via === "agent" && current?.owner_touched && current.revision === deck.revision)
    throw new PermissionError("The owner has assessed this revision. Their correction takes precedence; discuss a different assessment with them instead of overwriting it.");
  lintProse(db, value.notes);
  return withTransaction(db, () => {
    const row = db.prepare(`INSERT INTO power_assessments (deck_id, profile_json, revision, evidence, confidence, notes, author, owner_touched, game_changers_json, author_user_id, author_via, author_label)
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`).run(deckId, JSON.stringify(value.profile), value.revision, value.evidence, value.confidence, value.notes, authorOf(actor), Number(actor.via === "ui"), JSON.stringify(gameChangerFacts(db, actor, deckId)), actor.user_id, actor.via, actor.label);
    recordActivity(db, actor, "assess_deck_power", deckId, `Assessed power at r${value.revision} (${value.confidence}, ${value.evidence}): ${value.notes}`);
    return Number(row.lastInsertRowid);
  });
}

export function gameChangerFacts(db: Database, actor: Actor, deckId: number) {
  requireDeck(db, actor, deckId);
  // Nullable Scryfall game_changer: missing metadata is unknown, not false.
  // https://scryfall.com/docs/api/cards (game_changer, nullable boolean)
  const rows = db.prepare(`SELECT c.oracle_id, c.name, c.game_changer, dc.quantity FROM deck_cards dc
    JOIN cards c ON c.oracle_id = dc.oracle_id WHERE dc.deck_id = ? AND dc.maybe = 0 ORDER BY c.name`).all(deckId) as unknown as { oracle_id: string; name: string; game_changer: number | null; quantity: number }[];
  return {
    count: rows.reduce((sum, r) => sum + (r.game_changer === 1 ? r.quantity : 0), 0),
    unknown_cards: rows.reduce((sum, r) => sum + (r.game_changer == null ? r.quantity : 0), 0),
    cards: rows.filter(r => r.game_changer === 1).map(({ oracle_id, name, quantity }) => ({ oracle_id, name, quantity })),
  };
}

export function getPower(db: Database, actor: Actor, deckId: number) {
  const deck = requireDeck(db, actor, deckId);
  const targetValue = getPowerTarget(db, actor, deckId);
  const targetMeta = db.prepare("SELECT author, author_user_id, author_via, author_label, updated_at FROM power_targets WHERE deck_id = ?").get(deckId) as (Attribution & { author: string; updated_at: string }) | undefined;
  const facts = gameChangerFacts(db, actor, deckId);
  const history = assessmentRows(db, actor, deckId).map(({ profile_json, owner_touched, game_changers_json, ...row }) => {
    const profile = PowerProfileSchema.parse(JSON.parse(profile_json));
    const stale = row.revision !== deck.revision;
    const captured = JSON.parse(game_changers_json) as ReturnType<typeof gameChangerFacts>;
    const gameChangers = stale ? captured : facts;
    return { ...row, author_display: displayAuthor(db, row), profile, game_changers: gameChangers, owner_touched: !!owner_touched, stale,
      // Historical mappings use the facts captured with the assessed list.
      mapping: mapBracket(profile, gameChangers.count, { incompleteGameChangers: gameChangers.unknown_cards > 0 }) };
  });
  const target = targetValue ? { ...targetValue, ...targetMeta!, author_display: displayAuthor(db, targetMeta!), mapping: mapBracket(targetValue.profile, targetValue.game_changers_max, { target: true }) } : null;
  const assessment: (typeof history)[number] | null = history.length ? history[0] : null;
  const differences: { field: string; target: string; current: string }[] = [];
  if (target && assessment && !assessment.stale) {
    for (const field of POWER_FIELDS) {
      const desired = target.profile[field.key];
      const actual = assessment.profile[field.key];
      if (desired != null && actual != null && desired !== actual)
        differences.push({ field: field.label, target: field.options.find(o => o.value === desired)!.label, current: field.options.find(o => o.value === actual)!.label });
    }
    const desired = target.profile.win_turn;
    const actual = assessment.profile.win_turn;
    if (desired && actual && (actual.min < desired.min || actual.max > desired.max))
      differences.push({ field: "Usual win range", target: `Turns ${desired.min}–${desired.max}`, current: `Turns ${actual.min}–${actual.max}` });
    if (target.profile.mass_land_denial != null && assessment.profile.mass_land_denial != null && target.profile.mass_land_denial !== assessment.profile.mass_land_denial)
      differences.push({ field: "Mass land denial", target: target.profile.mass_land_denial ? "Allowed" : "None", current: assessment.profile.mass_land_denial ? "Present" : "None" });
    if (target.game_changers_max != null && facts.count > target.game_changers_max)
      differences.push({ field: "Game Changers", target: `At most ${target.game_changers_max}`, current: `${facts.count}${facts.unknown_cards ? "+ (some metadata unknown)" : ""}` });
    if (target.profile.early_win_turn != null && assessment.profile.early_win_turn != null && assessment.profile.early_win_turn < target.profile.early_win_turn)
      differences.push({ field: "Occasional early win", target: `Turn ${target.profile.early_win_turn} or later`, current: `Turn ${assessment.profile.early_win_turn}` });
  }
  return { target, assessment, history, game_changers: facts, differences, fields: POWER_FIELDS as readonly PowerField[], guidance: BRACKET_GUIDANCE };
}

/** Forks inherit the target; a fresh assessment must describe the new deck. */
export function copyPowerTarget(db: Database, actor: Actor, parentId: number, forkId: number) {
  requireDeckOwner(db, actor, parentId);
  requireEditableDeck(db, actor, forkId);
  db.prepare(`INSERT INTO power_targets (deck_id, target_json, author, author_user_id, author_via, author_label)
    SELECT ?, target_json, author, author_user_id, author_via, author_label FROM power_targets WHERE deck_id = ?`).run(forkId, parentId);
}
