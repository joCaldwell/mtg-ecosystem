import type { DatabaseSync } from "node:sqlite";
import { readFileSync } from "node:fs";
import { normalizeName, withTransaction } from "./db.ts";

// Non-cards that pollute search results. Everything else stays; illegal
// cards are the audit's job, not the ingest filter's.
const SKIP_LAYOUTS = new Set([
  "token",
  "double_faced_token",
  "emblem",
  "art_series",
  "vanguard",
  "scheme",
  "planar",
]);

export const COLOR_BIT: Record<string, number> = { W: 1, U: 2, B: 4, R: 8, G: 16 };

export function maskOf(colors: string[] | undefined | null): number {
  let m = 0;
  for (const c of colors ?? []) m |= COLOR_BIT[c] ?? 0;
  return m;
}

const WUBRG = ["W", "U", "B", "R", "G"];
function canonicalColors(colors: string[] | undefined | null): string {
  const set = new Set(colors ?? []);
  return WUBRG.filter((c) => set.has(c)).join("");
}

export function stripReminderText(text: string): string {
  return text.replace(/\([^)]*\)/g, "").replace(/[ \t]+/g, " ").replace(/ ?\n ?/g, "\n").trim();
}

interface ScryfallFace {
  name: string;
  mana_cost?: string;
  type_line?: string;
  oracle_text?: string;
  power?: string;
  toughness?: string;
  loyalty?: string;
  colors?: string[];
}

export interface ScryfallCard {
  oracle_id: string;
  name: string;
  mana_cost?: string;
  cmc?: number;
  type_line?: string;
  oracle_text?: string;
  power?: string;
  toughness?: string;
  loyalty?: string;
  colors?: string[];
  color_identity: string[];
  layout: string;
  legalities?: { commander?: string };
  card_faces?: ScryfallFace[];
}

function numericOrNull(v: string | null | undefined): number | null {
  if (v == null) return null;
  return /^[+-]?\d+(\.\d+)?$/.test(v) ? Number(v) : null;
}

export interface CardRow {
  oracle_id: string;
  name: string;
  mana_cost: string | null;
  cmc: number;
  type_line: string;
  oracle_text: string;
  power: string | null;
  toughness: string | null;
  loyalty: string | null;
  power_num: number | null;
  toughness_num: number | null;
  loyalty_num: number | null;
  colors: string;
  color_identity: string;
  colors_mask: number;
  ci_mask: number;
  colors_count: number;
  ci_count: number;
  commander_legality: string;
  is_commander: number;
  layout: string;
  faces_json: string | null;
  search_text: string;
  full_search_text: string;
  face_names: string[];
}

export function toRow(card: ScryfallCard): CardRow | null {
  if (SKIP_LAYOUTS.has(card.layout) || !card.oracle_id) return null;

  const faces = card.card_faces ?? [];
  const front = faces[0];

  const displayText =
    card.oracle_text ??
    faces
      .map((f) => (f.oracle_text ? `${f.name}\n${f.oracle_text}` : f.name))
      .join("\n//\n");
  const fullSearch =
    card.oracle_text ?? faces.map((f) => f.oracle_text ?? "").filter(Boolean).join("\n");
  const searchText = stripReminderText(fullSearch);

  const manaCost =
    card.mana_cost ??
    (faces.length ? faces.map((f) => f.mana_cost ?? "").join(" // ") : null);
  const power = card.power ?? front?.power ?? null;
  const toughness = card.toughness ?? front?.toughness ?? null;
  const loyalty = card.loyalty ?? front?.loyalty ?? null;

  const colorList =
    card.colors ??
    [...new Set(faces.flatMap((f) => f.colors ?? []))];

  // Commander eligibility is determined by the front face for multi-faced
  // cards, or by explicit "can be your commander" text anywhere.
  const frontTypeLine = front?.type_line ?? card.type_line ?? "";
  const isCommander =
    (/\bLegendary\b/.test(frontTypeLine) && /\bCreature\b/.test(frontTypeLine)) ||
    searchText.toLowerCase().includes("can be your commander");

  return {
    oracle_id: card.oracle_id,
    name: card.name,
    mana_cost: manaCost || null,
    cmc: card.cmc ?? 0,
    type_line: card.type_line ?? "",
    oracle_text: displayText,
    power,
    toughness,
    loyalty,
    power_num: numericOrNull(power),
    toughness_num: numericOrNull(toughness),
    loyalty_num: numericOrNull(loyalty),
    colors: canonicalColors(colorList),
    color_identity: canonicalColors(card.color_identity),
    colors_mask: maskOf(colorList),
    ci_mask: maskOf(card.color_identity),
    colors_count: (colorList ?? []).length,
    ci_count: (card.color_identity ?? []).length,
    commander_legality: card.legalities?.commander ?? "not_legal",
    is_commander: isCommander ? 1 : 0,
    layout: card.layout,
    faces_json: faces.length
      ? JSON.stringify(
          faces.map((f) => ({
            name: f.name,
            mana_cost: f.mana_cost ?? null,
            type_line: f.type_line ?? null,
            oracle_text: f.oracle_text ?? null,
            power: f.power ?? null,
            toughness: f.toughness ?? null,
            loyalty: f.loyalty ?? null,
          })),
        )
      : null,
    search_text: searchText,
    full_search_text: fullSearch,
    face_names: [card.name, ...faces.map((f) => f.name)],
  };
}

