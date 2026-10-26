#!/usr/bin/env node
// Builds the deterministic fixture database the capture runs against.
//
//   node seed.ts [--out /tmp/ux-eval/seed/seed.db] [--source <owner-copy.db>]
//
// Card facts are copied from a snapshot of a real database (cards, card_names,
// card_prices and the ingest/price meta keys only; no user row is copied).
// Everything else is built through the capability layer: the owner's own
// actions as a UI actor, agent writes through the MCP tool adapters as an
// agent actor, so activity and attribution are what a real session leaves.
// Time and randomness are fixed (lib/determinism.ts), so the same inputs give
// the same database.

import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { DatabaseSync, backup } from "node:sqlite";
import { FROZEN_NOW, RealDate, installClock, installRandom, installSqliteClock, setNow } from "./lib/determinism.ts";

const HERE = dirname(fileURLToPath(import.meta.url));
const PKG = resolve(HERE, "..");
const FIXTURES = join(PKG, "test", "fixtures", "moxfield");

function arg(name: string, fallback: string): string {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 && process.argv[i + 1] ? process.argv[i + 1] : fallback;
}
const OUT = resolve(arg("out", "/tmp/ux-eval/seed/seed.db"));
const SOURCE = resolve(arg("source", "/home/josh/.t3/worktrees/mtg-ecosystem/t3-aa7eb73f/packages/deck-builder/.dev/owner-copy.db"));
const CACHE = "/tmp/ux-eval/cache";
// The snapshot predates stored image URLs and artists. When this Scryfall
// oracle bulk file is present, those two facts are filled from it through the
// app's own toRow() mapping; nothing else is taken from it.
const IMAGES = resolve(arg("images", "/home/josh/Code/mtg-ecosystem/.scryfall-cache/oracle-cards.json"));

// Never let the app's default data folder be touched: src/db.ts reads these at import.
const scratch = join(dirname(OUT), "app-data");
mkdirSync(scratch, { recursive: true });
process.env.DECKBUILDER_DATA = scratch;
process.env.DECKBUILDER_DB = join(scratch, "unused.db");
process.env.DECKBUILDER_MODE = "local";

installClock();
installRandom(0x5eed);

const { openDb } = await import("../src/db.ts");
const { beginSignup, provisionIdentity } = await import("../src/auth.ts");
const { createDeck, setDeckCategory, createSlot, createTag, organizeCard, shortlistCard } = await import("../src/deck/service.ts");
const { applyImport } = await import("../src/deck/interop.ts");
const { updateBrief, setEngine } = await import("../src/deck/brief.ts");
const { acceptItems, rejectItems, pendingItemIds } = await import("../src/deck/proposals.ts");
const { setVisibility, setPublishDecisions } = await import("../src/sharing.ts");
const { setProfile } = await import("../src/documents.ts");
const { remember } = await import("../src/memory.ts");
const { createToken } = await import("../src/tokens.ts");
const { createDemo } = await import("../src/demo.ts");
const { callTool } = await import("../src/mcp/tools.ts");
const { tryResolveCard } = await import("../src/cards.ts");
const { toRow } = await import("../src/ingest.ts");
const { readJsonArray } = await import("../src/json-array.ts");

type Actor = { user_id: number; via: "ui" | "agent"; label: string };

// ---------- 1. card data from a snapshot of a real database ----------

/** A migrated copy of the source, made with SQLite's online backup API from a read-only connection. */
async function cardSource(): Promise<string> {
  mkdirSync(CACHE, { recursive: true });
  const st = statSync(SOURCE);
  const images = existsSync(IMAGES) ? statSync(IMAGES).size : 0;
  const target = join(CACHE, `cards-source-${st.size}-${Math.floor(st.mtimeMs)}-${images}.db`);
  if (existsSync(target)) return target;
  const tmp = target + ".partial";
  rmSync(tmp, { force: true });
  const ro = new DatabaseSync(SOURCE, { readOnly: true });
  await backup(ro, tmp);
  ro.close();
  const migrated = openDb(tmp); // migrates the copy to the current schema
  if (existsSync(IMAGES)) {
    const update = migrated.prepare(`UPDATE cards SET image_small = :image_small, image_normal = :image_normal,
      image_art_crop = :image_art_crop, artist = :artist WHERE oracle_id = :oracle_id AND image_normal IS NULL`);
    migrated.exec("BEGIN");
    let n = 0;
    for await (const card of readJsonArray(IMAGES)) {
      const row = toRow(card as any) as any;
      if (!row?.image_normal) continue;
      n += Number(update.run({ oracle_id: row.oracle_id, image_small: row.image_small, image_normal: row.image_normal, image_art_crop: row.image_art_crop, artist: row.artist }).changes);
    }
    migrated.exec("COMMIT");
    console.warn(`Filled image URLs and artists for ${n} cards from ${IMAGES}`);
  } else console.warn(`No ${IMAGES}: cards have no image URLs, so card images show their placeholder`);
  migrated.close();
  copyFileSync(tmp, target);
  for (const ext of ["", "-wal", "-shm"]) rmSync(tmp + ext, { force: true });
  return target;
}

