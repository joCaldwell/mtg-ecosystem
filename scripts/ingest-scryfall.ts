// Atomic oracle_cards ingestion. Protocol: https://scryfall.com/docs/api/bulk-data
import { createWriteStream } from "node:fs";
import { mkdir, readFile, rename, rm, stat } from "node:fs/promises";
import path from "node:path";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { fileURLToPath, pathToFileURL } from "node:url";
import { randomUUID } from "node:crypto";
import { readCards, readOracleManifest } from "./lib/scryfall.ts";

const CACHE_FILE = fileURLToPath(new URL("../.scryfall-cache/oracle-cards.json", import.meta.url));
const HEADERS = { "User-Agent": "MTGEcosystemParser/1.0.0 (joCaldwell/mtg-ecosystem)", Accept: "application/json" };

/** Keep the last known good corpus until the replacement is fully received
 * and validated. A same-directory rename publishes the complete file. */
export async function ingest(options: {
  cacheFile?: string;
  fetcher?: typeof fetch;
  now?: number;
  log?: (message: string) => void;
} = {}): Promise<"cached" | "downloaded"> {
  const cacheFile = options.cacheFile ?? CACHE_FILE;
  const fetcher = options.fetcher ?? fetch;
  const log = options.log ?? console.log;
  const now = options.now ?? Date.now();
  await mkdir(path.dirname(cacheFile), { recursive: true });
  try {
    const info = await stat(cacheFile);
    if (now - info.mtimeMs < 24 * 60 * 60 * 1000) {
      readCards(await readFile(cacheFile, "utf8"));
      log("Validated cache is fresh; skipping download.");
      return "cached";
    }
  } catch (error) {
    // A missing or invalid cache needs a replacement. Other filesystem errors
    // must surface instead of being mistaken for a stale download.
    if (error && typeof error === "object" && "code" in error && error.code !== "ENOENT") throw error;
  }
  const manifestResponse = await fetcher("https://api.scryfall.com/bulk-data", { headers: HEADERS });
  if (!manifestResponse.ok) throw new Error(`Manifest request failed: ${manifestResponse.status}`);
  const entry = readOracleManifest(await manifestResponse.json());
  log(`Downloading oracle_cards updated ${entry.updated_at}`);
  const response = await fetcher(entry.download_uri, { headers: HEADERS });
  if (!response.ok || !response.body) throw new Error(`Bulk download failed: ${response.status}`);

  const temporary = `${cacheFile}.${randomUUID()}.tmp`;
  try {
    await pipeline(Readable.fromWeb(response.body), createWriteStream(temporary, { flags: "wx" }));
    const cards = readCards(await readFile(temporary, "utf8"));
    await rename(temporary, cacheFile);
    log(`Cached ${cards.length} validated records.`);
    return "downloaded";
  } finally {
    await rm(temporary, { force: true });
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  ingest().catch(error => { console.error("Ingestion failed:", error); process.exitCode = 1; });
}
