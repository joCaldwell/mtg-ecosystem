// Build once from a frozen complete Scryfall default_cards JSONL download.
// The Oracle text corpus is intentionally NOT replaced by this printing data.
import fs from "node:fs";
import { createHash } from "node:crypto";
import { createInterface } from "node:readline";
import { Readable } from "node:stream";
import { pathToFileURL } from "node:url";
import path from "node:path";
import { readCards, type ScryfallCard } from "./lib/scryfall.ts";
import { eligiblePaperPrinting, oracleIdentity, printingWitness, readPaperEvidence, type PaperEvidenceDocument } from "./lib/paper-evidence.ts";

export async function buildPaperEvidence(inputFile: string, provenance: Pick<PaperEvidenceDocument["source"], "downloadUri" | "updatedAt">) {
  const sourceHash = createHash("sha256");
  let bytes = 0, records = 0;
  const input = fs.createReadStream(inputFile);
  const decoded = Readable.from((async function* () {
    const decoder = new TextDecoder("utf-8", { fatal: true });
    for await (const chunk of input) {
      sourceHash.update(chunk); bytes += chunk.length;
      yield decoder.decode(chunk, { stream: true });
    }
    yield decoder.decode();
  })());
  const printings = new Map<string, ScryfallCard>();
  for await (const line of createInterface({ input: decoded, crlfDelay: Infinity })) {
    if (!line.trim()) throw new Error("Empty record in printing JSONL");
    const cards = readCards(`[${line}]`);
    if (cards.length !== 1) throw new Error("Expected exactly one printing per JSONL record");
    const card = cards[0];
    records++;
    if (!eligiblePaperPrinting(card)) continue;
    const id = oracleIdentity(card), previous = printings.get(id);
    if (!previous || card.id < previous.id) printings.set(id, printingWitness(card));
  }
  if (!records) throw new Error("Empty printing corpus");
  const document: PaperEvidenceDocument = { schemaVersion: 1,
    source: { sha256: sourceHash.digest("hex"), bytes, records, format: "jsonl", ...provenance },
    printings: [...printings].sort(([a], [b]) => a.localeCompare(b)).map(([, card]) => card) };
  return readPaperEvidence(Buffer.from(`${JSON.stringify(document)}\n`));
}
async function main() {
  const [inputFile, manifestFile, outputFile] = process.argv.slice(2);
  if (!inputFile || !manifestFile || !outputFile || process.argv.length !== 5)
    throw new Error("Usage: node scripts/build-paper-evidence.ts default-cards.jsonl bulk-manifest.json evidence.json");
  const manifest = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(fs.readFileSync(manifestFile)));
  const entry = manifest.data?.find((value: { type?: string }) => value.type === "default_cards");
  if (typeof entry?.jsonl_download_uri !== "string" || typeof entry.updated_at !== "string") throw new Error("Invalid default_cards JSONL manifest");
  const evidence = await buildPaperEvidence(inputFile, { downloadUri: entry.jsonl_download_uri, updatedAt: entry.updated_at });
  fs.writeFileSync(outputFile, evidence.bytes, { flag: "wx" });
  console.log(JSON.stringify({ ...evidence.fingerprint, identities: evidence.byIdentity.size, source: evidence.document.source }));
}
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href)
  main().catch(error => { console.error(error); process.exitCode = 1; });
