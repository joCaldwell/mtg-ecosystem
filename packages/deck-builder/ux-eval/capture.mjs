#!/usr/bin/env node
// Captures every screen in its states, and the task flows, with hard metrics.
//
//   node capture.mjs [--port 8830] [--seed /tmp/ux-eval/seed/seed.db] [--dist <web/dist>]
//                    [--out /tmp/ux-eval/runs] [--only <regex over capture keys>]
//
// Writes /tmp/ux-eval/runs/<random-id>/ with screens/*.png, flows/<flow>/<step>.png,
// metrics.json and manifest.json. Capture keys never change between runs, so
// two runs pair screen by screen. run.json (private: commit, time, arguments)
// is for the operator only; packet.mjs never copies it.

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
const SEED = resolve(arg("seed", "/tmp/ux-eval/seed/seed.db"));
const SEED_INFO = JSON.parse(readFileSync(join(dirname(SEED), "seed.json"), "utf8"));
const ONLY = arg("only") ? new RegExp(arg("only")) : null;
const RUN_ID = randomBytes(6).toString("hex");
const RUN = join(resolve(arg("out", "/tmp/ux-eval/runs")), RUN_ID);
mkdirSync(join(RUN, "screens"), { recursive: true });
mkdirSync(join(RUN, "flows"), { recursive: true });

// ---------- build and server ----------

let DIST = arg("dist");
let buildDir;
if (!DIST) {
  buildDir = join("/tmp/ux-eval/build", RUN_ID);
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
const ALL = [375, 768, 1280, 1440];
const APP = [768, 1280, 1440];
const PUBLIC = [375, 768, 1280];

const cardRow = (page, name) => page.getByRole("button", { name: new RegExp(`^${name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}`) }).first();

/** One screen: a path, a user, and the states reached from it in order. */
const SCREENS = [
  { screen: "sign-in", user: 0, path: "/signin", widths: PUBLIC, states: [{ state: "default", caption: "Sign-in page, signed out" }] },
  { screen: "first-run", user: U.empty, path: "/", widths: ALL, states: [{ state: "default", caption: "A new account with no decks" }] },
  { screen: "decks", user: U.owner, path: "/", widths: ALL, states: [{ state: "default", caption: "All decks, six decks across Paper, Theorycrafted and Archived" }] },
  {
    screen: "deck", user: U.owner, path: `/deck/${D.najeela}`, widths: ALL, states: [
      { state: "default", caption: "A Paper deck with open agent proposals" },
    ],
  },
  {
    screen: "deck", user: U.owner, path: `/deck/${D.najeela}`, widths: APP, states: [
      { state: "card-selected", caption: "A card in the list opened", act: async ({ page, vp }) => press(page, cardRow(page, "Underworld Breach"), vp) },
    ],
  },
  {
    screen: "deck", user: U.owner, path: `/deck/${D.najeela}`, widths: APP, states: [
      { state: "review", caption: "Reviewing proposals in the list", act: async ({ page, vp }) => press(page, page.getByRole("button", { name: /^Review in the list/ }), vp) },
    ],
  },
  {
    screen: "deck", user: U.owner, path: `/deck/${D.najeela}`, widths: ALL, states: [
      {
        state: "maybe-list", caption: "Scrolled to the maybe list",
        act: async ({ page, vp }) => scrollUntilVisible(page, page.getByRole("heading", { name: "Maybe list" }).first(), vp, { top: true }),
      },
    ],
  },
  {
    screen: "deck", user: U.owner, path: `/deck/${D.najeela}`, widths: APP, states: [
      {
        state: "filtered", caption: "The deck searched for “tutor”",
        act: async ({ page, vp }) => {
          const field = page.getByRole("textbox", { name: /search this deck/i }).or(page.getByPlaceholder(/search this deck/i)).first();
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
  {
    screen: "import", user: U.owner, path: `/deck/${D.najeela}`, widths: APP, fresh: true, states: [
      {
        state: "empty", caption: "Import tab of the import and export dialog",
        act: async ({ page, vp }) => {
          await press(page, page.getByRole("button", { name: /^More/ }), vp);
          await press(page, page.getByRole("menuitem", { name: /import/i }), vp);
          await press(page, page.getByRole("tab", { name: /^Import/ }), vp);
        },
      },
      {
        state: "preview", caption: "A Moxfield export of the same deck pasted and previewed",
        act: async ({ page, vp }) => {
          const box = page.getByRole("textbox", { name: /list to import/i });
          await press(page, box, vp);
          await box.fill(readFileSync(join(PKG, "test/fixtures/moxfield/najeela.copy-for-moxfield.txt"), "utf8"));
          await press(page, page.getByRole("button", { name: /^Preview/ }), vp);
        },
      },
      {
        state: "result", caption: "After applying the import",
        act: async ({ page, vp }) => {
          await press(page, page.getByRole("button", { name: /^Apply the import/ }).last(), vp);
          const confirm = page.getByRole("button", { name: /^Apply the import/ });
          await page.waitForTimeout(300);
          if (await confirm.count()) await press(page, confirm.last(), vp).catch(() => {});
        },
      },
    ],
  },
  { screen: "inbox", user: U.owner, path: "/inbox", widths: ALL, states: [{ state: "default", caption: "Inbox with proposals and a brief edit" }] },
  { screen: "memories", user: U.owner, path: "/memories", widths: ALL, states: [{ state: "default", caption: "Memories" }] },
  { screen: "activity", user: U.owner, path: "/activity", widths: ALL, states: [{ state: "default", caption: "Activity feed of agent writes" }] },
  { screen: "settings", user: U.owner, path: "/settings", widths: ALL, states: [{ state: "default", caption: "Settings" }] },
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
      } catch (e) {
        failures.push({ key, error: String(e.message ?? e).split("\n")[0] });
      }
      const file = join(RUN, "screens", `${key}.png`);
      const full = await shoot(page, file);
      const m = await pageMetrics(page, vp);
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
    metrics.public_bundle = { gzip_js_bytes: out.bytes, budget_bytes: 90000, within_budget: out.bytes < 90000, chunks: out.chunks.length };
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
