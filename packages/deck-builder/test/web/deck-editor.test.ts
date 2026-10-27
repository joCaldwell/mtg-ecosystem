import { test } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "vite";
import { renderToStaticMarkup } from "react-dom/server";
import { createElement } from "react";

async function modules(t: { after: (fn: () => unknown) => void }) {
  const server = await createServer({ server: { middlewareMode: true, hmr: false }, appType: "custom", logLevel: "silent" });
  t.after(() => server.close());
  return server;
}

test("search scrolls the first rendered match without opening contributor disclosures", async (t) => {
  const { scrollToFirstMatch } = await (await modules(t)).ssrLoadModule("/src/features/deck/list/DeckList.tsx");
  const calls: unknown[] = [];
  const row = (visible: boolean, name: string) => ({
    getClientRects: () => visible ? [{}] : [],
    scrollIntoView: (options: unknown) => calls.push([name, options]),
  });
  const rows = [row(false, "closed secondary contributor"), row(true, "first match"), row(true, "later match")];
  const list = { querySelectorAll: (selector: string) => {
    assert.equal(selector, '[data-filter-match="true"]');
    return rows;
  } };
  scrollToFirstMatch(list);
  assert.deepEqual(calls, [["first match", { block: "nearest", inline: "nearest" }]]);
  calls.length = 0;
  scrollToFirstMatch(null);
  scrollToFirstMatch({ querySelectorAll: () => [] });
  scrollToFirstMatch({ querySelectorAll: () => [row(false, "hidden")] });
  assert.deepEqual(calls, [], "empty queries and closed disclosures do not move the page");
});

test("concept help uses a specific, keyboard-operable question label", async (t) => {
  const { Explainer } = await (await modules(t)).ssrLoadModule("/src/ui/Explainer.tsx");
  const html = renderToStaticMarkup(createElement(Explainer, { title: "Deck lifecycle", label: "Who can edit?" }, "Lifecycle guidance"));
  assert.match(html, /<button[^>]*type="button"[^>]*aria-expanded="false"/);
  assert.match(html, /Who can edit?/);
  assert.doesNotMatch(html, /Lifecycle guidance/, "guidance is behind help until requested");
});

test("colour identity indicators expose colour names and colourless in their accessible labels", async (t) => {
  const { ColorIdentity } = await (await modules(t)).ssrLoadModule("/src/mtg/ColorIdentity.tsx");
  assert.match(renderToStaticMarkup(createElement(ColorIdentity, { colors: "UBR" })), /role="img" aria-label="Colour identity: blue, black, red"/);
  assert.match(renderToStaticMarkup(createElement(ColorIdentity, { colors: "" })), /role="img" aria-label="Colour identity: colourless"/);
});

test("review arrival reveals the first item of the chosen decision, skipping hidden contributors", async (t) => {
  const { scrollToReviewCard } = await (await modules(t)).ssrLoadModule("/src/features/deck/list/DeckList.tsx");
  const calls: unknown[] = [];
  const row = (id: string, visible: boolean, label: string) => ({
    dataset: { oracleId: id }, getClientRects: () => visible ? [{}] : [],
    scrollIntoView: (options: unknown) => calls.push([label, options]),
  });
  const rows = [row("cut", true, "cut later in decision"), row("add", false, "hidden add"), row("add", true, "first changed card")];
  scrollToReviewCard({ querySelectorAll: () => rows }, "add");
  assert.deepEqual(calls, [["first changed card", { block: "nearest", inline: "nearest" }]]);
  calls.length = 0;
  scrollToReviewCard(null, "add");
  assert.deepEqual(calls, []);
});

test("proposal arrival focuses its own reason and scrolls only its panel, preserving the list reveal", async (t) => {
  const { focusReviewArrival } = await (await modules(t)).ssrLoadModule("/src/features/deck/reviewArrival.ts");
  const calls: unknown[] = [];
  const pane = { scrollTop: 100, clientHeight: 500, getBoundingClientRect: () => ({ top: 200, bottom: 700 }) };
  const target = {
    dataset: { reviewUnit: "chosen" },
    closest: (selector: string) => selector === '[data-panel-body]' ? pane : { getBoundingClientRect: () => ({ top: 800 }) },
    getClientRects: () => [{}], getBoundingClientRect: () => ({ top: 900, bottom: 1200 }),
    querySelector: () => ({ focus: (options: unknown) => calls.push(options) }),
  };
  const root = { querySelectorAll: () => [{ dataset: { reviewUnit: "older" } }, target] };
  focusReviewArrival(root, "chosen");
  assert.equal(pane.scrollTop, 700, "the summary and decision fit together at the panel's top");
  assert.deepEqual(calls, [{ preventScroll: true }]);
  focusReviewArrival(root, "missing");
  assert.equal(calls.length, 1, "a missing proposal does not focus another decision");
});
