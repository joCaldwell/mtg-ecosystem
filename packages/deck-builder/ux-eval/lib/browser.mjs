// Browser-side helpers for capture.mjs: deterministic contexts, settling,
// real-input scrolling, screenshots and the hard metrics.

import { UX_EVAL_ROOT } from "./paths.mjs";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";

const require = createRequire(import.meta.url);
const AXE_SOURCE = readFileSync(require.resolve("axe-core/axe.min.js"), "utf8");
const IMAGE_CACHE = join(UX_EVAL_ROOT, "cache", "images");

/** The four capture viewports. Under 1100 px the device is touch (coarse pointer), as tablets and phones are. */
/**
 * Chromium flags for repeatable pixels: software raster on one thread, no
 * partial or checker-imaged tiles, a fixed colour profile and no LCD text.
 * Without them a dashed or rounded border can differ by a few antialiased
 * pixels between runs.
 */
export const BROWSER_ARGS = [
  "--disable-smooth-scrolling", "--font-render-hinting=none", "--disable-lcd-text", "--disable-gpu",
  "--num-raster-threads=1", "--disable-partial-raster", "--disable-checker-imaging", "--force-color-profile=srgb",
  "--disable-skia-runtime-opts", "--disable-threaded-animation", "--disable-threaded-scrolling",
];

export const VIEWPORTS = {
  375: { width: 375, height: 812, touch: true, mobile: true },
  768: { width: 768, height: 1024, touch: true, mobile: false },
  1280: { width: 1280, height: 800, touch: false, mobile: false },
  1440: { width: 1440, height: 900, touch: false, mobile: false },
  1920: { width: 1920, height: 1080, touch: false, mobile: false },
  2560: { width: 2560, height: 1440, touch: false, mobile: false },
};

/**
 * Runs in the page before any app script: frozen Date, seeded Math.random,
 * layout-shift tally, and held long timers.
 *
 * Long timers (3 s or more: toasts, delayed hints) would otherwise fire
 * whenever wall-clock time says, so a toast could be in one run's screenshot
 * and gone from the next. They are held instead, and released by the capture
 * at a fixed point: a timer started before one screenshot fires just before
 * the next one. A toast is therefore in exactly the screenshot after the
 * action that raised it.
 */
function initScript(frozenNow) {
  const FIXED = Date.parse(frozenNow);
  const RealDate = Date;
  function FrozenDate(...args) {
    if (!new.target) return new RealDate(FIXED).toString();
    return args.length ? new RealDate(...args) : new RealDate(FIXED);
  }
  FrozenDate.prototype = RealDate.prototype;
  FrozenDate.now = () => FIXED;
  FrozenDate.parse = RealDate.parse;
  FrozenDate.UTC = RealDate.UTC;
  Object.defineProperty(FrozenDate, "name", { value: "Date" });
  globalThis.Date = FrozenDate;
  let a = 0x2545f491;
  Math.random = () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  const realSetTimeout = window.setTimeout.bind(window);
  const realClearTimeout = window.clearTimeout.bind(window);
  const held = new Map();
  let generation = 0, nextId = 1e9;
  window.setTimeout = function (fn, delay, ...args) {
    if (typeof fn === "function" && Number(delay) >= 3000) {
      const id = nextId++;
      held.set(id, { fn, args, generation });
      return id;
    }
    return realSetTimeout(fn, delay, ...args);
  };
  window.clearTimeout = function (id) {
    if (held.delete(id)) return;
    return realClearTimeout(id);
  };
  window.__uxReleaseHeld = () => {
    for (const [id, t] of [...held]) {
      if (t.generation >= generation) continue;
      held.delete(id);
      try { t.fn(...t.args); } catch (e) { realSetTimeout(() => { throw e; }, 0); }
    }
  };
  window.__uxShot = () => { generation++; };
  window.__uxShift = 0;
  try {
    new PerformanceObserver((list) => {
      for (const e of list.getEntries()) if (!e.hadRecentInput) window.__uxShift += e.value;
    }).observe({ type: "layout-shift", buffered: true });
  } catch { /* not supported: reported as 0 */ }
}

