// scripts/ingest-scryfall.ts
const fs = require("fs");
const path = require("path");
const { Readable } = require("stream");
const { finished } = require("stream/promises");

const CACHE_DIR = path.join(__dirname, "../.scryfall-cache");
const CACHE_FILE = path.join(CACHE_DIR, "oracle-cards.json");
const MANIFEST_URL = "https://api.scryfall.com/bulk-data";

async function main() {
  fs.mkdirSync(CACHE_DIR, { recursive: true });

  // 1. Check cache freshness (24h)
  if (fs.existsSync(CACHE_FILE)) {
    const stats = fs.statSync(CACHE_FILE);
    const ageInHours = (Date.now() - stats.mtimeMs) / (1000 * 60 * 60);
    if (ageInHours < 24) {
      console.log(`Cache is fresh (${ageInHours.toFixed(1)}h old). Skipping download.`);
      return;
    }
    console.log(`Cache is stale (${ageInHours.toFixed(1)}h old). Re-downloading...`);
  } else {
    console.log("No cached data found. Downloading...");
  }

  // 2. Fetch Bulk Data Manifest
  console.log(`Fetching manifest from ${MANIFEST_URL}...`);
  const manifestRes = await fetch(MANIFEST_URL, {
    headers: {
      "User-Agent": "MTGEcosystemParser/1.0.0 (contact: joCaldwell/mtg-ecosystem)",
      "Accept": "application/json"
    }
  });
  if (!manifestRes.ok) {
    throw new Error(`Failed to fetch manifest: ${manifestRes.status} ${manifestRes.statusText}`);
  }
  const manifestJson = (await manifestRes.json()) as any;

  // 3. Find oracle_cards bulk file
  const oracleCardsMeta = manifestJson.data.find(
    (item: any) => item.type === "oracle_cards"
  );
  if (!oracleCardsMeta) {
    throw new Error("Could not find 'oracle_cards' export in Scryfall manifest.");
  }

  const downloadUrl = oracleCardsMeta.download_uri;
  console.log(`Found bulk file updated at: ${oracleCardsMeta.updated_at}`);
  console.log(`Size: ${(oracleCardsMeta.size / (1024 * 1024)).toFixed(1)} MB`);
  console.log(`Downloading from ${downloadUrl}...`);

  // 4. Download and stream to cache
  const fileRes = await fetch(downloadUrl, {
    headers: {
      "User-Agent": "MTGEcosystemParser/1.0.0 (contact: joCaldwell/mtg-ecosystem)"
    }
  });
  if (!fileRes.ok) {
    throw new Error(`Failed to download file: ${fileRes.status} ${fileRes.statusText}`);
  }
  if (!fileRes.body) {
    throw new Error("Response body is empty or not readable.");
  }

  const fileStream = fs.createWriteStream(CACHE_FILE);
  const nodeReadable = Readable.fromWeb(fileRes.body as any);
  
  await finished(nodeReadable.pipe(fileStream));
  console.log(`Successfully cached Scryfall bulk data to: ${CACHE_FILE}`);
}

main().catch((err) => {
  console.error("Ingestion failed:", err);
  process.exit(1);
});
