// Shared pages over HTTP (spec §18.5, §18.6, §11.1): link previews a plain
// unauthenticated request can read, the D16 static fallback, and what a
// hosted visitor without a session may and may not reach. Sessions are
// mocked through resolveUiActor; the hosted session resolver itself is
// covered in hosted-auth.test.ts.
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { AddressInfo } from "node:net";
import { test } from "node:test";
import assert from "node:assert/strict";
import type { Actor } from "../src/actor.ts";
import { createApp } from "../src/app.ts";
import { remember } from "../src/memory.ts";
import { setVisibility } from "../src/sharing.ts";
import { setPowerTarget } from "../src/deck/power.ts";
import { OWNER, agent } from "./actors.ts";
import { add, newDeck, testDb } from "./helpers.ts";

const ART = "https://cards.scryfall.io/art_crop/front/0/0/teferi.jpg";

async function served(mode: "local" | "hosted") {
  const db = testDb();
  db.exec("INSERT INTO users(id,handle,display_name) VALUES (2,'second','Second')");
  db.prepare("UPDATE cards SET image_art_crop = ?, artist = 'Test Artist' WHERE oracle_id = 'id-teferi'").run(ART);
  const id = newDeck(db, "paper", "Teferi <control>");
  add(db, id, "id-teferi", { role: "commander" });
  add(db, id, "id-counterspell");
  setPowerTarget(db, OWNER, id, { profile: { intent: "social" }, game_changers_max: 0 });
  remember(db, agent("test-agent"), { scope: "deck", deck_id: id, text: "Private deck preference" });
  const hidden = newDeck(db, "paper", "Hidden list");
  const web = mkdtempSync(join(tmpdir(), "shared-web-"));
  mkdirSync(join(web, "assets"));
  writeFileSync(join(web, "index.html"), "<!doctype html><html><head><meta charset=\"UTF-8\" /><title>MTG Deck Builder</title></head><body><div id=\"root\"></div></body></html>");
  writeFileSync(join(web, "assets", "app.js"), "export {}");
  let principal: Actor | null = mode === "local" ? OWNER : null;
  const saved = process.env.DECKBUILDER_PUBLIC_ORIGIN;
  const app = createApp(db, { mode, webDist: web, resolveUiActor: () => principal });
  await new Promise<void>((r) => app.server.listen(0, "127.0.0.1", r));
  const base = `http://127.0.0.1:${(app.server.address() as AddressInfo).port}`;
  if (mode === "hosted") process.env.DECKBUILDER_PUBLIC_ORIGIN = base;
  return {
    db, id, hidden, base,
    as(actor: Actor | null) { principal = actor; },
    async close() {
      await app.close(); db.close(); rmSync(web, { recursive: true, force: true });
      if (saved === undefined) delete process.env.DECKBUILDER_PUBLIC_ORIGIN; else process.env.DECKBUILDER_PUBLIC_ORIGIN = saved;
    },
  };
}

const meta = (html: string, key: string) => html.match(new RegExp(`<meta (?:name|property)="${key}" content="([^"]*)" />`))?.[1];

