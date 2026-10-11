// Scryfall's digital/games/border fields describe a PRINTING, not an Oracle
// identity. Oracle bulk chooses one representative printing per identity.
// Primary field definitions (checked 2026-10-10):
// https://github.com/scryfall/api-types/blob/main/src/objects/Card/CardFields.ts
// https://api.scryfall.com/bulk-data
import fs from "node:fs";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";
import { readCards, type ScryfallCard } from "./scryfall.ts";

export const PAPER_SCOPE = "paper-printing-evidence-non-silver-non-acorn-v2";
export const DEFAULT_PAPER_EVIDENCE = fileURLToPath(new URL("../../.scryfall-cache/paper-printing-evidence.json", import.meta.url));
export const SKIP_LAYOUTS = new Set(["token", "double_faced_token", "emblem", "art_series", "planar", "vanguard", "scheme"]);
const hash = (bytes: Buffer): string => createHash("sha256").update(bytes).digest("hex");
export function oracleIdentity(card: ScryfallCard): string {
  return card.oracle_id ?? card.card_faces!.map(face => face.oracle_id!).join("/");
}
export function eligiblePaperPrinting(card: ScryfallCard): boolean {
  return !card.digital && card.games?.includes("paper") === true && card.border_color !== "silver" && card.security_stamp !== "acorn";
}
/** Only source fields that establish printing identity and eligibility. Full
 * Oracle source objects remain in the card archive, without this projection. */
export function printingWitness(card: ScryfallCard): ScryfallCard {
  return { id: card.id, name: card.name, oracle_id: card.oracle_id, layout: card.layout,
    digital: card.digital, games: card.games, border_color: card.border_color, security_stamp: card.security_stamp,
    ...(card.card_faces ? { card_faces: card.card_faces.map(face => ({ name: face.name, oracle_id: face.oracle_id })) } : {}) };
}
export interface PaperEvidenceDocument {
  schemaVersion: 1;
  source: { sha256: string; bytes: number; records: number; format: "jsonl" | "json"; downloadUri: string; updatedAt: string };
  /** One actual eligible printing for each identity, chosen deterministically
   * by printing ID. Never inferred from a name, legality, or oracle wording. */
  printings: ScryfallCard[];
}
export interface PaperEvidence {
  document: PaperEvidenceDocument;
  bytes: Buffer;
  byIdentity: ReadonlyMap<string, ScryfallCard>;
  fingerprint: { sha256: string; bytes: number; snapshotFile: string };
}
export function readPaperEvidence(bytes: Buffer): PaperEvidence {
  const value = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)) as PaperEvidenceDocument;
  if (value?.schemaVersion !== 1 || !value.source || typeof value.source.sha256 !== "string" || !/^[a-f0-9]{64}$/.test(value.source.sha256) ||
      !Number.isSafeInteger(value.source.bytes) || value.source.bytes < 0 ||
      !Number.isSafeInteger(value.source.records) || value.source.records < 0 ||
      !["json", "jsonl"].includes(value.source.format) || typeof value.source.downloadUri !== "string" ||
      !value.source.downloadUri.startsWith("https://") || typeof value.source.updatedAt !== "string" || !Number.isFinite(Date.parse(value.source.updatedAt)) ||
      !Array.isArray(value.printings)) throw new Error("Invalid paper-printing evidence");
  const printings = value.printings.length ? readCards(JSON.stringify(value.printings)) : [];
  const byIdentity = new Map<string, ScryfallCard>();
  for (const card of printings) {
    if (!eligiblePaperPrinting(card)) throw new Error("Evidence contains an ineligible paper printing");
    const id = oracleIdentity(card);
    if (byIdentity.has(id)) throw new Error("Duplicate Oracle identity in paper evidence");
    byIdentity.set(id, card);
  }
  const sha256 = hash(bytes);
  return { document: value, bytes, byIdentity, fingerprint: { sha256, bytes: bytes.length, snapshotFile: `paper-printing-evidence-${sha256}.json` } };
}
export function loadPaperEvidence(file = DEFAULT_PAPER_EVIDENCE): PaperEvidence {
  return readPaperEvidence(fs.readFileSync(file));
}
export function paperExclusion(card: ScryfallCard, evidence: PaperEvidence): string | undefined {
  if (SKIP_LAYOUTS.has(card.layout)) return `layout:${card.layout}`;
  if (eligiblePaperPrinting(card) || evidence.byIdentity.has(oracleIdentity(card))) return undefined;
  if (card.border_color === "silver" || card.security_stamp === "acorn") return "silver-or-acorn";
  return "no-eligible-paper-printing";
}
