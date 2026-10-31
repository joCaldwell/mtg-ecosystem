#!/usr/bin/env node
// Captures every screen in its states, and the task flows, with hard metrics.
//
//   node capture.mjs [--port 8830] [--seed <UX_EVAL_ROOT>/seed/seed.db] [--dist <web/dist>]
//                    [--out <UX_EVAL_ROOT>/runs] [--only <regex over capture keys>]
//
// Writes <UX_EVAL_ROOT>/runs/<random-id>/ with screens/*.png, flows/<flow>/<step>.png,
// metrics.json and manifest.json. Capture keys never change between runs, so
// two runs pair screen by screen. run.json (private: commit, time, arguments)
// is for the operator only; packet.mjs never copies it.

import { UX_EVAL_ROOT } from "./lib/paths.mjs";
import { spawn, spawnSync } from "node:child_process";
import { randomBytes } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";
import { BROWSER_ARGS, VIEWPORTS, axe, newContext, pageMetrics, press, releaseHeld, scrollUntilVisible, settle, shoot, watchPage } from "./lib/browser.mjs";
import { FLOWS } from "./lib/flows.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
const PKG = resolve(HERE, "..");
const FROZEN_NOW = "2026-10-20T17:00:00.000Z"; // lib/determinism.ts FROZEN_NOW

function arg(name, fallback) {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 && process.argv[i + 1] ? process.argv[i + 1] : fallback;
}
const PORT = Number(arg("port", "8830"));
const ORIGIN = `http://127.0.0.1:${PORT}`;
const SEED = resolve(arg("seed", join(UX_EVAL_ROOT, "seed", "seed.db")));
const SEED_INFO = JSON.parse(readFileSync(join(dirname(SEED), "seed.json"), "utf8"));
const ONLY = arg("only") ? new RegExp(arg("only")) : null;
const RUN_ID = randomBytes(6).toString("hex");
const RUN = join(resolve(arg("out", join(UX_EVAL_ROOT, "runs"))), RUN_ID);
mkdirSync(join(RUN, "screens"), { recursive: true });
mkdirSync(join(RUN, "flows"), { recursive: true });

// ---------- build and server ----------

let DIST = arg("dist");
let buildDir;
if (!DIST) {
  buildDir = join(UX_EVAL_ROOT, "build", RUN_ID);
  const r = spawnSync(process.execPath, [join(HERE, "serve.ts"), "--build-only", "--build-dir", buildDir], { encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] });
  if (r.status !== 0) throw new Error("web build failed");
  DIST = r.stdout.trim().split("\n").at(-1);
} else buildDir = resolve(DIST, "..", "..");

let server = null;
async function startServer() {
  await stopServer();
  const child = spawn(process.execPath, [join(HERE, "serve.ts"), "--port", String(PORT), "--seed", SEED, "--dist", DIST], { stdio: ["ignore", "ignore", "pipe"] });
  let log = "";
  await new Promise((ok, fail) => {
    const timer = setTimeout(() => fail(new Error(`server did not start:\n${log}`)), 30000);
    child.stderr.on("data", (d) => {
      log += d;
      if (log.includes("UX-EVAL READY")) { clearTimeout(timer); ok(); }
    });
    child.on("exit", (code) => { clearTimeout(timer); fail(new Error(`server exited ${code}:\n${log}`)); });
  });
  child.removeAllListeners("exit");
  server = child;
}
async function stopServer() {
  if (!server) return;
  const s = server;
  server = null;
  await new Promise((r) => { s.once("exit", r); s.kill("SIGTERM"); setTimeout(() => { s.kill("SIGKILL"); r(); }, 5000); });
}

async function api(path, user) {
  const res = await fetch(ORIGIN + path, { headers: user ? { Cookie: `uxeval_user=${user}` } : {} });
  if (!res.ok) throw new Error(`${path}: ${res.status}`);
  return res.json();
}

// ---------- the screens ----------

const D = SEED_INFO.decks;
const U = SEED_INFO.users;
const WIDE = process.argv.includes("--wide") ? [1920, 2560] : [];
const ALL = [375, 768, 1280, 1440, ...WIDE];
const APP = [768, 1280, 1440, ...WIDE];
const PUBLIC = [375, 768, 1280, 1440, ...WIDE];

const cardRow = (page, name) => page.getByRole("button", { name: new RegExp(`^${name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}`) }).first();

/** End geometry is recorded with the screenshots, so a pane clipped above
 * the viewport bottom or an outer scroll tail cannot pass visual review. */