test("an unauthenticated request for /d/<slug> gets HTML with the deck's Open Graph tags; a private slug is 404", async () => {
  const s = await served("hosted");
  try {
    const { slug } = setVisibility(s.db, OWNER, s.id, "public");
    const res = await fetch(`${s.base}/d/${slug}`);
    assert.equal(res.status, 200);
    assert.match(res.headers.get("content-type")!, /^text\/html/);
    const html = await res.text();
    assert.match(html, /<title>Teferi &lt;control&gt; · owner<\/title>/, "names are escaped");
    assert.equal(meta(html, "og:title"), "Teferi &lt;control&gt; · owner");
    assert.equal(meta(html, "og:image"), ART, "the commander's stored art crop, never a Scryfall API lookup");
    assert.equal(meta(html, "og:url"), `${s.base}/d/${slug}`);
    assert.equal(meta(html, "og:type"), "website");
    assert.equal(meta(html, "twitter:card"), "summary_large_image");
    assert.equal(meta(html, "description"), "Teferi, Temporal Archmage · Blue · Bracket 2 (provisional) · Core target · Commander deck by owner · Art by Test Artist");
    assert.equal(meta(html, "robots"), undefined, "public pages may be indexed");
    assert.ok(!html.includes("Private deck preference"));
    assert.equal(html.match(/<title>/g)!.length, 1, "the page keeps one title");

    setVisibility(s.db, OWNER, s.id, "unlisted");
    assert.equal(meta(await (await fetch(`${s.base}/d/${slug}`)).text(), "robots"), "noindex", "unlisted pages are not indexed");

    setVisibility(s.db, OWNER, s.id, "private");
    const hidden = await fetch(`${s.base}/d/${slug}`);
    assert.equal(hidden.status, 404);
    const body = await hidden.text();
    assert.ok(!body.includes("Teferi"), "a private slug says nothing about the deck");
    assert.equal(meta(body, "robots"), "noindex");
    assert.equal((await fetch(`${s.base}/api/public/decks/${slug}`)).status, 404);
    s.as({ user_id: 2, via: "ui", label: "ui" });
    assert.equal((await fetch(`${s.base}/d/${slug}`)).status, 404, "another user too");
    s.as(OWNER);
    assert.equal((await fetch(`${s.base}/d/${slug}`)).status, 200, "the owner previews their own private page");
    assert.equal((await fetch(`${s.base}/d/NoSuchSlug`)).status, 404);
  } finally { await s.close(); }
});

