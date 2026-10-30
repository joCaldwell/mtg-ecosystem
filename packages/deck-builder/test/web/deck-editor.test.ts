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

test("filter results start at the top, and clearing returns to the unfiltered position", async (t) => {
  const { revealFilterResults, restoreScroll } = await (await modules(t)).ssrLoadModule("/src/features/deck/list/DeckList.tsx");
  const g = globalThis as any;
  const saved = { getComputedStyle: g.getComputedStyle, scrollY: g.scrollY, scrollTo: g.scrollTo };
  t.after(() => Object.assign(g, saved));
  const calls: unknown[] = [];
  g.scrollTo = (options: unknown) => calls.push(["window", options]);
  const list = (top: number, margin: string) => {
    g.getComputedStyle = () => ({ scrollMarginTop: margin });
    return { scrollTop: 900, getBoundingClientRect: () => ({ top }), scrollIntoView: (options: unknown) => calls.push(["list", options]) };
  };

  // Desktop: the list pane scrolls itself and sits below the toolbar.
  const desktop = list(180, "0px");
  revealFilterResults(desktop);
  assert.equal(desktop.scrollTop, 0);
  assert.deepEqual(calls, [], "the window is not moved");

  // Tablet: the window scrolls; the list's top went under the sticky filter row.
  const tablet = list(40, "120px");
  revealFilterResults(tablet);
  assert.deepEqual(calls, [["list", { block: "start", inline: "nearest" }]]);
  calls.length = 0;
  revealFilterResults(list(200, "120px"));
  assert.deepEqual(calls, [], "already below the filter row: nothing moves");
  revealFilterResults(null);

  g.scrollY = 0;
  const restored = list(180, "0px");
  restoreScroll(restored, { list: 1234, window: 0 });
  assert.equal(restored.scrollTop, 1234);
  assert.deepEqual(calls, [], "desktop: the window stays put");
  g.scrollY = 10;
  restoreScroll(list(40, "120px"), { list: 0, window: 2400 });
  assert.deepEqual(calls, [["window", { top: 2400, behavior: "instant" }]]);
  restoreScroll(null, { list: 0, window: 0 });
});

test("concept help uses a specific, keyboard-operable question label", async (t) => {
  const { Explainer } = await (await modules(t)).ssrLoadModule("/src/ui/Explainer.tsx");
  const html = renderToStaticMarkup(createElement(Explainer, { title: "Deck lifecycle", label: "Who can edit?" }, "Lifecycle guidance"));
  assert.match(html, /<button[^>]*type="button"[^>]*aria-expanded="false"/);
  assert.match(html, /Who can edit?/);
  assert.doesNotMatch(html, /Lifecycle guidance/, "guidance is behind help until requested");
});

test("an explanation opened from a menu item draws no trigger of its own", async (t) => {
  const { Explainer } = await (await modules(t)).ssrLoadModule("/src/ui/Explainer.tsx");
  const html = renderToStaticMarkup(createElement(Explainer, { title: "Deck lifecycle", control: { anchor: null, open: false, onClose: () => undefined } }, "Lifecycle guidance"));
  assert.equal(html, "");
});

test("a folded diff shows only the changes, each unchanged run one closed row", async (t) => {
  const { DiffView } = await (await modules(t)).ssrLoadModule("/src/ui/DiffView.tsx");
  const same = (text: string) => ({ kind: "same", text });
  const html = renderToStaticMarkup(createElement(DiffView, { foldSame: true, lines: [same("a"), same("b"), same("c"), same("d"), { kind: "add", text: "new" }, same("e")] }));
  assert.match(html, /<details[^>]*><summary[^>]*>4 unchanged lines<\/summary>/);
  assert.match(html, /<details[^>]*><summary[^>]*>1 unchanged line<\/summary>/);
  assert.doesNotMatch(html, /<details[^>]*\bopen/);
  assert.match(html, /aria-label="added"[^>]*>\+<\/span>new/);
  assert.equal((html.match(/<details/g) ?? []).length, 2);
});

test("colour identity indicators expose colour names and colourless in their accessible labels", async (t) => {
  const { ColorIdentity } = await (await modules(t)).ssrLoadModule("/src/mtg/ColorIdentity.tsx");
  assert.match(renderToStaticMarkup(createElement(ColorIdentity, { colors: "UBR" })), /role="img" aria-label="Colour identity: blue, black, red"/);
  assert.match(renderToStaticMarkup(createElement(ColorIdentity, { colors: "" })), /role="img" aria-label="Colour identity: colourless"/);
});

