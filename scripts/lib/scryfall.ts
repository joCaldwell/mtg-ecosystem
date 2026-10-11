// Fields verified against https://scryfall.com/docs/api/cards and
// https://scryfall.com/docs/api/bulk-data (2026-09-05). Oracle identity is not
// printing identity; reversible cards carry oracle IDs on their faces.
export interface CardFace {
  name: string;
  oracle_text?: string | null;
  oracle_id?: string | null;
  type_line?: string | null;
}
export interface ScryfallCard extends CardFace {
  id: string;
  layout: string;
  card_faces?: CardFace[] | null;
  digital: boolean;
  games?: string[];
  border_color: string;
  security_stamp?: string | null;
}

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
function face(value: unknown): value is CardFace {
  return record(value) && typeof value.name === "string" && value.name.length > 0 &&
    (value.oracle_text == null || typeof value.oracle_text === "string") &&
    (value.type_line == null || typeof value.type_line === "string") &&
    (value.oracle_id == null || (typeof value.oracle_id === "string" && value.oracle_id.length > 0));
}

/** Validate the fields we consume, allowing Scryfall to add unrelated fields. */
export function readCards(text: string | Uint8Array): ScryfallCard[] {
  // Buffer.toString silently substitutes U+FFFD for corrupt input. A card's
  // name or rules must never be repaired implicitly at this boundary.
  const data: unknown = JSON.parse(typeof text === "string" ? text : new TextDecoder("utf-8", { fatal: true }).decode(text));
  if (!Array.isArray(data) || data.length === 0) throw new Error("Expected a nonempty Scryfall card array");
  for (const [index, card] of data.entries()) {
    if (!record(card) || !face(card) || (typeof card.id !== "string" || !card.id) ||
      (typeof card.layout !== "string" || !card.layout) || typeof card.digital !== "boolean" ||
      typeof card.border_color !== "string" ||
      !(card.games === undefined || (Array.isArray(card.games) && card.games.every(game => typeof game === "string"))) ||
      !(card.security_stamp === undefined || card.security_stamp === null || typeof card.security_stamp === "string") ||
      !(card.card_faces == null || (Array.isArray(card.card_faces) && card.card_faces.length > 0 && card.card_faces.every(face)))) {
      throw new Error(`Invalid Scryfall card at index ${index}`);
    }
    if (!card.oracle_id && !(card.card_faces as CardFace[] | null | undefined)?.every(f => f.oracle_id))
      throw new Error(`Missing oracle identity at index ${index}`);
  }
  return data as ScryfallCard[];
}

export interface BulkEntry { download_uri: string; updated_at: string; format: "json-array" | "gzip-jsonl" }
export function readOracleManifest(value: unknown): BulkEntry {
  if (!record(value) || !Array.isArray(value.data)) throw new Error("Invalid Scryfall bulk manifest");
  const item: unknown = value.data.find(item => record(item) && item.type === "oracle_cards");
  if (!record(item) ||
      typeof item.updated_at !== "string" || !Number.isFinite(Date.parse(item.updated_at)))
    throw new Error("Missing or invalid oracle_cards manifest entry");
  // The live Scryfall manifest (https://api.scryfall.com/bulk-data,
  // 2026-10-10) exposes gzip JSONL via jsonl_download_uri. Historical array
  // manifests remain a distinct input format; never assume JSONL is an array.
  const format = typeof item.jsonl_download_uri === "string" ? "gzip-jsonl" : "json-array";
  const uri = format === "gzip-jsonl" ? item.jsonl_download_uri : item.download_uri;
  if (typeof uri !== "string") throw new Error("Missing or invalid oracle_cards manifest download URI");
  const url = new URL(uri);
  if (url.protocol !== "https:") throw new Error("Bulk download URL must use HTTPS");
  if (format === "gzip-jsonl" && !url.pathname.endsWith(".jsonl.gz"))
    throw new Error("Unsupported Scryfall JSONL compression format");
  return { download_uri: uri, updated_at: item.updated_at, format };
}
