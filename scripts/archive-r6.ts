import { createHash, randomUUID } from "node:crypto";
import { createReadStream, readFileSync } from "node:fs";
import { link, mkdir, open, rm, stat } from "node:fs/promises";
import { basename, dirname, resolve } from "node:path";
import type { PrintedCard, PrintedNode } from "../packages/oracle-parser/src/printed/schema.ts";
import {
  prepareArchive, publishSourceSnapshot, requireSnapshot, verifyLegacyHashes,
  type ArchiveManifest, type ArchiveRecord,
} from "./archive-parser.ts";
import { printedIdentity } from "./generate-rust-ast.ts";
import { sha256, sourceDigest } from "./validate-parser.ts";

// Frozen input identity and limits come from the R5 compile manifest and Rust
// bulk contracts. D0 proves the transition only; Rust acceptance belongs to B1.
const frozen = JSON.parse(readFileSync(new URL(
  "../packages/game-engine/tests/fixtures/compile-inputs-phase1-r5.json", import.meta.url,
), "utf8")) as {
  oracle: { sha256: string; bytes: number };
  paperEvidence: { sha256: string; bytes: number };
  rules: { edition: string; sha256: string };
  archive: { sha256: string; headerSha256: string; bytes: number };
};
const ARCHIVE_LIMITS = { bytes: 1024 ** 3, lineBytes: 1024 ** 2, rows: 60_000 };
const BULK_LIMITS = { catalogBytes: 128 * 1024 ** 2, indexBytes: 32 * 1024 ** 2, manifestBytes: 64 * 1024, reasonBytes: 256 };
const digestPattern = /^[0-9a-f]{64}$/;
const utf8 = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });
const fingerprint = () => ({ revision: null, sourceSha256: sourceDigest(["scripts/archive-r6.ts"]) });
const PRINTED_FIELD = ',"printed":';
const PRINTED_SUFFIX = /^,"printed":(\{.*\}),"printedSha256":"([0-9a-f]{64})"\}\n$/s;

interface PrintedFieldOrder {
  keys: readonly string[];
  children?: Readonly<Record<string, PrintedFieldOrder>>;
}

// JSON.stringify preserves the order used by the capture constructors. Each
// list covers the keys of the writer's variants, including optional fields.
const TEXT_DIGEST_ORDER = { keys: ["bytes", "sha256"] };
const PRINTED_FIELD_ORDER: PrintedFieldOrder = {
  keys: ["artifactKind", "schemaVersion", "faces"],
  children: {
    faces: {
      keys: ["sourceFaceIndex", "status", "field", "reason", "original", "normalized", "edits", "lines", "blocks", "removedReminders"],
      children: {
        original: TEXT_DIGEST_ORDER,
        normalized: TEXT_DIGEST_ORDER,
        edits: { keys: ["edit", "reminder", "form", "role", "as", "slot", "via", "o", "n"] },
        lines: {
          keys: ["sourceId", "n", "status", "tokens", "error", "at"],
          children: { tokens: { keys: ["t", "possessive", "grammarIndex", "n", "o"] } },
        },
        blocks: {
          keys: ["status", "lines", "n", "facts", "nodes", "error", "at", "location"],
          children: {
            facts: { keys: ["fact", "form", "possessive", "value", "role", "moved", "n", "reminderIndex", "o",
              "at", "tok", "targetIndex", "scope", "selectionId", "field"] },
            nodes: { keys: ["path", "basis", "span", "folds", "clauses", "of", "reason"] },
          },
        },
        removedReminders: { keys: ["fact", "editIndex", "o"] },
      },
    },
  },
};