async function deckScrollMetrics(page) {
  return page.evaluate(() => {
    const list = document.querySelector("[data-deck-list]");
    const panel = document.querySelector("[data-deck-panel]");
    const root = document.querySelector("[data-deck-page]");
    const workspace = document.querySelector("[data-deck-workspace]");
    const hero = document.querySelector("[data-deck-hero]");
    const compact = document.querySelector("[data-deck-compact] > header");
    const rect = list.getBoundingClientRect();
    const contentBottom = Math.max(...[...list.children].map(el => el.getBoundingClientRect().bottom));
    const controls = document.querySelector('[aria-label="Deck view controls"]');
    const views = panel.querySelector('nav, [role="tablist"], select');
    return {
      list_range: list.scrollHeight - list.clientHeight,
      list_remaining: list.scrollHeight - list.clientHeight - list.scrollTop,
      list_top: rect.top, list_bottom: rect.bottom,
      content_bottom: contentBottom, bottom_gap: rect.bottom - contentBottom,
      panel_top: panel.getBoundingClientRect().top,
      panel_bottom: panel.getBoundingClientRect().bottom,
      panel_views_top: views?.getBoundingClientRect().top ?? null,
      controls_top: controls?.getBoundingClientRect().top ?? null,
      page_range: root ? root.scrollHeight - root.clientHeight : 0,
      page_remaining: root ? root.scrollHeight - root.clientHeight - root.scrollTop : 0,
      header_travel: hero && compact ? hero.offsetHeight - compact.offsetHeight : 0,
      workspace_range: workspace ? workspace.scrollHeight - workspace.clientHeight : 0,
      viewport_bottom: innerHeight,
    };
  });
}

async function scrollDeckToEnd({ page, watch }) {
  const list = page.locator("[data-deck-list]");
  const box = await list.boundingBox();
  if (!box) throw new Error("Deck list has no rendered bounds");
  await page.mouse.move(box.x + box.width / 2, Math.min(box.y + 120, 700));
  // Extra input after reaching the list end exposes accidental scroll
  // chaining into an oversized ancestor, including on older builds.
  for (let pass = 0; pass < 3; pass++) {
    await page.mouse.wheel(0, await list.evaluate(el => Math.max(2000, el.scrollHeight * 2)));
    await settle(page, watch);
  }
  const m = await deckScrollMetrics(page);
  if (Math.abs(m.list_remaining) > 1 || Math.abs(m.list_bottom - m.viewport_bottom) > 1
    || Math.abs(m.panel_bottom - m.viewport_bottom) > 1 || m.panel_top < 0
    || (m.panel_views_top != null && m.panel_views_top < 0)
    || (m.controls_top != null && m.controls_top < 0)
    || m.page_range > m.header_travel + 1 || m.workspace_range > 1) {
    throw new Error(`Deck end lost its viewport: ${JSON.stringify(m)}`);
  }
}

/** Select the same destination across navigation compositions. Older builds
 * kept statistics in Checks; they remain a valid comparison of that task. */
async function panelView(page, vp, view) {
  if (vp.width < 1100) await press(page, page.getByRole("button", { name: /^(Deck details|Open deck details)$/ }).first(), vp);
  const selector = page.getByRole("combobox", { name: "Panel view", exact: true });
  const links = page.getByRole("navigation", { name: "Deck panel views", exact: true });
  if (await links.isVisible()) await press(page, links.getByRole("button", { name: view === "stats" ? "Stats" : "Checks", exact: true }), vp);
  else if (await selector.isVisible()) {
    // Native keys produce the same trusted input as the legacy tab click.
    // A synthetic change event would count intentional view changes as load CLS.
    const index = await selector.locator(`option[value="${view}"]`).evaluate(option => option.index);
    await selector.focus();
    await selector.press("Home");
    for (let step = 0; step < index; step++) await selector.press("ArrowDown");
  }
  else await press(page, page.getByRole("tab", { name: "Checks", exact: true }), vp);
}

async function statsSection(page, vp, watch, section) {
  await panelView(page, vp, "stats");
  await settle(page, watch);
  const panel = page.getByRole("complementary", { name: "Deck panel" });
  const modern = panel.getByRole("region", { name: "Mana demand and sources", exact: true });
  const modernBuild = await panel.getByRole("navigation", { name: "Deck panel views", exact: true }).count() > 0 || await page.getByRole("combobox", { name: "Panel view", exact: true }).count() > 0;
  if (modernBuild) await modern.waitFor();
  const target = section === "colours"
    ? (modernBuild ? modern : panel.getByText("Colour · share of coloured symbols in costs", { exact: true }))
    : panel.getByText(modernBuild ? "Card types" : "Types", { exact: true });
  await target.waitFor();
  const currentPane = panel.locator("[data-panel-body]");
  const pane = await currentPane.count() ? currentPane : panel.getByRole("tabpanel");
  for (let pass = 0; pass < 3; pass++) {
    const row = await target.boundingBox(), box = await pane.boundingBox();
    if (!row || !box) throw new Error("Stats section has no rendered bounds");
    const distance = row.y - box.y - 16;
    if (Math.abs(distance) < 3) break;
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.wheel(0, Math.round(distance));
    await settle(page, watch);
  }
}

