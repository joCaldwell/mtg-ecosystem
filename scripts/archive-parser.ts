// A reproducible, nonexecutable archive of parser results and complete source
// records. This is deliberately separate from semantic lowering / Card IR.
import fs from "node:fs";
import { link, mkdir, open, rename, rm } from "node:fs/promises";
import path from "node:path";
import { createHash, randomUUID } from "node:crypto";
import { createInterface } from "node:readline";
import { fileURLToPath, pathToFileURL } from "node:url";
import { parseOracleText } from "../packages/oracle-parser/src/index.ts";
import { buildCardNameCatalog } from "../packages/oracle-parser/src/card-names.ts";
import { parseOracleTextWithPrintedEvidence } from "../packages/oracle-parser/src/printed/evidence.ts";
import type { PrintedCard, PrintedFace } from "../packages/oracle-parser/src/printed/schema.ts";
import type { ParseCardResult } from "../packages/oracle-parser/src/ast.ts";
import { analyzeCardBindings } from "../packages/oracle-parser/src/semantic/bindings.ts";
import type { CardBindingAnalysis } from "../packages/oracle-parser/src/semantic/types.ts";
import { resolveCard, type CardResolution } from "../packages/oracle-parser/src/semantic/resolution.ts";
import { readCards, type ScryfallCard } from "./lib/scryfall.ts";
import { SCOPE, identity, sha256, sourceIdentity, emptyBindingSummary, addBindingSummary } from "./validate-parser.ts";
import { loadPaperEvidence, paperExclusion, readPaperEvidence, type PaperEvidence } from "./lib/paper-evidence.ts";

const ROOT = fileURLToPath(new URL("../", import.meta.url));
const DEFAULT_INPUT = path.join(ROOT, ".scryfall-cache/oracle-cards.json");
const DEFAULT_OUTPUT = path.join(ROOT, "generated/oracle-ast.jsonl");
const CANDIDATE_OUTPUT = path.join(ROOT, "generated/oracle-ast-phase1-r6-candidate.jsonl");
const DEFAULT_RULES = path.join(ROOT, ".scryfall-cache/ComprehensiveRules-20260925.txt");
type Fingerprint = ReturnType<typeof sourceIdentity>;
type Fingerprinter = () => Fingerprint;

type ArchivedFace =
  | { sourceFaceIndex: number; status: "missing-oracle-text"; name: string }
  | { sourceFaceIndex: number; status: "attempted"; result: ParseCardResult };
interface RecordSource {
  recordType: "card";
  sourceIndex: number;
  oracleIdentity: string;
  sourceSha256: string;
  /** The original object is retained intact, including fields not consumed
   * by the parser's narrow Scryfall adapter. No characteristics are projected. */
  source: ScryfallCard;
}
/** `resolution` is the authoritative reference/step/span contract (schema 4).
 * `binding` is the diagnostic graph it is derived from; only whole-card
 * family lowerers still read it, until they retire. */
type Analyzed = { binding: CardBindingAnalysis; bindingSha256: string; resolution: CardResolution; resolutionSha256: string };
export type ArchiveRecord = RecordSource & (
  | { status: "excluded"; reason: string }
  | ({ status: "parsed"; faces: ArchivedFace[]; astSha256: string } & Analyzed)
  | ({ status: "unsupported"; faces: ArchivedFace[] } & Analyzed)
);
export interface ArchiveManifest {
  recordType: "manifest";
  artifactKind: "oracle-ast-archive";
  schemaVersion: 4;
  executable: false;
  corpus: { sha256: string; bytes: number; canonicalRecordsSha256: string; snapshotFile: string };
  paperEvidence: PaperEvidence["fingerprint"];
  parser: Fingerprint;
  rules: { edition: string; url: string };
  scope: string;
  inputRecords: number;
  excluded: Record<string, number>;
  cardsChecked: number;
  cardsOk: number;
  linesChecked: number;
  linesOk: number;
  bindingSummary: ReturnType<typeof emptyBindingSummary>;
  resolutionSummary: ReturnType<typeof emptyResolutionSummary>;
}
export interface ParserArchive {
  manifest: ArchiveManifest;
  records: ArchiveRecord[];
  /** Exact original bytes, published separately to preserve JSON number
   * spellings, whitespace, and future fields without any decoding losses. */
  sourceBytes: Buffer;
  paperEvidenceBytes: Buffer;
}

