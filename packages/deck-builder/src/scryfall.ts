import { createWriteStream, existsSync, mkdirSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { Readable } from "node:stream";
import { finished } from "node:stream/promises";
import { fileURLToPath } from "node:url";

// Shared with scripts/ingest-scryfall.ts at the repo root so the bulk file
// is only ever downloaded once per day for the whole monorepo.
const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
export const CACHE_FILE = join(REPO_ROOT, ".scryfall-cache", "oracle-cards.json");

const MANIFEST_URL = "https://api.scryfall.com/bulk-data";
const USER_AGENT = "mtg-ecosystem-deckbuilder/0.1";
const MAX_AGE_HOURS = 24;

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
    data: Array<{ type: string; download_uri: string; size: number }>;
  };
  const entry = manifest.data.find((d) => d.type === "oracle_cards");
  if (!entry) throw new Error("oracle_cards entry not found in bulk-data manifest");

  console.log(`Downloading oracle-cards (${(entry.size / 1e6).toFixed(0)} MB)...`);
  const res = await fetch(entry.download_uri, { headers: { "User-Agent": USER_AGENT } });
  if (!res.ok || !res.body) throw new Error(`Bulk download failed: HTTP ${res.status}`);

  mkdirSync(dirname(CACHE_FILE), { recursive: true });
  const tmp = CACHE_FILE + ".tmp";
  await finished(
    Readable.fromWeb(res.body as import("node:stream/web").ReadableStream).pipe(
      createWriteStream(tmp),
    ),
  );
  const { renameSync } = await import("node:fs");
  renameSync(tmp, CACHE_FILE);
  console.log(`Saved to ${CACHE_FILE}`);
  return CACHE_FILE;
}