/** One screen: a path, a user, and the states reached from it in order. */
const SCREENS = [
  { screen: "sign-in", user: 0, path: "/signin", widths: PUBLIC, states: [{ state: "default", caption: "Sign-in page, signed out" }] },
  { screen: "first-run", user: U.empty, path: "/", widths: ALL, states: [{ state: "default", caption: "A new account with no decks" }] },
  { screen: "decks", user: U.owner, path: "/", widths: ALL, states: [{ state: "default", caption: "My decks, six decks across Paper, Theorycrafted and Archived" }] },
  { screen: "decks", user: U.owner, path: "/", widths: ALL, states: [{ state: "filtered", caption: "The roster filtered to Paper decks", act: async ({ page, vp }) => {
    const lifecycle = page.getByRole("combobox", { name: "Lifecycle", exact: true });
    // The older read-only phone roster has no filters. Capture its original
    // roster there so the same harness can still recapture that build.
    if (vp.width < 720 && !(await lifecycle.count())) return;
    await lifecycle.focus();
    await lifecycle.press("Home");
    await lifecycle.press("ArrowDown");
    // A native select changed on those keys; the listbox commits on Enter.
    await lifecycle.press(await lifecycle.evaluate(el => el.tagName === "SELECT") ? "Escape" : "Enter");
  } }] },
  { screen: "decks", user: U.owner, path: "/", widths: [375, 768, 1440], states: [{ state: "lifecycle-open", caption: "The roster's lifecycle filter open", act: async ({ page, vp }) => {
    await press(page, page.getByRole("combobox", { name: "Lifecycle", exact: true }), vp);
    await page.getByRole("listbox", { name: "Lifecycle", exact: true }).waitFor();
  } }] },
  { screen: "decks", user: U.owner, path: "/", widths: [1280, 1440, ...WIDE], states: [{ state: "tile-hover", caption: "Hovering the second deck tile", act: async ({ page }) => {
    // Both the old strip and the gallery use the deck title's stretched link.
    const link = page.getByRole("main").getByRole("heading", { level: 2 }).nth(1).getByRole("link");
    await link.hover();
  } }] },
  { screen: "decks", user: U.owner, path: "/", widths: [1440], states: [{ state: "tile-actions-hover", caption: "Hovering the second deck's actions button", act: async ({ page }) => {
    const main = page.getByRole("main");
    await main.getByRole("heading", { level: 2 }).nth(1).getByRole("link").hover();
    // Keep the index stable when only the hovered tile reveals its trigger.
    await main.getByRole("button", { name: /^Actions for /, includeHidden: true }).nth(1).hover();
  } }] },
  {
    screen: "deck", user: U.owner, path: `/deck/${D.najeela}`, widths: ALL, states: [
      { state: "default", caption: "A Paper deck with open agent proposals" },
    ],
  },
  {
    screen: "deck", user: U.owner, path: `/deck/${D.najeela}`, widths: [1280, 1440, ...WIDE], states: [
      { state: "list-scrolled", caption: "Working in the deck after scrolling the card list", act: async ({ page, watch }) => {
        // This stable marker also exists in older builds. Trusted wheel input
        // exercises both an independent pane and a yielding commander header.
        const box = await page.locator("[data-deck-list]").boundingBox();
        if (!box) throw new Error("Deck list has no rendered bounds");
        await page.mouse.move(box.x + box.width / 2, box.y + Math.min(120, box.height / 2));
        await page.mouse.wheel(0, 480);
        await settle(page, watch);
      } },
    ],
  },
  {
    screen: "deck", user: U.owner, path: `/deck/${D.najeela}`, widths: [1280, 1440, 2560], states: [
      { state: "list-end", caption: "The card list at its end, with the toolbar and full-height panel pinned", act: scrollDeckToEnd },
    ],
  },
  {
    // The sample has the fewest saved rows in this seed and fits at 2560.
    screen: "deck", user: U.sample, path: `/deck/${D.sample}`, widths: [2560], states: [
      { state: "short-deck-open", caption: "The shortest seeded deck, with no list overflow" },
      { state: "short-deck", caption: "The short deck after yielding its header, still without list overflow", act: scrollDeckToEnd },
    ],
  },
  {
    screen: "deck", user: U.owner, path: `/deck/${D.najeela}`, widths: APP, states: [
      { state: "card-selected", caption: "A card in the list opened", act: async ({ page, vp }) => press(page, cardRow(page, "Underworld Breach"), vp) },
    ],
  },
  ...[["group", "Group the list"], ["sort", "Order the cards in each group"]].map(([state, name]) => ({
    screen: "deck", user: U.owner, path: `/deck/${D.najeela}`, widths: [768, 1440], states: [
      { state: `toolbar-${state}-open`, caption: `The toolbar's ${state} list open`, act: async ({ page, vp }) => {
        await press(page, page.getByRole("combobox", { name, exact: true }), vp);
        await page.getByRole("listbox", { name, exact: true }).waitFor();
      } },
    ],
  })),
  {
    screen: "deck", user: U.owner, path: `/deck/${D.najeela}`, widths: [768, 1440], states: [
      { state: "card-primary-slot-open", caption: "A card's primary slot list open beside the Board control", act: async ({ page, vp }) => {
        await press(page, cardRow(page, "Underworld Breach"), vp);
        await press(page, page.getByRole("combobox", { name: "Primary slot", exact: true }), vp);
        await page.getByRole("listbox", { name: "Primary slot", exact: true }).waitFor();
      } },
    ],
  },
  {
    screen: "deck", user: U.owner, path: `/deck/${D.caesar}`, widths: [768, 1440], states: [
      { state: "copies", caption: "A basic land's copies in the card panel", act: async ({ page, vp }) => {
        await press(page, page.locator("[data-deck-list] button[data-oracle-id]").filter({ hasText: "Mountain" }).first(), vp);
        await page.getByRole("button", { name: "Add a copy of Mountain", exact: true }).waitFor();
      } },
    ],
  },
  {
    screen: "deck", user: U.owner, path: `/deck/${D.najeela}`, widths: [768, 1440], states: [
      { state: "dialog-select-open", caption: "A list open inside the Compare dialog", act: async ({ page, vp }) => {
        await press(page, page.getByRole("button", { name: "More", exact: true }), vp);
        await press(page, page.getByRole("menuitem", { name: "Compare", exact: true }), vp);
        await press(page, page.getByRole("dialog", { name: "Compare decks", exact: true }).getByRole("combobox", { name: "Compare with", exact: true }), vp);
        await page.getByRole("listbox", { name: "Compare with", exact: true }).waitFor();
      } },
    ],
  },
  {
    screen: "deck", user: U.owner, path: `/deck/${D.najeela}`, widths: APP, states: [
      { state: "review", caption: "Reviewing proposals in the list", act: async ({ page, vp }) => press(page, page.getByRole("button", { name: /^Review in the list/ }), vp) },
    ],
  },
  ...["stats", "checks"].map(view => ({
    screen: "deck", user: U.owner, path: `/deck/${D.najeela}`, widths: APP,
    states: [{ state: view, caption: view === "stats" ? "Deck statistics" : "Deck checks and slot targets", act: async ({ page, vp }) => panelView(page, vp, view) }],
  })),
  ...["colours", "types"].map(section => ({
    screen: "deck", user: U.owner, path: `/deck/${D.najeela}`, widths: APP,
    states: [{ state: `stats-${section}`, caption: section === "colours" ? "Mana colour demand and sources" : "Deck type breakdown", act: async ({ page, vp, watch }) => statsSection(page, vp, watch, section) }],
  })),
  {
    screen: "deck", user: U.owner, path: `/deck/${D.najeela}`, widths: ALL, states: [
      {
        state: "maybe-list", caption: "Scrolled to the maybe list",
        act: async ({ page, vp }) => scrollUntilVisible(page, page.getByRole("region", { name: "Maybe list" }).first(), vp, { top: true }),
      },
    ],
  },
  {
    // Needs a seed with the token catalog (seed-tokens); without one the
    // section reads "Token list not available yet" and still captures.
    screen: "deck", user: U.owner, path: `/deck/${D.najeela}`, widths: [1280, 1440, ...WIDE], states: [
      {
        state: "tokens", caption: "Scrolled to the deck's tokens",
        act: async ({ page, vp }) => scrollUntilVisible(page, page.getByRole("region", { name: "Tokens", exact: true }).first(), vp, { top: true }),
      },
      {
        state: "token-selected", caption: "A token's details",
        act: async ({ page, vp }) => {
          const row = page.getByRole("region", { name: "Tokens", exact: true }).locator("[data-token-id]").first();
          if (!(await row.count())) throw Error("Token capture needs a seed with the token catalog");
          await scrollUntilVisible(page, row, vp);
          await press(page, row, vp);
          await page.getByText("Made by", { exact: true }).first().waitFor();
        },
      },
    ],
  },
  ...[false, true].map(costless => ({
    screen: "deck", user: U.owner, path: `/deck/${D.najeela}`, widths: [1280, 1440, ...WIDE],
    states: [{ state: costless ? "maybe-note-costless" : "maybe-note", caption: costless ? "A costless shortlisted card and its saved note" : "A shortlisted card and its saved note", act: async ({ page, vp }) => {
      const maybe = (await api(`/api/decks/${D.najeela}`, U.owner)).state.cards.filter(card => card.maybe && card.maybe_note && (costless ? !card.mana_cost : !!card.mana_cost));
      const card = maybe.find(card => card.maybe_note.includes("\n\n")) ?? maybe[0];
      if (!card) throw Error("Maybe-note capture requires a matching noted card in the fixture");
      const row = page.getByRole("region", { name: "Maybe list", exact: true }).locator(`[data-oracle-id="${card.oracle_id}"]`).getByRole("button", { name: card.name, exact: true }).first();
      await press(page, row, vp);
      await page.getByRole("heading", { name: card.name, exact: true }).waitFor();
    } }],
  })),
  {
    screen: "deck", user: U.owner, path: `/deck/${D.najeela}`, widths: APP, states: [
      {
        state: "filtered", caption: "The deck searched for “tutor”",
        act: async ({ page, vp }) => {
          const field = page.getByRole("searchbox", { name: "Search this deck", exact: true });
          await press(page, field, vp);
          await field.pressSequentially("tutor");
        },
      },
    ],
  },
  {
    screen: "deck", user: U.owner, path: `/deck/${D.najeela}`, widths: APP, states: [
      {
        state: "dialog-slots", caption: "The slots and tags dialog",
        act: async ({ page, vp }) => {
          await press(page, page.getByRole("button", { name: /^More/ }), vp);
          await press(page, page.getByRole("menuitem", { name: /slots and tags/i }), vp);
        },
      },
    ],
  },
  ...[false, true].map(filtered => ({
    screen: "deck", user: U.owner, path: `/deck/${D.najeela}`, widths: [1280, 1440, ...WIDE],
    states: [{ state: filtered ? "export-filtered-maybe" : "export-whole", caption: filtered ? "Export matching maybe cards" : "Export the saved deck", act: async ({ page, vp, watch }) => {
      if (filtered) {
        const card = (await api(`/api/decks/${D.najeela}`, U.owner)).state.cards.find(card => card.maybe);
        if (!card) throw Error("Export fixture needs a maybe card");
        // Typed like a person, as in the "filtered" state: fill() is not user
        // input to Chromium, so rows the filter hides would count as layout shift.
        const field = page.getByRole("searchbox", { name: "Search this deck", exact: true });
        await press(page, field, vp);
        await field.pressSequentially(card.name);
        await settle(page, watch);
      }
      await press(page, page.getByRole("button", { name: "More", exact: true }), vp);
      await press(page, page.getByRole("menuitem", { name: "Export", exact: true }), vp);
      const dialog = page.getByRole("dialog", { name: "Export", exact: true });
      // Every match is on the maybe list, so Export opens with it included.
      await dialog.getByRole("textbox", { name: "Exported list", exact: true }).waitFor();
    } }],
  })),
  ...[false, true].map(empty => ({
    screen: "deck", user: U.owner, path: `/deck/${D.najeela}`, widths: [1280, 1440, ...WIDE],
    states: [{ state: empty ? "search-empty" : "search-results", caption: empty ? "A catalogue query with no matches" : "Catalogue results for a saved card", act: async ({ page }) => {
      const input = page.getByRole("combobox", { name: "Add a card or search", exact: true });
      const card = (await api(`/api/decks/${D.najeela}`, U.owner)).state.cards.find(card => !card.maybe && card.role === "card");
      if (!card) throw Error("Search fixture needs a saved card");
      await input.fill(empty ? "mv<0" : card.name);
      if (empty) await page.getByText(/^(?:No cards? match|No card matches)/).waitFor();
      else await page.getByRole("option").filter({ has: page.getByText(card.name, { exact: true }) }).waitFor();
    } }],
  })),
  {
    screen: "import", user: U.owner, path: `/deck/${D.najeela}`, widths: APP, fresh: true, states: [
      {
        state: "empty", caption: "Open Import",
        act: async ({ page, vp }) => {
          await press(page, page.getByRole("button", { name: /^More/ }), vp);
          await press(page, page.getByRole("menuitem", { name: /import/i }), vp);
        },
      },
      {
        state: "preview", caption: "Paste a Moxfield export and click Preview",
        act: async ({ page, vp }) => {
          const box = page.getByRole("textbox", { name: /list to import/i });
          await press(page, box, vp);
          await box.fill(readFileSync(join(PKG, "test/fixtures/moxfield/najeela.copy-for-moxfield.txt"), "utf8"));
          await press(page, page.getByRole("button", { name: /^Preview/ }), vp);
        },
      },
      {
        state: "result", caption: "Apply the import",
        act: async ({ page, vp }) => {
          await press(page, page.getByRole("button", { name: /^Apply the import/ }).last(), vp);
          const confirm = page.getByRole("button", { name: /^Apply the import/ });
          if (await confirm.isVisible()) await press(page, confirm.last(), vp);
        },
      },
    ],
  },
  // The desktop manual table is a core workflow too. Every width starts from
  // a fresh disposable seed so New game and the opening hand are comparable.
  {
    screen: "playtest", user: U.owner, path: `/deck/${D.najeela}/playtest`,
    widths: [1280, 1440, ...WIDE], fresh: true,
    states: [
      { state: "new-game", caption: "Start a manual playtest from the deck", act: async ({ page }) => { await page.getByRole("dialog", { name: "New game", exact: true }).waitFor(); } },
      { state: "opening", caption: "Shuffle and draw an opening hand", act: async ({ page, vp }) => {
        await press(page, page.getByRole("button", { name: "Shuffle and draw seven", exact: true }), vp);
        await page.getByRole("region", { name: "Opening hand", exact: true }).waitFor();
      } },
      { state: "playing", caption: "Keep the opening hand", act: async ({ page, vp }) => {
        await press(page, page.getByRole("button", { name: "Keep hand", exact: true }), vp);
        await page.getByRole("button", { name: /^Draw(?:\s|$)/ }).waitFor();
      } },
      { state: "card-selected", caption: "Select an actual hand card for inspection", act: async ({ page, vp }) => {
        const card = page.getByRole("region", { name: "Hand", exact: true }).locator("[data-object-id]").first();
        await press(page, card, vp);
      } },
      { state: "library", caption: "Open the ordered library viewer", act: async ({ page, vp }) => {
        await press(page, page.getByRole("navigation", { name: "Table zones" }).getByRole("button", { name: /^Library/ }), vp);
        await page.getByRole("combobox", { name: "Library viewer mode" }).waitFor();
      } },
    ],
  },
  { screen: "inbox", user: U.owner, path: "/inbox", widths: ALL, states: [{ state: "default", caption: "Inbox with proposals and a brief edit" }] },
  { screen: "inbox", user: U.owner, path: "/inbox", widths: APP, states: [{ state: "aggregate", caption: "A proposal with the deck-wide pending forecast visible", act: async ({ page, vp }) => {
    const summary = page.locator("summary").filter({ hasText: /^Deck impact if all / });
    if (await summary.count()) await press(page, summary, vp);
  } }] },
  { screen: "collection-stats", user: U.owner, path: "/", widths: APP, states: [{ state: "default", caption: "Collection statistics for Paper decks", act: async ({ page, vp }) => scrollUntilVisible(page, page.getByText("Across your paper decks", { exact: true }), vp, { top: true }) }] },
  ...["inbox", "memories", "activity"].map(screen => ({ screen: `${screen}-empty`, user: U.empty, path: `/${screen}`, widths: APP, states: [{ state: "default", caption: `${screen[0].toUpperCase() + screen.slice(1)} in a new account` }] })),
  { screen: "memories", user: U.owner, path: "/memories", widths: ALL, states: [{ state: "default", caption: "Memories" }] },
  { screen: "activity", user: U.owner, path: "/activity", widths: ALL, states: [{ state: "default", caption: "Activity feed of agent writes" }] },
  { screen: "settings", user: U.owner, path: "/settings", widths: ALL, states: [{ state: "default", caption: "Settings" }] },
  { screen: "settings", user: U.owner, path: "/settings", widths: APP, states: [{ state: "pricing-budgets", caption: "Price source, memory budgets and card data", act: async ({ page, vp }) => scrollUntilVisible(page, page.getByRole("heading", { name: "Price source", exact: true }), vp, { top: true }) }] },
  { screen: "settings-new-account", user: U.empty, path: "/settings", widths: APP, states: [{ state: "default", caption: "Settings before an agent is connected, regular account" }] },
  { screen: "profile", user: U.owner, path: "/profile", widths: ALL, states: [{ state: "default", caption: "The owner's profile" }] },
  { screen: "sample-deck", user: U.sample, path: `/deck/${D.sample}`, widths: [375, 1440], states: [{ state: "default", caption: "The sample deck in a new account" }] },
  { screen: "public-deck", user: 0, path: "/d/{slug}", widths: PUBLIC, states: [{ state: "signed-out", caption: "A public deck page, signed out" }] },
  { screen: "public-profile", user: 0, path: `/u/${SEED_INFO.handles.owner}`, widths: PUBLIC, states: [{ state: "signed-out", caption: "A public profile, signed out" }] },
];

