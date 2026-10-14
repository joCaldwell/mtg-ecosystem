import { test } from "node:test";
import assert from "node:assert/strict";
import { OWNER, PermissionError } from "../src/actor.ts";
import { applyChanges, undo } from "../src/deck/membership.ts";
import { getBrief, updateBrief, setEngine, removeEngine } from "../src/deck/brief.ts";
import { forkDeck } from "../src/deck/fork.ts";
import { applyImport, exportDeck } from "../src/deck/interop.ts";
import { createProposal, acceptItems, rejectItems, pendingItemIds } from "../src/deck/proposals.ts";
import { createSlot, updateSlot, deleteSlot, createTag, renameTag, deleteTag, organizeCard,
  fileCards, defineSlots, shortlistCard, unshortlistCard, renameDeck, deleteDeck,
  setDeckCategory, setDeckParent, getDeck } from "../src/deck/service.ts";
import { proposeDocumentEdit, ruleDocumentEdit, listDocumentEdits } from "../src/documents.ts";
import { remember, reviseMemory, getMemory } from "../src/memory.ts";
import { recordPowerAssessment, setPowerTarget } from "../src/deck/power.ts";
import { callTool } from "../src/mcp/tools.ts";
import { AGENT, testDb, newDeck, add } from "./helpers.ts";

test("lifecycle gates card edits, while every agent brief edit needs approval", () => {
  const db = testDb();
  try {
    for (const category of ["paper", "theorycrafted"] as const) {
      const id = newDeck(db, category);
      const changes = [{ action: "add" as const, oracle_id: "id-solring" }];
      if (category === "paper") assert.throws(() => applyChanges(db, AGENT, id, changes, { kind: "edit" }), PermissionError);
      else applyChanges(db, AGENT, id, changes, { kind: "edit" });
      assert.throws(() => setDeckCategory(db, AGENT, id, "theorycrafted"), PermissionError);
      assert.throws(() => updateBrief(db, AGENT, id, { thesis: "Changed directly" }), PermissionError);
      assert.throws(() => setEngine(db, AGENT, id, "Plan", "", []), PermissionError);
      const proposed = proposeDocumentEdit(db, AGENT, id, "thesis", { content: "Owner-approved plan" }, "Owner requested a plan.");
      assert.equal(proposed.applied, false);
      assert.equal(getBrief(db, id).thesis, "");
      ruleDocumentEdit(db, OWNER, proposed.edit_id!, "accept");
      assert.equal(getBrief(db, id).thesis, "Owner-approved plan");
    }
  } finally { db.close(); }
});

test("Legacy blocks owner and agent writes, preserves pending work, and restores or forks", () => {
  const db = testDb();
  try {
    const id = newDeck(db, "paper");
    const slot = createSlot(db, id, "Utility");
    const tag = createTag(db, id, "Have");
    const log = add(db, id, "id-solring", { slotId: slot })!;
    shortlistCard(db, id, "id-counterspell", "Later");
    const engine = setEngine(db, OWNER, id, "Plan", "", [{ oracle_id: "id-solring" }]);
    const memory = remember(db, AGENT, { scope: "deck", deck_id: id, text: "Prefers slower games.", basis: "stated" }).memory;
    const proposal = createProposal(db, AGENT, id, { items: [{ action: "cut", oracle_id: "id-solring", reason: "Owner wants less ramp." }] });
    const items = pendingItemIds(db, proposal);
    const brief = proposeDocumentEdit(db, AGENT, id, "thesis", { content: "New plan" }, "Owner requested a plan.");
    setDeckCategory(db, OWNER, id, "legacy");
    const before = getDeck(db, id);
    const name = before.cards.find(c => !c.maybe)!.name;

    for (const actor of [OWNER, AGENT]) {
      const writes = [
        () => applyChanges(db, actor, id, [{ action: "cut", oracle_id: "id-solring" }], { kind: "edit" }),
        () => renameDeck(db, actor, id, "Renamed"),
        () => deleteDeck(db, actor, id),
        () => updateBrief(db, actor, id, { thesis: "Changed" }),
        () => proposeDocumentEdit(db, actor, id, "thesis", { content: "Changed" }, "Requested."),
        () => createProposal(db, actor, id, { items: [{ action: "cut", oracle_id: "id-solring", reason: "Requested." }] }),
        () => recordPowerAssessment(db, actor, id, { revision: before.deck.revision, profile: {}, notes: "Estimate", evidence: "list", confidence: "tentative" }),
        () => remember(db, actor, { scope: "deck", deck_id: id, text: "New note" }),
        () => reviseMemory(db, actor, memory.id, { action: "confirm" }),
      ];
      for (const write of writes) assert.throws(write, PermissionError);
    }
    const ownerWrites = [
      () => acceptItems(db, OWNER, items), () => rejectItems(db, OWNER, items),
      () => ruleDocumentEdit(db, OWNER, brief.edit_id!, "accept"),
      () => ruleDocumentEdit(db, OWNER, brief.edit_id!, "reject"),
      () => undo(db, OWNER, log), () => applyImport(db, OWNER, id, `1 ${name}`),
      () => setDeckParent(db, OWNER, id, null), () => setPowerTarget(db, OWNER, id, null),
      () => setEngine(db, OWNER, id, "New plan", "", []), () => removeEngine(db, OWNER, id, engine),
      () => createSlot(db, id, "Ramp"), () => updateSlot(db, id, slot, { name: "Ramp" }),
      () => deleteSlot(db, id, slot), () => createTag(db, id, "Ordered"),
      () => renameTag(db, id, tag, "Ordered"), () => deleteTag(db, id, tag),
      () => organizeCard(db, id, "id-solring", { tagIds: [tag] }),
      () => fileCards(db, id, []), () => defineSlots(db, id, []),
      () => shortlistCard(db, id, "id-forest"), () => unshortlistCard(db, id, "id-counterspell"),
    ];
    for (const write of ownerWrites) assert.throws(write, PermissionError);
    assert.deepEqual(getDeck(db, id), before);
    assert.equal(getMemory(db, memory.id).read_only, true);
    assert.equal(listDocumentEdits(db, { deckId: id })[0].read_only, true);
    assert.deepEqual(pendingItemIds(db, proposal), items);
    assert.ok(exportDeck(db, id).text.includes(name));
    assert.match(callTool({ db, actor: AGENT }, "get_deck", { deck: id, sections: [] }).text, /Legacy: read-only/);
    const fork = forkDeck(db, AGENT, id, "Editable fork");
    assert.equal(getDeck(db, fork).deck.category, "theorycrafted");
    applyChanges(db, AGENT, fork, [{ action: "cut", oracle_id: "id-solring" }], { kind: "edit" });
    assert.deepEqual(getDeck(db, id), before);
    assert.throws(() => setDeckCategory(db, AGENT, id, "paper"), PermissionError);
    setDeckCategory(db, OWNER, id, "paper");
    ruleDocumentEdit(db, OWNER, brief.edit_id!, "accept");
    acceptItems(db, OWNER, items);
    assert.equal(getDeck(db, id).computed.card_count, 0);
    assert.equal(getBrief(db, id).thesis, "New plan");
  } finally { db.close(); }
});