function verifyPrintedFieldOrder(value: unknown, order?: PrintedFieldOrder): void {
  if (value === null || typeof value !== "object") return;
  if (Array.isArray(value)) {
    for (const entry of value) verifyPrintedFieldOrder(entry, order);
    return;
  }
  if (!order) throw new Error("unexpected object in printed value");
  let previous = -1;
  for (const [key, child] of Object.entries(value)) {
    const index = order.keys.indexOf(key);
    if (index === -1) throw new Error(`unexpected printed field "${key}"`);
    if (index <= previous) throw new Error(`printed field order differs from the writer at "${key}"`);
    previous = index;
    verifyPrintedFieldOrder(child, order.children?.[key]);
  }
}

function emptyPrintedSummary() {
  return {
    records: 0,
    faces: { read: 0, "missing-oracle-text": 0, unmappable: 0 },
    blocks: { read: 0, failed: 0 },
    failedLexLines: 0,
    bases: { recorded: 0, hull: 0, alias: 0, implicit: 0, unlocated: 0 } satisfies Record<PrintedNode["basis"], number>,
  };
}

function countPrinted(summary: ReturnType<typeof emptyPrintedSummary>, printed: PrintedCard): void {
  summary.records++;
  for (const face of printed.faces) {
    summary.faces[face.status]++;
    if (face.status !== "read") continue;
    summary.failedLexLines += face.lines.filter(line => line.status === "failed").length;
    for (const block of face.blocks) {
      summary.blocks[block.status]++;
      if (block.status === "read") for (const node of block.nodes) summary.bases[node.basis]++;
    }
  }
}

interface LegacyPin { sha256: string; headerSha256: string; bytes: number }
interface R6Manifest extends Omit<ArchiveManifest, "schemaVersion" | "parser" | "rules"> {
  schemaVersion: 5;
  cut: "r6";
  parser: { sourceSha256: string };
  rules: ArchiveManifest["rules"] & { sha256: string };
  printed: ReturnType<typeof printedIdentity> & { recordsSha256: string };
  nameCatalog: { sha256: string; names: number };
  printedSummary: ReturnType<typeof emptyPrintedSummary>;
  legacy: { manifestSha256: string; fileSha256: string };
}

function orderedDigestEntry(row: ArchiveRecord, printedSha256: string): string {
  if (typeof row.oracleIdentity !== "string" || !row.oracleIdentity || /[\t\r\n]/.test(row.oracleIdentity))
    throw new Error("invalid Oracle identity for the ordered digest");
  return `${row.sourceIndex}\t${row.oracleIdentity}\t${printedSha256}\n`;
}

export function serializeR6Row(record: ArchiveRecord, printed?: PrintedCard) {
  const legacy = JSON.stringify(record);
  if (record.status === "excluded" && !printed) return { line: Buffer.from(legacy + "\n"), printedSha256: "excluded" };
  if (record.status === "excluded" || !printed) throw new Error("R6 printed evidence must occur on every in-scope row only");
  const printedBytes = Buffer.from(JSON.stringify(printed));
  const printedSha256 = sha256(printedBytes);
  const trailer = `${PRINTED_FIELD}${printedBytes.toString("utf8")},"printedSha256":"${printedSha256}"}\n`;
  return { line: Buffer.from(legacy.slice(0, -1) + trailer), printedSha256 };
}

function limitLine(line: Buffer, lineNumber: number): void {
  if (line.length > ARCHIVE_LIMITS.lineBytes)
    throw new Error(`line ${lineNumber}: archive exceeds the ${ARCHIVE_LIMITS.lineBytes}-byte line limit`);
}

async function* archiveLines(file: string): AsyncGenerator<Buffer> {
  let tail = Buffer.alloc(0);
  let bytes = 0;
  let count = 0;
  for await (const chunk of createReadStream(file)) {
    bytes += chunk.length;
    const data = tail.length ? Buffer.concat([tail, chunk]) : chunk;
    let start = 0;
    for (let lf = data.indexOf(10); lf !== -1; lf = data.indexOf(10, start)) {
      const line = data.subarray(start, lf + 1);
      count++;
      limitLine(line, count);
      if (count > ARCHIVE_LIMITS.rows + 1) throw new Error(`line ${count}: archive row limit exceeded`);
      yield line;
      start = lf + 1;
    }
    tail = data.subarray(start);
    limitLine(tail, count + 1);
    if (bytes > ARCHIVE_LIMITS.bytes) throw new Error(`line ${count + 1}: archive byte limit exceeded`);
  }
  if (tail.length) throw new Error(`line ${count + 1}: missing LF terminator`);
}

