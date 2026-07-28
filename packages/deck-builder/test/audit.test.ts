import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { openDb } from "../src/db.ts";
import { ingestCards } from "../src/ingest.ts";
import { FIXTURES } from "./fixtures.ts";
import { addCard, createDeck, createSlot, updateCard } from "../src/deck/service.ts";
import { listProposals } from "../src/deck/proposals.ts";
import { getLog, listCardNotes } from "../src/deck/log.ts";
import {
  AUDIT_RUN_RETENTION,
  auditState,
  computeFindings,
  dismissFinding,
  finishAuditRun,
  listAuditRuns,
  lookupFinding,
  promoteFinding,
  reclaimStaleRuns,
  runAudit,
  runningAuditRun,
  startAuditRun,
  undismissFinding,
} from "../src/deck/audit.ts";

const db = openDb(":memory:");
ingestCards(db, FIXTURES);

function keys(deckId: number): string[] {
  return computeFindings(db, deckId).map((f) => f.key);
}

describe("deterministic findings (spec §8.1)", () => {
  test("card count, no commander", () => {
    const id = createDeck(db, "Empty audit");
    const k = keys(id);
    assert.ok(k.includes("card_count"));
    assert.ok(k.includes("no_commander"));
  });
  test("ineligible commander flagged", () => {
    const id = createDeck(db, "Bad commander");
    addCard(db, id, "id-llanowar", { role: "commander" });
    assert.ok(keys(id).includes("commander_ineligible:id-llanowar"));
  });
  test("pair without partner abilities flagged as heuristic warn", () => {
    const id = createDeck(db, "Pair");
    addCard(db, id, "id-atraxa", { role: "commander" });
    addCard(db, id, "id-kenrith", { role: "commander" });
    const finding = computeFindings(db, id).find((f) => f.key === "commander_pair");
    assert.equal(finding?.severity, "warn");
  });
  test("identity, banned, singleton, slot targets", () => {
    const id = createDeck(db, "Violations");
    addCard(db, id, "id-teferi", { role: "commander" }); // mono-U
    addCard(db, id, "id-llanowar"); // G in a U deck
    addCard(db, id, "id-golos"); // banned
    addCard(db, id, "id-solring");
    addCard(db, id, "id-solring"); // 2× Sol Ring
    const slot = createSlot(db, id, "Ramp", 5, 8);
    updateCard(db, id, "id-solring", { slotId: slot });

    const k = keys(id);
    assert.ok(k.includes("identity:id-llanowar"));
    assert.ok(k.includes("banned:id-golos"));
    assert.ok(k.includes("identity:id-golos")); // five-color identity in mono-U too
    assert.ok(k.includes("singleton:id-solring"));
    assert.ok(k.includes(`slot_under:${slot}`));
  });
  test("companion condition reported as unchecked", () => {
    const id = createDeck(db, "Companion audit");
    addCard(db, id, "id-goyf", { role: "companion" });
    assert.ok(keys(id).some((k) => k.startsWith("companion_unchecked:")));
  });
});

describe("audit runs and dismissals (spec §8.3)", () => {
  test("dismissed findings are suppressed and listed separately; typed reason required", () => {
    const id = createDeck(db, "Dismiss");
    assert.throws(() => dismissFinding(db, id, "card_count", "soft", " "), /requires a reason/);
    assert.throws(() => dismissFinding(db, id, "nope", "soft", "r"), /No current finding/);

    dismissFinding(db, id, "no_commander", "soft", "still brewing, commander undecided");
    const run = runAudit(db, id);
    assert.ok(!run.findings.some((f) => f.key === "no_commander"));
    assert.equal(run.dismissed[0]?.key, "no_commander");
    assert.equal(run.dismissed[0]?.dismissal.reason, "still brewing, commander undecided");

    // logged exactly like a rejection
    const log = getLog(db, id) as any[];
    assert.equal(log[0].kind, "reject");
    assert.equal(log[0].rejection_type, "soft");

    undismissFinding(db, id, "no_commander");
    assert.ok(runAudit(db, id).findings.some((f) => f.key === "no_commander"));
  });
  test("playtest dismissal of a card finding writes a card note; thesis flags brief", () => {
    const id = createDeck(db, "Dismiss routing");
    addCard(db, id, "id-teferi", { role: "commander" });
    addCard(db, id, "id-llanowar");
    dismissFinding(db, id, "identity:id-llanowar", "playtest_finding", "house rules allow it, performs fine");
    assert.equal((listCardNotes(db, id) as any[])[0].card_name, "Llanowar Elves");

    dismissFinding(db, id, "card_count", "thesis_change", "this is a 60-card test list");
    assert.equal((getLog(db, id) as any[])[0].brief_flag, 1);
  });
  test("soft dismissals decay after revisions; harder types persist", () => {
    const id = createDeck(db, "Decay");
    dismissFinding(db, id, "no_commander", "soft", "not yet");
    dismissFinding(db, id, "card_count", "thesis_change", "intentionally small");
    // simulate 10 applied mutations
    db.prepare("UPDATE decks SET revision = revision + 10 WHERE id = ?").run(id);
    const run = runAudit(db, id);
    assert.ok(run.findings.some((f) => f.key === "no_commander")); // resurfaced
    assert.ok(!run.findings.some((f) => f.key === "card_count")); // still dismissed
  });
  test("runs are recorded with revision and instructions", () => {
    const id = createDeck(db, "History run");
    const run = runAudit(db, id, "focus on the mana base");
    assert.ok(run.run_id > 0);
    const row = db.prepare("SELECT * FROM audit_runs WHERE id = ?").get(run.run_id) as any;
    assert.equal(row.instructions, "focus on the mana base");
    assert.equal(row.deck_id, id);
  });
});