/**
 * A fresh browser context for one capture. External requests are refused,
 * except card images, which are served from a local cache after the first
 * fetch so every run sees the same bytes.
 */
export async function newContext(browser, { origin, vp, user, frozenNow, notes }) {
  const context = await browser.newContext({
    viewport: { width: vp.width, height: vp.height },
    deviceScaleFactor: 1,
    hasTouch: vp.touch,
    isMobile: vp.mobile,
    locale: "en-US",
    timezoneId: "America/Chicago",
    colorScheme: "dark",
    reducedMotion: "reduce",
    serviceWorkers: "block",
  });
  if (user) await context.addCookies([{ name: "uxeval_user", value: String(user), url: origin }]);
  await context.addInitScript(initScript, frozenNow);
  mkdirSync(IMAGE_CACHE, { recursive: true });
  await context.route((url) => !url.href.startsWith(origin), async (route) => {
    const url = route.request().url();
    if (!url.startsWith("https://cards.scryfall.io/")) {
      notes.blocked.add(new URL(url).origin);
      return route.abort();
    }
    const file = join(IMAGE_CACHE, createHash("sha256").update(url).digest("hex").slice(0, 32));
    if (!existsSync(file)) {
      let body = null;
      for (let attempt = 0; attempt < 4 && !body; attempt++) {
        if (attempt) await new Promise((r) => setTimeout(r, 500 * 2 ** attempt));
        try {
          // Scryfall rejects requests without these headers (https://scryfall.com/docs/api, "Required headers").
          const res = await fetch(url, { headers: { "User-Agent": "deck-builder-ux-eval/1.0", Accept: "image/*" } });
          if (res.ok) body = Buffer.from(await res.arrayBuffer());
        } catch { /* retried */ }
      }
      if (!body) {
        notes.imageFailures.add(url);
        return route.fulfill({ status: 404, body: "" });
      }
      writeFileSync(file, body);
    }
    return route.fulfill({ status: 200, contentType: url.includes(".png") ? "image/png" : "image/jpeg", body: readFileSync(file) });
  });
  return context;
}

/** Console errors and uncaught exceptions, from page creation on. */
export function watchPage(page) {
  const errors = [];
  page.on("console", (m) => {
    if (m.type() !== "error") return;
    const at = m.location()?.url;
    errors.push(`${m.text().slice(0, 300)}${at ? ` (${at.replace(/^https?:\/\/127\.0\.0\.1:\d+/, "")})` : ""}`);
  });
  page.on("pageerror", (e) => errors.push(`Uncaught: ${String(e.message ?? e).slice(0, 300)}`));
  let inflight = 0;
  const live = (r) => !r.url().includes("/api/events");
  page.on("request", (r) => { if (live(r)) inflight++; });
  const done = (r) => { if (live(r)) inflight = Math.max(0, inflight - 1); };
  page.on("requestfinished", done);
  page.on("requestfailed", done);
  return { errors, inflight: () => inflight };
}

/** Waits until the network is quiet, fonts and visible images are loaded, and the DOM stops changing. */
export async function settle(page, watch, { timeout = 15000 } = {}) {
  const start = performance.now();
  let quietSince = null;
  while (performance.now() - start < timeout) {
    if (watch.inflight() === 0) {
      quietSince ??= performance.now();
      if (performance.now() - quietSince > 250) break;
    } else quietSince = null;
    await page.waitForTimeout(50);
  }
  await page.evaluate(async () => {
    await document.fonts.ready;
    const imgs = [...document.images].filter((i) => {
      const r = i.getBoundingClientRect();
      return r.bottom > 0 && r.top < innerHeight && r.width > 0;
    });
    await Promise.all(imgs.map((i) => (i.complete ? null : new Promise((r) => { i.onload = i.onerror = r; setTimeout(r, 5000); }))));
  });
  let last = "";
  for (let i = 0; i < 40; i++) {
    const sig = await page.evaluate(() => {
      const b = document.body;
      return `${b.getElementsByTagName("*").length}:${b.innerText.length}:${scrollY}:${document.activeElement?.tagName}`;
    });
    if (sig === last) break;
    last = sig;
    await page.waitForTimeout(120);
  }
  // Two frames, so the last layout is painted.
  await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
}

