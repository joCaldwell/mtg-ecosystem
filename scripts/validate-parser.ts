// Acceptance scoreboard, not semantic correctness certification.
import fs from "node:fs";
import path from "node:path";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { fileURLToPath, pathToFileURL } from "node:url";
import { parseOracleText } from "../packages/oracle-parser/src/index.ts";
import { buildCardNameCatalog } from "../packages/oracle-parser/src/card-names.ts";
import { analyzeCardBindings } from "../packages/oracle-parser/src/semantic/bindings.ts";
import type { CardBindingAnalysis } from "../packages/oracle-parser/src/semantic/types.ts";
import type { ParseCardResult } from "../packages/oracle-parser/src/ast.ts";
import { readCards, type ScryfallCard } from "./lib/scryfall.ts";
import { PAPER_SCOPE, SKIP_LAYOUTS, oracleIdentity, paperExclusion, loadPaperEvidence, type PaperEvidence } from "./lib/paper-evidence.ts";

const ROOT = fileURLToPath(new URL("../", import.meta.url));
const CACHE_FILE = path.join(ROOT, ".scryfall-cache/oracle-cards.json");
export const SCOPE = PAPER_SCOPE;
export const LEGACY_SCOPE = "paper-non-silver-non-acorn-v1";
export const sha256 = (text: string | Buffer): string => createHash("sha256").update(text).digest("hex");

export function emptyBindingSummary() {
  return { cardsAnalyzed: 0, cardsFullyBound: 0, cardsReferencesBound: 0, obligations: 0, bound: 0, unresolved: 0,
    diagnostics: {} as Record<string, number>, engineFeatures: {} as Record<string, number> };
}
export function addBindingSummary(summary: ReturnType<typeof emptyBindingSummary>, analysis: CardBindingAnalysis): void {
  summary.cardsAnalyzed++;
  if (analysis.fullyBound) summary.cardsFullyBound++;
  if (analysis.referencesBound) summary.cardsReferencesBound++;
  // Cards whose references are bound, by the engine feature they still need.
  if (analysis.referencesBound) for (const feature of new Set(analysis.engineFeatures.map(need => need.feature)))
    summary.engineFeatures[feature] = (summary.engineFeatures[feature] ?? 0) + 1;
  summary.obligations += analysis.counts.obligations;
  summary.bound += analysis.counts.bound;
  summary.unresolved += analysis.counts.unresolved;
  for (const diagnostic of analysis.diagnostics) summary.diagnostics[diagnostic.code] = (summary.diagnostics[diagnostic.code] ?? 0) + 1;
}

/** Historical checkpoint-v1 filter ONLY. It mistakes an Oracle representative's
 * digital printing for lack of paper printings. Retained for delta audits;
 * current reports and archives always use paperExclusion with frozen evidence. */
export function exclusion(card: ScryfallCard): string | undefined {
  if (SKIP_LAYOUTS.has(card.layout)) return `layout:${card.layout}`;
  if (card.digital) return "digital";
  if (card.border_color === "silver" || card.security_stamp === "acorn") return "silver-or-acorn";
  return undefined;
}
export function identity(card: ScryfallCard): string {
  return oracleIdentity(card);
}

export function evaluateCorpus(cards: ScryfallCard[], options: { parser?: typeof parseOracleText; paperEvidence?: PaperEvidence } = {}) {
  const parser = options.parser ?? parseOracleText;
  const evidence = options.paperEvidence ?? loadPaperEvidence();
  const cardNames = buildCardNameCatalog(cards);
  const excluded: Record<string, number> = {};
  const unique = new Map<string, ScryfallCard>();
  for (const card of cards) {
    const reason = paperExclusion(card, evidence);
    if (reason) { excluded[reason] = (excluded[reason] ?? 0) + 1; continue; }
    const key = identity(card);
    if (unique.has(key)) { excluded.duplicate = (excluded.duplicate ?? 0) + 1; continue; }
    unique.set(key, card);
  }
  let cardsOk = 0, linesChecked = 0, linesOk = 0;
  // The current coverage target counts only cards legal in Commander.
  const commanderLegal = { cardsChecked: 0, cardsOk: 0 };
  const bindingSummary = emptyBindingSummary();
  const failures = new Map<string, { count: number; examples: { name: string; text: string; error: string }[] }>();
  const outcomes: Record<string, { name: string; ok: boolean; astSha256?: string; referencesFullyBound: boolean; referencesBound: boolean; bindingSha256: string }> = {};
  for (const [id, card] of [...unique].sort(([a], [b]) => a.localeCompare(b))) {
    const faces = card.card_faces ?? [card];
    let ok = true;
    const ast = [];
    const bindingFaces: { sourceFaceIndex: number; result?: ParseCardResult }[] = [];
    for (const [sourceFaceIndex, face] of faces.entries()) {
      // Empty oracle text is a valid zero-ability face; missing text is not.
      if (face.oracle_text == null) {
        bindingFaces.push({ sourceFaceIndex });
        ok = false;
        const key = "missing-oracle-text";
        const group = failures.get(key) ?? { count: 0, examples: [] };
        group.count++;
        if (group.examples.length < 2) group.examples.push({ name: face.name, text: "", error: key });
        failures.set(key, group);
        continue;
      }
      const result = parser(face.oracle_text, face.name, {
        layout: card.layout, typeLine: face.type_line ?? card.type_line ?? undefined, includeSourceProvenance: true, cardNames,
      });
      bindingFaces.push({ sourceFaceIndex, result });
      if (!result.ok) ok = false;
      else ast.push(result.abilities);
      for (const line of result.lines) {
        linesChecked++;
        if (line.ok) { linesOk++; continue; }
        // Group by the failure location instead of an already-supported opener
        // such as "At the beginning of". Keep complete examples for investigation.
        const at = / at ("[^"\n]*"|end of line) \(token/.exec(line.error)?.[1];
        const key = at ? `at ${at.toLowerCase()}` : line.error;
        const group = failures.get(key) ?? { count: 0, examples: [] };
        group.count++;
        if (group.examples.length < 2) group.examples.push({ name: face.name, text: line.text, error: line.error });
        failures.set(key, group);
      }
    }
    if (ok) cardsOk++;
    if (card.legalities?.commander === "legal") { commanderLegal.cardsChecked++; if (ok) commanderLegal.cardsOk++; }
    const binding = analyzeCardBindings({ identity: id, source: card, faces: bindingFaces });
    addBindingSummary(bindingSummary, binding);
    outcomes[id] = { name: card.name, ok, ...(ok ? { astSha256: sha256(JSON.stringify(ast)) } : {}),
      referencesFullyBound: binding.fullyBound, referencesBound: binding.referencesBound, bindingSha256: sha256(JSON.stringify(binding)) };
  }
  return {
    scope: SCOPE, paperEvidence: evidence.fingerprint, inputRecords: cards.length, excluded,
    cardsChecked: unique.size, cardsOk, commanderLegal, linesChecked, linesOk, bindingSummary,
    failures: [...failures].map(([key, group]) => ({ key, ...group }))
      .sort((a, b) => b.count - a.count || a.key.localeCompare(b.key)),
    outcomes,
  };
}

