// Acceptance scoreboard, not semantic correctness certification.
import fs from "node:fs";
import path from "node:path";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { fileURLToPath, pathToFileURL } from "node:url";
import { parseOracleText } from "../packages/oracle-parser/src/index.ts";
import { readCards, type ScryfallCard } from "./lib/scryfall.ts";

const ROOT = fileURLToPath(new URL("../", import.meta.url));
const CACHE_FILE = path.join(ROOT, ".scryfall-cache/oracle-cards.json");
export const SCOPE = "paper-non-silver-non-acorn-v1";
const SKIP_LAYOUTS = new Set([
  "token", "double_faced_token", "emblem", "art_series", "planar", "vanguard", "scheme",
]);
export const sha256 = (text: string | Buffer): string => createHash("sha256").update(text).digest("hex");

function exclusion(card: ScryfallCard): string | undefined {
  if (SKIP_LAYOUTS.has(card.layout)) return `layout:${card.layout}`;
  if (card.digital) return "digital";
  if (card.border_color === "silver" || card.security_stamp === "acorn") return "silver-or-acorn";
  return undefined;
}
function identity(card: ScryfallCard): string {
  return card.oracle_id ?? card.card_faces!.map(face => face.oracle_id!).join("/");
}

export function evaluateCorpus(cards: ScryfallCard[], parser = parseOracleText) {
  const excluded: Record<string, number> = {};
  const unique = new Map<string, ScryfallCard>();
  for (const card of cards) {
    const reason = exclusion(card);
    if (reason) { excluded[reason] = (excluded[reason] ?? 0) + 1; continue; }
    const key = identity(card);
    if (unique.has(key)) { excluded.duplicate = (excluded.duplicate ?? 0) + 1; continue; }
    unique.set(key, card);
  }
  let cardsOk = 0, linesChecked = 0, linesOk = 0;
  const failures = new Map<string, { count: number; examples: { name: string; text: string; error: string }[] }>();
  const outcomes: Record<string, { name: string; ok: boolean; astSha256?: string }> = {};
  for (const [id, card] of [...unique].sort(([a], [b]) => a.localeCompare(b))) {
    const faces = card.card_faces ?? [card];
    let ok = true;
    const ast = [];
    for (const face of faces) {
      // Empty oracle text is a valid zero-ability face; missing text is not.
      if (face.oracle_text == null) {
        ok = false;
        const key = "missing-oracle-text";
        const group = failures.get(key) ?? { count: 0, examples: [] };
        group.count++;
        if (group.examples.length < 2) group.examples.push({ name: face.name, text: "", error: key });
        failures.set(key, group);
        continue;
      }
      const result = parser(face.oracle_text, face.name);
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
    outcomes[id] = { name: card.name, ok, ...(ok ? { astSha256: sha256(JSON.stringify(ast)) } : {}) };
  }
  return {
    scope: SCOPE, inputRecords: cards.length, excluded,
    cardsChecked: unique.size, cardsOk, linesChecked, linesOk,
    failures: [...failures].map(([key, group]) => ({ key, ...group }))
      .sort((a, b) => b.count - a.count || a.key.localeCompare(b.key)),
    outcomes,
  };
}

function sourceIdentity() {
  const files = ["packages/oracle-parser/package.json", "scripts/validate-parser.ts", "scripts/lib/scryfall.ts"];
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

export function buildReport(cacheFile = CACHE_FILE) {
  const corpus = fs.readFileSync(cacheFile);
  return {
    reportVersion: 1, corpus: { sha256: sha256(corpus), bytes: corpus.length },
    parser: sourceIdentity(), ...evaluateCorpus(readCards(corpus.toString("utf8"))),
  };
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
  console.log(`Ability blocks accepted: ${report.linesOk}/${report.linesChecked} (${percent(report.linesOk, report.linesChecked)})`);
  console.log("Acceptance is not semantic correctness; run npm run check in oracle-parser.");
  for (const group of report.failures.slice(0, 20)) {
    console.log(`\n${group.count} failures ${group.key}`);
    for (const example of group.examples) console.log(`  ${example.name}: ${example.text}\n    ${example.error}`);
  }
}
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  try { main(); } catch (error) { console.error(error); process.exitCode = 1; }
}