function canonicalObject<T>(line: Buffer): T {
  const text = utf8.decode(line.subarray(0, -1));
  const decoded = JSON.parse(text);
  if (!decoded || typeof decoded !== "object" || Array.isArray(decoded) || JSON.stringify(decoded) !== text)
    throw new Error("expected a canonically serialized JSON object");
  return decoded as T;
}

type R6HeaderFields = Pick<R6Manifest, "parser" | "printed" | "nameCatalog" | "printedSummary" | "legacy"> &
  { rules: Pick<R6Manifest["rules"], "sha256"> };

function r6Manifest(old: ArchiveManifest, fields: R6HeaderFields): R6Manifest {
  const { schemaSha256, vocabularySha256, generatorSha256, recordsSha256 } = fields.printed;
  const { sha256, names } = fields.nameCatalog;
  const { manifestSha256, fileSha256 } = fields.legacy;
  return {
    ...old, schemaVersion: 5, cut: "r6",
    parser: { sourceSha256: fields.parser.sourceSha256 },
    rules: { ...old.rules, sha256: fields.rules.sha256 },
    printed: { schemaVersion: 1, schemaSha256, vocabularySha256, generatorSha256, recordsSha256 },
    nameCatalog: { sha256, names },
    printedSummary: fields.printedSummary,
    legacy: { manifestSha256, fileSha256 },
  };
}

function serializeR6Manifest(manifest: R6Manifest): Buffer {
  return Buffer.from(JSON.stringify(manifest) + "\n");
}

function matchesSummaryFields(actual: unknown, template: object): boolean {
  if (!actual || typeof actual !== "object" || Array.isArray(actual)) return false;
  const entries = Object.entries(actual);
  const fields = Object.entries(template);
  return entries.length === fields.length && fields.every(([key, child], index) => {
    const [actualKey, actualChild] = entries[index];
    return key === actualKey && (typeof child === "object" ? matchesSummaryFields(actualChild, child)
      : Number.isSafeInteger(actualChild) && actualChild >= 0);
  });
}

function headerPair(baseline: Buffer, candidate: Buffer, pin: LegacyPin): R6Manifest {
  if (sha256(baseline.subarray(0, -1)) !== pin.headerSha256) throw new Error("R5 header hash differs from the pin");
  const old = canonicalObject<ArchiveManifest>(baseline);
  const header = canonicalObject<R6Manifest>(candidate);
  const { parser, rules, printed, nameCatalog, printedSummary, legacy } = header;
  if (old.schemaVersion !== 4 || old.rules.edition !== frozen.rules.edition || rules?.sha256 !== frozen.rules.sha256 ||
      typeof parser?.sourceSha256 !== "string" || !digestPattern.test(parser.sourceSha256))
    throw new Error("R6 header differs from the frozen legacy metadata or has an invalid cut identity");
  if (legacy?.manifestSha256 !== pin.headerSha256 || legacy?.fileSha256 !== pin.sha256 ||
      ![printed?.schemaSha256, printed?.vocabularySha256, printed?.generatorSha256, printed?.recordsSha256, nameCatalog?.sha256]
        .every(value => typeof value === "string" && digestPattern.test(value)) ||
      !Number.isSafeInteger(nameCatalog.names) || nameCatalog.names < 1)
    throw new Error("R6 header has invalid archive or evidence pins");
  if (!matchesSummaryFields(printedSummary, emptyPrintedSummary()) ||
      !candidate.equals(serializeR6Manifest(r6Manifest(old, header))))
    throw new Error("R6 header fields or field order differ from the schema-5 writer");
  return header;
}