// ---------- capture ----------

const manifest = { frozen_now: FROZEN_NOW, screens: [], flows: [] };
const metrics = { screens: {}, flows: {}, public_bundle: null, notes: {} };
const notes = { blocked: new Set(), imageFailures: new Set() };
const browser = await chromium.launch({ args: BROWSER_ARGS });
const failures = [];

async function runScreen(def, width, slug) {
  const vp = { ...VIEWPORTS[width], key: width };
  const keys = def.states.map((s) => `${def.screen}--${s.state}--${width}`);
  if (ONLY && !keys.some((k) => ONLY.test(k))) return;
  if (def.fresh) await startServer();
  const context = await newContext(browser, { origin: ORIGIN, vp, user: def.user, frozenNow: FROZEN_NOW, notes });
  const page = await context.newPage();
  const watch = watchPage(page);
  try {
    await page.goto(ORIGIN + def.path.replace("{slug}", slug), { waitUntil: "load" });
    await settle(page, watch);
    for (const s of def.states) {
      const key = `${def.screen}--${s.state}--${width}`;
      const before = watch.errors.length;
      try {
        if (s.act) await s.act({ page, vp, watch });
        await releaseHeld(page);
        await settle(page, watch);
        // Dynamic Stats can finish just below the first viewport. Wait for
        // its actual content before taking a full public-page screenshot.
        if (def.screen === "public-deck") {
          const loading = page.getByRole("status").filter({ hasText: /^Loading…$/ });
          if (await loading.count()) await loading.waitFor({ state: "hidden" });
          await settle(page, watch);
        }
      } catch (e) {
        failures.push({ key, error: String(e.message ?? e).split("\n")[0] });
      }
      const file = join(RUN, "screens", `${key}.png`);
      // Full-page screenshots can reset Chromium touch emulation; measure first.
      const m = await pageMetrics(page, vp);
      if (def.screen === "deck" && ["list-end", "short-deck", "short-deck-open"].includes(s.state)) m.deck_scroll = await deckScrollMetrics(page);
      const full = await shoot(page, file);
      m.axe = await axe(page);
      m.axe_violations = m.axe.reduce((n, v) => n + v.nodes, 0);
      m.console_errors = watch.errors.slice(before);
      metrics.screens[key] = m;
      manifest.screens.push({
        key, screen: def.screen, state: s.state, caption: s.caption,
        viewport: { width: vp.width, height: vp.height, touch: vp.touch },
        signed_in: def.user ? true : false,
        files: { viewport: `screens/${key}.png`, full: full ? `screens/${key}.full.png` : null },
        ...(failures.some((f) => f.key === key) ? { incomplete: true } : {}),
      });
      process.stderr.write(`  ${key}${failures.some((f) => f.key === key) ? " (incomplete)" : ""}\n`);
    }
  } finally {
    await context.close();
  }
}

