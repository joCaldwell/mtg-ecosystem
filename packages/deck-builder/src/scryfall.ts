import { createWriteStream, existsSync, mkdirSync, rmSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { Readable, Transform, pipeline as pipelineOf } from "node:stream";
import { StringDecoder } from "node:string_decoder";
import { pipeline } from "node:stream/promises";
import { createGunzip } from "node:zlib";
import type { TokenPrinting } from "./token-catalog.ts";
import type { ScryfallPrinting } from "./prices.ts";

import { DATA_DIR } from "./db.ts";

// Per-instance cache: jobs and CLI use the same DATA_DIR as the database.
export const CACHE_FILE = join(DATA_DIR, ".scryfall-cache", "oracle-cards.json");

const MANIFEST_URL = "https://api.scryfall.com/bulk-data";
const USER_AGENT = "mtg-ecosystem-deckbuilder/0.1";
const MAX_AGE_HOURS = 24;

// Scryfall changed the bulk-data manifest in August 2026 from an uncompressed
// JSON-array `download_uri` to a gzipped JSONL `jsonl_download_uri`.
// Source: https://api.scryfall.com/bulk-data (live manifest, verified
// 2026-08-25). Keep the cache as a JSON array so the existing deck-builder
// ingest and the parser-side consumers of this shared file remain compatible.
export function jsonlToJsonArray(): Transform {
  // A chunk can end partway through a character ("—" is three bytes), so the
  // bytes are decoded as a stream rather than one chunk at a time.
  const decoder = new StringDecoder("utf8");
  let remainder = "";
  let first = true;
  return new Transform({
    construct(callback) {
      this.push("[");
      callback();
    },
    transform(chunk, _encoding, callback) {
      const lines = (remainder + decoder.write(chunk)).split(/\r?\n/);
      remainder = lines.pop() ?? "";
      for (const line of lines) {
        if (!line.trim()) continue;
        this.push(first ? line : `,${line}`);
        first = false;
      }
      callback();
    },
    flush(callback) {
      remainder += decoder.end();
      if (remainder.trim()) this.push(first ? remainder : `,${remainder}`);
      this.push("]\n");
      callback();
    },
  });
}

export interface BulkEntry {
  type: string;
  /** When Scryfall generated this file. */
  updated_at: string;
  download_uri?: string;
  size?: number;
  jsonl_download_uri?: string;
  compressed_size?: number;
}

export async function bulkEntry(type: string): Promise<BulkEntry> {
  const res = await fetch(MANIFEST_URL, {
    headers: { "User-Agent": USER_AGENT, Accept: "application/json" },
  });
  if (!res.ok) throw new Error(`Manifest fetch failed: HTTP ${res.status}`);
  const manifest = (await res.json()) as { data: BulkEntry[] };
  const entry = manifest.data.find((d) => d.type === type);
  if (!entry) throw new Error(`${type} entry not found in bulk-data manifest`);
  return entry;
}

// Every printing, for pricing (spec §4.4). The file is 79 MB compressed and
// over 600 MB as text, and one number per card is kept from it, so it is read
// as it arrives and never written to disk.
export async function* streamPrintings(entry: BulkEntry): AsyncGenerator<ScryfallPrinting & TokenPrinting> {
  if (!entry.jsonl_download_uri) throw new Error(`${entry.type} entry has no JSONL download URL`);
  const res = await fetch(entry.jsonl_download_uri, { headers: { "User-Agent": USER_AGENT } });
  if (!res.ok || !res.body) throw new Error(`Bulk download failed: HTTP ${res.status}`);
  // The callback form returns the last stream and fails it if the download
  // does, so a broken connection ends the loop below instead of stalling it.
  const text = pipelineOf(
    Readable.fromWeb(res.body as import("node:stream/web").ReadableStream),
    createGunzip(),
    () => {},
  );
  const decoder = new StringDecoder("utf8");
  let remainder = "";
  for await (const chunk of text) {
    const lines = (remainder + decoder.write(chunk as Buffer)).split(/\r?\n/);
    remainder = lines.pop() ?? "";
    for (const line of lines) if (line.trim()) yield JSON.parse(line) as ScryfallPrinting & TokenPrinting;
  }
  remainder += decoder.end();
  if (remainder.trim()) yield JSON.parse(remainder) as ScryfallPrinting & TokenPrinting;
}

export async function ensureBulkFile(force = false): Promise<string> {
  if (!force && existsSync(CACHE_FILE)) {
    const ageHours = (Date.now() - statSync(CACHE_FILE).mtimeMs) / 3_600_000;
    if (ageHours < MAX_AGE_HOURS) {
      console.log(`Bulk file is fresh (${ageHours.toFixed(1)}h old); skipping download.`);
      return CACHE_FILE;
    }
  }

  console.log("Fetching Scryfall bulk-data manifest...");
  const entry = await bulkEntry("oracle_cards");
  const downloadUrl = entry.jsonl_download_uri ?? entry.download_uri;
  if (!downloadUrl) throw new Error("oracle_cards entry has no supported download URL");
  const size = entry.compressed_size ?? entry.size;

  console.log(`Downloading oracle-cards${size == null ? "" : ` (${(size / 1e6).toFixed(0)} MB)`}...`);
  const res = await fetch(downloadUrl, { headers: { "User-Agent": USER_AGENT } });
  if (!res.ok || !res.body) throw new Error(`Bulk download failed: HTTP ${res.status}`);

  mkdirSync(dirname(CACHE_FILE), { recursive: true });
  const tmp = CACHE_FILE + ".tmp";
  try {
    const body = Readable.fromWeb(res.body as import("node:stream/web").ReadableStream);
    if (entry.jsonl_download_uri) {
      await pipeline(body, createGunzip(), jsonlToJsonArray(), createWriteStream(tmp));
    } else {
      await pipeline(body, createWriteStream(tmp));
    }
  } catch (error) {
    rmSync(tmp, { force: true });
    throw error;
  }
  const { renameSync } = await import("node:fs");
  renameSync(tmp, CACHE_FILE);
  console.log(`Saved to ${CACHE_FILE}`);
  return CACHE_FILE;
}

/** Rare all_parts links point to non-English printings absent from default_cards.
 * Resolve only those IDs in the refresh child, never on a deck read. The
 * collection endpoint accepts 75 identifiers, at most two requests per second:
 * https://scryfall.com/docs/api/cards/collection (verified 2026-10-29).
 */
export async function* relatedTokenPrintings(ids: string[]): AsyncGenerator<TokenPrinting> {
  if (ids.length > 750) throw new Error("Too many unresolved token printings; refusing incomplete bulk data");
  for (let i = 0; i < ids.length; i += 75) {
    if (i) await new Promise(resolve => setTimeout(resolve, 510));
    const batch = ids.slice(i, i + 75);
    const res = await fetch("https://api.scryfall.com/cards/collection", {
      method: "POST", headers: { "User-Agent": USER_AGENT, Accept: "application/json", "Content-Type": "application/json" },
      body: JSON.stringify({ identifiers: batch.map(id => ({ id })) }), signal: AbortSignal.timeout(30_000),
    });
    if (!res.ok) throw new Error(`Related token lookup failed: HTTP ${res.status}`);
    const result = await res.json() as { data: TokenPrinting[]; not_found?: unknown[] };
    if (result.not_found?.length || !Array.isArray(result.data) || result.data.length !== batch.length || result.data.some(c => !batch.includes(c.id)))
      throw new Error("Scryfall could not resolve every related token printing; previous catalog retained");
    yield* result.data;
  }
}