function verifyRow(baseline: Buffer, line: Buffer, sourceIndex: number) {
  const record = canonicalObject<ArchiveRecord>(baseline);
  const rowStart = `{"recordType":"card","sourceIndex":${sourceIndex},`;
  if (record.recordType !== "card" || record.sourceIndex !== sourceIndex ||
      !line.subarray(0, rowStart.length).equals(Buffer.from(rowStart))) throw new Error("source order differs from R5");
  if (record.status === "excluded") {
    if (!line.equals(baseline)) throw new Error("excluded row differs from R5");
    return { record, printedSha256: "excluded" };
  }
  if (record.status !== "parsed" && record.status !== "unsupported") throw new Error("unknown R5 row status");
  const prefix = baseline.subarray(0, -2); // Drop only the final object brace and LF.
  if (!line.subarray(0, prefix.length).equals(prefix)) throw new Error("legacy row prefix differs from R5");
  const appended = line.subarray(prefix.length);
  const suffix = PRINTED_SUFFIX.exec(utf8.decode(appended));
  if (!suffix) throw new Error("expected exactly printed and printedSha256 appended after resolutionSha256");
  let printed: PrintedCard;
  try { printed = JSON.parse(suffix[1]) as PrintedCard; }
  catch { throw new Error("expected exactly printed and printedSha256 appended after resolutionSha256"); }
  const printedSha256 = suffix[2];
  const rawPrinted = appended.subarray(PRINTED_FIELD.length, PRINTED_FIELD.length + Buffer.byteLength(suffix[1]));
  if (sha256(rawPrinted) !== printedSha256) throw new Error("printed hash mismatch");
  if (!rawPrinted.equals(Buffer.from(JSON.stringify(printed))))
    throw new Error("printed value is not compact canonical JSON");
  if (printed.artifactKind !== "oracle-printed-evidence" || printed.schemaVersion !== 1 || !Array.isArray(printed.faces))
    throw new Error("invalid printed evidence envelope");
  verifyPrintedFieldOrder(printed, PRINTED_FIELD_ORDER);
  return { record, printed, printedSha256 };
}

/** D0 reads exact LF-terminated bytes from both files. Canonical R5 serialization
 * plus prefix equality establishes the legacy preimages before checking their
 * hashes, including the array of face abilities used by the legacy AST hash. */
export async function verifyR6Archive(baselineFile: string, candidateFile: string, pin = frozen.archive) {
  const start = performance.now();
  const baseline = archiveLines(baselineFile);
  const baselineHash = createHash("sha256");
  const fileHash = createHash("sha256");
  const sourceHash = createHash("sha256");
  const printedHash = createHash("sha256");
  const printedSummary = emptyPrintedSummary();
  let manifest: R6Manifest | undefined;
  let manifestSha256 = "";
  let baselineBytes = 0;
  let lineNumber = 0;
  let bytes = 0;
  let largestLine = { bytes: 0, line: 0, sourceIndex: null as number | null, name: null as string | null };
  try {
    for await (const line of archiveLines(candidateFile)) {
      lineNumber++;
      const { value: oldLine } = await baseline.next();
      if (!oldLine) throw new Error(`D0 line ${lineNumber}: candidate has a row beyond R5`);
      try {
        fileHash.update(line);
        bytes += line.length;
        baselineHash.update(oldLine);
        baselineBytes += oldLine.length;
        if (lineNumber === 1) {
          manifest = headerPair(oldLine, line, pin);
          manifestSha256 = sha256(line.subarray(0, -1));
          largestLine = { ...largestLine, bytes: line.length, line: 1 };
          continue;
        }
        const { record, printed, printedSha256 } = verifyRow(oldLine, line, lineNumber - 2);
        sourceHash.update(verifyLegacyHashes(record)).update("\n");
        printedHash.update(orderedDigestEntry(record, printedSha256));
        if (printed) countPrinted(printedSummary, printed);
        if (line.length > largestLine.bytes) {
          largestLine = { bytes: line.length, line: lineNumber, sourceIndex: record.sourceIndex, name: record.source.name };
        }
      } catch (error) {
        throw new Error(`D0 line ${lineNumber}: ${error instanceof Error ? error.message : error}`, { cause: error });
      }
    }
    if (!manifest) throw new Error("D0 line 1: candidate header is missing");
    if (!(await baseline.next()).done || lineNumber !== manifest.inputRecords + 1)
      throw new Error(`D0 line ${lineNumber + 1}: candidate ended before the last pinned R5 row`);
    if (baselineHash.digest("hex") !== pin.sha256 || baselineBytes !== pin.bytes)
      throw new Error("D0 line 1: R5 file bytes differ from the pin");
    if (sourceHash.digest("hex") !== manifest.corpus.canonicalRecordsSha256)
      throw new Error("D0 line 1: canonical source records digest mismatch");
    if (printedHash.digest("hex") !== manifest.printed.recordsSha256 ||
        JSON.stringify(printedSummary) !== JSON.stringify(manifest.printedSummary))
      throw new Error("D0 line 1: ordered printed digest or capture summary mismatch");
    return {
      manifest, manifestSha256, fileSha256: fileHash.digest("hex"), bytes, largestLine,
      rows: lineNumber - 1, lines: lineNumber, elapsedMs: Math.round(performance.now() - start),
    };
  } finally {
    await baseline.return(undefined);
  }
}