await startServer();
const slug = (await api("/api/decks", U.owner)).find((d) => d.id === D.caesar).slug;
for (const def of SCREENS) {
  for (const width of def.widths) await runScreen(def, width, slug);
  if (def.fresh) await startServer();
}

for (const flow of FLOWS) {
  if (ONLY && !ONLY.test(`flow:${flow.name}`)) continue;
  await startServer();
  const vp = { ...VIEWPORTS[flow.width], key: flow.width };
  const context = await newContext(browser, { origin: ORIGIN, vp, user: flow.user(U), frozenNow: FROZEN_NOW, notes });
  const extra = new Map();
  // Every page the flow opens (a new tab) is watched from its first request.
  context.on("page", (p) => extra.set(p, watchPage(p)));
  const page = await context.newPage();
  await new Promise((r) => setTimeout(r, 0));
  const watch = extra.get(page) ?? watchPage(page);
  extra.delete(page);
  const dir = join(RUN, "flows", flow.name);
  mkdirSync(dir, { recursive: true });
  const steps = [];
  let clicks = 0, typed = 0, keypresses = 0, completed = true, error = null;
  const tools = {
    page, vp, watch, origin: ORIGIN, decks: D, users: U, slug, api, PKG,
    press: async (locator) => { clicks++; await press(page, locator, vp); },
    type: async (locator, text) => { typed++; await locator.fill(text); },
    keys: async (locator, text) => { typed++; await locator.pressSequentially(text); },
    key: async (locator, combo) => { keypresses++; await locator.press(combo); },

    scroll: (locator, opts) => scrollUntilVisible(page, locator, vp, opts),
    settle: () => settle(page, watch),
    /** Records the current screen as the next step of the flow. */
    step: async (caption, p = page) => {
      await releaseHeld(p);
      await settle(p, extra.get(p) ?? watch);
      const n = String(steps.length + 1).padStart(2, "0");
      await shoot(p, join(dir, `${n}.png`), { full: false });
      steps.push({ step: n, caption, clicks_so_far: clicks, console_errors: watch.errors.length });
      process.stderr.write(`  flow ${flow.name} ${n} ${caption}\n`);
    },
  };
  let check = null;
  try {
    check = await flow.run(tools);
    if (check && check.completed === false) completed = false;
  } catch (e) {
    completed = false;
    error = String(e.message ?? e).split("\n")[0];
    failures.push({ key: `flow:${flow.name}`, error });
  } finally {
    await context.close();
  }
  manifest.flows.push({
    key: flow.name, caption: flow.caption, viewport: { width: vp.width, height: vp.height, touch: vp.touch },
    steps: steps.map((s) => ({ key: `${flow.name}/${s.step}`, file: `flows/${flow.name}/${s.step}.png`, caption: s.caption })),
  });
  const flowErrors = [...watch.errors, ...[...extra.values()].flatMap((w) => w.errors)];
  metrics.flows[flow.name] = { steps: steps.length, clicks, fields_typed: typed, key_presses: keypresses, completed, ...(check?.facts ? { facts: check.facts } : {}), ...(error ? { error } : {}), console_errors: flowErrors };
}
await stopServer();
await browser.close();

