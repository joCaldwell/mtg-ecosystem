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

test("coloured mana pips draw their letter but keep the spoken colour as their only name", async (t) => {
  const { ManaSymbol } = await (await modules(t)).ssrLoadModule("/src/mtg/ManaSymbol.tsx");
  const pip = (symbol: string) => renderToStaticMarkup(createElement(ManaSymbol, { symbol }));
  for (const [letter, name] of [["W", "white"], ["U", "blue"], ["B", "black"], ["R", "red"], ["G", "green"], ["C", "colourless"]]) {
    assert.match(pip(letter), new RegExp(`^<span[^>]*role="img" aria-label="${name}"[^>]*>${letter}</span>$`));
  }
  const hybrid = pip("B/G");
  assert.match(hybrid, /aria-label="black or green"/);
  assert.match(hybrid, />B<\/span><span[^>]*>G<\/span><\/span>$/, "each half carries its letter");
  assert.match(pip("2/W"), />2<\/span><span[^>]*>W<\/span><\/span>$/);
  assert.match(pip("U/P"), />P<\/span>$/, "Phyrexian keeps its P");
});

test("the lifecycle pill is a menu button named by its state", async (t) => {
  const { Menu } = await (await modules(t)).ssrLoadModule("/src/ui/Menu.tsx");
  const items = ["Paper", "Theorycrafted", "Archived"].map((v) => ({ id: v, label: v, checked: v === "Paper", onSelect: () => undefined }));
  const html = renderToStaticMarkup(createElement(Menu, { pill: true, label: "Paper", ariaLabel: "Lifecycle: Paper", items }));
  assert.match(html, /^<button[^>]*aria-haspopup="menu"[^>]*aria-expanded="false"[^>]*aria-label="Lifecycle: Paper"/);
  assert.doesNotMatch(html, /aria-pressed|role="group"/, "a state, not a segmented filter");
});

test("a decision reveals all its rows when they fit together, else its first item, skipping hidden contributors", async (t) => {
  const { scrollToReviewCard } = await (await modules(t)).ssrLoadModule("/src/features/deck/list/DeckList.tsx");
  const calls: unknown[] = [];
  const nearest = { block: "nearest", inline: "nearest" };
  const row = (id: string, visible: boolean, top: number, label: string) => ({
    dataset: { oracleId: id }, getClientRects: () => visible ? [{}] : [],
    getBoundingClientRect: () => ({ top }),
    scrollIntoView: (options: unknown) => calls.push([label, options]),
  });
  // A swap: the add is drawn high in one group, its cut lower in another.
  const rows = [row("cut", true, 900, "cut in a later group"), row("add", false, 0, "hidden add"), row("add", true, 100, "first changed card")];
  scrollToReviewCard({ querySelectorAll: () => rows }, "add");
  assert.deepEqual(calls, [["cut in a later group", nearest], ["first changed card", nearest]],
    "lowest then highest: both end in view when they fit, the highest when they do not");
  calls.length = 0;
  // Named item below the topmost row: it is revealed last so it stays in view.
  scrollToReviewCard({ querySelectorAll: () => [row("cut", true, 100, "cut above"), row("add", true, 900, "named add")] }, "add");
  assert.deepEqual(calls, [["named add", nearest], ["cut above", nearest], ["named add", nearest]]);
  calls.length = 0;
  scrollToReviewCard({ querySelectorAll: () => [row("add", true, 100, "only row")] }, "add");
  assert.deepEqual(calls, [["only row", nearest]]);
  calls.length = 0;
  scrollToReviewCard(null, "add");
  scrollToReviewCard({ querySelectorAll: () => [row("add", false, 0, "hidden")] }, "add");
  assert.deepEqual(calls, []);
});

test("stepping through deck matches centres an off-screen match, smoothly unless motion is reduced", async (t) => {
  const { scrollToFindCurrent } = await (await modules(t)).ssrLoadModule("/src/features/deck/list/DeckList.tsx");
  const calls: unknown[] = [];
  const row = (visible: boolean, top: number, name: string) => ({
    getClientRects: () => visible ? [{}] : [],
    getBoundingClientRect: () => ({ top, bottom: top + 24 }),
    scrollIntoView: (options: unknown) => calls.push([name, options]),
  });
  const list = (current: ReturnType<typeof row>) => ({
    scrollHeight: 3000, clientHeight: 800, getBoundingClientRect: () => ({ top: 100, bottom: 900 }),
    querySelectorAll: (selector: string) => {
      assert.equal(selector, '[data-find-current="true"]');
      return [row(false, 0, "closed secondary contributor"), current];
    },
  });
  scrollToFindCurrent(list(row(true, 1200, "below the fold")), false);
  scrollToFindCurrent(list(row(true, 40, "above the list")), true);
  scrollToFindCurrent(list(row(true, 400, "already in view")), false);
  scrollToFindCurrent(null, false);
  assert.deepEqual(calls, [
    ["below the fold", { block: "center", inline: "nearest", behavior: "smooth" }],
    ["above the list", { block: "center", inline: "nearest", behavior: "auto" }],
  ]);
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
