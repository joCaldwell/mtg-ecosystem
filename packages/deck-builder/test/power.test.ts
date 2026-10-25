import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { PermissionError } from "../src/actor.ts";
import { OWNER } from "./actors.ts";
import { listActivity } from "../src/activity.ts";
import { checkDeck } from "../src/deck/checks.ts";
import { forkDeck } from "../src/deck/fork.ts";
import { PowerProfileSchema, gameChangerFacts, getPower, mapBracket, recordPowerAssessment, setPowerTarget, writePowerTarget } from "../src/deck/power.ts";
import { deleteDeck, getDeck, setDeckCategory, shortlistCard } from "../src/deck/service.ts";
import { listDocumentEdits, proposeDocumentEdit, ruleDocumentEdit } from "../src/documents.ts";
import { ingestCards } from "../src/ingest.ts";
import { callTool } from "../src/mcp/tools.ts";
import { FIXTURES } from "./fixtures.ts";
import { AGENT, add, newDeck, testDb } from "./helpers.ts";

const profile = (patch: object = {}) => PowerProfileSchema.parse({
  intent: "social", win_turn: { min: 9, max: 11 }, consistency: "reliable", resilience: "recovering",
  interaction: "flexible", win_style: "incremental", combos: "none", extra_turns: "none", mass_land_denial: false, ...patch,
});
const target = (patch: object = {}) => ({ profile: profile(patch), game_changers_max: 0, notes: "Low pressure games with room to develop." });
const assessment = (db: ReturnType<typeof testDb>, id: number, patch: object = {}) => ({
  profile: profile(patch), revision: getDeck(db, OWNER, id).deck.revision,
  evidence: "list", confidence: "tentative", notes: "Untested list estimate; validate timing with actual games.",
});

describe("power profile → community brackets", () => {
  test("uses intent and exact turn expectations, without turning speed into cEDH", () => {
    for (const [min, intent, expected] of [[10, "theme", 1], [9, "theme", 2], [9, "social", 2], [8, "social", 3], [7, "social", 3], [6, "social", 4], [1, "social", 4], [12, "competitive", 5]] as const)
      assert.equal(mapBracket(profile({ intent, win_turn: { min, max: min + 2 } }), 0).bracket, expected);
    assert.equal(mapBracket(profile(), 0).provisional, false);
    assert.ok(mapBracket(profile({ win_turn: { min: 5, max: 7 } }), 0).conflicts.length);
  });
  test("does not average away restricted patterns or misclassify outliers", () => {
    assert.equal(mapBracket(profile({ early_win_turn: 4 }), 0).bracket, 2);
    assert.equal(mapBracket(profile({ resilience: "fragile", combos: "early_two_card" }), 0).bracket, 4);
    assert.equal(mapBracket(profile({ combos: "late_two_card" }), 0).bracket, 3);
    assert.equal(mapBracket(profile({ combos: "multi_card" }), 0).bracket, 2);
    assert.equal(mapBracket(profile({ extra_turns: "chained" }), 0).bracket, 4);
    assert.equal(mapBracket(profile({ intent: "theme", win_turn: { min: 10, max: 12 }, extra_turns: "single" }), 0).bracket, 2);
    assert.equal(mapBracket(profile({ mass_land_denial: true }), 0).bracket, 4);
    assert.equal(mapBracket(profile({ win_style: "burst" }), 0).bracket, 3);
    assert.equal(mapBracket(profile(), 3).bracket, 3);
    assert.equal(mapBracket(profile(), 4).bracket, 4);
    const incomplete = mapBracket(profile(), 4, { incompleteGameChangers: true });
    assert.equal(incomplete.bracket, 4);
    assert.equal(incomplete.provisional, true);
  });
  test("unknowns remain visible and Exhibition exceptions require discussion", () => {
    const empty = mapBracket(PowerProfileSchema.parse({}), null);
    assert.equal(empty.bracket, null);
    assert.equal(empty.provisional, true);
    assert.ok(empty.unknowns.includes("Mass land denial"));
    assert.equal(mapBracket(profile(), null).provisional, true);
    const exception = mapBracket(profile({ intent: "theme", win_turn: { min: 10, max: 12 } }), 1);
    assert.ok(exception.conflicts.some(s => s.includes("pod agreement")));
  });
});

