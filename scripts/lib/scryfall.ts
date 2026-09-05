// Fields verified against https://scryfall.com/docs/api/cards and
// https://scryfall.com/docs/api/bulk-data (2026-09-05). Oracle identity is not
// printing identity; reversible cards carry oracle IDs on their faces.
export interface CardFace {
  name: string;
  oracle_text?: string | null;
  oracle_id?: string | null;
}
export interface ScryfallCard extends CardFace {
  id: string;
  layout: string;
  card_faces?: CardFace[] | null;
  digital: boolean;
  border_color: string;
  security_stamp?: string | null;
}

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
function face(value: unknown): value is CardFace {
  return record(value) && typeof value.name === "string" && value.name.length > 0 &&
    (value.oracle_text == null || typeof value.oracle_text === "string") &&
    (value.oracle_id == null || (typeof value.oracle_id === "string" && value.oracle_id.length > 0));
}

/** Validate the fields we consume, allowing Scryfall to add unrelated fields. */
export function readCards(text: string): ScryfallCard[] {
  const data: unknown = JSON.parse(text);
  if (!Array.isArray(data) || data.length === 0) throw new Error("Expected a nonempty Scryfall card array");
  for (const [index, card] of data.entries()) {
    if (!record(card) || !face(card) || (typeof card.id !== "string" || !card.id) ||
      (typeof card.layout !== "string" || !card.layout) || typeof card.digital !== "boolean" ||
      typeof card.border_color !== "string" ||
      !(card.security_stamp === undefined || card.security_stamp === null || typeof card.security_stamp === "string") ||
      !(card.card_faces == null || (Array.isArray(card.card_faces) && card.card_faces.length > 0 && card.card_faces.every(face)))) {
      throw new Error(`Invalid Scryfall card at index ${index}`);
    }
    if (!card.oracle_id && !(card.card_faces as CardFace[] | null | undefined)?.every(f => f.oracle_id))
      throw new Error(`Missing oracle identity at index ${index}`);
  }
  return data as ScryfallCard[];
}

export interface BulkEntry { download_uri: string; updated_at: string }
export function readOracleManifest(value: unknown): BulkEntry {
  if (!record(value) || !Array.isArray(value.data)) throw new Error("Invalid Scryfall bulk manifest");
  const item: unknown = value.data.find(item => record(item) && item.type === "oracle_cards");
  if (!record(item) || typeof item.download_uri !== "string" ||
      typeof item.updated_at !== "string" || !Number.isFinite(Date.parse(item.updated_at)))
    throw new Error("Missing or invalid oracle_cards manifest entry");
  if (new URL(item.download_uri).protocol !== "https:") throw new Error("Bulk download URL must use HTTPS");
  return { download_uri: item.download_uri, updated_at: item.updated_at };
}