export function sourceIdentity() {
  const files = ["packages/oracle-parser/package.json", "scripts/validate-parser.ts", "scripts/archive-parser.ts", "scripts/ingest-scryfall.ts", "scripts/lib/scryfall.ts", "scripts/lib/paper-evidence.ts", "scripts/build-paper-evidence.ts"];
  function walk(directory: string) {
    for (const entry of fs.readdirSync(path.join(ROOT, directory), { withFileTypes: true }).sort((a,b) => a.name.localeCompare(b.name))) {
      const name = `${directory}/${entry.name}`;
      if (entry.isDirectory()) walk(name); else if (name.endsWith(".ts")) files.push(name);
    }
  }
  walk("packages/oracle-parser/src");
  let revision: string | null = null;
  try { revision = execFileSync("git", ["rev-parse", "HEAD"], { cwd: ROOT, encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim(); } catch { /* source archives have no git */ }
  return { revision, sourceSha256: sha256(files.sort().map(name => `${name}\0${fs.readFileSync(path.join(ROOT, name), "utf8")}`).join("\0")) };
}

export function buildReport(cacheFile = CACHE_FILE, paperEvidence = loadPaperEvidence()) {
  const corpus = fs.readFileSync(cacheFile);
  const parser = sourceIdentity();
  const report = {
    reportVersion: 3, corpus: { sha256: sha256(corpus), bytes: corpus.length },
    parser, ...evaluateCorpus(readCards(corpus), { paperEvidence }),
  };
  if (JSON.stringify(sourceIdentity()) !== JSON.stringify(parser)) throw new Error("Parser sources changed during validation; retry after edits finish");
  return report;
}

function main() {
  const args = process.argv.slice(2);
  if (args.some(arg => arg !== "--json")) throw new Error("Usage: npm run validate -- [--json]");
  const report = buildReport();
  if (args.includes("--json")) { console.log(JSON.stringify(report, null, 2)); return; }
  const percent = (n: number, d: number) => d ? `${(100 * n / d).toFixed(2)}%` : "n/a";
  console.log(`Corpus SHA-256: ${report.corpus.sha256}`);
  console.log(`Parser revision: ${report.parser.revision ?? "unknown"}`);
  console.log(`Source SHA-256 (includes working changes): ${report.parser.sourceSha256}`);
  console.log(`Scope: ${report.scope}; excluded records: ${JSON.stringify(report.excluded)}`);
  console.log(`Cards accepted: ${report.cardsOk}/${report.cardsChecked} (${percent(report.cardsOk, report.cardsChecked)})`);
  console.log(`Commander-legal cards accepted: ${report.commanderLegal.cardsOk}/${report.commanderLegal.cardsChecked} (${percent(report.commanderLegal.cardsOk, report.commanderLegal.cardsChecked)})`);
  console.log(`Ability blocks accepted: ${report.linesOk}/${report.linesChecked} (${percent(report.linesOk, report.linesChecked)})`);
  console.log(`Reference analysis fully bound: ${report.bindingSummary.cardsFullyBound}/${report.cardsChecked}; unresolved obligations: ${report.bindingSummary.unresolved}`);
  console.log(`References bound: ${report.bindingSummary.cardsReferencesBound}/${report.cardsChecked}; awaiting engine features: ${
    Object.entries(report.bindingSummary.engineFeatures).map(([feature, count]) => `${feature} ${count}`).join(", ") || "none"}`);
  console.log("Acceptance is not semantic correctness; run npm run check in oracle-parser.");
  for (const group of report.failures.slice(0, 20)) {
    console.log(`\n${group.count} failures ${group.key}`);
    for (const example of group.examples) console.log(`  ${example.name}: ${example.text}\n    ${example.error}`);
  }
}
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  try { main(); } catch (error) { console.error(error); process.exitCode = 1; }
}