/** JSON.parse can produce Infinity for a number such as 1e400. JSON.stringify
 * would silently turn that into null. Reject that loss instead of archiving it. */
function requireFiniteJson(value: unknown): void {
  if (typeof value === "number" && !Number.isFinite(value)) throw new Error("Non-finite number cannot be archived losslessly");
  if (Array.isArray(value)) for (const item of value) requireFiniteJson(item);
  else if (value !== null && typeof value === "object") {
    for (const item of Object.values(value)) requireFiniteJson(item);
  }
}

export function requireSnapshot(expected: Fingerprint, fingerprint: Fingerprinter): void {
  if (JSON.stringify(fingerprint()) !== JSON.stringify(expected)) {
    throw new Error("Parser sources changed during archive generation; retry after edits finish");
  }
}

export function emptyResolutionSummary() {
  return { cardsComplete: 0, blocks: 0, blocksComplete: 0, residuals: {} as Record<string, number> };
}
/** Cards and blocks by completeness; residual reasons count blocks (or cards
 * for card-level reasons) that carry them. */
export function addResolutionSummary(summary: ReturnType<typeof emptyResolutionSummary>, resolution: CardResolution): void {
  if (resolution.complete) summary.cardsComplete++;
  for (const reason of resolution.residuals) summary.residuals[reason] = (summary.residuals[reason] ?? 0) + 1;
  for (const block of resolution.faces.flatMap(face => face.blocks ?? [])) {
    summary.blocks++;
    if (!block.residuals.length) summary.blocksComplete++;
    for (const reason of block.residuals) summary.residuals[reason] = (summary.residuals[reason] ?? 0) + 1;
  }
}

function successfulAstHash(faces: ArchivedFace[]): string {
  const abilities = faces.map(face => {
    if (face.status !== "attempted" || !face.result.ok) throw new Error("Unsupported face cannot have a complete-card AST hash");
    return face.result.abilities;
  });
  return sha256(JSON.stringify(abilities));
}

type ArchiveOptions = { cacheFile?: string; fingerprint?: Fingerprinter; paperEvidence?: PaperEvidence };

