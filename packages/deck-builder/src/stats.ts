// Player statistics (spec §3, §10.3): what the owner builds, computed across
// their Paper decks. "The owner has three Golgari decks" is a query, never a
// memory — so the app answers it here, and the playbook forbids storing it.

import type { Database } from "./sqlite.ts";
import { getDeck, listDecks } from "./deck/service.ts";

const COLOR_ORDER = ["W", "U", "B", "R", "G"];

// The card types a type line can carry, in the order a card is classified:
// a "Land Creature" counts as a land, an "Artifact Creature" as a creature.
const TYPE_GROUPS = ["Land", "Creature", "Planeswalker", "Battle", "Instant", "Sorcery", "Artifact", "Enchantment"];

function typeGroup(typeLine: string): string {
  const front = typeLine.split(" // ")[0].split(" — ")[0];
  return TYPE_GROUPS.find((t) => new RegExp(`\\b${t}\\b`).test(front)) ?? "Other";
}

export interface PlayerStats {
  /** Only decks categorized as Paper contribute to these statistics. */
  deck_count: number;
  decks: Array<{
    id: number;
    name: string;
    forked_from: string | null;
    commanders: string[];
    color_identity: string;
    card_count: number;
    land_count: number;
    average_mana_value: number | null;
  }>;
  /** How many decks include each colour. */
  colors: Record<string, number>;
  /** Decks per exact colour identity, most common first. "C" is colourless. */
  identities: Array<{ identity: string; decks: number }>;
  /** Creature types of the commanders, most common first. */
  commander_types: Array<{ type: string; decks: number }>;
  averages: {
    mana_value: number | null;
    lands: number | null;
    /** Average number of cards of each type per deck. */
    by_type: Record<string, number>;
    curve: Record<string, number>;
  };
  /** Slot names the owner uses, with how full they run. */
  slots: Array<{ name: string; decks: number; average_count: number }>;
  /** Nonbasic cards that appear in more than one deck family, most reused first. */
  reused_cards: Array<{ name: string; type_line: string; decks: number }>;
  /** A fork and its parent count once toward reuse; this says how many families there are. */
  families: number;
}