describe("recorded runs (spec §8)", () => {
  const reasoning = (slug: string, title: string) => ({
    summary: `Summary for ${slug}`,
    findings: [{ key: `reasoning:${slug}`, severity: "warn", title, detail: "Because of X." }],
    dismissed: [],
    dropped: 0,
  });

  test("a run is open while it runs and finishes asynchronously", () => {
    const id = createDeck(db, "Async run");
    const runId = startAuditRun(db, id, "focus on lands");
    assert.equal(runningAuditRun(db, id)?.id, runId);

    const [pending] = listAuditRuns(db, id);
    assert.equal(pending.status, "running");
    assert.equal(pending.instructions, "focus on lands");
    // The deterministic half is complete the moment the run opens.
    assert.ok(pending.findings.some((f) => f.key === "card_count"));
    assert.equal(pending.reasoning, null);

    finishAuditRun(db, runId, reasoning("no-win-path", "No route to winning"));
    const [done] = listAuditRuns(db, id);
    assert.equal(done.status, "done");
    assert.ok(done.finished_at);
    assert.equal(done.reasoning?.findings[0].title, "No route to winning");
    assert.equal(runningAuditRun(db, id), null);
  });

  test("a failed reasoning pass records the failure, not a lost run", () => {
    const id = createDeck(db, "Failed run");
    const runId = startAuditRun(db, id);
    finishAuditRun(db, runId, null, "provider returned 500");
    const [run] = listAuditRuns(db, id);
    assert.equal(run.status, "error");
    assert.equal(run.error, "provider returned 500");
    // The deterministic findings still stand.
    assert.ok(run.findings.length > 0);
  });

  test("runs in flight at shutdown are reclaimed, never left claiming to run", () => {
    const id = createDeck(db, "Reclaim");
    startAuditRun(db, id);
    assert.ok(reclaimStaleRuns(db) >= 1);
    assert.equal(runningAuditRun(db, id), null);
    assert.equal(listAuditRuns(db, id)[0].status, "error");
  });

  test(`only the newest ${AUDIT_RUN_RETENTION} runs per deck are kept, and per deck`, () => {
    const id = createDeck(db, "Retention");
    const other = createDeck(db, "Retention other");
    const otherRun = runAudit(db, other).run_id;
    const ids: number[] = [];
    for (let i = 0; i < AUDIT_RUN_RETENTION + 3; i++) ids.push(runAudit(db, id, `run ${i}`).run_id);

    const kept = listAuditRuns(db, id);
    assert.equal(kept.length, AUDIT_RUN_RETENTION);
    assert.deepEqual(
      kept.map((r) => r.id),
      ids.slice(-AUDIT_RUN_RETENTION).reverse(),
    );
    assert.equal(listAuditRuns(db, other)[0].id, otherRun);
  });

  test("the section reads live checks plus the newest stored reasoning", () => {
    const id = createDeck(db, "Section state");
    runAudit(db, id, "", reasoning("slow-start", "Slow start"));
    // A run with no reasoning must not hide the last one that had it.
    runAudit(db, id);

    const state = auditState(db, id);
    assert.ok(state.findings.some((f) => f.key === "card_count")); // live, recomputed
    assert.equal(state.runs.length, 2);
    assert.equal(state.reasoning_run?.reasoning?.findings[0].title, "Slow start");
    assert.ok(state.reasoning_run!.id < state.runs[0].id);
  });

  test("reasoning findings stay dismissable after newer runs land", () => {
    const id = createDeck(db, "Old finding");
    const first = runAudit(db, id, "", reasoning("fragile-engine", "Engine has one copy"));
    for (let i = 0; i < 3; i++) runAudit(db, id);

    // Still reachable by key alone, and by its own run id.
    const hit = lookupFinding(db, id, "reasoning:fragile-engine", first.run_id);
    assert.equal(hit?.source, "reasoning");
    assert.equal(hit?.run?.id, first.run_id);
    assert.equal(lookupFinding(db, id, "reasoning:fragile-engine")?.finding.title, "Engine has one copy");

    dismissFinding(db, id, "reasoning:fragile-engine", "thesis_change", "one copy is the plan");
    assert.equal(
      lookupFinding(db, id, "reasoning:fragile-engine")?.dismissal?.reason,
      "one copy is the plan",
    );
  });

  test("a live deterministic finding resolves with no run at all", () => {
    const id = createDeck(db, "Live lookup");
    const hit = lookupFinding(db, id, "card_count");
    assert.equal(hit?.source, "deterministic");
    assert.equal(hit?.run, null);
    assert.equal(lookupFinding(db, id, "reasoning:never-happened"), null);
  });
});

describe("promotion (queue-to-promote knob)", () => {
  test("actionable finding becomes an audit-source proposal", () => {
    const id = createDeck(db, "Promote");
    addCard(db, id, "id-teferi", { role: "commander" });
    addCard(db, id, "id-golos");
    const pid = promoteFinding(db, id, "banned:id-golos");
    const [p] = listProposals(db, id, "open");
    assert.equal(p.id, pid);
    assert.equal(p.source, "audit");
    assert.equal(p.items[0].action, "cut");
    assert.equal(p.items[0].card_name, "Golos, Tireless Pilgrim");
    assert.match(p.items[0].rationale, /banned/i);
  });
  test("non-actionable findings refuse promotion", () => {
    const id = createDeck(db, "No promote");
    assert.throws(() => promoteFinding(db, id, "card_count"), /no directly promotable/);
  });
});