/**
 * Clicks with a mouse on desktop, taps on touch viewports. The target is
 * first brought into view by real scrolling, so Playwright's own
 * scroll-into-view never has anything to do.
 */
export async function press(page, locator, vp) {
  await locator.waitFor({ state: "visible", timeout: 10000 });
  await scrollUntilVisible(page, locator, vp);
  if (vp.touch) await locator.tap();
  else {
    await locator.click();
    await park(page, vp);
  }
}

/** Moves the pointer off the content, so no hover state or hover card is in a screenshot. */
export async function park(page, vp) {
  if (!vp.touch) await page.mouse.move(vp.width - 1, vp.height - 1);
}

/**
 * Scrolls with real input until the target is in view: the mouse wheel over
 * the target's scroll container. Never scrollTo or scrollIntoView.
 *
 * Touch viewports use the wheel too. A CDP touch drag does scroll, but its
 * touch slop and fling depend on event timing, so the same drag lands a few
 * pixels apart from run to run; the wheel moves by exactly its delta.
 * (Input.synthesizeScrollGesture does not scroll in headless Chromium.)
 */
export async function scrollUntilVisible(page, locator, vp, { maxSteps = 60, step = 450, margin = 0, top = false } = {}) {
  let previous = null;
  for (let i = 0; i < maxSteps; i++) {
    const box = await locator.boundingBox();
    // In view: entirely, or (top) with its top in the upper part of the screen.
    if (box && box.y >= 0 && (top ? box.y <= vp.height * 0.35 : box.y + Math.min(box.height, vp.height) <= vp.height - margin)) break;
    if (box && previous !== null && Math.abs(box.y - previous) < 1) break; // nothing more to scroll
    previous = box ? box.y : null;
    // The point to scroll at: the middle of the target's nearest scroll container, or the viewport.
    const at = await locator.evaluate((el) => {
      let n = el.parentElement;
      while (n && n !== document.body) {
        const s = getComputedStyle(n);
        if (/(auto|scroll)/.test(s.overflowY) && n.scrollHeight > n.clientHeight + 1) {
          const r = n.getBoundingClientRect();
          const t = Math.max(r.top, 0), b = Math.min(r.bottom, innerHeight);
          return { x: r.left + r.width / 2, y: (t + b) / 2 };
        }
        n = n.parentElement;
      }
      return { x: innerWidth / 2, y: innerHeight / 2 };
    }).catch(() => ({ x: vp.width / 2, y: vp.height / 2 }));
    const down = !box || box.y > 0;
    const need = !box ? step : !down ? -box.y + 24
      : top ? box.y - vp.height * 0.15 : box.y + Math.min(box.height, vp.height) - (vp.height - margin) + 24;
    const distance = Math.round(Math.min(step, Math.max(40, need)));
    await page.mouse.move(at.x, at.y);
    await page.mouse.wheel(0, down ? distance : -distance);
    await page.waitForTimeout(150);
  }
  await park(page, vp);
}

/** Releases long timers held since before the previous screenshot (see initScript). Call, then settle, then shoot. */
export async function releaseHeld(page) {
  await page.evaluate(() => window.__uxReleaseHeld?.());
}

