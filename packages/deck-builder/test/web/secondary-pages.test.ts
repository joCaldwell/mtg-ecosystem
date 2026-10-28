// Render through the real Vite pipeline. Capture the shared controls' callbacks
// to exercise the inbox/API seam without adding a DOM test dependency.
import { test } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "vite";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { readFileSync } from "node:fs";
import { deckState, item, proposal } from "./fixtures.ts";

async function harness(t: { after: (fn: () => unknown) => void }, deckQuery = false) {
  const calls: unknown[][] = [];
  const store = {
    waiting: 0, inbox: null,
    apply: (value: unknown) => calls.push(["inbox", value]),
    state: deckState(), controls: null as any, entries: [] as any[],
    review: {
      confirmation: null,
      accept: (...args: unknown[]) => calls.push(["accept", ...args]),
      reject: (...args: unknown[]) => calls.push(["reject", ...args]),
      acceptAll: (...args: unknown[]) => calls.push(["acceptAll", ...args]),
      rejectAll: (...args: unknown[]) => calls.push(["rejectAll", ...args]),
    },
  };
  (globalThis as any).__secondaryPagesTest = store;
  const server = await createServer({
    server: { middlewareMode: true, hmr: false }, appType: "custom", logLevel: "silent",
    plugins: [{ name: "secondary-page-boundaries", enforce: "pre", load(id) {
      if (deckQuery && id.endsWith("/src/api/useQuery.ts")) return `export function useQuery() { return { data: { state: globalThis.__secondaryPagesTest.state }, loading: false, error: null, reload() {} }; }`;
      if (id.endsWith("/src/ui/Toast.tsx")) return `export function ToastProvider({children}) { return children; } export function useToast() { return { toast() {} }; }`;
      if (id.endsWith("/src/features/inbox/InboxEntry.tsx")) return `export function InboxEntry(props) { globalThis.__secondaryPagesTest.entries.push(props); return null; }`;
      if (id.endsWith("/src/app/Inbox.tsx")) return `export function useInbox() { return globalThis.__secondaryPagesTest; }`;
      if (id.endsWith("/src/features/deck/index.ts")) return `
        import { createElement } from 'react';
        export function useDeckState() { return globalThis.__secondaryPagesTest.state; }
        export function useReview() { return globalThis.__secondaryPagesTest.review; }
        export function ReviewItemLine({item}) { return createElement('p', null, item.card_name); }
        export function DeckProvider({children}) { return children; }
        export function ReadOnlyDeck() { return null; }
        export function useDeck() { return {}; }
        export function useDeckView() { return {}; }
      `;
      if (id.endsWith("/src/features/decisions/DecisionControls.tsx")) return `
        export function DecisionControls(props) { globalThis.__secondaryPagesTest.controls = props; return null; }
      `;
    } }],
  });
  t.after(async () => { await server.close(); delete (globalThis as any).__secondaryPagesTest; });
  return { server, store, calls };
}

test("inbox decisions send only the selected proposal, its optional reason and the reviewed revision", async t => {
  const { server, store, calls } = await harness(t);
  const { ProposalDecision } = await server.ssrLoadModule("/src/features/inbox/ProposalDecision.tsx");
  const { api } = await server.ssrLoadModule("/src/api/client.ts");
  const { ToastProvider } = await server.ssrLoadModule("/src/ui/Toast.tsx");
  const selected = proposal([item()], { id: 22, deck_id: 7 });
  const other = proposal([item()], { id: 11, deck_id: 7 });
  const state = deckState({ proposals: [other, selected] });
  const next = { proposals: [], document_edits: [], unreviewed: 0 };
  api.ruleProposal = async (...args: unknown[]) => { calls.push(["api", ...args]); return { state, inbox: next }; };
  const render = (p = selected, s = state) => renderToStaticMarkup(createElement(ToastProvider, null,
    createElement(ProposalDecision, { proposal: p, state: s, onDecided: () => calls.push(["reload"]) })));
  render();
  assert.equal(store.controls.reason.label, "Reason for this proposal, optional");
  store.controls.onAccept("  Fits this plan  ");
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(calls, [["api", 7, 22, "accept", "Fits this plan", { expected_revision: state.deck.revision }], ["inbox", next], ["reload"]]);
  calls.length = 0;
  render();
  store.controls.onReject("");
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(calls, [["api", 7, 22, "reject", "", { expected_revision: state.deck.revision }], ["inbox", next], ["reload"]]);

  calls.length = 0;
  const stale = { ...selected, stale: true };
  render(stale, { ...state, proposals: [stale] });
  store.controls.onAccept("Review first");
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(calls, [], "stale acceptance waits for confirmation");
  store.controls.onReject("No longer fits");
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(calls[0]?.[3], "reject", "stale rejection does not require acceptance confirmation");

  store.controls = null;
  assert.match(render(selected, { ...state, deck: { ...state.deck, category: "archived" } }), /read-only/);
  assert.equal(store.controls, null, "archived decks have no mutation controls");
  assert.match(render(selected, { ...state, proposals: [] }), /no longer waiting/);
});