function reportCandidate(result: Awaited<ReturnType<typeof verifyR6Archive>>) {
  const { manifest, manifestSha256, fileSha256, rows, lines, bytes, largestLine, elapsedMs } = result;
  const manifestFile = `archive-manifest-r6-candidate-${manifestSha256}.json`;
  return {
    status: "candidate",
    registryEntry: {
      cut: manifest.cut,
      manifest_file: manifestFile,
      manifest_sha256: manifestSha256,
      file_sha256: fileSha256,
      parser_revision: "",
      parser_source_sha256: manifest.parser.sourceSha256,
      corpus_sha256: manifest.corpus.sha256,
      r6: {
        file_bytes: bytes, max_line_bytes: largestLine.bytes, input_records: rows,
        canonical_records_sha256: manifest.corpus.canonicalRecordsSha256,
        printed_schema_sha256: manifest.printed.schemaSha256,
        printed_vocabulary_sha256: manifest.printed.vocabularySha256,
        printed_generator_sha256: manifest.printed.generatorSha256,
        printed_records_sha256: manifest.printed.recordsSha256,
        name_catalog_sha256: manifest.nameCatalog.sha256,
        name_catalog_names: manifest.nameCatalog.names,
      },
      record_sha256: [],
    },
    rulesSha256: manifest.rules.sha256,
    legacy: manifest.legacy,
    sizes: { bytes, rows, lines, largestLine, archiveLimits: ARCHIVE_LIMITS, bulkLimits: BULK_LIMITS },
    printedSummary: manifest.printedSummary,
    d0: { status: "passed", elapsedMs },
  };
}

export async function checkR6Candidate(baseline: string, candidate: string) {
  return reportCandidate(await verifyR6Archive(baseline, candidate));
}

