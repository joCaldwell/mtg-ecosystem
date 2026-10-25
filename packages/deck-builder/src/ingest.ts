import type { Database } from "./sqlite.ts";
import { readJsonArray } from "./json-array.ts";
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

interface ImageUris { small?: string; normal?: string; art_crop?: string }

interface ScryfallFace {
  image_uris?: ImageUris;
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
  image_uris?: ImageUris;
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
  /** Nullable Game Changer flag: https://scryfall.com/docs/api/cards */
  game_changer?: boolean | null;
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
  game_changer: number | null;
  layout: string;
  faces_json: string | null;
  search_text: string;
  full_search_text: string;
  face_names: string[];
  image_small: string | null;
  image_normal: string | null;
  image_art_crop: string | null;
  retired: number;
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
    // Scryfall Card Objects + Card Faces (verified 2026-10-25): single-sided
    // imagery lives on the card; double-sided imagery lives on each face.
    // https://scryfall.com/docs/api/cards — use the front face for thumbnails.
    image_small: (card.image_uris ?? front?.image_uris)?.small ?? null,
    image_normal: (card.image_uris ?? front?.image_uris)?.normal ?? null,
    image_art_crop: (card.image_uris ?? front?.image_uris)?.art_crop ?? null,
    retired: 0,
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
    game_changer: card.game_changer == null ? null : Number(card.game_changer),
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
  commander_legality, is_commander, game_changer, layout, faces_json, search_text, full_search_text, image_small, image_normal, image_art_crop, retired
) VALUES (
  :oracle_id, :name, :mana_cost, :cmc, :type_line, :oracle_text,
  :power, :toughness, :loyalty, :power_num, :toughness_num, :loyalty_num,
  :colors, :color_identity, :colors_mask, :ci_mask, :colors_count, :ci_count,
  :commander_legality, :is_commander, :game_changer, :layout, :faces_json, :search_text, :full_search_text, :image_small, :image_normal, :image_art_crop, :retired
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
  commander_legality = excluded.commander_legality, is_commander = excluded.is_commander, game_changer = excluded.game_changer,
  image_small = excluded.image_small, image_normal = excluded.image_normal,
  image_art_crop = excluded.image_art_crop, retired = 0,
  layout = excluded.layout, faces_json = excluded.faces_json,
  search_text = excluded.search_text, full_search_text = excluded.full_search_text
`;

// Stage and validate before changing live facts: a truncated/malformed file must
// leave cards, aliases, prices and the freshness marker untouched.
function stage(db: Database) {
  db.exec("PRAGMA temp_store = FILE; DROP TABLE IF EXISTS temp.ingest_staging; CREATE TEMP TABLE ingest_staging (oracle_id TEXT PRIMARY KEY, row_json TEXT NOT NULL)");
  const insert = db.prepare("INSERT INTO ingest_staging VALUES (?, ?)");
  let skipped = 0;
  return {
    add(card: ScryfallCard) {
      const row = toRow(card);
      if (!row) { skipped++; return; }
      insert.run(row.oracle_id, JSON.stringify(row));
    },
    finish() {
      const inserted = (db.prepare("SELECT COUNT(*) n FROM ingest_staging").get() as { n: number }).n;
      const current = (db.prepare("SELECT COUNT(*) n FROM cards WHERE retired = 0").get() as { n: number }).n;
      if (!inserted || inserted < current * 0.9)
        throw new Error(`Refusing truncated bulk file: ${inserted} cards, fewer than 90% of ${current} current cards`);
      const upsert = db.prepare(UPSERT_SQL);
      let last = "";
      for (;;) {
        const batch = db.prepare("SELECT oracle_id, row_json FROM ingest_staging WHERE oracle_id > ? ORDER BY oracle_id LIMIT 500").all(last) as unknown as { oracle_id: string; row_json: string }[];
        if (!batch.length) break;
        withTransaction(db, () => {
          for (const r of batch) {
            const { face_names, ...cols } = JSON.parse(r.row_json) as CardRow;
            upsert.run(cols);
          }
        });
        last = batch.at(-1)!.oracle_id;
      }
      withTransaction(db, () => {
        db.exec(`UPDATE cards SET retired = 1 WHERE oracle_id NOT IN (SELECT oracle_id FROM ingest_staging);
          DELETE FROM card_names; DELETE FROM cards_fts;`);
        const name = db.prepare("INSERT OR IGNORE INTO card_names VALUES (?, ?)");
        const fts = db.prepare("INSERT INTO cards_fts (name, search_text, oracle_id) VALUES (?, ?, ?)");
        // Retired IDs keep aliases and facts for existing decks and history.
        for (const r of db.prepare("SELECT oracle_id, name, faces_json, search_text FROM cards").iterate() as Iterable<{ oracle_id: string; name: string; faces_json: string | null; search_text: string }>) {
          const faces = r.faces_json ? JSON.parse(r.faces_json) as { name: string }[] : [];
          for (const n of new Set([r.name, ...faces.map(f => f.name)])) name.run(normalizeName(n), r.oracle_id);
          fts.run(r.name, r.search_text, r.oracle_id);
        }
        db.prepare("INSERT INTO meta (key, value) VALUES ('last_ingest', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value").run(new Date().toISOString());
      });
      return { inserted, skipped };
    },
    close() { db.exec("DROP TABLE IF EXISTS temp.ingest_staging"); },
  };
}

export function ingestCards(db: Database, cards: ScryfallCard[]): { inserted: number; skipped: number } {
  const staging = stage(db);
  try {
    for (let i = 0; i < cards.length; i += 500)
      withTransaction(db, () => { for (const card of cards.slice(i, i + 500)) staging.add(card); });
    return staging.finish();
  } finally { staging.close(); }
}

export async function ingestFromFile(db: Database, path: string) {
  console.log(`Streaming ${path}...`);
  const staging = stage(db);
  let batch: ScryfallCard[] = [];
  const flush = () => { withTransaction(db, () => { for (const card of batch) staging.add(card); }); batch = []; };
  try {
    for await (const card of readJsonArray(path)) {
      batch.push(card as ScryfallCard);
      if (batch.length >= 500) flush();
    }
    flush();
    const result = staging.finish();
    console.log(`Done: ${result.inserted} cards ingested, ${result.skipped} non-card entries skipped.`);
    return result;
  } finally { staging.close(); }
}