function columns(db: DatabaseSync, schema: string, table: string): string[] {
  return (db.prepare(`PRAGMA ${schema}.table_info(${table})`).all() as { name: string }[]).map(c => c.name);
}

const source = await cardSource();
mkdirSync(dirname(OUT), { recursive: true });
for (const ext of ["", "-wal", "-shm"]) rmSync(OUT + ext, { force: true });
const db = openDb(OUT) as unknown as DatabaseSync;
installSqliteClock(db);
db.exec(`ATTACH DATABASE '${source.replaceAll("'", "''")}' AS src`);
db.exec("BEGIN");
for (const table of ["cards", "card_names", "card_prices"]) {
  const cols = columns(db, "main", table).filter(c => columns(db, "src", table).includes(c)).map(c => `"${c}"`).join(", ");
  db.exec(`INSERT INTO main.${table} (${cols}) SELECT ${cols} FROM src.${table} ORDER BY rowid`);
}
db.exec("INSERT INTO cards_fts (name, search_text, oracle_id) SELECT name, search_text, oracle_id FROM cards ORDER BY rowid");
db.exec(`INSERT INTO main.meta (key, value) SELECT key, value FROM src.meta
  WHERE key IN ('last_ingest', 'prices_as_of', 'price_stores_version')
  ON CONFLICT(key) DO UPDATE SET value = excluded.value`);
db.exec("COMMIT");
db.exec("DETACH DATABASE src");

// ---------- 2. people ----------

// A fictional timeline ending shortly before the frozen "now" the browser sees.
const T = Date.parse(FROZEN_NOW);
function at(daysAgo: number, hhmm: string) {
  const [h, m] = hhmm.split(":").map(Number);
  const d = new Date(T - daysAgo * 86_400_000);
  setNow(Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate(), h, m, 0));
}

const env = {
  ...process.env,
  DECKBUILDER_CLAIM_CODE: "ux-eval-fixture-claim-code-0123456789abcdef",
  DECKBUILDER_INVITES_REQUIRED: "0",
};
const OWNER: Actor = { user_id: 1, via: "ui", label: "ui" };
const CLAUDE: Actor = { user_id: 1, via: "agent", label: "claude-code" };
const CODEX: Actor = { user_id: 1, via: "agent", label: "codex" };
const tool = (actor: Actor, name: string, args: unknown) => callTool({ db: db as any, actor }, name, args);

function oracle(name: string): string {
  const hit = tryResolveCard(db as any, name) as { oracle_id: string } | { problem: string };
  if ("problem" in hit) throw new Error(`Seed card does not resolve: ${name} (${hit.problem})`);
  return hit.oracle_id;
}
const list = (file: string) => readFileSync(join(FIXTURES, file), "utf8");

/** A new deck filled the way the app's Import does it: createDeck, then applyImport. */
function imported(name: string, file: string, category: "paper" | "theorycrafted" | "archived"): number {
  const id = createDeck(db as any, OWNER, name, category === "theorycrafted" ? {} : { category });
  applyImport(db as any, OWNER, id, list(file));
  return id;
}
/** The items of the deck's newest proposal. */
function pendingIds(deckId: number): number[] {
  const { id } = db.prepare("SELECT MAX(id) id FROM proposals WHERE deck_id = ?").get(deckId) as { id: number };
  return pendingItemIds(db as any, OWNER, id);
}

// Everything below runs in time order, so ids and timestamps agree in every list.

// ---------- 3. Rowan: the owner, with six decks and connected agents ----------

at(40, "18:00");
// User 1 exists from the first migration; claiming gives it a real identity.
provisionIdentity(db as any, { id: "uxeval-rowan", name: "Rowan Ellis", signupHandle: "rowan" }, beginSignup(db as any, env.DECKBUILDER_CLAIM_CODE, env), env);
// The first migration stamped user 1 with the real clock; give it the fictional one.
db.prepare("UPDATE users SET created_at = datetime('now') WHERE id = 1").run();

at(38, "17:30");
const rograkh = imported("Rograkh Silas", "rograkh-silas.copy-plain-text.txt", "paper");