const UPSERT_SQL = `
INSERT INTO cards (
  oracle_id, name, mana_cost, cmc, type_line, oracle_text,
  power, toughness, loyalty, power_num, toughness_num, loyalty_num,
  colors, color_identity, colors_mask, ci_mask, colors_count, ci_count,
  commander_legality, is_commander, layout, faces_json, search_text, full_search_text
) VALUES (
  :oracle_id, :name, :mana_cost, :cmc, :type_line, :oracle_text,
  :power, :toughness, :loyalty, :power_num, :toughness_num, :loyalty_num,
  :colors, :color_identity, :colors_mask, :ci_mask, :colors_count, :ci_count,
  :commander_legality, :is_commander, :layout, :faces_json, :search_text, :full_search_text
)
ON CONFLICT(oracle_id) DO UPDATE SET
  name = excluded.name, mana_cost = excluded.mana_cost, cmc = excluded.cmc,
  type_line = excluded.type_line, oracle_text = excluded.oracle_text,
  power = excluded.power, toughness = excluded.toughness, loyalty = excluded.loyalty,
  power_num = excluded.power_num, toughness_num = excluded.toughness_num,
  loyalty_num = excluded.loyalty_num, colors = excluded.colors,
  color_identity = excluded.color_identity, colors_mask = excluded.colors_mask,
  ci_mask = excluded.ci_mask, colors_count = excluded.colors_count,
  ci_count = excluded.ci_count,
  commander_legality = excluded.commander_legality, is_commander = excluded.is_commander,
  layout = excluded.layout, faces_json = excluded.faces_json,
  search_text = excluded.search_text, full_search_text = excluded.full_search_text
`;

export function ingestCards(db: DatabaseSync, cards: ScryfallCard[]): { inserted: number; skipped: number } {
  const upsert = db.prepare(UPSERT_SQL);
  const insertName = db.prepare(
    "INSERT OR IGNORE INTO card_names (name_norm, oracle_id) VALUES (?, ?)",
  );
  const insertFts = db.prepare(
    "INSERT INTO cards_fts (name, search_text, oracle_id) VALUES (?, ?, ?)",
  );

  let inserted = 0;
  let skipped = 0;

  withTransaction(db, () => {
    const seen: string[] = [];
    for (const card of cards) {
      const row = toRow(card);
      if (!row) {
        skipped++;
        continue;
      }
      const { face_names, ...cols } = row;
      upsert.run(
        Object.fromEntries(Object.entries(cols).map(([k, v]) => [k, v])) as Record<
          string,
          string | number | null
        >,
      );
      seen.push(row.oracle_id);
      inserted++;
    }

    // Drop cards no longer in the bulk file (Scryfall merges/removes oracle ids).
    db.exec("CREATE TEMP TABLE seen_ids (oracle_id TEXT PRIMARY KEY)");
    const insSeen = db.prepare("INSERT OR IGNORE INTO seen_ids VALUES (?)");
    for (const id of seen) insSeen.run(id);
    db.exec("DELETE FROM cards WHERE oracle_id NOT IN (SELECT oracle_id FROM seen_ids)");
    db.exec("DROP TABLE seen_ids");

    // Name aliases and FTS are cheap to rebuild wholesale.
    db.exec("DELETE FROM card_names");
    db.exec("DELETE FROM cards_fts");
    for (const card of cards) {
      const row = toRow(card);
      if (!row) continue;
      for (const n of new Set(row.face_names)) insertName.run(normalizeName(n), row.oracle_id);
      insertFts.run(row.name, row.search_text, row.oracle_id);
    }

    db.prepare(
      "INSERT INTO meta (key, value) VALUES ('last_ingest', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    ).run(new Date().toISOString());
  });

  return { inserted, skipped };
}

export function ingestFromFile(db: DatabaseSync, path: string) {
  console.log(`Parsing ${path}...`);
  const cards = JSON.parse(readFileSync(path, "utf-8")) as ScryfallCard[];
  console.log(`Ingesting ${cards.length} entries...`);
  const result = ingestCards(db, cards);
  console.log(`Done: ${result.inserted} cards ingested, ${result.skipped} non-card entries skipped.`);
  return result;
}
