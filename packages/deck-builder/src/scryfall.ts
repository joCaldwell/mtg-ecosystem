import { createWriteStream, existsSync, mkdirSync, rmSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { Readable, Transform } from "node:stream";
import { pipeline } from "node:stream/promises";
import { fileURLToPath } from "node:url";
import { createGunzip } from "node:zlib";

// Shared with scripts/ingest-scryfall.ts at the repo root so the bulk file
// is only ever downloaded once per day for the whole monorepo.
const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
export const CACHE_FILE = join(REPO_ROOT, ".scryfall-cache", "oracle-cards.json");

const MANIFEST_URL = "https://api.scryfall.com/bulk-data";
const USER_AGENT = "mtg-ecosystem-deckbuilder/0.1";
const MAX_AGE_HOURS = 24;

// Scryfall changed the bulk-data manifest in August 2026 from an uncompressed
// JSON-array `download_uri` to a gzipped JSONL `jsonl_download_uri`.
// Source: https://api.scryfall.com/bulk-data (live manifest, verified
// 2026-08-25). Keep the cache as a JSON array so the existing deck-builder
// ingest and the parser-side consumers of this shared file remain compatible.
export function jsonlToJsonArray(): Transform {
  let remainder = "";
  let first = true;
  return new Transform({
    construct(callback) {
      this.push("[");
      callback();
    },
    transform(chunk, _encoding, callback) {
      const lines = (remainder + chunk.toString("utf8")).split(/\r?\n/);
      remainder = lines.pop() ?? "";
      for (const line of lines) {
        if (!line.trim()) continue;
        this.push(first ? line : `,${line}`);
        first = false;
      }
      callback();
    },
    flush(callback) {
      if (remainder.trim()) this.push(first ? remainder : `,${remainder}`);
      this.push("]\n");
      callback();
    },
  });
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
  const manifestRes = await fetch(MANIFEST_URL, {
    headers: { "User-Agent": USER_AGENT, Accept: "application/json" },
  });
  if (!manifestRes.ok) throw new Error(`Manifest fetch failed: HTTP ${manifestRes.status}`);
  const manifest = (await manifestRes.json()) as {
    data: Array<{
      type: string;
      download_uri?: string;
      size?: number;
      jsonl_download_uri?: string;
      compressed_size?: number;
    }>;
  };
  const entry = manifest.data.find((d) => d.type === "oracle_cards");
  if (!entry) throw new Error("oracle_cards entry not found in bulk-data manifest");
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