at(34, "19:05");
const najeela = imported("Najeela Warriors", "najeela.copy-plain-text.txt", "paper");
at(33, "20:00");
createSlot(db as any, OWNER, najeela, "fast mana", 14, 18);
createSlot(db as any, OWNER, najeela, "tutors", 10, 14);
createSlot(db as any, OWNER, najeela, "interaction", 12, 16);
createSlot(db as any, OWNER, najeela, "win lines", 3, 5);
const nTags = {
  buy: createTag(db as any, OWNER, najeela, "Need to buy"),
  testing: createTag(db as any, OWNER, najeela, "Testing"),
  proxy: createTag(db as any, OWNER, najeela, "Proxy"),
};
for (const [name, tagIds] of [
  ["Imperial Seal", [nTags.buy]],
  ["Gemstone Caverns", [nTags.buy, nTags.proxy]],
  ["Lion's Eye Diamond", [nTags.proxy]],
  ["Hexing Squelcher", [nTags.testing]],
  ["Voice of Victory", [nTags.testing]],
  ["The Cabbage Merchant", [nTags.testing]],
] as [string, number[]][]) organizeCard(db as any, OWNER, najeela, oracle(name), { tagIds });
updateBrief(db as any, OWNER, najeela, {
  thesis:
    "Najeela makes a warrior army every combat, and the deck turns that into extra combats with " +
    "[[Derevi, Empyrial Tactician]] or ends the game on the spot with [[Thassa's Oracle]] or an " +
    "[[Underworld Breach]] loop. Fast mana and tutors find a line by turn three; free counters protect it.",
  constraints_md:
    "- Competitive table: assume fast combo from every opponent.\n" +
    "- Every win line must be findable with a single tutor.\n" +
    "- Keep the land count at 26 or fewer.\n" +
    "- No new cards over $150 until the reserved-list duals are bought.",
});
setEngine(db as any, OWNER, najeela, "Breach loop", "Lion's Eye Diamond and Brain Freeze recur from the graveyard until every library is empty.",
  ["Underworld Breach", "Lion's Eye Diamond", "Brain Freeze"].map(name => ({ oracle_id: oracle(name) })));
at(33, "20:20");
tool(CLAUDE, "file_cards", {
  deck: najeela,
  filings: [
    ...["Ancient Tomb", "Chrome Mox", "Lotus Petal", "Mana Vault", "Mox Diamond", "Mox Opal", "Sol Ring", "Dark Ritual", "Elvish Spirit Guide", "Simian Spirit Guide", "Rite of Flame", "Lion's Eye Diamond", "Gemstone Caverns", "Birds of Paradise", "Noble Hierarch"].map(card => ({ card, slot: "fast mana" })),
    ...["Demonic Tutor", "Vampiric Tutor", "Imperial Seal", "Mystical Tutor", "Enlightened Tutor", "Worldly Tutor", "Demonic Consultation", "Tainted Pact", "Diabolic Intent", "Gamble", "Summoner's Pact", "Green Sun's Zenith", "Eladamri's Call"].map(card => ({ card, slot: "tutors" })),
    ...["Force of Will", "Fierce Guardianship", "Flusterstorm", "Pact of Negation", "Swan Song", "Mental Misstep", "Mindbreak Trap", "Red Elemental Blast", "Deflecting Swat", "Swords to Plowshares", "Silence", "Veil of Summer", "Chain of Vapor"].map(card => ({ card, slot: "interaction" })),
    ...["Thassa's Oracle", "Underworld Breach", "Brain Freeze", "Ad Nauseam"].map(card => ({ card, slot: "win lines" })),
  ],
});

at(31, "18:00");
setProfile(db as any, OWNER,
  "I play cEDH on Thursdays and casual Commander on Sundays.\n\n" +
  "- I like decks that win the turn they start, and I dislike long, symmetrical lock pieces.\n" +
  "- I buy slowly; proxies stand in until the real card arrives.\n" +
  "- I keep a deck's history: tell me why a card left before suggesting it again.");
at(31, "18:05");
createToken(db as any, OWNER, "claude-code");
at(31, "18:06");
createToken(db as any, OWNER, "codex");

at(30, "20:40");
const kinnan = imported("Kinnan Midrange", "kinnan-midrange.copy-plain-text.txt", "paper");
at(29, "19:00");
createSlot(db as any, OWNER, kinnan, "mana dorks", 8, 10);
createSlot(db as any, OWNER, kinnan, "counterspells", 8, 12);
const kTesting = createTag(db as any, OWNER, kinnan, "Testing");
organizeCard(db as any, OWNER, kinnan, oracle("The Unagi of Kyoshi Island"), { tagIds: [kTesting] });
updateBrief(db as any, OWNER, kinnan, {
  thesis: "Kinnan untaps nonland mana into a huge creature from the library. Midrange first: it wins long games when the combo is answered.",
});
shortlistCard(db as any, OWNER, kinnan, oracle("Sylvan Library"), "Card selection that never leaves the table.");