test("a deck name with replacement patterns ($' $& $`) stays inside its escaped tags", async () => {
  const s = await served("hosted");
  try {
    s.db.prepare("UPDATE decks SET name = ? WHERE id = ?").run("A$'B$&C$`D", s.id);
    const { slug } = setVisibility(s.db, OWNER, s.id, "public");
    const html = await (await fetch(`${s.base}/d/${slug}`)).text();
    assert.equal(meta(html, "og:title"), "A$&#39;B$&amp;C$`D · owner");
    assert.equal(html.match(/<div id="root">/g)!.length, 1, "the rest of the page is not spliced into the head");
    assert.match(html, /<title>A\$&#39;B\$&amp;C\$`D · owner<\/title>\n/);
  } finally { await s.close(); }
});

test("anonymous search has its own lower budget per IP, and an oversized search is a 400", async () => {
  const s = await served("hosted");
  try {
    const big = await fetch(`${s.base}/api/search?q=${encodeURIComponent(Array.from({ length: 40 }, (_, i) => `o:a${i}`).join(" "))}`);
    assert.equal(big.status, 400, "30 terms at most without an account");
    assert.match((await big.json()).error, /the most is 30/);
    let last = 200, n = 1;
    for (; n <= 31 && last !== 429; n++) last = (await fetch(`${s.base}/api/search?q=t:land`)).status;
    assert.equal(last, 429);
    assert.equal(n - 1, 30, "the 30th anonymous search in a minute is refused (the oversized one counted)");
  } finally { await s.close(); }
});

test("no art and no large card without an artist credit (review S6)", async () => {
  const s = await served("hosted");
  try {
    s.db.prepare("UPDATE cards SET artist = NULL WHERE oracle_id = 'id-teferi'").run();
    const { slug } = setVisibility(s.db, OWNER, s.id, "public");
    const html = await (await fetch(`${s.base}/d/${slug}`)).text();
    assert.equal(meta(html, "og:image"), undefined);
    assert.equal(meta(html, "twitter:card"), "summary");
    assert.doesNotMatch(meta(html, "description")!, /Art by/);
  } finally { await s.close(); }
});

test("HEAD reads a public page and route like GET, with no body; log ids are not mappable (review nits)", async () => {
  const s = await served("hosted");
  try {
    const { slug } = setVisibility(s.db, OWNER, s.id, "public");
    for (const path of [`/d/${slug}`, `/api/public/decks/${slug}`]) {
      const head = await fetch(s.base + path, { method: "HEAD" });
      assert.equal(head.status, 200, path);
      assert.equal(await head.text(), "");
    }
    assert.equal((await fetch(`${s.base}/api/decks`, { method: "HEAD" })).status, 401, "HEAD opens nothing GET does not");
    s.as({ user_id: 2, via: "ui", label: "ui" });
    const other = await fetch(`${s.base}/api/decks/${s.id}/log/1`, { method: "PATCH", headers: { "Content-Type": "application/json", Origin: s.base }, body: JSON.stringify({ hidden: true }) });
    const missing = await fetch(`${s.base}/api/decks/${s.id}/log/999999`, { method: "PATCH", headers: { "Content-Type": "application/json", Origin: s.base }, body: JSON.stringify({ hidden: true }) });
    assert.equal(other.status, 404);
    assert.equal(missing.status, 404);
    assert.equal((await other.json()).error, (await missing.json()).error, "the same answer either way");
  } finally { await s.close(); }
});

test("imports, preview and apply, have their own 20-a-minute budget per account (review round 2)", async () => {
  const s = await served("hosted");
  try {
    s.as(OWNER);
    const post = (path: string) => fetch(`${s.base}/api/decks/${s.id}/${path}`, { method: "POST", headers: { "Content-Type": "application/json", Origin: s.base }, body: JSON.stringify({ text: "1 Forest" }) });
    const statuses: number[] = [];
    for (let i = 0; i < 21; i++) statuses.push((await post("import/preview")).status);
    assert.deepEqual(statuses.slice(0, 20), Array(20).fill(200));
    assert.equal(statuses[20], 429);
    assert.equal((await post("import")).status, 429, "apply shares the budget");
    assert.equal((await fetch(`${s.base}/api/decks/import/preview`, { method: "POST", headers: { "Content-Type": "application/json", Origin: s.base }, body: JSON.stringify({ text: "1 Forest" }) })).status, 429, "new-deck preview shares the same budget");
    assert.equal((await fetch(`${s.base}/api/decks/${s.id}`)).status, 200, "the rest of the app is unaffected");
  } finally { await s.close(); }
});

test("/u/<handle> lists public decks only, with profile tags", async () => {
  const s = await served("hosted");
  try {
    setVisibility(s.db, OWNER, s.id, "public");
    setVisibility(s.db, OWNER, s.hidden, "unlisted");
    const html = await (await fetch(`${s.base}/u/owner`)).text();
    assert.equal(meta(html, "og:type"), "profile");
    assert.equal(meta(html, "description"), "1 public Commander deck by owner.");
    const profile = await (await fetch(`${s.base}/api/public/users/owner`)).json();
    assert.deepEqual(profile.profile.decks.map((d: any) => d.name), ["Teferi <control>"]);
    assert.deepEqual(Object.keys(profile.profile).sort(), ["decks", "display_name", "handle"]);
    assert.equal((await fetch(`${s.base}/u/nobody-here`)).status, 404);
  } finally { await s.close(); }
});

test("hosted without a session: shared reads, export and card search work; everything else is 401", async () => {
  const s = await served("hosted");
  try {
    const { slug } = setVisibility(s.db, OWNER, s.id, "unlisted");
    const shared = await fetch(`${s.base}/api/public/decks/${slug}`);
    assert.equal(shared.status, 200);
    const { deck } = await shared.json();
    assert.equal(deck.deck.owner.handle, "owner");
    assert.ok(!JSON.stringify(deck).includes("Private deck preference"));
    const exported = await (await fetch(`${s.base}/api/public/decks/${slug}/export`)).json();
    assert.equal(exported.text, "1 Teferi, Temporal Archmage [Commander]\n1 Counterspell [Unslotted]\n");
    assert.equal((await fetch(`${s.base}/api/search?q=t:creature`)).status, 200);
    assert.equal((await fetch(`${s.base}/api/resolve?name=Forest`)).status, 200);
    assert.equal((await fetch(`${s.base}/api/cards/id-forest/table`)).status, 200);
    assert.equal((await fetch(`${s.base}/api/search?q=t:creature&deck=${s.id}`)).status, 404, "a deck context is the owner's");
    for (const path of ["/api/decks", `/api/decks/${s.id}`, "/api/inbox", "/api/events", "/api/settings"])
      assert.equal((await fetch(s.base + path)).status, 401, path);
    const fork = await fetch(`${s.base}/api/public/decks/${slug}/fork`, { method: "POST", headers: { "Content-Type": "application/json", Origin: s.base }, body: JSON.stringify({ name: "Anonymous fork" }) });
    assert.equal(fork.status, 401, "forking needs an account");
    s.as({ user_id: 2, via: "ui", label: "ui" });
    const forked = await fetch(`${s.base}/api/public/decks/${slug}/fork`, { method: "POST", headers: { "Content-Type": "application/json", Origin: s.base }, body: JSON.stringify({ name: "Second's fork" }) });
    assert.equal(forked.status, 200);
    const state = (await forked.json()).state;
    assert.deepEqual(state.deck.upstream, { owner_handle: "owner", slug, name: "Teferi <control>" });
    assert.equal(state.memory_budget.used, 0, "no memories came with it");
  } finally { await s.close(); }
});

test("D16: index.html for app routes only; API, MCP, well-known, assets and unknown extensions are 404", async () => {
  const s = await served("local");
  try {
    for (const path of ["/", "/decks", `/deck/${s.id}`, `/deck/${s.id}/review`, `/deck/${s.id}/playtest`, "/inbox", "/memories", "/profile", "/activity", "/settings", "/design", "/signin"]) {
      const res = await fetch(s.base + path);
      assert.equal(res.status, 200, path);
      assert.match(await res.text(), /<div id="root">/, path);
    }
    for (const path of ["/api/nothing", "/mcpx", "/.well-known/anything", "/assets/missing.js", "/deck.php", "/favicon.xyz", "/deck/1/other", "/nowhere"]) {
      const res = await fetch(s.base + path);
      assert.equal(res.status, 404, path);
      assert.doesNotMatch(await res.text(), /<div id="root">/, path);
    }
    assert.equal((await fetch(`${s.base}/assets/app.js`)).status, 200);
  } finally { await s.close(); }
});


test("new-deck import previews require a session, write nothing, and retain parser limits", async () => {
  const s = await served("hosted");
  try {
    const post = (text: string, path = "/api/decks/import/preview") => fetch(s.base + path, {
      method: "POST", headers: { "Content-Type": "application/json", Origin: s.base }, body: JSON.stringify({ text }),
    });
    assert.equal((await post("1 Forest")).status, 401);
    s.as(OWNER);
    const before = s.db.prepare("SELECT total_changes() AS n").get();
    const preview = await post("Commander\n1 Teferi, Temporal Archmage\nMainboard\n2 Forest\nSideboard\n1 Counterspell");
    assert.equal(preview.status, 200);
    const diff = await preview.json();
    assert.equal(diff.add_count, 3);
    assert.equal(diff.cut_count, 0);
    assert.equal(diff.adds.find((c: any) => c.oracle_id === "id-teferi").role, "commander");
    assert.equal(diff.set_aside[0].oracle_id, "id-counterspell");
    assert.equal(diff.revision, 0);
    assert.deepEqual(s.db.prepare("SELECT total_changes() AS n").get(), before, "preview writes no deck, log, activity or focus");
    const created = await fetch(s.base + "/api/decks", { method: "POST", headers: { "Content-Type": "application/json", Origin: s.base }, body: JSON.stringify({ name: "Preview parity", category: "paper" }) });
    assert.equal(created.status, 200);
    const id = (await created.json()).state.deck.id;
    const text = "1 Forest\n1 Counterspell [Maybeboard]";
    assert.deepEqual(await (await post(text)).json(), await (await post(text, `/api/decks/${id}/import/preview`)).json());
    assert.equal((await post(Array(5002).fill("1 Forest").join("\n"))).status, 400);
    const longLine = await post("1 " + "x".repeat(301));
    assert.equal(longLine.status, 200);
    const rejected = await longLine.json();
    assert.equal(rejected.unparsed.length, 1, "oversized lines are reported, never parsed as cards");
    assert.equal(rejected.add_count, 0);
    assert.equal(rejected.unresolved.length, 0);
  } finally { await s.close(); }
});