/** First-viewport shot, and a full-page shot when the window itself scrolls past the viewport. */
export async function shoot(page, file, { full = true } = {}) {
  try {
    await page.screenshot({ path: file, animations: "disabled", caret: "hide", scale: "css" });
    if (!full) return null;
    const tall = await page.evaluate(() => document.documentElement.scrollHeight > innerHeight + 1);
    if (!tall) return null;
    const fullFile = file.replace(/\.png$/, ".full.png");
    await page.screenshot({ path: fullFile, fullPage: true, animations: "disabled", caret: "hide", scale: "css" });
    return fullFile;
  } finally {
    await page.evaluate(() => window.__uxShot?.());
  }
}

/** axe-core violations, injected from node_modules (page.evaluate is not subject to the page's CSP). */
export async function axe(page) {
  const has = await page.evaluate(() => typeof window.axe === "object");
  if (!has) await page.evaluate(AXE_SOURCE);
  return page.evaluate(async () => {
    const r = await window.axe.run(document, { resultTypes: ["violations"] });
    return r.violations.map((v) => ({ id: v.id, impact: v.impact, nodes: v.nodes.length, help: v.help, targets: v.nodes.slice(0, 3).map((n) => n.target.join(" ")) }));
  });
}

/** Layout and size metrics that need no library. */
export async function pageMetrics(page, vp) {
  return page.evaluate(({ measureTaps }) => {
    const visible = (el) => {
      const r = el.getBoundingClientRect();
      if (r.width <= 1 || r.height <= 1) return null;
      const s = getComputedStyle(el);
      if (s.visibility === "hidden" || s.display === "none" || Number(s.opacity) === 0) return null;
      if (el.closest(".visually-hidden,[aria-hidden=true],[inert],[role=img],svg")) return null;
      return r;
    };
    const label = (el) => (el.getAttribute("aria-label") || el.textContent || el.getAttribute("placeholder") || el.tagName).trim().replace(/\s+/g, " ").slice(0, 60);
    const de = document.documentElement;
    const overflowX = Math.max(0, de.scrollWidth - de.clientWidth);

    const small = new Map();
    let smallTextNodes = 0;
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) {
      const n = walker.currentNode;
      const t = n.textContent.trim();
      if (!t || !n.parentElement) continue;
      if (!visible(n.parentElement)) continue;
      const size = parseFloat(getComputedStyle(n.parentElement).fontSize);
      if (size < 11) {
        smallTextNodes++;
        const key = `${size}px "${t.slice(0, 40)}"`;
        small.set(key, (small.get(key) ?? 0) + 1);
      }
    }

    let taps = null;
    if (measureTaps) {
      const sel = 'a[href], button, input:not([type=hidden]), select, textarea, summary, [role=button], [role=tab], [role=link], [role=menuitem], [role=option], [role=checkbox], [role=radio], [role=switch], [tabindex]:not([tabindex="-1"])';
      const seen = new Set();
      const under = [];
      let total = 0, inline = 0;
      for (const el of document.querySelectorAll(sel)) {
        if (seen.has(el) || el.disabled) continue;
        seen.add(el);
        const r = visible(el);
        if (!r) continue;
        // WCAG 2.5.5 exempts targets inside a sentence or block of text.
        if (el.tagName === "A" && getComputedStyle(el).display === "inline" && el.parentElement && /^(P|LI|SPAN|DD|TD)$/.test(el.parentElement.tagName)) { inline++; continue; }
        total++;
        if (r.width < 44 || r.height < 44) under.push({ label: label(el), w: Math.round(r.width), h: Math.round(r.height) });
      }
      taps = { checked: total, under_44: under.length, inline_links_exempt: inline, sample: under.slice(0, 12) };
    }

    return {
      horizontal_overflow_px: overflowX,
      small_text: { nodes: smallTextNodes, sample: [...small.entries()].slice(0, 10).map(([k, n]) => `${k} ×${n}`) },
      tap_targets: taps,
      layout_shift: Math.round((window.__uxShift ?? 0) * 10000) / 10000,
      document_height: de.scrollHeight,
    };
  }, { measureTaps: vp.width < 1100 && vp.touch });
}
