// The reason field the desktop review shares with the phone (UX pass 1): shown
// above Accept and Reject and sent with either decision. Rendered through the
// package's Vite pipeline; the buttons' handlers are read from the element
// tree, since the package has no DOM test dependency.
import { test } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "vite";
import { renderToStaticMarkup } from "react-dom/server";
import { createElement, isValidElement, type ReactElement, type ReactNode } from "react";

async function load(t: { after: (fn: () => unknown) => void }) {
  const server = await createServer({ server: { middlewareMode: true, hmr: false }, appType: "custom", logLevel: "silent" });
  t.after(() => server.close());
  return server.ssrLoadModule("/src/features/decisions/DecisionControls.tsx");
}

/** Every element in a tree, depth first. */
function elements(node: ReactNode): ReactElement<any>[] {
  if (Array.isArray(node)) return node.flatMap(elements);
  if (!isValidElement(node)) return [];
  return [node, ...elements((node.props as { children?: ReactNode }).children)];
}

function controls(reason: string | undefined) {
  const calls: Array<[string, string | undefined]> = [];
  const props = {
    rejecting: false,
    onAccept: (text?: string) => calls.push(["accept", text]),
    onStartReject: () => calls.push(["startReject", undefined]),
    onReject: (text: string) => calls.push(["reject", text]),
    onCancelReject: () => calls.push(["cancel", undefined]),
    ...(reason === undefined ? {} : {
      reason: { value: reason, onChange: () => {}, label: "Reason for decision 1, optional", placeholder: "Why? Optional." },
    }),
  };
  return { props, calls };
}

test("a decision with a reason shows the labelled field above Accept and Reject", async (t) => {
  const { DecisionControls } = await load(t);
  const html = renderToStaticMarkup(createElement(DecisionControls, controls("Too slow for this table").props));
  const field = html.indexOf('aria-label="Reason for decision 1, optional"');
  assert.ok(field >= 0, "the reason field is there");
  assert.doesNotMatch(html, /placeholder=/, "the persistent label supplies the prompt");
  assert.ok(html.includes('value="Too slow for this table"'));
  assert.ok(field < html.indexOf(">Accept<") && field < html.indexOf(">Reject<"), "the field comes before both buttons");
});

test("Accept and Reject each send the reason; Reject decides at once", async (t) => {
  const { DecisionControls } = await load(t);
  const { props, calls } = controls("Too slow for this table");
  const buttons = elements(DecisionControls(props)).filter((e) => e.props.variant === "accept" || e.props.variant === "reject");
  assert.equal(buttons.length, 2);
  for (const b of buttons) b.props.onClick();
  assert.deepEqual(calls, [["accept", "Too slow for this table"], ["reject", "Too slow for this table"]]);
});

test("without a reason field, Reject still opens the reason step first", async (t) => {
  const { DecisionControls } = await load(t);
  const { props, calls } = controls(undefined);
  const html = renderToStaticMarkup(createElement(DecisionControls, props));
  assert.ok(!html.includes("<input"), "no field until Reject is pressed");
  for (const b of elements(DecisionControls(props)).filter((e) => e.props.variant === "accept" || e.props.variant === "reject")) b.props.onClick();
  assert.deepEqual(calls, [["accept", undefined], ["startReject", undefined]]);
});

test("the reject key's reason field starts from what was typed above", async (t) => {
  const { DecisionControls } = await load(t);
  const { props } = controls("Too slow for this table");
  const html = renderToStaticMarkup(createElement(DecisionControls, { ...props, rejecting: true }));
  assert.ok(html.includes('aria-label="Reason for rejecting, optional"'));
  assert.ok(html.includes('value="Too slow for this table"'));
});