test("an inbox decision failure never applies an inbox result, and an in-flight decision cannot submit twice", async t => {
  const { server, store, calls } = await harness(t);
  const { ProposalDecision } = await server.ssrLoadModule("/src/features/inbox/ProposalDecision.tsx");
  const { api } = await server.ssrLoadModule("/src/api/client.ts");
  const { ToastProvider } = await server.ssrLoadModule("/src/ui/Toast.tsx");
  const selected = proposal([item()]);
  let fail: (error: Error) => void = () => {};
  api.ruleProposal = (...args: unknown[]) => { calls.push(args); return new Promise((_resolve, reject) => { fail = reject; }); };
  renderToStaticMarkup(createElement(ToastProvider, null, createElement(ProposalDecision, {
    proposal: selected, state: deckState({ proposals: [selected] }), onDecided: () => calls.push(["reload"]),
  })));
  store.controls.onAccept("Keep this reason");
  store.controls.onAccept("Keep this reason");
  assert.equal(calls.length, 1);
  fail(new Error("Revision changed"));
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(calls.length, 1, "failure does not apply the inbox or reload as a success");
});

test("phone decisions put the selected proposal first and keep bulk decisions inside a closed labelled disclosure", async t => {
  const { server, store } = await harness(t);
  const { PhoneDecisions } = await server.ssrLoadModule("/src/features/phone/PhoneDecisions.tsx");
  const { reviewUnits } = await server.ssrLoadModule("/src/domain/review.ts");
  const older = proposal([item({ name: "Test Card A" })], { id: 11, summary: "Older proposal" });
  const selected = proposal([item({ name: "Test Card B" })], { id: 22, summary: "Selected proposal" });
  store.state = deckState({ proposals: [older, selected] });
  const view = { units: reviewUnits(store.state.proposals) };
  const html = renderToStaticMarkup(createElement(PhoneDecisions, { view, proposalId: 22 }));
  const disclosure = html.indexOf("<details");
  assert.ok(html.indexOf("Selected proposal") < disclosure);
  assert.ok(html.indexOf('aria-label="Reason for decision 2, optional"') < html.indexOf(">Accept<"));
  assert.ok(html.indexOf(">Accept<") < disclosure && html.indexOf(">Reject<") < disclosure);
  assert.ok(html.indexOf("Older proposal") > disclosure);
  assert.match(html, /<details[^>]*><summary>Review all decisions · 2<\/summary>/);
  assert.doesNotMatch(html, /<details[^>]*\bopen/);
  assert.ok(html.indexOf(">Accept all<") > disclosure && html.indexOf(">Reject all<") > disclosure);
  assert.equal((html.match(/Reason for decision/g) ?? []).length, 2, "every individual decision stays available");
  const remaining = { units: view.units.filter((unit: { proposalId: number }) => unit.proposalId !== 22) };
  const resolved = renderToStaticMarkup(createElement(PhoneDecisions, { view: remaining, proposalId: 22 }));
  assert.doesNotMatch(resolved, /This proposal has no pending decisions/);
  assert.match(resolved, />Proposal 11</);
  assert.ok(resolved.indexOf("Older proposal") < resolved.indexOf("<details"), "resolving the selected proposal exposes the next pending decision directly");
});

