import { test } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "vite";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { applyImportDraft } from "../../web/src/features/import/workflow.ts";
import { hasImportChanges, importName } from "../../web/src/domain/import.ts";
import type { api, ImportDiff } from "../../web/src/api/client.ts";
import { deckState } from "./fixtures.ts";

function diff(over: Partial<ImportDiff> = {}): ImportDiff {
  return { adds: [], cuts: [], quantity_changes: [], role_changes: [], add_count: 0, cut_count: 0, unchanged: 0,
    unresolved: [], ambiguous: [], set_aside: [], unparsed: [], ignored: [], notes: [], revision: 0, ...over };
}

test("new imports infer a name only from resolved commanders; an explicit name wins", () => {
  const preview = diff({ adds: [
    { oracle_id: "a", name: "Test commander A", quantity: 1, slot_id: null, slot_name: null, category: null, role: "commander" },
    { oracle_id: "b", name: "Test card", quantity: 1, slot_id: null, slot_name: null, category: null, role: "card" },
    { oracle_id: "c", name: "Test commander B", quantity: 1, slot_id: null, slot_name: null, category: null, role: "commander" },
  ] });
  assert.equal(importName("  My deck  ", preview), "My deck");
  assert.equal(importName("  ", preview), "Test commander A + Test commander B");
  assert.equal(importName("", diff()), "Imported deck");
});

test("apply creates once, then imports; a failed import retains its id for retry", async () => {
  const calls: unknown[] = [];
  const state = deckState(); state.deck.id = 42;
  let fail = true;
  const result = { state, import: { applied: { added: 1, cut: 0, quantity_changed: 0, role_changed: 0 }, log_id: 7, diff: diff() } };
  const client: Pick<typeof api, "createDeck" | "importList"> = {
    createDeck: async (name, category) => { calls.push(["create", name, category]); return { state }; },
    importList: async (id, text, note) => { calls.push(["import", id, text, note]); if (fail) throw new Error("Import failed"); return result; },
  };
  let created: number | null = null;
  const draft = { deckId: null, name: "Test", category: "paper" as const, paste: "source text", note: "  My note  " };
  await assert.rejects(() => applyImportDraft(client, draft, id => { created = id; }), /Import failed/);
  assert.equal(created, 42);
  fail = false;
  assert.equal(await applyImportDraft(client, { ...draft, deckId: created }, () => assert.fail("must not create on retry")), result);
  assert.deepEqual(calls, [["create", "Test", "paper"], ["import", 42, "source text", "My note"], ["import", 42, "source text", "My note"]]);
});

test("a failed create never attempts import", async () => {
  const client: Pick<typeof api, "createDeck" | "importList"> = {
    createDeck: async () => { throw new Error("Create failed"); },
    importList: async () => assert.fail("must not import"),
  };
  await assert.rejects(() => applyImportDraft(client, { deckId: null, name: "Test", category: "theorycrafted", paste: "text", note: "" }, () => assert.fail("must not announce a deck")), /Create failed/);
});

test("maybe-only imports remain actionable; identical and unresolved-only lists do not", () => {
  assert.equal(hasImportChanges(diff()), false);
  assert.equal(hasImportChanges(diff({ set_aside: [{ oracle_id: "a", name: "Test card", category: "Sideboard" }] })), true);
  assert.equal(hasImportChanges(diff({ role_changes: [{ oracle_id: "a", name: "Test card", from: "card", to: "commander" }] })), true);
  assert.equal(hasImportChanges(diff({ unresolved: [{ line_no: 1, name: "Test unknown", suggestions: [] }] })), false);
});