/** Both cuts use the same source order, paper scope and legacy row builder. */
export function prepareArchive(options: ArchiveOptions = {}) {
  const fingerprint = options.fingerprint ?? sourceIdentity;
  const parser = fingerprint();
  const corpus = fs.readFileSync(options.cacheFile ?? DEFAULT_INPUT);
  const evidence = options.paperEvidence ?? loadPaperEvidence();
  const cards = readCards(corpus);
  const cardNames = buildCardNameCatalog(cards);
  requireFiniteJson(cards);
  const sourceRecordsHash = createHash("sha256");
  const corpusSha256 = sha256(corpus);
  const manifest: ArchiveManifest = {
    recordType: "manifest", artifactKind: "oracle-ast-archive", schemaVersion: 4, executable: false,
    corpus: { sha256: corpusSha256, bytes: corpus.length, canonicalRecordsSha256: "", snapshotFile: `oracle-source-${corpusSha256}.json` },
    paperEvidence: evidence.fingerprint,
    parser,
    rules: { edition: "2026-09-25", url: "https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt" },
    scope: SCOPE, inputRecords: cards.length, excluded: {},
    cardsChecked: 0, cardsOk: 0, linesChecked: 0, linesOk: 0,
    bindingSummary: emptyBindingSummary(),
    resolutionSummary: emptyResolutionSummary(),
  };
  function* records(capturePrinted: boolean): Generator<{ record: ArchiveRecord; printed?: PrintedCard }> {
    const seen = new Set<string>();
    for (const [sourceIndex, source] of cards.entries()) {
      const canonicalSource = JSON.stringify(source);
      sourceRecordsHash.update(canonicalSource).update("\n");
      const oracleIdentity = identity(source);
      const base: RecordSource = {
        recordType: "card", sourceIndex, oracleIdentity, sourceSha256: sha256(canonicalSource), source,
      };
      const reason = paperExclusion(source, evidence) ?? (seen.has(oracleIdentity) ? "duplicate" : undefined);
      if (reason) {
        manifest.excluded[reason] = (manifest.excluded[reason] ?? 0) + 1;
        yield { record: { ...base, status: "excluded", reason } };
        continue;
      }
      seen.add(oracleIdentity);
      manifest.cardsChecked++;
      const printedFaces: PrintedFace[] = [];
      const faces = (source.card_faces ?? [source]).map((face, sourceFaceIndex): ArchivedFace => {
        if (face.oracle_text == null) {
          if (capturePrinted) printedFaces.push({ sourceFaceIndex, status: "missing-oracle-text",
            field: Object.hasOwn(face, "oracle_text") ? "null" : "absent" });
          return { sourceFaceIndex, status: "missing-oracle-text", name: face.name };
        }
        const context = { layout: source.layout, typeLine: face.type_line ?? source.type_line ?? undefined, cardNames };
        const captured = capturePrinted ? parseOracleTextWithPrintedEvidence(face.oracle_text, face.name, context, sourceFaceIndex) : undefined;
        if (captured) printedFaces.push(captured.printed);
        const result = captured?.result ?? parseOracleText(face.oracle_text, face.name, { ...context, includeSourceProvenance: true });
        for (const line of result.lines) {
          manifest.linesChecked++;
          if (line.ok) manifest.linesOk++;
        }
        requireFiniteJson(result);
        return { sourceFaceIndex, status: "attempted", result };
      });
      const input = { identity: oracleIdentity, source, faces: faces.map(face => ({
        sourceFaceIndex: face.sourceFaceIndex, ...(face.status === "attempted" ? { result: face.result } : {}),
      })) };
      const binding = analyzeCardBindings(input);
      addBindingSummary(manifest.bindingSummary, binding);
      const resolution = resolveCard(input, binding);
      addResolutionSummary(manifest.resolutionSummary, resolution);
      const analyzed: Analyzed = { binding, bindingSha256: sha256(JSON.stringify(binding)), resolution, resolutionSha256: sha256(JSON.stringify(resolution)) };
      const parsed = faces.every(face => face.status === "attempted" && face.result.ok);
      if (parsed) manifest.cardsOk++;
      const record: ArchiveRecord = parsed ? { ...base, status: "parsed", faces, astSha256: successfulAstHash(faces), ...analyzed }
        : { ...base, status: "unsupported", faces, ...analyzed };
      const printed: PrintedCard | undefined = capturePrinted
        ? { artifactKind: "oracle-printed-evidence", schemaVersion: 1, faces: printedFaces } : undefined;
      if (printed) requireFiniteJson(printed);
      yield { record, printed };
    }
    manifest.corpus.canonicalRecordsSha256 = sourceRecordsHash.digest("hex");
    requireSnapshot(parser, fingerprint);
  }
  return { manifest, records, cardNames, sourceBytes: corpus, paperEvidenceBytes: evidence.bytes };
}

/** Build against one local corpus snapshot. Missing texts and unsupported
 * blocks remain failures. Excluded and duplicate source rows are retained. */
export function buildArchive(options: ArchiveOptions = {}): ParserArchive {
  const { records, manifest, sourceBytes, paperEvidenceBytes } = prepareArchive(options);
  return { records: Array.from(records(false), ({ record }) => record), manifest, sourceBytes, paperEvidenceBytes };
}

async function verifySourceSnapshot(snapshotFile: string, expected: { sha256: string; bytes: number }): Promise<void> {
  const hash = createHash("sha256");
  let bytes = 0;
  for await (const chunk of fs.createReadStream(snapshotFile)) {
    hash.update(chunk);
    bytes += chunk.length;
  }
  if (hash.digest("hex") !== expected.sha256 || bytes !== expected.bytes) {
    throw new Error("Raw source snapshot hash mismatch");
  }
}

