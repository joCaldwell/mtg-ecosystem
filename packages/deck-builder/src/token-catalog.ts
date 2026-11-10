// Related Card Objects use printing IDs, not oracle IDs. Resolve them while
// streaming default_cards (already downloaded for prices), never by name or
// with API calls on a deck read. Verified 2026-10-29:
// https://scryfall.com/docs/api/cards#related-card-objects
// https://api.scryfall.com/bulk-data — default_cards contains every printing.
import type { Database } from "./sqlite.ts";
import { withTransaction } from "./db.ts";
import type { ScryfallCard } from "./ingest.ts";

export const TOKEN_CATALOG_VERSION = "1";
const TOKEN_LAYOUTS = new Set(["token", "double_faced_token", "emblem"]);

export interface TokenFacts {
  oracle_id: string;
  name: string;
  kind: "token" | "emblem";
  type_line: string;
  oracle_text: string;
  power: string | null;
  toughness: string | null;
  colors: string;
  image_small: string | null;
  image_normal: string | null;
  image_art_crop: string | null;
  artist: string | null;
}

export function tokenStatus(db: Database) {
  const meta = new Map((db.prepare("SELECT key, value FROM meta WHERE key IN ('token_catalog_version', 'tokens_as_of')").all() as {key: string; value: string}[]).map(r => [r.key, r.value]));
  return { ready: meta.get("token_catalog_version") === TOKEN_CATALOG_VERSION,
    as_of: meta.get("tokens_as_of") ?? null, source: "Scryfall" as const };
}

export interface TokenPrinting extends Omit<ScryfallCard, "oracle_id" | "color_identity"> {
  id: string;
  oracle_id?: string;
  color_identity?: string[];
  artist?: string | null;
}

function facts(card: TokenPrinting): TokenFacts {
  const front = card.card_faces?.[0];
  const images = card.image_uris ?? front?.image_uris;
  return {
    oracle_id: card.oracle_id!, name: card.name,
    kind: card.layout === "emblem" || /^Emblem\b/.test(card.type_line ?? "") ? "emblem" : "token",
    type_line: card.type_line ?? front?.type_line ?? "",
    oracle_text: card.oracle_text ?? (card.card_faces ?? []).map(face => `${face.name}\n${face.oracle_text ?? ""}`).join("\n//\n"),
    power: card.power ?? front?.power ?? null, toughness: card.toughness ?? front?.toughness ?? null,
    colors: ["W", "U", "B", "R", "G"].filter(c => (card.colors ?? front?.colors ?? []).includes(c)).join(""),
    image_small: images?.small ?? null, image_normal: images?.normal ?? null,
    image_art_crop: images?.art_crop ?? null, artist: card.artist ?? front?.artist ?? null,
  };
}

/** Disk-backed staging; bounded batches and no live changes until the entire
 * download and validation succeed. finish composes with the price transaction. */