// A decided history: one accepted and one rejected proposal, each with a reason.
at(27, "21:10");
tool(CLAUDE, "propose_changes", {
  deck: najeela, summary: "Trim the weakest mana piece",
  items: [{ action: "cut", card: "Tinder Wall", reason: "The weakest mana piece once the deck has eight free fast-mana cards." }],
});
at(27, "21:40");
acceptItems(db as any, OWNER, pendingIds(najeela), "Agreed, it has been a dead draw in every game.");

at(26, "18:15");
const caesar = imported("Hail Caesar", "precon-hail-caesar.copy-plain-text.txt", "paper");
at(25, "18:00");
updateBrief(db as any, OWNER, caesar, {
  thesis: "The Fallout precon, upgraded slowly. Caesar makes soldier tokens, sacrifices them for value, and drains the table.",
  constraints_md: "- Casual table, bracket 2.\n- Upgrades stay under $5 each.",
});
setVisibility(db as any, OWNER, caesar, "public");
setPublishDecisions(db as any, OWNER, caesar, true);

at(24, "19:30");
tool(CLAUDE, "propose_changes", {
  deck: najeela, summary: "A sturdier top end",
  items: [{ action: "add", card: "Necropotence", reason: "Draws far more than [[Ad Nauseam]] at a table that rarely pressures life totals.", slot: "win lines" }],
});
at(24, "20:05");
const rejection = rejectItems(db as any, OWNER, pendingIds(najeela), "Too slow here. Necropotence waits a turn; Ad Nauseam wins the turn it resolves.");
at(24, "20:07");
tool(CLAUDE, "remember", {
  text: "Rowan prefers win lines that finish the turn they start; [[Necropotence]] was rejected because it waits a turn.",
  scope: "deck", deck: najeela, basis: "stated", source_decision: rejection,
});

at(22, "21:00");
const bombadil = imported("Bombadil Enchantments", "bombadil-jegantha.copy-plain-text.txt", "theorycrafted");
// Theorycrafted, so an agent edits it directly.
at(20, "22:30");
tool(CLAUDE, "edit_cards", {
  deck: bombadil,
  remove: [{ card: "Mana Crypt" }],
  add: [{ card: "Arcane Signet" }],
  note: "Mana Crypt is banned in Commander; Arcane Signet keeps the colour fixing.",
});
at(20, "22:34");
tool(CLAUDE, "shortlist_cards", {
  deck: bombadil,
  add: [
    { card: "Enlightened Tutor", note: "Finds a Saga or an enchantress at instant speed." },
    { card: "Estrid's Invocation", note: "Copies the best enchantment on the table." },
  ],
});

at(19, "19:45");
const stickers = imported("Stickers and Attractions", "stickers-attractions.copy-plain-text.txt", "theorycrafted");

at(15, "17:00");
setDeckCategory(db as any, OWNER, rograkh, "archived");

at(12, "20:00");
tool(CLAUDE, "remember", {
  text: "Rowan plays at a competitive table on Thursdays and a casual one on Sundays; ask which table a change is for.",
  scope: "global", basis: "stated",
});
at(9, "19:20");
tool(CODEX, "remember", {
  text: "Rowan tends to keep cards that protect the commander over cards that draw.",
  scope: "global", basis: "inferred",
});
at(6, "21:05");
tool(CLAUDE, "remember", {
  text: "[[Gemstone Caverns]] is a proxy until the real copy arrives; do not count it toward the budget.",
  scope: "deck", deck: najeela, basis: "stated",
});
at(5, "18:45");
remember(db as any, OWNER, { text: "Kinnan wins most games through [[Hullbreaker Horror]], not the combo.", scope: "deck", deck_id: kinnan, basis: "stated" });

// Open work in the inbox, written by agents. One Kinnan proposal is decided on a phone.
at(3, "18:30");
tool(CLAUDE, "propose_changes", {
  deck: kinnan, summary: "Card selection over a tax piece",
  items: [
    { action: "cut", card: "Trinisphere", reason: "Taxes Kinnan's own cheap creatures as much as the opponents' spells.", to_maybe: true, group: "selection" },
    { action: "add", card: "Sylvan Library", reason: "Already on the maybe list; sees three cards a turn for one green mana.", group: "selection" },
  ],
});