export async function publishSourceSnapshot(sourceBytes: Buffer, expected: { sha256: string; bytes: number; snapshotFile: string }, directory: string): Promise<void> {
  if (sourceBytes.length !== expected.bytes || sha256(sourceBytes) !== expected.sha256) {
    throw new Error("Raw source bytes do not match the archive corpus hash");
  }
  const destination = path.join(directory, expected.snapshotFile);
  const temporary = `${destination}.${randomUUID()}.tmp`;
  try {
    const handle = await open(temporary, "wx");
    try {
      await handle.writeFile(sourceBytes);
      await handle.sync();
    } finally { await handle.close(); }
    // Hard-link publication is atomic and exclusive. An existing content-
    // addressed snapshot is verified, never overwritten (including races).
    try { await link(temporary, destination); }
    catch (error) {
      if (!(error instanceof Error) || !("code" in error) || error.code !== "EEXIST") throw error;
    }
    await verifySourceSnapshot(destination, expected);
  } finally { await rm(temporary, { force: true }); }
}

/** Call after canonical serialization is established, so these preimages
 * are exactly the ones written in the legacy row. */
export function verifyLegacyHashes(record: ArchiveRecord): string {
  const canonicalSource = JSON.stringify(record.source);
  if (sha256(canonicalSource) !== record.sourceSha256) throw new Error("source hash mismatch (SHA-256 preimage)");
  if (record.status === "parsed" && successfulAstHash(record.faces) !== record.astSha256)
    throw new Error("AST hash mismatch (SHA-256 preimage)");
  if (record.status !== "excluded" && sha256(JSON.stringify(record.binding)) !== record.bindingSha256)
    throw new Error("binding hash mismatch (SHA-256 preimage)");
  if (record.status !== "excluded" && sha256(JSON.stringify(record.resolution)) !== record.resolutionSha256)
    throw new Error("resolution hash mismatch (SHA-256 preimage)");
  return canonicalSource;
}

/** Verify the persisted file by parsing each record back, comparing its exact
 * serialized representation, and independently recomputing source/AST hashes.
 * Streaming avoids retaining a second copy of the entire bulk corpus. */
export async function verifyArchiveFile(file: string, archive: ParserArchive): Promise<void> {
  await verifySourceSnapshot(path.join(path.dirname(file), archive.manifest.corpus.snapshotFile), archive.manifest.corpus);
  const evidenceFile = path.join(path.dirname(file), archive.manifest.paperEvidence.snapshotFile);
  await verifySourceSnapshot(evidenceFile, archive.manifest.paperEvidence);
  const evidence = readPaperEvidence(fs.readFileSync(evidenceFile));
  const input = fs.createReadStream(file, { encoding: "utf8" });
  const lines = createInterface({ input, crlfDelay: Infinity });
  const sourceRecordsHash = createHash("sha256");
  let lineIndex = 0;
  const seen = new Set<string>();
  try {
    for await (const line of lines) {
      const expected = lineIndex === 0 ? archive.manifest : archive.records[lineIndex - 1];
      if (!expected || line !== JSON.stringify(expected)) throw new Error(`Archive round-trip mismatch at line ${lineIndex + 1}`);
      const decoded: unknown = JSON.parse(line);
      requireFiniteJson(decoded);
      if (JSON.stringify(decoded) !== line) throw new Error(`Archive serialization mismatch at line ${lineIndex + 1}`);
      if (lineIndex > 0) {
        // Equality with the expected typed record above establishes its shape.
        const record = decoded as ArchiveRecord;
        const reason = paperExclusion(record.source, evidence) ?? (seen.has(record.oracleIdentity) ? "duplicate" : undefined);
        if (reason ? record.status !== "excluded" || record.reason !== reason : record.status === "excluded")
          throw new Error("Archived paper scope mismatch");
        if (!reason) seen.add(record.oracleIdentity);
        sourceRecordsHash.update(verifyLegacyHashes(record)).update("\n");
      }
      lineIndex++;
    }
    if (lineIndex !== archive.records.length + 1) throw new Error("Truncated archive");
    if (sourceRecordsHash.digest("hex") !== archive.manifest.corpus.canonicalRecordsSha256) {
      throw new Error("Archived corpus records hash mismatch");
    }
  } finally {
    lines.close();
    input.destroy();
  }
}