describe("power capabilities and document permissions", () => {
  test("all agent targets require approval and retain honest diffs", () => {
    const db = testDb(); const id = newDeck(db, "theorycrafted");
    assert.throws(() => setPowerTarget(db, AGENT, id, target()), PermissionError);
    const r = proposeDocumentEdit(db, AGENT, id, "power_target", { target: target() }, "Owner requested low pressure games.");
    assert.equal(r.applied, false);
    assert.equal(getPower(db, OWNER, id).target, null);
    ruleDocumentEdit(db, OWNER, r.edit_id!, "accept");
    assert.equal(getPower(db, OWNER, id).target!.mapping.bracket, 2);
    assert.equal(getDeck(db, OWNER, id).deck.revision, 0);
    setDeckCategory(db, OWNER, id, "paper");
    assert.throws(() => writePowerTarget(db, AGENT, id, target()), PermissionError);
    const next = target({ intent: "synergy" });
    const pending = proposeDocumentEdit(db, AGENT, id, "power_target", { target: next }, "Owner asked for more synergy.");
    assert.equal(pending.applied, false);
    const edit = listDocumentEdits(db, OWNER, { deckId: id })[0];
    assert.match(edit.payload.base!, /Intent: Social/);
    assert.match(edit.payload.content!, /Intent: Strong synergy/);
    assert.equal(getPower(db, OWNER, id).target!.profile.intent, "social");
    setPowerTarget(db, OWNER, id, target({ intent: "optimized" }));
    assert.equal(listDocumentEdits(db, OWNER, { deckId: id })[0].stale, true);
    assert.throws(() => ruleDocumentEdit(db, OWNER, pending.edit_id!, "accept"), /stale/);
    ruleDocumentEdit(db, OWNER, pending.edit_id!, "accept", "Reviewed changed target", { acknowledge_stale: true, expected_revision: 0 });
    assert.equal(getPower(db, OWNER, id).target!.profile.intent, "synergy");
    assert.deepEqual(listActivity(db, OWNER).map(a => a.action), ["propose_power_target", "propose_power_target"]);
    assert.equal(getDeck(db, OWNER, id).deck.revision, 0);
    db.close();
  });
  test("targets can be rejected or cleared, without changing the assessment", () => {
    const db = testDb(); const id = newDeck(db);
    recordPowerAssessment(db, AGENT, id, assessment(db, id));
    const r = proposeDocumentEdit(db, AGENT, id, "power_target", { target: target() }, "A tentative target.");
    ruleDocumentEdit(db, OWNER, r.edit_id!, "reject");
    assert.equal(getPower(db, OWNER, id).target, null);
    setPowerTarget(db, OWNER, id, target());
    setPowerTarget(db, OWNER, id, null);
    assert.equal(getPower(db, OWNER, id).target, null);
    assert.equal(getPower(db, OWNER, id).assessment!.id, 1);
    db.close();
  });
  test("agent assessments are attributed, owner corrections prevail, and changed lists need review", () => {
    const db = testDb(); const id = newDeck(db);
    add(db, id, "id-solring");
    const rev = getDeck(db, OWNER, id).deck.revision;
    recordPowerAssessment(db, AGENT, id, assessment(db, id));
    assert.equal(getPower(db, OWNER, id).assessment!.author, "test-agent");
    recordPowerAssessment(db, OWNER, id, assessment(db, id, { consistency: "variable" }));
    assert.throws(() => recordPowerAssessment(db, AGENT, id, assessment(db, id)), PermissionError);
    assert.equal(getPower(db, OWNER, id).history.length, 2);
    add(db, id, "id-counterspell");
    assert.equal(getPower(db, OWNER, id).assessment!.stale, true);
    assert.ok(checkDeck(db, OWNER, id).some(f => f.key === "power_assessment_stale"));
    assert.throws(() => recordPowerAssessment(db, AGENT, id, { ...assessment(db, id), revision: rev }), /re-read/);
    recordPowerAssessment(db, AGENT, id, assessment(db, id));
    assert.equal(getPower(db, OWNER, id).assessment!.stale, false);
    assert.equal(getPower(db, OWNER, id).history.length, 3);
    assert.equal(listActivity(db, OWNER).filter(a => a.action === "assess_deck_power").length, 2);
    db.close();
  });
  test("differences are computed only for a current assessment; a range inside the target fits", () => {
    const db = testDb(); const id = newDeck(db);
    setPowerTarget(db, OWNER, id, target());
    recordPowerAssessment(db, AGENT, id, assessment(db, id, { win_turn: { min: 9, max: 10 } }));
    assert.deepEqual(getPower(db, OWNER, id).differences, []);
    recordPowerAssessment(db, AGENT, id, assessment(db, id, { resilience: "fragile", win_turn: { min: 7, max: 9 } }));
    assert.deepEqual(getPower(db, OWNER, id).differences.map(d => d.field), ["Resilience", "Usual win range"]);
    assert.ok(checkDeck(db, OWNER, id).some(f => f.key === "power_target_gap"));
    add(db, id, "id-solring");
    assert.deepEqual(getPower(db, OWNER, id).differences, []);
    db.close();
  });
  test("strict validation and prose linting fail atomically below both front doors", () => {
    const db = testDb(); const id = newDeck(db, "theorycrafted");
    for (const value of [undefined, {}, { ...target(), game_changers_max: -1 }, { ...target(), profile: { ...profile(), win_turn: { min: 10, max: 8 } } }, { ...target(), profile: { ...profile(), early_win_turn: 12 } }, { ...target(), profile: { ...profile(), consistency: "excellent" } }])
      assert.throws(() => setPowerTarget(db, OWNER, id, value), /Invalid power target/);
    assert.throws(() => setPowerTarget(db, OWNER, id, { ...target(), notes: "[[Imaginary Power Card]]" }), /did not resolve/);
    assert.throws(() => recordPowerAssessment(db, AGENT, id, { ...assessment(db, id), game_changers: 0 }), /Invalid power assessment/);
    assert.throws(() => recordPowerAssessment(db, AGENT, id, { ...assessment(db, id), notes: "" }), /Invalid power assessment/);
    assert.throws(() => proposeDocumentEdit(db, AGENT, id, "power_target", { target: target() }, "[[Imaginary Power Card]]"), /did not resolve/);
    assert.equal(getPower(db, OWNER, id).assessment, null);
    assert.equal(getPower(db, OWNER, id).target, null);
    assert.deepEqual(listActivity(db, OWNER), []);
    db.close();
  });
  test("forks inherit the target, start unassessed, and delete cascades preserve the fork", () => {
    const db = testDb(); const id = newDeck(db);
    setPowerTarget(db, OWNER, id, target());
    recordPowerAssessment(db, OWNER, id, assessment(db, id));
    const fork = forkDeck(db, AGENT, id, "Power fork");
    assert.equal(getPower(db, OWNER, fork).target!.profile.intent, "social");
    assert.equal(getPower(db, OWNER, fork).assessment, null);
    setPowerTarget(db, OWNER, fork, target({ intent: "synergy" }));
    assert.equal(getPower(db, OWNER, id).target!.profile.intent, "social");
    deleteDeck(db, OWNER, id);
    assert.equal(getPower(db, OWNER, fork).target!.profile.intent, "synergy");
    assert.equal(db.prepare("SELECT COUNT(*) n FROM power_assessments").get()!.n, 0);
    db.close();
  });
});