// ---------- 4. Sam: a new, empty account for the first-run views ----------
at(2, "20:10");
const sam = provisionIdentity(db as any, { id: "uxeval-sam", name: "Sam Okafor", signupHandle: "sam" }, beginSignup(db as any, "", env), env).user_id;

at(2, "22:15");
tool(CLAUDE, "propose_changes", {
  deck: najeela, summary: "A faster turn-two Najeela",
  items: [
    { action: "cut", card: "Final Fortune", reason: "The extra turn only matters after a win line has already failed.", group: "fast-start" },
    { action: "add", card: "Gaea's Cradle", reason: "Najeela's warrior tokens make it tap for three or more from the second attack onward.", slot: "fast mana", group: "fast-start" },
  ],
});
at(2, "22:18");
tool(CLAUDE, "propose_changes", {
  deck: najeela, summary: "Protect the combo turn",
  items: [{ action: "add", card: "Pyroblast", reason: "Answers a blue counterspell on the combo turn for one red mana, like [[Red Elemental Blast]].", slot: "interaction" }],
});
at(1, "09:40");
tool(CODEX, "propose_changes", {
  deck: najeela, summary: "Trim a slow card",
  items: [{ action: "cut", card: "Mockingbird", reason: "Copies a creature for full mana; rarely cast before turn five in recorded games.", to_maybe: true }],
});
at(1, "09:44");
tool(CODEX, "propose_brief_edit", {
  deck: najeela, kind: "constraints",
  text:
    "- Competitive table: assume fast combo from every opponent.\n" +
    "- Every win line must be findable with a single tutor.\n" +
    "- Keep the land count at 26 or fewer.\n" +
    "- No new cards over $150 until the reserved-list duals are bought.\n" +
    "- A win line must finish the turn it starts.",
  reason: "Rowan rejected [[Necropotence]] because it waits a turn; the brief should say so.",
});

// ---------- 5. Jordan: the sample deck, as first run creates it ----------
at(1, "21:30");
const jordan = provisionIdentity(db as any, { id: "uxeval-jordan", name: "Jordan Lee", signupHandle: "jordan" }, beginSignup(db as any, "", env), env).user_id;
if (sam !== 2 || jordan !== 3) throw new Error(`Unexpected user ids ${sam}, ${jordan}`);
at(1, "21:35");
const demo = createDemo(db as any, { user_id: jordan, via: "ui", label: "ui" });
if (demo.missing.length) console.warn(`Sample deck skipped cards not in this card data: ${demo.missing.join(", ")}`);

at(0, "16:20");
tool(CLAUDE, "propose_changes", {
  deck: kinnan, summary: "A second big payoff",
  items: [{ action: "add", card: "Koma, Cosmos Serpent", reason: "A second creature [[Kinnan, Bonder Prodigy]] can find that ends long games on its own." }],
});

// ---------- 6. integrity and the absence of real time ----------

setNow(FROZEN_NOW);
const fk = db.prepare("PRAGMA foreign_key_check").all();
if (fk.length) throw new Error(`Foreign key problems: ${JSON.stringify(fk.slice(0, 5))}`);
const realToday = new RealDate(performance.timeOrigin + performance.now()).toISOString().slice(0, 10);
const leaks: string[] = [];
for (const { name } of db.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE '%fts%' AND name NOT IN ('cards','card_names','card_prices')").all() as { name: string }[])
  for (const col of columns(db, "main", name))
    if ((db.prepare(`SELECT COUNT(*) n FROM "${name}" WHERE CAST("${col}" AS TEXT) LIKE ?`).get(`%${realToday}%`) as { n: number }).n)
      leaks.push(`${name}.${col}`);
if (leaks.length && !FROZEN_NOW.startsWith(realToday)) throw new Error(`Real-time timestamps leaked into: ${leaks.join(", ")}`);

const counts = Object.fromEntries(["users", "decks", "deck_cards", "proposals", "document_edits", "memories", "activity", "log", "cards", "card_prices"].map(t =>
  [t, (db.prepare(`SELECT COUNT(*) n FROM ${t}`).get() as { n: number }).n]));
db.exec("PRAGMA wal_checkpoint(TRUNCATE)");
db.close();
for (const ext of ["-wal", "-shm"]) rmSync(OUT + ext, { force: true });
writeFileSync(join(dirname(OUT), "seed.json"), JSON.stringify({ users: { owner: 1, empty: sam, sample: jordan }, decks: { najeela, kinnan, caesar, bombadil, stickers, rograkh, sample: demo.deckId }, handles: { owner: "rowan", empty: "sam", sample: "jordan" } }, null, 2) + "\n");
console.log(JSON.stringify({ out: OUT, counts }));