test("mana pips draw Scryfall's symbol silently inside one named image, with no letters in the fallback", async (t) => {
  const { ManaSymbol } = await (await modules(t)).ssrLoadModule("/src/mtg/ManaSymbol.tsx");
  const pip = (symbol: string) => renderToStaticMarkup(createElement(ManaSymbol, { symbol }));
  const image = (file: string) =>
    `<img class="[^"]*" src="https://svgs\\.scryfall\\.io/card-symbols/${file}\\.svg" alt="" width="14" height="14" loading="lazy" decoding="async" draggable="false"/>`;
  for (const [letter, name] of [["W", "white"], ["U", "blue"], ["B", "black"], ["R", "red"], ["G", "green"], ["C", "colourless"]]) {
    assert.match(pip(letter), new RegExp(`^<span[^>]*role="img" aria-label="${name}" title="${name}">${image(letter)}</span>$`), "no letter beside the image");
  }
  // Hybrid and Phyrexian symbols are Scryfall's combined drawings, not halves.
  assert.match(pip("B/G"), new RegExp(`aria-label="black or green"[^>]*>${image("BG")}</span>$`));
  assert.match(pip("2/W"), new RegExp(`aria-label="generic 2 or white"[^>]*>${image("2W")}</span>$`));
  assert.match(pip("U/P"), new RegExp(`aria-label="blue or 2 life"[^>]*>${image("UP")}</span>$`));
  assert.match(pip("G/U/P"), new RegExp(`aria-label="green or blue or 2 life"[^>]*>${image("GUP")}</span>$`));
  // A generic cost keeps its numeral for the fallback disc, hidden from assistive technology.
  const three = pip("3");
  assert.match(three, /^<span[^>]*role="img" aria-label="generic 3"[^>]*><svg[^>]*aria-hidden="true"[^>]*><text[^>]*>3<\/text><\/svg><img /);
  // A symbol Scryfall does not draw keeps the plain disc and requests nothing.
  assert.doesNotMatch(pip("PW"), /<img/);
});

test("the lifecycle pill is a menu button named by its state", async (t) => {
  const { Menu } = await (await modules(t)).ssrLoadModule("/src/ui/Menu.tsx");
  const items = ["Paper", "Theorycrafted", "Archived"].map((v) => ({ id: v, label: v, checked: v === "Paper", onSelect: () => undefined }));
  const html = renderToStaticMarkup(createElement(Menu, { pill: true, label: "Paper", ariaLabel: "Lifecycle: Paper", items }));
  assert.match(html, /^<button[^>]*aria-haspopup="menu"[^>]*aria-expanded="false"[^>]*aria-label="Lifecycle: Paper"/);
  assert.doesNotMatch(html, /aria-pressed|role="group"/, "a state, not a segmented filter");
});

