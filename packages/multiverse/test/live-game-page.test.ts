import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

// Execute the actual component/effect with deterministic hooks and deferred API
// responses. These are UI races, not native rules or gameplay acceptance.
const source = readFileSync(new URL("../web/src/features/live-game/LiveGamePage.tsx", import.meta.url), "utf8");
const code = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX, target: ts.ScriptTarget.ES2022 } }).outputText;
const gameId = "a".repeat(32);
function deferred() {
  let resolve!: (value: unknown) => void, reject!: (error: unknown) => void;
  const promise = new Promise((done, fail) => { resolve = done; reject = fail; });
  return { promise, resolve, reject };
}
async function flush() { for (let i = 0; i < 25; i++) await Promise.resolve(); }
interface Element { type: string; props: Record<string, any> }
function elements(value: any): Element[] {
  if (Array.isArray(value)) return value.flatMap(elements);
  return value?.props ? [value, ...elements(value.props.children)] : [];
}
function text(value: any): string {
  if (Array.isArray(value)) return value.map(text).join(" ");
  return value?.props ? text(value.props.children) : typeof value === "string" ? value : "";
}
function rig() {
  const slots: any[] = [], reads: ReturnType<typeof deferred>[] = [], grants: ReturnType<typeof deferred>[] = [];
  const timers = new Map<number, { at: number; run: () => void }>();
  let cursor = 0, now = 0, nextTimer = 0, cleanup: (() => void) | undefined;
  let dependencies: unknown[] | undefined, pendingEffect: (() => (() => void)) | undefined;
  class ApiError extends Error {
    status: number;
    constructor(status: number) { super("Fixture API error"); this.status = status; }
  }
  const jsx = (type: string, props: Record<string, any>) => ({ type, props });
  const hooks = {
    useState(initial: unknown) {
      const slot = cursor++;
      if (!(slot in slots)) slots[slot] = initial;
      return [slots[slot], (next: any) => { slots[slot] = typeof next === "function" ? next(slots[slot]) : next; }];
    },
    useEffect(effect: () => (() => void), deps: unknown[]) {
      if (!dependencies || deps.some((value, index) => value !== dependencies![index])) { dependencies = deps; pendingEffect = effect; }
    },
  };
  const exported: Record<string, any> = {};
  const context = vm.createContext({
    exports: exported, AbortController, File, TextEncoder,
    setTimeout(run: () => void, ms: number) { const id = ++nextTimer; timers.set(id, { at: now + ms, run }); return id; },
    clearTimeout(id: number) { timers.delete(id); },
    require(name: string) {
      if (name === "react") return hooks;
      if (name === "react/jsx-runtime") return { jsx, jsxs: jsx };
      if (name === "@multiverse/game-client/session") return { GameSession: "GameSession" };
      if (name === "../../api/client.ts") return { ApiError, api: {
        getGame() { const d = deferred(); reads.push(d); return d.promise; },
        humanGameSeat() { const d = deferred(); grants.push(d); return d.promise; },
      } };
      if (name === "../../app/routes.ts") return { href: () => "/play" };
      if (name === "../../ui/index.ts") return Object.fromEntries(["Button", "ButtonLink", "ErrorNote", "Loading", "Page", "PageHeader"].map(key => [key, key]));
      throw new Error(`Unexpected component import: ${name}`);
    },
  });
  new vm.Script(code).runInContext(context);
  const render = () => {
    cursor = 0;
    const view = exported.LiveGamePage({ gameId });
    if (pendingEffect) { const effect = pendingEffect; pendingEffect = undefined; cleanup?.(); cleanup = effect(); }
    return view;
  };
  render();
  return { reads, grants, render, ApiError,
    status(state: string, outcome: unknown = null) { return { game: { id: gameId, state, outcome, pause: state === "paused" ? "human_timeout" : null } }; },
    nextPoll() {
      const entry = [...timers].sort((a, b) => a[1].at - b[1].at)[0];
      assert.ok(entry, "Expected a scheduled status read");
      timers.delete(entry[0]); now = entry[1].at; entry[1].run();
    },
    stop() { cleanup?.(); }, timers,
  };
}

