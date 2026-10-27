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

test("collapsed sidebar keeps named navigation and an expanded-state control; tablets retain their full rail", async (t) => {
  const server = await vite(t);
  const { Sidebar } = await server.ssrLoadModule("/src/app/Sidebar.tsx");
  const render = (collapsed: boolean, onToggle?: () => void) => renderToStaticMarkup(createElement(Sidebar, { route: { page: "activity" }, collapsed, onToggle }));
  const expanded = render(false, () => {}), collapsed = render(true, () => {});
  assert.match(expanded, /aria-label="Collapse sidebar"[^>]*aria-expanded="true"/);
  assert.match(collapsed, /aria-label="Expand sidebar"[^>]*aria-expanded="false"/);
  for (const html of [expanded, collapsed]) {
    assert.ok(html.indexOf('href="/"') < html.indexOf('href="/inbox"'), "All decks comes first");
    assert.match(html, /href="\/profile"[^>]*aria-current="page"[^>]*aria-label="Profile"/);
    for (const label of ["All decks", "Inbox", "Memories", "Profile", "Settings"]) assert.ok(html.includes(`aria-label="${label}"`), label);
    assert.doesNotMatch(html, /aria-label="Activity"/);
  }
  assert.match(collapsed, /href="\/inbox"[^>]*title="Inbox"/);
  const tablet = render(false);
  assert.doesNotMatch(tablet, /aria-label="(?:Collapse|Expand) sidebar"/);
});

test("Profile uses the shared tabs and renders Activity inside its page", async (t) => {
  const { ProfilePage } = await (await vite(t)).ssrLoadModule("/src/features/profile/ProfilePage.tsx");
  const html = renderToStaticMarkup(createElement(ProfilePage, { tab: "activity" }));
  assert.match(html, /role="tablist" aria-label="Profile tabs"/);
  assert.match(html, /role="tab" aria-selected="true"[^>]*>Activity/);
  assert.match(html, /role="tabpanel" aria-label="Activity"/);
  assert.match(html, /role="tabpanel" aria-label="Profile" hidden=""/);
  assert.equal((html.match(/<h1/g) ?? []).length, 1, "one Profile page header around both tabs");
});