export async function writeR6Candidate(options: {
  baseline: string; output: string; rulesFile: string;
  archive: Parameters<typeof prepareArchive>[0];
}) {
  const { baseline, output, rulesFile } = options;
  if (resolve(output) === resolve(baseline) || basename(output) === "oracle-ast-phase1-r5.jsonl")
    throw new Error("R6 output must be separate from the frozen R5 archive");
  const baselineStat = await stat(baseline);
  if (!baselineStat.isFile() || baselineStat.size !== frozen.archive.bytes)
    throw new Error("D0 line 1: R5 must be a regular file with the pinned length");
  const rulesSha256 = sha256(readFileSync(rulesFile));
  if (rulesSha256 !== frozen.rules.sha256) throw new Error("Rules file differs from the frozen edition pin");
  const archive = prepareArchive({ ...options.archive, fingerprint });
  const { manifest: legacy, cardNames } = archive;
  if (legacy.inputRecords > ARCHIVE_LIMITS.rows) throw new Error("line 1: archive row limit exceeded");
  if (legacy.corpus.sha256 !== frozen.oracle.sha256 || legacy.corpus.bytes !== frozen.oracle.bytes ||
      legacy.paperEvidence.sha256 !== frozen.paperEvidence.sha256 || legacy.paperEvidence.bytes !== frozen.paperEvidence.bytes)
    throw new Error("Oracle or paper evidence bytes differ from the frozen R5 inputs");
  const printedContract = printedIdentity();
  const names = createHash("sha256");
  for (const name of Array.from(cardNames.spellings(), name => Buffer.from(name)).sort(Buffer.compare))
    names.update(name).update("\n");
  const nameCatalog = { sha256: names.digest("hex"), names: cardNames.size };
  const directory = dirname(output);
  await mkdir(directory, { recursive: true });
  const temporary = `${output}.${randomUUID()}.tmp`;
  const bodyFile = `${temporary}.rows`;
  try {
    const body = await open(bodyFile, "wx");
    const printedHash = createHash("sha256");
    const printedSummary = emptyPrintedSummary();
    let bytes = 0;
    try {
      for (const { record, printed } of archive.records(true)) {
        const { line, printedSha256 } = serializeR6Row(record, printed);
        printedHash.update(orderedDigestEntry(record, printedSha256));
        if (printed) countPrinted(printedSummary, printed);
        limitLine(line, record.sourceIndex + 2);
        bytes += line.length;
        if (bytes > ARCHIVE_LIMITS.bytes) throw new Error(`line ${record.sourceIndex + 2}: archive byte limit exceeded`);
        await body.writeFile(line);
      }
    } finally {
      await body.close();
    }
    requireSnapshot({ revision: null, sourceSha256: legacy.parser.sourceSha256 }, fingerprint);
    const header = r6Manifest(legacy, {
      parser: { sourceSha256: legacy.parser.sourceSha256 },
      rules: { sha256: rulesSha256 },
      printed: { ...printedContract, recordsSha256: printedHash.digest("hex") },
      nameCatalog, printedSummary,
      legacy: { manifestSha256: frozen.archive.headerSha256, fileSha256: frozen.archive.sha256 },
    });
    const headerLine = serializeR6Manifest(header);
    limitLine(headerLine, 1);
    if (bytes + headerLine.length > ARCHIVE_LIMITS.bytes) throw new Error("line 1: archive byte limit exceeded");
    const candidate = await open(temporary, "wx");
    try {
      await candidate.writeFile(headerLine);
      await candidate.writeFile(createReadStream(bodyFile));
      await candidate.sync();
    } finally {
      await candidate.close();
    }
    const result = await verifyR6Archive(baseline, temporary);
    requireSnapshot({ revision: null, sourceSha256: legacy.parser.sourceSha256 }, fingerprint);
    if (JSON.stringify(printedIdentity()) !== JSON.stringify(printedContract))
      throw new Error("Printed decoder identity changed during archive generation");
    const report = reportCandidate(result);
    await publishSourceSnapshot(archive.sourceBytes, header.corpus, directory);
    await publishSourceSnapshot(archive.paperEvidenceBytes, header.paperEvidence, directory);
    await publishSourceSnapshot(headerLine, {
      sha256: sha256(headerLine), bytes: headerLine.length, snapshotFile: report.registryEntry.manifest_file,
    }, directory);
    // Publish this candidate exclusively. The immutable, content-addressed
    // header and snapshots are already visible when the archive appears.
    await link(temporary, output);
    return report;
  } finally {
    await rm(bodyFile, { force: true });
    await rm(temporary, { force: true });
  }
}
