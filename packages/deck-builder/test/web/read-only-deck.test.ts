// ReadOnlyDeck's read-only guarantee is structural (review nit): it renders a
// real shared deck with no DeckProvider above it, so no editing store, run()
// or mutation exists to call. Rendered through the package's Vite pipeline.
import { test } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "vite";
import { renderToStaticMarkup } from "react-dom/server";
import { createElement } from "react";
import { OWNER } from "../actors.ts";
import { setVisibility } from "../../src/sharing.ts";
import { publicDeckState } from "../../src/state.ts";
import { add, newDeck, testDb } from "../helpers.ts";

test("ReadOnlyDeck renders a shared deck without a DeckProvider, and offers no edits", async (t) => {
  // DeckUiProvider reads the stored grouping; the server render has no storage.
  const stored = new Map<string, string>();
  (globalThis as any).localStorage ??= { getItem: (k: string) => stored.get(k) ?? null, setItem: (k: string, v: string) => stored.set(k, v) };
  const vite = await createServer({ server: { middlewareMode: true }, appType: "custom" });
  t.after(() => vite.close());
  const db = testDb();
  t.after(() => db.close());
  const deck = newDeck(db, "paper", "Shared Teferi");
  add(db, deck, "id-teferi", { role: "commander" });
  add(db, deck, "id-counterspell");
  const { slug } = setVisibility(db, OWNER, deck, "public");
  const { ReadOnlyDeck } = await vite.ssrLoadModule("/src/features/deck/ReadOnlyDeck.tsx");
  const { fromPublic } = await vite.ssrLoadModule("/src/domain/readOnlyDeck.ts");
  const html = renderToStaticMarkup(createElement(ReadOnlyDeck, { model: fromPublic(publicDeckState(db, null, slug!)) }));
  assert.ok(html.includes("Teferi, Temporal Archmage") && html.includes("Counterspell"));
  assert.ok(!html.includes("Ctrl-click"), "no selection in a read-only list");
  assert.ok(!html.includes("Memory linked") && !html.includes("Decided before"), "a viewer gets no owner-workflow marker legend");
  assert.ok(!/>Edit</.test(html), "no Edit buttons on the brief or engines");
  assert.ok(!html.includes("Edit slots"));

  const phone = renderToStaticMarkup(createElement(ReadOnlyDeck, {
    model: fromPublic(publicDeckState(db, null, slug!)),
    header: createElement("a", { href: "#deck-list" }, "Read deck · 2 cards"),
    decisions: () => createElement("section", { id: "deck-proposals" }, "Pending decisions"),
    listNavigation: createElement("a", { href: "#deck-proposals" }, "Review proposals"),
  }));
  assert.match(phone, /href="#deck-list">Read deck · 2 cards/);
  assert.match(phone, /<section id="deck-list" tabindex="-1"[^>]*aria-label="Deck list"/);
  assert.ok(phone.indexOf('id="deck-proposals"') < phone.indexOf('id="deck-list"'));
  assert.ok(phone.indexOf('id="deck-list"') < phone.indexOf('href="#deck-proposals"'));
  assert.ok(phone.indexOf('href="#deck-proposals"') < phone.indexOf("Commander"));
});