describe("computed Game Changers and power tools", () => {
  test("ingests nullable metadata, excludes maybe cards, counts commanders and keeps historical facts", () => {
    const db = testDb(); const id = newDeck(db);
    // Checked against the installed Scryfall bulk data on 2026-10-11:
    // Seedborn Muse game_changer=true; Sol Ring and Counterspell=false.
    const metadata = FIXTURES.filter(c => ["id-seedborn", "id-solring", "id-counterspell"].includes(c.oracle_id))
      .map(c => ({ ...c, game_changer: c.oracle_id === "id-seedborn" }));
    ingestCards(db, FIXTURES.map(c => metadata.find(m => m.oracle_id === c.oracle_id) ?? c));
    shortlistCard(db, OWNER, id, "id-seedborn");
    add(db, id, "id-solring");
    assert.equal(gameChangerFacts(db, OWNER, id).count, 0);
    add(db, id, "id-seedborn", { role: "commander" });
    assert.equal(gameChangerFacts(db, OWNER, id).count, 1);
    recordPowerAssessment(db, AGENT, id, assessment(db, id));
    assert.equal(getPower(db, OWNER, id).assessment!.mapping.bracket, 3);
    add(db, id, "id-counterspell");
    assert.equal(getPower(db, OWNER, id).assessment!.mapping.bracket, 3);
    assert.equal(getPower(db, OWNER, id).assessment!.game_changers.count, 1);
    const unknown = newDeck(db); add(db, unknown, "id-forest");
    assert.equal(gameChangerFacts(db, OWNER, unknown).unknown_cards, 1);
    db.close();
  });
  test("tools return profiles, computed deltas and actionable errors without obsolete assembly state", () => {
    const db = testDb(); const id = newDeck(db, "theorycrafted"); const ctx = { db, actor: AGENT };
    const proposed = callTool(ctx, "propose_power_target", { deck: id, target: target(), reason: "Owner's intended play experience." });
    assert.equal(proposed.changed, true);
    assert.match(proposed.text, /Proposed power target edit #1; awaiting the owner/);
    ruleDocumentEdit(db, OWNER, 1, "accept");
    const assessed = callTool(ctx, "assess_deck_power", { deck: id, assessment: assessment(db, id) });
    assert.match(assessed.text, /Recorded power assessment #1/);
    assert.match(assessed.text, /test-agent, r0, tentative, evidence: list/);
    const read = callTool(ctx, "get_deck", { deck: id, sections: ["power"] });
    assert.match(read.text, /Suggested bracket: 2 — Core/);
    assert.match(read.text, /Field anchors:/);
    assert.ok(!read.text.includes("in_paper"));
    db.close();
  });
});