// ---------- the public page budget, with the repository's own script ----------
{
  const r = spawnSync(process.execPath, [join(buildDir, "scripts", "check-public-bundle.mjs")], { encoding: "utf8" });
  try {
    const out = JSON.parse(r.stdout);
    metrics.public_bundle = { gzip_js_bytes: out.bytes, budget_bytes: 110000, within_budget: out.bytes < 110000, chunks: out.chunks.length };
  } catch {
    metrics.public_bundle = { error: (r.stderr || r.stdout).slice(0, 300) };
  }
}

// ---------- summary ----------
const S = Object.values(metrics.screens);
metrics.summary = {
  screens: manifest.screens.length,
  full_page_shots: manifest.screens.filter((s) => s.files.full).length,
  flows: manifest.flows.length,
  flow_steps: manifest.flows.reduce((n, f) => n + f.steps.length, 0),
  axe_violation_nodes: S.reduce((n, m) => n + m.axe_violations, 0),
  axe_rules: [...new Set(S.flatMap((m) => m.axe.map((v) => v.id)))].sort(),
  screens_with_horizontal_overflow: Object.entries(metrics.screens).filter(([, m]) => m.horizontal_overflow_px > 0).map(([k]) => k),
  small_text_nodes: S.reduce((n, m) => n + m.small_text.nodes, 0),
  tap_targets_under_44: S.reduce((n, m) => n + (m.tap_targets?.under_44 ?? 0), 0),
  console_errors: S.reduce((n, m) => n + m.console_errors.length, 0) + Object.values(metrics.flows).reduce((n, f) => n + f.console_errors.length, 0),
  layout_shift_total: Math.round(S.reduce((n, m) => n + m.layout_shift, 0) * 10000) / 10000,
  flows_completed: Object.values(metrics.flows).filter((f) => f.completed).length,
  flow_clicks: Object.fromEntries(Object.entries(metrics.flows).map(([k, f]) => [k, f.clicks])),
  public_gzip_js_bytes: metrics.public_bundle?.gzip_js_bytes ?? null,
  incomplete_captures: failures,
};
metrics.notes = { blocked_origins: [...notes.blocked].sort(), image_fetch_failures: [...notes.imageFailures].length };

writeFileSync(join(RUN, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
writeFileSync(join(RUN, "metrics.json"), JSON.stringify(metrics, null, 2) + "\n");
const sha = spawnSync("git", ["rev-parse", "HEAD"], { cwd: PKG, encoding: "utf8" }).stdout.trim();
const dirty = spawnSync("git", ["status", "--porcelain", "--", "src", "web"], { cwd: PKG, encoding: "utf8" }).stdout.trim() !== "";
writeFileSync(join(RUN, "run.json"), JSON.stringify({ private: "Never give this file to a judge.", commit: sha, app_tree_dirty: dirty, captured_at: new Date().toISOString(), seed: SEED, dist: DIST }, null, 2) + "\n");
console.log(JSON.stringify({ run: RUN, ...metrics.summary, incomplete_captures: failures.length }, null, 2));
