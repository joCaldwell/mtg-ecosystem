import { backupDatabase, restoreDatabase } from "./backups.ts";
import { runJob } from "./jobs.ts";
import { openDb, DEFAULT_DB_PATH } from "./db.ts";
import { formatUsd } from "./prices.ts";
import { refreshCardData } from "./refresh.ts";
import { search, SearchError } from "./search/index.ts";

function formatCard(c: {
  name: string;
  mana_cost: string | null;
  type_line: string;
  oracle_text: string;
  power: string | null;
  toughness: string | null;
  loyalty: string | null;
  color_identity: string;
  usd_cents: number | null;
}): string {
  const cost = c.mana_cost ? `  ${c.mana_cost}` : "";
  const pt =
    c.power != null && c.toughness != null
      ? `  [${c.power}/${c.toughness}]`
      : c.loyalty != null
        ? `  [${c.loyalty}]`
        : "";
  const id = c.color_identity || "C";
  const text = c.oracle_text ? "\n  " + c.oracle_text.replaceAll("\n", "\n  ") : "";
  const price = c.usd_cents == null ? "" : `  ${formatUsd(c.usd_cents)}`;
  return `${c.name}${cost}  (${c.type_line})${pt}  id:${id}${price}${text}`;
}

async function main() {
  const [cmd, ...rest] = process.argv.slice(2);

  switch (cmd) {
    case "refresh": {
      const db = openDb();
      try { await runJob(db, "refresh", () => refreshCardData(db, { force: rest.includes("--force") })); }
      finally { db.close(); }
      break;
    }
    case "backup": {
      const db = openDb();
      try { console.log(await runJob(db, "backup", () => backupDatabase(db))); }
      finally { db.close(); }
      break;
    }
    case "restore": {
      if (rest.length !== 1) throw new Error("Usage: cli.ts restore <backup-file>");
      const result = await restoreDatabase(rest[0]);
      console.log(`Restored ${result.decks} decks to ${DEFAULT_DB_PATH}`);
      break;
    }
    case "search": {
      const query = rest.join(" ").trim();
      if (!query) {
        console.error('Usage: npm run search -- "t:creature id<=g cmc<=2"');
        process.exit(1);
      }
      const db = openDb();
      const count = db.prepare("SELECT count(*) n FROM cards").get() as { n: number };
      if (!count.n) {
        console.error(`Card database at ${DEFAULT_DB_PATH} is empty — run 'npm run refresh' first.`);
        process.exit(1);
      }
      try {
        const results = search(db, query, { limit: 20 });
        for (const c of results) console.log(formatCard(c) + "\n");
        console.log(`${results.length} result(s)${results.length === 20 ? " (limit 20)" : ""}`);
      } catch (e) {
        if (e instanceof SearchError) {
          console.error(`Search error: ${e.message}`);
          process.exit(1);
        }
        throw e;
      }
      db.close();
      break;
    }
    default:
      console.error("Usage: cli.ts <refresh [--force] | backup | restore <file> | search <query>>");
      process.exit(1);
  }
}

main().catch(error => { console.error(error); process.exitCode = 1; });