test("paused status, late grant, then active status offers retry and reacquires a fresh File", async () => {
  const r = rig();
  try {
    r.reads[0]!.resolve(r.status("paused")); await flush();
    r.grants[0]!.resolve({ bootstrap_file: "{}" }); await flush();
    assert.equal(elements(r.render()).some(node => node.type === "GameSession"), false);
    r.nextPoll(); r.reads[1]!.resolve(r.status("active")); await flush();
    const retry = elements(r.render()).find(node => node.type === "Button" && node.props.children === "Try again");
    assert.ok(retry, "Discarded grants must expose a retry once active");
    retry.props.onClick(); r.render();
    assert.equal(r.grants.length, 2);
    r.reads[2]!.resolve(r.status("active")); r.grants[1]!.resolve({ bootstrap_file: "{}" }); await flush();
    const session = elements(r.render()).find(node => node.type === "GameSession");
    assert.ok(session?.props.bootstrapFile instanceof File);
    assert.equal(session.props.interactive, true);
  } finally { r.stop(); }
});

test("account denial fences a late successful grant and exposes no game session", async () => {
  const r = rig();
  try {
    r.reads[0]!.reject(new r.ApiError(404)); await flush();
    r.grants[0]!.resolve({ bootstrap_file: "{}" }); await flush();
    assert.equal(elements(r.render()).some(node => node.type === "GameSession"), false);
    assert.match(text(r.render()), /no longer available from your account/);
    assert.equal(r.timers.size, 0);
  } finally { r.stop(); }
});

test("durable outcomes disable only commands; transport failure does not invent an outcome", async () => {
  const r = rig();
  try {
    r.reads[0]!.resolve(r.status("active")); r.grants[0]!.resolve({ bootstrap_file: "{}" }); await flush();
    const originalFile = elements(r.render()).find(node => node.type === "GameSession")!.props.bootstrapFile;
    r.nextPoll(); r.reads[1]!.reject(new Error("network")); await flush();
    assert.doesNotMatch(text(r.render()), /Game complete|You won|draw/);
    r.nextPoll(); r.reads[2]!.resolve(r.status("completed", { kind: "win", winner: 0 })); await flush();
    const view = r.render(), session = elements(view).find(node => node.type === "GameSession")!;
    assert.match(text(view), /Game complete/); assert.match(text(view), /You won/);
    assert.equal(session.props.interactive, false);
    assert.equal(session.props.bootstrapFile, originalFile, "Result updates preserve the independent session/journal lifetime");
    assert.equal(r.timers.size, 0);
  } finally { r.stop(); }
});

test("unmount fences all deferred replies and clears status/grant deadlines", async () => {
  const r = rig(); r.stop();
  r.reads[0]!.resolve(r.status("active")); r.grants[0]!.resolve({ bootstrap_file: "{}" }); await flush();
  assert.equal(elements(r.render()).some(node => node.type === "GameSession"), false);
  assert.equal(r.timers.size, 0);
});

for (const [pause, cause] of [["human_timeout", "human decision time limit"], ["lifetime_exceeded", "game time limit"]]) {
  test(`interrupted ${pause} displays its recorded cause without inventing a rules outcome`, async () => {
    const r = rig();
    try {
      r.reads[0]!.resolve(r.status("active")); r.grants[0]!.resolve({ bootstrap_file: "{}" }); await flush();
      const originalFile = elements(r.render()).find(node => node.type === "GameSession")!.props.bootstrapFile;
      r.nextPoll();
      r.reads[1]!.resolve({ game: { id: gameId, state: "interrupted", outcome: null, pause } }); await flush();
      const view = r.render(), session = elements(view).find(node => node.type === "GameSession")!;
      assert.match(text(view), /Game interrupted/);
      assert.ok(text(view).includes(`The game stopped after the ${cause}.`));
      assert.doesNotMatch(text(view), /Game complete|You won|draw|to resume/);
      assert.equal(session.props.interactive, false);
      assert.equal(session.props.bootstrapFile, originalFile, "Timeout is independent of command-journal disposition");
      assert.equal(r.timers.size, 0);
    } finally { r.stop(); }
  });
}
