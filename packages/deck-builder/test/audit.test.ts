import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { openDb } from "../src/db.ts";
import { ingestCards } from "../src/ingest.ts";
import { FIXTURES } from "./fixtures.ts";
import { addCard, createDeck, createSlot, updateCard } from "../src/deck/service.ts";
import { listCardNotes, listProposals, getLog } from "../src/deck/proposals.ts";
import {
  computeFindings,
  dismissFinding,
  promoteFinding,
  runAudit,
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
