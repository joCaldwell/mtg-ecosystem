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

test("compact help retains its named, keyboard-operable Learn more control", async (t) => {
  const { Explainer } = await (await modules(t)).ssrLoadModule("/src/ui/Explainer.tsx");
  const html = renderToStaticMarkup(createElement(Explainer, { title: "Deck lifecycle" }, "Lifecycle guidance"));
  assert.match(html, /<button[^>]*type="button"[^>]*aria-expanded="false"/);
  assert.match(html, /Learn more.*about deck lifecycle/);
  assert.doesNotMatch(html, /Lifecycle guidance/, "guidance is behind help until requested");
});

test("colour identity indicators expose colour names and colourless in their accessible labels", async (t) => {
  const { ColorIdentity } = await (await modules(t)).ssrLoadModule("/src/mtg/ColorIdentity.tsx");
  assert.match(renderToStaticMarkup(createElement(ColorIdentity, { colors: "UBR" })), /role="img" aria-label="Colour identity: blue, black, red"/);
  assert.match(renderToStaticMarkup(createElement(ColorIdentity, { colors: "" })), /role="img" aria-label="Colour identity: colourless"/);
});