test("preview pins totals and warnings and links problem count to omissions before any changes", async t => {
  const server = await createServer({ server: { middlewareMode: true, hmr: false }, appType: "custom", logLevel: "silent" });
  t.after(() => server.close());
  const { ImportPreview } = await server.ssrLoadModule("/src/features/import/ImportPreview.tsx");
  const preview = diff({
    cut_count: 2, add_count: 1, notes: ["Test warning"],
    cuts: [{ oracle_id: "c", name: "Test cut", quantity: 2, role: "card" }],
    adds: [{ oracle_id: "a", name: "Test add", quantity: 1, slot_id: null, slot_name: null, category: null, role: "card" }],
    unresolved: [{ line_no: 3, name: "Test unknown", suggestions: [] }],
    set_aside: [{ oracle_id: "m", name: "Test maybe", category: "Sideboard" }],
  });
  const html = renderToStaticMarkup(createElement(ImportPreview, { diff: preview }));
  const changes = html.indexOf('aria-label="Import changes"');
  assert.ok(changes > html.indexOf('role="status"'));
  assert.ok(changes > html.indexOf("Test warning"));
  assert.ok(changes > html.indexOf("problem line"));
  assert.ok(changes > html.indexOf("Removes"));
  const target = html.match(/<a[^>]*href="#([^"]+)"[^>]*>(?:(?!<\/a>).)*?problem line<\/a>/s)?.[1];
  assert.ok(target, "the problem count is a fragment link");
  const region = html.match(new RegExp(`id="${target}" role="region" aria-labelledby="([^"]+)" tabindex="0"`))?.[1];
  assert.ok(region, "it targets the focusable warning callout");
  for (const id of region.split(" ")) assert.ok(html.includes(`id="${id}"`), "the callout is named by its own headings");
  assert.ok(html.indexOf("Not imported") < html.indexOf("Test warning"), "skipped lines and checks share one callout, skipped lines first");
  assert.ok(html.indexOf("Test warning") < changes);
  assert.ok(html.indexOf("Not imported") < changes, "omission warnings remain outside and before the scrolling change list");
  assert.ok(html.indexOf("Test unknown") < changes, "the warning details stay outside it too");
  assert.ok(changes < html.indexOf("Will be cut"), "card changes are inside the scroll region");
  assert.ok(html.indexOf("Not imported") < html.indexOf("Will be cut"));
  assert.ok(html.indexOf("Will be cut") < html.indexOf("Will be added"));
  assert.ok(html.indexOf("Not imported") < html.indexOf("Set aside to the maybe list"));
  assert.ok(!html.includes("textarea"));
});


test("an empty target hides zero counts and starts the add and maybe lists closed", async t => {
  const server = await createServer({ server: { middlewareMode: true, hmr: false }, appType: "custom", logLevel: "silent" });
  t.after(() => server.close());
  const { ImportPreview } = await server.ssrLoadModule("/src/features/import/ImportPreview.tsx");
  const adds = { add_count: 1, adds: [{ oracle_id: "a", name: "Test add", quantity: 1, slot_id: null, slot_name: null, category: null, role: "card" as const }],
    set_aside: [{ oracle_id: "m", name: "Test maybe", category: "Sideboard" }], unresolved: [{ line_no: 3, name: "Test unknown", suggestions: [] }] };
  const empty = renderToStaticMarkup(createElement(ImportPreview, { diff: diff(adds) }));
  assert.ok(!empty.includes("to cut") && !empty.includes("unchanged"), "zero-count segments are hidden");
  assert.match(empty, /to add/);
  assert.ok(!/<details[^>]*open/.test(empty), "adds and set-aside start closed behind their counts");
  assert.match(empty, /Will be added · <span[^>]*>1<\/span>/);
  assert.ok(empty.indexOf("Not imported") < empty.indexOf('aria-label="Import changes"'), "skipped lines stay open above the list");
  const existing = renderToStaticMarkup(createElement(ImportPreview, { diff: diff({ ...adds, unchanged: 4 }) }));
  assert.match(existing, /to cut/);
  assert.match(existing, /unchanged/);
  assert.match(existing, /<details[^>]*open/);
});

test("All decks offers Import deck beside New deck before any deck is opened", async t => {
  const server = await createServer({ server: { middlewareMode: true, hmr: false }, appType: "custom", logLevel: "silent" });
  t.after(() => server.close());
  const { DecksPage } = await server.ssrLoadModule("/src/features/decks/DecksPage.tsx");
  const html = renderToStaticMarkup(createElement(DecksPage));
  assert.match(html, />New deck<\/button>/);
  assert.match(html, />Import deck<\/button>/);
  assert.ok(html.indexOf(">New deck<") < html.indexOf(">Import deck<"));
  assert.ok(!html.includes("Create and import"));
});