test("a decision reveals all its rows with a row's clearance when they fit together, else its named item, centred when it must move", async (t) => {
  const { reviewRevealDelta, scrollToReviewCard } = await (await modules(t)).ssrLoadModule("/src/features/deck/list/DeckList.tsx");
  const view = { top: 200, bottom: 1000 };
  const at = (top: number) => ({ top, bottom: top + 24 });
  // Already clear of both edges by a row: nothing moves.
  assert.equal(reviewRevealDelta(view, [at(400)], undefined, 24), 0);
  // Flush with the bottom edge: no row of clearance below, so it is centred.
  assert.equal(reviewRevealDelta(view, [at(976)], undefined, 24), 988 - 600);
  // Just under the top edge: centred too.
  assert.equal(reviewRevealDelta(view, [at(210)], undefined, 24), 222 - 600);
  // A swap that fits together is centred as a whole.
  assert.equal(reviewRevealDelta(view, [at(900), at(1100)], at(1100), 24), (900 + 1124) / 2 - 600);
  // Too far apart to fit with clearance: the named card is centred instead.
  assert.equal(reviewRevealDelta(view, [at(100), at(1500)], at(1500), 24), 1512 - 600);
  // ... or, with no named card visible, the first.
  assert.equal(reviewRevealDelta(view, [at(1500), at(100)], undefined, 24), 112 - 600);

  const g = globalThis as any;
  const saved = { getComputedStyle: g.getComputedStyle, scrollBy: g.scrollBy, innerHeight: g.innerHeight };
  t.after(() => Object.assign(g, saved));
  const row = (id: string, visible: boolean, top: number) => ({
    dataset: { oracleId: id }, getClientRects: () => visible ? [{}] : [],
    getBoundingClientRect: () => ({ top, bottom: top + 24, height: 24 }),
  });
  // Desktop: the list pane scrolls itself; hidden contributors are skipped.
  g.getComputedStyle = () => ({ overflowY: "auto", scrollPaddingTop: "16px", scrollMarginTop: "0px" });
  const list = { scrollTop: 50, clientHeight: 800, getBoundingClientRect: () => ({ top: 184 }), querySelectorAll: () => [row("add", false, 0), row("add", true, 976)] };
  scrollToReviewCard(list, "add");
  // Its view runs from the scroll padding to the pane's client bottom (200-984).
  assert.equal(list.scrollTop, 50 + 988 - 592);
  // Tablet: the window scrolls, below the rows' sticky scroll margin.
  const calls: unknown[] = [];
  g.scrollBy = (options: unknown) => calls.push(options);
  g.innerHeight = 1000;
  g.getComputedStyle = () => ({ overflowY: "visible", scrollPaddingTop: "0px", scrollMarginTop: "200px" });
  scrollToReviewCard({ ...list, querySelectorAll: () => [row("add", true, 976)] }, "add");
  assert.deepEqual(calls, [{ top: 988 - 600, behavior: "instant" }]);
  calls.length = 0;
  scrollToReviewCard({ ...list, querySelectorAll: () => [row("add", true, 500)] }, "add");
  scrollToReviewCard(null, "add");
  scrollToReviewCard({ ...list, querySelectorAll: () => [row("add", false, 0)] }, "add");
  assert.deepEqual(calls, [], "clear, missing or hidden rows: nothing moves");
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

test("an active filter hides nonmatching rows, empty groups and maybe cards, and counts what is left", async (t) => {
  const stored = new Map<string, string>();
  (globalThis as any).localStorage ??= { getItem: (k: string) => stored.get(k) ?? null, setItem: (k: string, v: string) => stored.set(k, v) };
  const vite = await modules(t);
  const { ReadOnlyDeck } = await vite.ssrLoadModule("/src/features/deck/ReadOnlyDeck.tsx");
  const { fromOwner } = await vite.ssrLoadModule("/src/domain/readOnlyDeck.ts");
  const { layoutDeck } = await vite.ssrLoadModule("/src/domain/grouping.ts");
  const { matchDeckList } = await vite.ssrLoadModule("/src/domain/deckSearch.ts");
  const { card, deckState, slot } = await import("./fixtures.ts");
  const state = deckState({
    slots: [slot({ id: 10, name: "Test Tutors", target_min: 4, target_max: 6 }), slot({ id: 11, name: "Test Lands" })],
    cards: [
      card({ name: "Test Other Commander", role: "commander" }),
      card({ name: "Test Match Tutor", slot_id: 10, quantity: 2 }),
      card({ name: "Test Other Tutor", slot_id: 10 }),
      card({ name: "Test Other Land", slot_id: 11, type_line: "Land" }),
      card({ name: "Test Match Maybe", maybe: 1 }),
      card({ name: "Test Other Maybe", maybe: 1 }),
    ],
  });
  const render = (query: string) => {
    const layout = layoutDeck(state, "slot", "curve");
    const view = {
      layout, units: [], marks: new Map(),
      overlay: { cuts: new Set(), addsByGroup: new Map(), projected: new Map(), projectedTotal: 0 },
      matches: matchDeckList(layout, new Map(), query),
    };
    return renderToStaticMarkup(createElement(ReadOnlyDeck, { model: fromOwner(state), useView: () => view }));
  };
  const list = (html: string) => html.slice(html.indexOf("data-deck-list") >= 0 ? html.indexOf("data-deck-list") : html.indexOf('aria-label="Deck list"'));

  const unfiltered = render("");
  for (const name of ["Test Other Tutor", "Test Other Land", "Test Other Maybe", "Test Other Commander"]) assert.ok(unfiltered.includes(name));
  assert.doesNotMatch(unfiltered, /data-filter-match/);

  const html = list(render("match"));
  assert.ok(html.includes("Test Match Tutor") && html.includes("Test Match Maybe"));
  for (const name of ["Test Other Tutor", "Test Other Land", "Test Other Maybe", "Test Other Commander"]) {
    assert.ok(!html.includes(name), `${name} is hidden`);
  }
  assert.ok(!html.includes("Test Lands") && !html.includes(">Commander<"), "groups without matches are hidden");
  assert.match(html, /Test Tutors<\/span>.*?2 of 3<\/span>/, "the header counts matching copies of the group's");
  assert.ok(!html.includes("4–6"), "the group's target is not set against the matches");
  assert.match(html, /1 of 2 · outside the 100/);

  const maybeOnly = list(render("maybe"));
  assert.ok(!maybeOnly.includes("Test Tutors") && maybeOnly.includes("Test Match Maybe"), "only the maybe list remains");

  const none = list(render("zzz"));
  // The wording comes from the find box's own term (noMatchesLine, tested in
  // deckSearch.test.ts); this view is fed matches without one.
  assert.match(none, /<p[^>]*>No cards in this deck match [^<]*\.<\/p>/);
  assert.ok(!none.includes("Maybe list"));
});