/** Publish only a complete verified file. Unique same-directory temporary
 * files and atomic rename leave a previous archive intact on every failure. */
export async function publishArchive(archive: ParserArchive, outputFile = DEFAULT_OUTPUT, fingerprint: Fingerprinter = sourceIdentity): Promise<void> {
  for (const snapshot of [archive.manifest.corpus.snapshotFile, archive.manifest.paperEvidence.snapshotFile]) {
    if (path.resolve(outputFile) === path.resolve(path.dirname(outputFile), snapshot))
      throw new Error("Archive output must not overwrite a source or evidence snapshot");
  }
  requireSnapshot(archive.manifest.parser, fingerprint);
  await mkdir(path.dirname(outputFile), { recursive: true });
  await publishSourceSnapshot(archive.sourceBytes, archive.manifest.corpus, path.dirname(outputFile));
  await publishSourceSnapshot(archive.paperEvidenceBytes, archive.manifest.paperEvidence, path.dirname(outputFile));
  const temporary = `${outputFile}.${randomUUID()}.tmp`;
  try {
    const handle = await open(temporary, "wx");
    try {
      await handle.writeFile(`${JSON.stringify(archive.manifest)}\n`);
      for (const record of archive.records) await handle.writeFile(`${JSON.stringify(record)}\n`);
      await handle.sync();
    } finally {
      await handle.close();
    }
    await verifyArchiveFile(temporary, archive);
    requireSnapshot(archive.manifest.parser, fingerprint);
    await rename(temporary, outputFile);
  } finally {
    await rm(temporary, { force: true });
  }
}

async function main(): Promise<void> {
  const args = process.argv.slice(2);
  const options = new Map<string, string>();
  const flags = ["--input", "--output", "--paper-evidence", "--legacy-baseline", "--rules", "--verify-candidate"];
  for (let i = 0; i < args.length; i += 2) {
    const [flag, value] = [args[i], args[i + 1]];
    if (!flags.includes(flag) || !value || value.startsWith("--") || options.has(flag))
      throw new Error(`Usage: npm run archive-parser -- [${flags.join(" PATH] [")} PATH]`);
    options.set(flag, path.resolve(value));
  }
  const baseline = options.get("--legacy-baseline");
  const verify = options.get("--verify-candidate");
  const rules = options.get("--rules");
  if (!baseline && (verify || rules)) throw new Error("--verify-candidate and --rules require --legacy-baseline");
  if (verify && options.size !== 2)
    throw new Error("--verify-candidate takes only --legacy-baseline in addition to its candidate path");
  const cacheFile = options.get("--input") ?? DEFAULT_INPUT;
  const outputFile = options.get("--output") ?? (baseline ? CANDIDATE_OUTPUT : DEFAULT_OUTPUT);
  const evidenceFile = options.get("--paper-evidence");
  if (baseline) {
    const { checkR6Candidate, writeR6Candidate } = await import("./archive-r6.ts");
    const report = verify ? await checkR6Candidate(baseline, verify) : await writeR6Candidate({
      baseline, output: outputFile, rulesFile: rules ?? DEFAULT_RULES,
      archive: { cacheFile, ...(evidenceFile ? { paperEvidence: loadPaperEvidence(evidenceFile) } : {}) },
    });
    console.log(JSON.stringify(report, null, 2));
    return;
  }
  if (path.resolve(cacheFile) === path.resolve(outputFile)) throw new Error("Archive output must not overwrite its source corpus");
  const archive = buildArchive({ cacheFile, ...(evidenceFile ? { paperEvidence: loadPaperEvidence(evidenceFile) } : {}) });
  await publishArchive(archive, outputFile);
  console.log(`Archived ${archive.records.length} complete source records to ${outputFile}`);
  console.log(`Accepted parsing: ${archive.manifest.cardsOk}/${archive.manifest.cardsChecked}; executable: false`);
  console.log(`Corpus SHA-256: ${archive.manifest.corpus.sha256}`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  main().catch(error => { console.error(error instanceof Error ? error.message : error); process.exitCode = 1; });
}
