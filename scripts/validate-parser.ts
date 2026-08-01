// scripts/validate-parser.ts — the oracle-parser scoreboard.
//
// Runs the parser across the full Scryfall oracle-card corpus and reports
// coverage at card AND line granularity, plus the largest failure groups.
// Run from the repo root: npm run validate

import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { parseOracleText } from "../packages/oracle-parser/src/index.ts";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const CACHE_FILE = path.join(__dirname, "../.scryfall-cache/oracle-cards.json");

const SKIP_LAYOUTS = new Set([
  "token", "double_faced_token", "emblem", "art_series", "planar", "vanguard", "scheme",
]);

interface ScryfallCard {
  name: string;
  layout: string;
  oracle_text?: string;
  card_faces?: { name: string; oracle_text?: string }[];
}

function main() {
  if (!fs.existsSync(CACHE_FILE)) {
    console.error(`Cached Scryfall data not found at ${CACHE_FILE}`);
    console.error("Run: npm run ingest");
    process.exit(1);
  }

  const allCards = JSON.parse(fs.readFileSync(CACHE_FILE, "utf-8")) as ScryfallCard[];
  const unique = new Map<string, ScryfallCard>();
  for (const card of allCards) {
    if (!card.name || SKIP_LAYOUTS.has(card.layout)) continue;
    if (!card.oracle_text && !card.card_faces) continue;
    if (!unique.has(card.name)) unique.set(card.name, card);
  }

  let cardsChecked = 0;
  let cardsOk = 0;
  let linesChecked = 0;
  let linesOk = 0;
  const failGroups = new Map<string, { count: number; examples: string[] }>();
  const started = Date.now();

  for (const card of unique.values()) {
    const faces = card.card_faces
      ? card.card_faces.filter((f) => f.oracle_text).map((f) => ({ name: f.name, text: f.oracle_text! }))
      : card.oracle_text
        ? [{ name: card.name, text: card.oracle_text }]
        : [];
    if (faces.length === 0) continue;

    cardsChecked++;
    let cardOk = true;
    for (const face of faces) {
      const result = parseOracleText(face.text, face.name);
      for (const line of result.lines) {
        linesChecked++;
        if (line.ok) {
          linesOk++;
        } else {
          cardOk = false;
          // Group by the first few words of the failing line — that's the
          // template we haven't implemented yet.
          const key = line.text.split(" ").slice(0, 4).join(" ").toLowerCase();
          const group = failGroups.get(key) ?? { count: 0, examples: [] };
          group.count++;
          if (group.examples.length < 2) {
            group.examples.push(`${card.name}: ${line.error ?? "?"}`);
          }
          failGroups.set(key, group);
        }
      }
    }
    if (cardOk) cardsOk++;
  }

  const secs = ((Date.now() - started) / 1000).toFixed(1);
  const pct = (n: number, d: number) => ((n / d) * 100).toFixed(2);

  console.log("==================================");
  console.log("     PARSER VALIDATION REPORT     ");
  console.log("==================================");
  console.log(`Duration            : ${secs}s`);
  console.log(`Cards fully parsed  : ${cardsOk}/${cardsChecked} (${pct(cardsOk, cardsChecked)}%)`);
  console.log(`Lines parsed        : ${linesOk}/${linesChecked} (${pct(linesOk, linesChecked)}%)`);
  console.log("==================================\n");

  console.log("Largest unparsed templates (by leading words):");
  const top = [...failGroups.entries()].sort((a, b) => b[1].count - a[1].count).slice(0, 20);
  for (const [key, group] of top) {
    console.log(`\n🔴 ${group.count}× "${key} …"`);
    for (const ex of group.examples) console.log(`     - ${ex}`);
  }
}

main();