export function stageTokenCatalog(db: Database) {
  db.exec(`PRAGMA temp_store = FILE;
    CREATE TEMP TABLE token_printings (id TEXT NOT NULL, oracle_id TEXT NOT NULL, facts_json TEXT NOT NULL, rank INTEGER NOT NULL, PRIMARY KEY (id, oracle_id));
    CREATE TEMP TABLE token_links (oracle_id TEXT NOT NULL, printing_id TEXT NOT NULL, PRIMARY KEY (oracle_id, printing_id));
    CREATE TEMP TABLE token_sources (oracle_id TEXT PRIMARY KEY);`);
  const printing = db.prepare("INSERT OR REPLACE INTO token_printings VALUES (?, ?, ?, ?)");
  const link = db.prepare("INSERT OR IGNORE INTO token_links VALUES (?, ?)");
  const source = db.prepare("INSERT OR IGNORE INTO token_sources VALUES (?)");
  let batch: TokenPrinting[] = [];
  const flush = () => {
    withTransaction(db, () => {
      for (const card of batch.flatMap(card => card.layout === "reversible_card"
        ? (card.card_faces ?? []).map(face => ({ ...card, ...face, card_faces: undefined, layout: "normal" }))
        : [card])) {
        if (!card.oracle_id) continue;
        if (TOKEN_LAYOUTS.has(card.layout) || /(?:^| \/\/ )(?:Token|Emblem)\b/.test(card.type_line ?? "")) {
          const row = facts(card);
          // Prefer a single-sided token with an image. Stable ties prevent
          // artwork changing when Scryfall changes the bulk iteration order.
          printing.run(card.id, card.oracle_id, JSON.stringify(row), (card.layout === "double_faced_token" ? 2 : 0) + (row.image_normal ? 0 : 1));
        } else {
          source.run(card.oracle_id);
          for (const part of card.all_parts ?? [])
            // Emblems are combo_piece, not token, in Scryfall's relationship.
            if (part.component === "token" || /^Emblem\b/.test(part.type_line))
              link.run(card.oracle_id, part.id);
        }
      }
    });
    batch = [];
  };
  return {
    add(card: TokenPrinting) { batch.push(card); if (batch.length >= 500) flush(); },
    missingPrintingIds() {
      flush();
      return (db.prepare(`SELECT DISTINCT l.printing_id FROM token_links l JOIN cards c ON c.oracle_id = l.oracle_id
        LEFT JOIN token_printings t ON t.id = l.printing_id WHERE t.id IS NULL ORDER BY l.printing_id`).all() as {printing_id: string}[]).map(r => r.printing_id);
    },
    finish(asOf: string) {
      flush();
      const current = (db.prepare("SELECT COUNT(*) n FROM cards WHERE retired = 0").get() as {n: number}).n;
      const present = (db.prepare("SELECT COUNT(*) n FROM cards c JOIN token_sources s USING (oracle_id) WHERE c.retired = 0").get() as {n: number}).n;
      const tokenCount = (db.prepare("SELECT COUNT(*) n FROM token_printings").get() as {n: number}).n;
      if (!tokenCount || present < current * 0.9) throw new Error("Refusing truncated token bulk data; previous catalog retained");
      const missing = (db.prepare(`SELECT COUNT(*) n FROM token_links l JOIN cards c ON c.oracle_id = l.oracle_id
        LEFT JOIN token_printings t ON t.id = l.printing_id WHERE t.id IS NULL`).get() as {n: number}).n;
      if (missing) throw new Error(`Token bulk data has ${missing} unresolved related printing IDs; previous catalog retained`);
      withTransaction(db, () => {
        db.exec(`INSERT INTO token_catalog (oracle_id, facts_json)
          SELECT oracle_id, facts_json FROM (
            SELECT *, row_number() OVER (PARTITION BY oracle_id ORDER BY rank, id) AS choice FROM token_printings
          ) WHERE choice = 1
          ON CONFLICT(oracle_id) DO UPDATE SET facts_json = excluded.facts_json;
          -- Retired cards retain their last known links, just as their facts.
          DELETE FROM card_token_links WHERE oracle_id IN (SELECT oracle_id FROM token_sources);
          INSERT OR IGNORE INTO card_token_links (oracle_id, token_oracle_id)
          SELECT l.oracle_id, t.oracle_id FROM token_links l JOIN token_printings t ON t.id = l.printing_id
          JOIN cards c ON c.oracle_id = l.oracle_id;`);
        const meta = db.prepare("INSERT INTO meta VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value");
        meta.run("token_catalog_version", TOKEN_CATALOG_VERSION);
        meta.run("tokens_as_of", asOf);
      });
      return { tokens: (db.prepare("SELECT COUNT(DISTINCT oracle_id) n FROM token_printings").get() as {n: number}).n,
        links: (db.prepare("SELECT COUNT(*) n FROM card_token_links").get() as {n: number}).n };
    },
    close() { db.exec("DROP TABLE token_printings; DROP TABLE token_links; DROP TABLE token_sources"); },
  };
}
