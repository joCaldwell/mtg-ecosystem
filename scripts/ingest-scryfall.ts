// Atomic oracle_cards ingestion. Protocol: https://scryfall.com/docs/api/bulk-data
import { createWriteStream } from "node:fs";
import { mkdir, readFile, rename, rm, stat } from "node:fs/promises";
import path from "node:path";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { fileURLToPath, pathToFileURL } from "node:url";
import { randomUUID } from "node:crypto";
import { createGunzip } from "node:zlib";
import { readCards, readOracleManifest } from "./lib/scryfall.ts";

const CACHE_FILE = fileURLToPath(new URL("../.scryfall-cache/oracle-cards.json", import.meta.url));
const HEADERS = { "User-Agent": "MTGEcosystemParser/1.0.0 (joCaldwell/mtg-ecosystem)", Accept: "application/json" };

/** Frame validated JSONL records as an array without reserializing them.
 * Preserve strings, future fields, and exact numeric spellings. Fatal UTF-8
 * decoding prevents silently replacing malformed input bytes. */
async function* jsonlArray(chunks: AsyncIterable<Uint8Array>): AsyncGenerator<string> {
  const decoder = new TextDecoder("utf-8", { fatal: true });
  let pending = "", records = 0;
  function record(line: string): string {
    if (readCards(`[${line}]`).length !== 1) throw new Error("Expected exactly one Scryfall card per JSONL record");
    return `${records++ ? ",\n" : ""}${line}`;
  }
  yield "[\n";
  for await (const chunk of chunks) {
    pending += decoder.decode(chunk, { stream: true });
    let end: number;
    while ((end = pending.indexOf("\n")) !== -1) {
      const line = pending.slice(0, end);
      pending = pending.slice(end + 1);
      yield record(line);
    }
  }
  pending += decoder.decode();
  if (pending.length) yield record(pending);
  if (!records) throw new Error("Expected nonempty Scryfall JSONL data");
  yield "\n]\n";
}

/** File-system failures retain their error codes; invalid corpus encoding
 * is a validation failure, so a corrupt fresh cache can be downloaded again.
 * Decode only for validation: legacy downloads keep their exact input bytes. */
function decodeCorpus(bytes: Uint8Array): string {
  try { return new TextDecoder("utf-8", { fatal: true }).decode(bytes); }
  catch (cause) { throw new Error("Invalid UTF-8 in Scryfall corpus", { cause }); }
}

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
      readCards(decodeCorpus(await readFile(cacheFile)));
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
    const input = Readable.fromWeb(response.body);
    const output = createWriteStream(temporary, { flags: "wx" });
    if (entry.format === "gzip-jsonl") await pipeline(input, createGunzip(), jsonlArray, output);
    else await pipeline(input, output);
    const cards = readCards(decodeCorpus(await readFile(temporary)));
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