export function playerStats(db: Database, opts: { reusedLimit?: number } = {}): PlayerStats {
  const allDecks = listDecks(db);
  const roster = allDecks.filter((d) => d.category === "paper");
  const views = roster.map((d) => getDeck(db, d.id));
  const filled = views.filter((v) => v.computed.card_count > 0);

  // A fork carries its parent's list, so counting both would report the same
  // taste twice. Reuse is counted per family: the root of each fork chain.
  // Keep ancestry through excluded decks so Paper forks in the same family
  // still count once. Only the filtered views contribute cards or totals.
  const parentOf = new Map(allDecks.map((d) => [d.id, d.parent_id]));
  const rootOf = (id: number): number => {
    const seen = new Set<number>();
    let cur = id;
    while (parentOf.get(cur) != null && !seen.has(cur)) {
      seen.add(cur);
      cur = parentOf.get(cur)!;
    }
    return cur;
  };

  const colors: Record<string, number> = { W: 0, U: 0, B: 0, R: 0, G: 0 };
  const identities = new Map<string, number>();
  const commanderTypes = new Map<string, number>();
  const slots = new Map<string, { decks: number; total: number; name: string }>();
  const reuse = new Map<string, { name: string; type_line: string; families: Set<number> }>();
  const typeTotals: Record<string, number> = {};
  const curveTotals: Record<string, number> = {};

  for (const v of filled) {
    const identity = v.deck.color_identity;
    for (const c of COLOR_ORDER) if (identity.includes(c)) colors[c]++;
    const key = identity || "C";
    identities.set(key, (identities.get(key) ?? 0) + 1);

    const main = v.cards.filter((c) => !c.maybe && c.role !== "companion");
    for (const c of main.filter((c) => c.role === "commander")) {
      // "Time Lord" is the one creature type spelled as two words
      // (Comprehensive Rules 205.3m), so it is kept whole.
      const subtypes =
        c.type_line
          .split(" // ")[0]
          .split(" — ")[1]
          ?.trim()
          .replace(/\bTime Lord\b/g, "Time\u00a0Lord")
          .split(/ +/)
          .map((t) => t.replace("\u00a0", " ")) ?? [];
      for (const t of new Set(subtypes))
        commanderTypes.set(t, (commanderTypes.get(t) ?? 0) + 1);
    }
    for (const c of main) {
      const group = typeGroup(c.type_line);
      typeTotals[group] = (typeTotals[group] ?? 0) + c.quantity;
      if (/\bBasic\b/.test(c.type_line)) continue;
      const entry = reuse.get(c.oracle_id) ?? {
        name: c.name,
        type_line: c.type_line,
        families: new Set<number>(),
      };
      entry.families.add(rootOf(v.deck.id));
      reuse.set(c.oracle_id, entry);
    }
    for (const [bucket, n] of Object.entries(v.computed.curve))
      curveTotals[bucket] = (curveTotals[bucket] ?? 0) + n;
    for (const s of v.computed.slot_deltas) {
      const k = s.name.toLowerCase();
      const entry = slots.get(k) ?? { decks: 0, total: 0, name: s.name };
      entry.decks++;
      entry.total += s.count;
      slots.set(k, entry);
    }
  }

  const n = filled.length;
  const avg = (total: number) => (n ? Math.round((total / n) * 10) / 10 : 0);
  const manaValues = filled
    .map((v) => v.computed.average_mana_value)
    .filter((x): x is number => x != null);

  return {
    deck_count: roster.length,
    decks: views.map((v, i) => ({
      id: v.deck.id,
      name: v.deck.name,
      forked_from: v.deck.parent?.name ?? null,
      commanders: roster[i].commanders,
      color_identity: v.deck.color_identity || (v.deck.ci_mask === 0 ? "C" : ""),
      card_count: v.computed.card_count,
      land_count: v.computed.land_count,
      average_mana_value:
        v.computed.average_mana_value == null
          ? null
          : Math.round(v.computed.average_mana_value * 100) / 100,
    })),
    colors,
    identities: [...identities.entries()]
      .map(([identity, decks]) => ({ identity, decks }))
      .sort((a, b) => b.decks - a.decks || a.identity.localeCompare(b.identity)),
    commander_types: [...commanderTypes.entries()]
      .map(([type, decks]) => ({ type, decks }))
      .sort((a, b) => b.decks - a.decks || a.type.localeCompare(b.type)),
    averages: {
      mana_value: manaValues.length
        ? Math.round((manaValues.reduce((a, b) => a + b, 0) / manaValues.length) * 100) / 100
        : null,
      lands: n ? avg(filled.reduce((t, v) => t + v.computed.land_count, 0)) : null,
      by_type: Object.fromEntries(
        Object.entries(typeTotals)
          .sort((a, b) => b[1] - a[1])
          .map(([k, t]) => [k, avg(t)]),
      ),
      curve: Object.fromEntries(Object.entries(curveTotals).map(([k, t]) => [k, avg(t)])),
    },
    slots: [...slots.values()]
      .map((s) => ({
        name: s.name,
        decks: s.decks,
        average_count: Math.round((s.total / s.decks) * 10) / 10,
      }))
      .sort((a, b) => b.decks - a.decks || a.name.localeCompare(b.name)),
    reused_cards: [...reuse.values()]
      .filter((r) => r.families.size > 1)
      .map((r) => ({ name: r.name, type_line: r.type_line, decks: r.families.size }))
      .sort((a, b) => b.decks - a.decks || a.name.localeCompare(b.name))
      .slice(0, opts.reusedLimit ?? 30),
    families: new Set(filled.map((v) => rootOf(v.deck.id))).size,
  };
}
