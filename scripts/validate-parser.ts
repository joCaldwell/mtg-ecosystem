// scripts/validate-parser.ts
const fs = require("fs");
const path = require("path");
const { normalizeOracleText } = require("../packages/oracle-parser/src/ingest/normalize.ts");
const { parseOracleTextDetails } = require("../packages/oracle-parser/src/index.ts");

const CACHE_FILE = path.join(__dirname, "../.scryfall-cache/oracle-cards.json");

interface ErrorRecord {
  cardName: string;
  oracleText: string;
  errorMsg: string;
}

async function main() {
  if (!fs.existsSync(CACHE_FILE)) {
    console.error(`Error: Cached Scryfall data not found at ${CACHE_FILE}`);
    console.error("Please run: npm run ingest");
    process.exit(1);
  }

  console.log("Loading cached Scryfall bulk data...");
  const rawData = fs.readFileSync(CACHE_FILE, "utf-8");
  const allCards = JSON.parse(rawData) as any[];
  console.log(`Loaded ${allCards.length} total card printings.`);

  // 1. Deduplicate cards by name to validate unique oracle text only
  console.log("Deduplicating cards by name...");
  const uniqueCardsMap = new Map<string, any>();
  for (const card of allCards) {
    if (!card.name) continue;

    // Filter out non-playable elements
    const skipLayouts = ["token", "double_faced_token", "emblem", "art_series", "planar", "vanguard", "scheme"];
    if (skipLayouts.includes(card.layout)) continue;

    // Skip if it doesn't have rules text or faces
    if (!card.oracle_text && !card.card_faces) continue;

    if (!uniqueCardsMap.has(card.name)) {
      uniqueCardsMap.set(card.name, card);
    }
  }

  const cards = Array.from(uniqueCardsMap.values());
  const totalCards = cards.length;
  console.log(`Found ${totalCards} unique playable cards.`);

  console.log("Starting parser validation...");
  let successCount = 0;
  let failCount = 0;
  const errors: ErrorRecord[] = [];
  const startTime = Date.now();

  for (let i = 0; i < totalCards; i++) {
    const card = cards[i];

    // Log progress every 5000 cards
    if (i > 0 && i % 5000 === 0) {
      console.log(`Processed ${i}/${totalCards} cards...`);
    }

    // Extract text from faces or top-level card
    const facesText: { faceName: string; text: string }[] = [];
    if (card.card_faces) {
      for (const face of card.card_faces) {
        if (face.oracle_text) {
          facesText.push({ faceName: face.name, text: face.oracle_text });
        }
      }
    } else if (card.oracle_text) {
      facesText.push({ faceName: card.name, text: card.oracle_text });
    }

    if (facesText.length === 0) continue;

    let cardAllFacesParsed = true;
    const cardErrors: string[] = [];

    for (const face of facesText) {
      const normalized = normalizeOracleText(face.text);
      if (!normalized) continue; // Skip blank rules text

      const result = parseOracleTextDetails(normalized);
      if (!result.success) {
        cardAllFacesParsed = false;
        cardErrors.push(...result.errors);
      }
    }

    if (cardAllFacesParsed) {
      successCount++;
    } else {
      failCount++;
      errors.push({
        cardName: card.name,
        oracleText: card.oracle_text || card.card_faces.map((f: any) => f.oracle_text).join(" // "),
        errorMsg: cardErrors[0], // record the first error encountered
      });
    }
  }

  const durationSec = ((Date.now() - startTime) / 1000).toFixed(1);
  const successRate = ((successCount / (successCount + failCount)) * 100).toFixed(2);

  console.log("\n=================================");
  console.log("    PARSER VALIDATION REPORT     ");
  console.log("=================================");
  console.log(`Validation Duration : ${durationSec}s`);
  console.log(`Total Cards Checked : ${successCount + failCount}`);
  console.log(`Successful Parses   : ${successCount}`);
  console.log(`Failed Parses       : ${failCount}`);
  console.log(`Parser Success Rate : ${successRate}%`);
  console.log("=================================\n");

  // 2. Failure Analysis: Group by error types
  console.log("Top Parsing Failures (Failure Analysis):");
  const errorGroups = new Map<string, { count: number; examples: string[] }>();

  for (const err of errors) {
    // Clean/generalize error messages to group them
    // E.g., "line 1:15 - mismatched input 'foo' expecting BAR" -> "mismatched input expecting BAR"
    let cleanMsg = err.errorMsg
      .replace(/^line \d+:\d+ - /, "")
      .replace(/'[^']+'/, "'<token>'");

    const group = errorGroups.get(cleanMsg) || { count: 0, examples: [] };
    group.count++;
    if (group.examples.length < 3) {
      group.examples.push(`"${err.cardName}" -> Oracle: "${err.oracleText.substring(0, 80)}${err.oracleText.length > 80 ? "..." : ""}"`);
    }
    errorGroups.set(cleanMsg, group);
  }

  // Sort and print the top 10 errors
  const sortedErrors = Array.from(errorGroups.entries())
    .sort((a, b) => b[1].count - a[1].count)
    .slice(0, 10);

  for (const [msg, data] of sortedErrors) {
    console.log(`\n🔴 Error: "${msg}" (Failed on ${data.count} cards)`);
    console.log("   Examples:");
    for (const ex of data.examples) {
      console.log(`     - ${ex}`);
    }
  }
}

main().catch(console.error);
