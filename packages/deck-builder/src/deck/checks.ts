import { memoRead } from "../read-context.ts";
import { requireDeck } from "./service.ts";
import type { Actor } from "../actor.ts";
// Deterministic checks (spec §9): computed on request from the current list,
// never stored, never a model's arithmetic. Judgement about a deck is an
// agent's work and reaches the app as proposals and memories, not here.

import type { Database } from "../sqlite.ts";
import { getPower } from "./power.ts";
import { getBrief } from "./brief.ts";
import { getDeck } from "./service.ts";
import { roleEligibility } from "./roles.ts";

export interface Finding {
  key: string;
  severity: "error" | "warn";
  title: string;
  detail: string;
  oracle_id?: string;
  card_name?: string;
}

const PARTNER_MARKERS = [
  "partner", // Partner, Partner with X, partner keyword text
  "friends forever",
  "choose a background",
];

// Doctor's companion needs the ability on only one commander. The other must
// be a legendary creature with exactly the Time Lord and Doctor subtypes.
// https://magic.wizards.com/en/news/feature/magic-the-gathering-doctor-who-release-notes
export function isDoctorPair(cards: { type_line: string; oracle_text: string }[]): boolean {
  const legendaryCreature = (c: (typeof cards)[0]) =>
    /\bLegendary\b/.test(c.type_line.split(" — ")[0]) &&
    /\bCreature\b/.test(c.type_line.split(" — ")[0]);
  return cards.length === 2 && cards.every(legendaryCreature) && cards.some((c, i) =>
    /^Doctor['’]s companion(?: \(|$)/mi.test(c.oracle_text) &&
    cards[1 - i].type_line.split(" — ")[1]?.trim() === "Time Lord Doctor"
  );
}

const cardCount = (n: number) => `${n} ${n === 1 ? "card" : "cards"}`;

export function checkDeck(db: Database, actor: Actor, deckId: number): Finding[] {
  return memoRead(db, `checks:${actor.user_id}:${deckId}`, () => readChecks(db, actor, deckId));
}
function readChecks(db: Database, actor: Actor, deckId: number): Finding[] {
  requireDeck(db, actor, deckId);
  const { deck, cards, computed } = getDeck(db, actor, deckId);
  const inDeck = cards.filter((c) => !c.maybe);
  const findings: Finding[] = [];

  if (computed.card_count !== 100) {
    findings.push({
      key: "card_count",
      severity: "error",
      title: `Deck is ${computed.card_count}/100`,
      detail:
        computed.delta_to_100 < 0
          ? `${cardCount(-computed.delta_to_100)} short of 100 · commanders count, companions don't`
          : `${cardCount(computed.delta_to_100)} over 100`,
    });
  }

  for (const c of cards.filter(c => c.retired)) findings.push({
    key: `retired:${c.oracle_id}`, severity: "warn", title: `${c.name}: retired printing/oracle id`,
    detail: "Absent from the latest Scryfall bulk data; retained for this deck and its history. Verify the current card identity.",
    oracle_id: c.oracle_id, card_name: c.name,
  });

  const commanders = inDeck.filter((c) => c.role === "commander");
  if (!commanders.length) {
    findings.push({
      key: "no_commander",
      severity: "error",
      title: "No commander",
      detail: "The deck has no card in the command zone.",
    });
  }
  for (const c of commanders) {
    if (!roleEligibility(c).can_be_commander) {
      findings.push({
        key: `commander_ineligible:${c.oracle_id}`,
        severity: "error",
        title: `${c.name} cannot be your commander`,
        detail: "Not a legendary creature and has no “can be your commander” text.",
        oracle_id: c.oracle_id,
        card_name: c.name,
      });
    }
  }
  if (commanders.length === 2) {
    const text = (c: (typeof commanders)[0]) => c.oracle_text.toLowerCase();
    if (!isDoctorPair(commanders) && !commanders.every((c) => PARTNER_MARKERS.some((m) => text(c).includes(m)))) {
      findings.push({
        key: "commander_pair",
        severity: "warn",
        title: "Commander pair may be illegal",
        detail: `${commanders[0].name} + ${commanders[1].name}: this pairing was not recognized. Verify the exact pairing rule; this check does not cover every pairing ability.`,
      });
    }
  }

  for (const v of computed.identity_violations) {
    findings.push({
      key: `identity:${v.oracle_id}`,
      severity: "error",
      title: `${v.name} is outside the deck's color identity`,
      detail: `Card identity ${v.color_identity || "C"} does not fit ${deck.color_identity || "C"}.`,
      oracle_id: v.oracle_id,
      card_name: v.name,
    });
  }

  for (const v of computed.legality_violations) {
    findings.push({
      key: `${v.legality}:${v.oracle_id}`,
      severity: v.legality === "banned" ? "error" : "warn",
      title: `${v.name} is ${v.legality.replace("_", " ")} in Commander`,
      detail:
        v.legality === "banned"
          ? "On the Commander banned list."
          : "Scryfall marks this card as not legal in Commander.",
      oracle_id: v.oracle_id,
      card_name: v.name,
    });
  }

  for (const v of computed.singleton_violations) {
    findings.push({
      key: `singleton:${v.oracle_id}`,
      severity: "error",
      title: `${v.quantity}× ${v.name} exceeds its copy limit`,
      detail: `Limit is ${v.limit ?? "unlimited"}.`,
      oracle_id: v.oracle_id,
      card_name: v.name,
    });
  }

  for (const s of computed.slot_deltas) {
    if (s.status === "under" || s.status === "over") {
      findings.push({
        key: `slot_${s.status}:${s.slot_id}`,
        severity: "warn",
        title: `${s.name} is ${s.status} target (${s.count}/${s.target_min ?? 0}–${s.target_max ?? "∞"})`,
        detail: `${cardCount(Math.abs(s.delta))} ${s.status === "under" ? "below the minimum" : "above the maximum"}. Targets are soft — adjust the slot target if this count is deliberate.`,
      });
    }
  }

  // Spec §6: an engine's pieces are card references so this is a lookup.
  for (const engine of getBrief(db, actor, deckId).engines) {
    const missing = engine.pieces.filter((p) => !p.in_deck);
    if (missing.length)
      findings.push({
        key: `engine_incomplete:${engine.id}`,
        severity: "warn",
        title: `Engine "${engine.name}" is missing ${missing.length} of ${engine.pieces.length} piece(s)`,
        detail: `Not in the deck: ${missing.map((p) => p.name).join(", ")}.`,
      });
  }

  for (const c of inDeck.filter((c) => c.role === "companion")) {
    findings.push({
      key: `companion_unchecked:${c.oracle_id}`,
      severity: "warn",
      title: `${c.name}'s companion condition is not checked`,
      detail: "Deck-building conditions need rules evaluation this app does not have; verify by hand.",
      oracle_id: c.oracle_id,
      card_name: c.name,
    });
  }

  const power = getPower(db, actor, deckId);
  if (power.assessment?.stale) findings.push({
    key: "power_assessment_stale", severity: "warn", title: "Power assessment needs review",
    detail: `Assessed at r${power.assessment.revision}; the current list is r${deck.revision}. Reassess before comparing it with the target.`,
  });
  if (power.differences.length) findings.push({
    key: "power_target_gap", severity: "warn", title: "Power profile differs from target",
    detail: power.differences.map(d => `${d.field}: target ${d.target}, current ${d.current}`).join("; "),
  });
  return findings;
}
