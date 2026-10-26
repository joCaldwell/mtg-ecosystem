// Phase 4b review fixes for the tablet frame, pinned where a server render or
// a plain call can see them. Remounting across a breakpoint, wheel and touch
// scrolling and focus order need a real browser; those were checked there.
import { test } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "vite";
import { renderToStaticMarkup } from "react-dom/server";
import { createElement } from "react";

async function vite(t: { after: (fn: () => unknown) => void }) {
  const server = await createServer({ server: { middlewareMode: true, hmr: false }, appType: "custom", logLevel: "silent" });
  t.after(() => server.close());
  return server;
}

test("the dialog scroll lock is counted, so nested dialogs closing together release the page", async (t) => {
  const { lockBodyScroll } = await (await vite(t)).ssrLoadModule("/src/ui/Dialog.tsx");
  const saved = (globalThis as any).document;
  t.after(() => { (globalThis as any).document = saved; });
  const body = { style: { overflow: "" } };
  (globalThis as any).document = { body };

  // A sheet, then a dialog opened from it; both unmount in one commit, parent first.
  const sheet = lockBodyScroll();
  const nested = lockBodyScroll();
  assert.equal(body.style.overflow, "hidden");
  sheet();
  assert.equal(body.style.overflow, "hidden", "the nested dialog still holds the page");
  nested();
  assert.equal(body.style.overflow, "", "the last release restores the page, whatever order they close in");

  body.style.overflow = "clip";
  const a = lockBodyScroll(), b = lockBodyScroll();
  b(); a();
  assert.equal(body.style.overflow, "clip", "the value from before the first lock comes back");
});

test("main is the frame's second child on desktop, rail and drawer layouts, with the page inside it", async (t) => {
  const server = await vite(t);
  const { AppShell } = await server.ssrLoadModule("/src/app/AppShell.tsx");
  const saved = (globalThis as any).window;
  t.after(() => { (globalThis as any).window = saved; });
  const frame = (width: number) => {
    // useMediaQuery reads matchMedia for its first value; max-width queries only.
    (globalThis as any).window = { matchMedia: (q: string) => ({ matches: width <= Number(/max-width: (\d+)px/.exec(q)![1]) }) };
    const html = renderToStaticMarkup(createElement(AppShell, { route: { page: "inbox" }, children: createElement("p", null, "PAGE") }));
    // The frame's direct children, by tag, in order.
    const inner = html.replace(/^<div[^>]*>/, "").replace(/<\/div>$/, "");
    const tags: string[] = [];
    let depth = 0;
    for (const m of inner.matchAll(/<(\/?)(\w+)[^>]*?(\/?)>/g)) {
      if (m[1]) depth--;
      else { if (depth === 0) tags.push(m[2]); if (!m[3] && !/^(input|img|br|hr|meta|link)$/.test(m[2])) depth++; }
    }
    return { tags, html };
  };
  for (const width of [1280, 1024, 800]) {
    const { tags, html } = frame(width);
    assert.equal(tags.indexOf("main"), 1, `${width}px: main is the frame's second child (${tags.join(",")})`);
    assert.ok(/<main[^>]*id="main-content"[^>]*>.*PAGE.*<\/main>/s.test(html), `${width}px: the page renders inside main`);
  }
  assert.ok(frame(800).tags[0] === "header" && frame(1024).tags[0] === "div" && frame(1280).tags[0] === "div");
});