test("phone inbox links retain the proposal selection", async t => {
  const { server, store } = await harness(t);
  const { PhoneInbox } = await server.ssrLoadModule("/src/features/phone/PhoneInbox.tsx");
  const selected = proposal([item()], { id: 22, deck_id: 7 });
  store.inbox = { proposals: [selected], document_edits: [], unreviewed: 0 } as any;
  renderToStaticMarkup(createElement(PhoneInbox));
  assert.equal(store.entries.length, 1);
  const saved = Object.fromEntries(["window", "location", "history"].map(key => [key, (globalThis as any)[key]]));
  const paths: string[] = [];
  Object.assign(globalThis, {
    window: { dispatchEvent() {}, scrollTo() {} },
    location: { pathname: "/inbox", search: "", hash: "" },
    history: { state: null, replaceState() {}, pushState(_state: unknown, _title: string, path: string) { paths.push(path); } },
  });
  try { store.entries[0].onSelect(); }
  finally { Object.assign(globalThis, saved); }
  assert.deepEqual(paths, ["/deck/7/review?proposal=22"]);
});

test("phone, tablet, expanded and collapsed desktop navigation hide empty badges and expose positive counts", async t => {
  const { server, store } = await harness(t);
  const { PhoneShell } = await server.ssrLoadModule("/src/features/phone/PhoneShell.tsx");
  const { TabletBar } = await server.ssrLoadModule("/src/app/TabletShell.tsx");
  const { Sidebar } = await server.ssrLoadModule("/src/app/Sidebar.tsx");
  for (const [Component, props] of [[PhoneShell, { route: { page: "inbox" } }], [TabletBar, { route: { page: "inbox" } }], [Sidebar, { route: { page: "inbox" }, collapsed: false }], [Sidebar, { route: { page: "inbox" }, collapsed: true }]] as const) {
    store.waiting = 0;
    const empty = renderToStaticMarkup(createElement(Component, props));
    assert.doesNotMatch(empty, />0<\/span>/);
    store.waiting = 7;
    const waiting = renderToStaticMarkup(createElement(Component, props));
    assert.match(waiting, />7<\/span>/);
    assert.match(waiting, /href="\/inbox"/);
  }
  const css = readFileSync(new URL("../../web/src/ui/Badge.module.css", import.meta.url), "utf8");
  assert.match(css, /\.agent\s*\{\s*background: var\(--agent\);\s*color: var\(--bg\);/);
});


test("inbox presents selected changes before a closed aggregate projection with server-provided totals", async t => {
  const { server, store } = await harness(t, true);
  const { ProposalSummary } = await server.ssrLoadModule("/src/features/inbox/ProposalSummary.tsx");
  const selected = proposal([item()], { id: 22, summary: "Selected changes" });
  const other = proposal([item()], { id: 11, summary: "Other changes" });
  store.state = deckState({ proposals: [other, selected] });
  store.state.computed.pending.projected_count = 103;
  const html = renderToStaticMarkup(createElement(ProposalSummary, { proposal: selected }));
  assert(html.indexOf("Changes in this proposal") < html.indexOf("<details"));
  assert.match(html, /<details[^>]*><summary>Deck impact if all 2 pending proposals are accepted<\/summary>/);
  assert.doesNotMatch(html, /<details[^>]*\bopen/);
  assert(html.indexOf("103") > html.indexOf("<details"), "all-proposal totals belong to the labelled aggregate only");
  assert.equal(store.controls.reason.label, "Reason for this proposal, optional");
  store.state = deckState({ proposals: [selected] });
  const single = renderToStaticMarkup(createElement(ProposalSummary, { proposal: selected }));
  assert.doesNotMatch(single, /<details/);
  assert.match(single, /1 pending proposal is/);
});
