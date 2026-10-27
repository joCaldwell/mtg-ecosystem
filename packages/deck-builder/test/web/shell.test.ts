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

test("main is the frame's second child on desktop and both tablet drawer sizes, with the page inside it", async (t) => {
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
  assert.ok(frame(800).tags[0] === "header" && frame(1024).tags[0] === "header" && frame(1280).tags[0] === "div");
});

test("collapsed sidebar keeps named navigation and an expanded-state control; drawer navigation keeps labels", async (t) => {
  const server = await vite(t);
  const { Sidebar } = await server.ssrLoadModule("/src/app/Sidebar.tsx");
  const render = (collapsed: boolean, onToggle?: () => void) => renderToStaticMarkup(createElement(Sidebar, { route: { page: "activity" }, collapsed, onToggle }));
  const expanded = render(false, () => {}), collapsed = render(true, () => {});
  assert.match(expanded, /aria-label="Collapse sidebar"[^>]*aria-expanded="true"/);
  assert.match(collapsed, /aria-label="Expand sidebar"[^>]*aria-expanded="false"/);
  for (const html of [expanded, collapsed]) {
    assert.ok(html.indexOf('href="/"') < html.indexOf('href="/inbox"'), "All decks comes first");
    assert.match(html, /href="\/activity"[^>]*aria-current="page"/);
    for (const label of ["All decks", "Inbox", "Memories", "Activity", "Profile", "Settings"]) assert.ok(html.includes(`>${label}</span>`), label);
    assert.doesNotMatch(html, /href="\/profile"[^>]*aria-current="page"/);
    assert.ok(html.indexOf('href="/memories"') < html.indexOf('href="/activity"'));
    assert.ok(html.indexOf('href="/activity"') < html.indexOf('href="/profile"'));
  }
  for (const label of ["All decks", "Inbox", "Memories", "Activity", "Profile", "Settings"]) assert.ok(collapsed.includes(`aria-label="${label}"`), label);
  assert.match(collapsed, /href="\/inbox"[^>]*title="Inbox"/);
  const tablet = render(false);
  assert.doesNotMatch(tablet, /aria-label="(?:Collapse|Expand) sidebar"/);
});

test("Profile and Activity render independent pages with their own titles and no tabs", async (t) => {
  const server = await vite(t);
  const { ProfilePage } = await server.ssrLoadModule("/src/features/profile/ProfilePage.tsx");
  const { ActivityPage } = await server.ssrLoadModule("/src/features/activity/ActivityPage.tsx");
  for (const [Component, title] of [[ProfilePage, "Profile"], [ActivityPage, "Activity"]] as const) {
    const html = renderToStaticMarkup(createElement(Component));
    assert.match(html, new RegExp(`<h1[^>]*>${title}</h1>`));
    assert.doesNotMatch(html, /role="tab(list|panel)?"/);
  }
});


test("tablet top bar names the current page and links directly to the inbox with its waiting count", async (t) => {
  const { TabletBar } = await (await vite(t)).ssrLoadModule("/src/app/TabletShell.tsx");
  const html = renderToStaticMarkup(createElement(TabletBar, { route: { page: "memories" } }));
  assert.match(html, />Memories<\/span>/);
  assert.match(html, /href="\/inbox"/);
  assert.match(html, />0<\/span>/);
  assert.match(html, /> waiting<\/span>/);
  assert.match(html, /aria-haspopup="dialog"/);
});


test("a routed inbox detail offers Back to inbox even while its data loads", async (t) => {
  const { InboxPage } = await (await vite(t)).ssrLoadModule("/src/features/inbox/InboxPage.tsx");
  const html = renderToStaticMarkup(createElement(InboxPage, { entryKey: "proposal-12" }));
  assert.match(html, /data-detail="true"/);
  assert.match(html, /<a[^>]*href="\/inbox"[^>]*>← Back to inbox<\/a>/);
  assert.ok(html.indexOf("Back to inbox") < html.indexOf("<h1"), "back precedes the page title");
});

test("inbox entries are detail links, while the phone's callback-only entry remains a button", async (t) => {
  const { InboxEntry } = await (await vite(t)).ssrLoadModule("/src/features/inbox/InboxEntry.tsx");
  const { proposal, item } = await import("./fixtures.ts");
  const entry = { key: "proposal-12", type: "proposal", created_at: "2026-10-20T17:00:00Z", proposal: proposal([item()], { id: 12 }) };
  const props = { entry, selected: true, onSelect: () => {} };
  const linked = renderToStaticMarkup(createElement(InboxEntry, { ...props, href: "/inbox?entry=proposal-12" }));
  assert.match(linked, /^<a[^>]*href="\/inbox\?entry=proposal-12"/);
  assert.match(linked, /aria-current="true"/);
  assert.doesNotMatch(linked, /<button/);
  const phone = renderToStaticMarkup(createElement(InboxEntry, props));
  assert.match(phone, /^<button[^>]*type="button"/);
  assert.doesNotMatch(phone, /href=/);
});

test("page-level help can follow h1 with h2 without changing nested help's h3 default", async (t) => {
  // This checks heading semantics; portal positioning and focus need the browser.
  const server = await createServer({
    server: { middlewareMode: true, hmr: false }, appType: "custom", logLevel: "silent",
    plugins: [{ name: "inline-popover-for-ssr", enforce: "pre", load(id) {
      if (id.endsWith("/src/ui/Popover.tsx")) return "export function Popover({ children }) { return children; }";
    } }],
  });
  t.after(() => server.close());
  const { Explainer } = await server.ssrLoadModule("/src/ui/Explainer.tsx");
  const props = { title: "Memories", summary: "Keep lessons.", children: "Details", defaultOpen: true };
  assert.match(renderToStaticMarkup(createElement(Explainer, { ...props, heading: "h2" })), /<h2[^>]*>Memories<\/h2>/);
  assert.match(renderToStaticMarkup(createElement(Explainer, props)), /<h3[^>]*>Memories<\/h3>/);
});
