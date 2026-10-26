import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { PermissionError } from "../src/actor.ts";
import { OWNER } from "./actors.ts";
import { cardHistory, getLogEntry, listLog, unreviewedCount } from "../src/deck/log.ts";
import { undo } from "../src/deck/membership.ts";
import {
  acceptItems,
  createProposal,
  getProposal,
  listProposals,
  pendingItemIds,
  rejectItems,
} from "../src/deck/proposals.ts";
import { createSlot, deleteSlot, getDeck } from "../src/deck/service.ts";
import { AGENT, add, newDeck, testDb } from "./helpers.ts";

const db = testDb();
const names = (id: number) => getDeck(db, OWNER, id).cards.filter((c) => !c.maybe).map((c) => c.name);

function seeded() {
  const id = newDeck(db);
  add(db, id, "id-teferi", { role: "commander" });
  add(db, id, "id-counterspell");
  return id;
}

describe("creating a proposal (spec §8.1)", () => {
  test("any number of items; each needs a reason; the author is recorded", () => {
    const id = seeded();
    const items = ["id-solring", "id-forest", "id-llanowar", "id-seedborn", "id-goyf", "id-rats", "id-bog"].map(
      (oracle_id) => ({ action: "add" as const, oracle_id, reason: "Fills the curve." }),
    );
    const pid = createProposal(db, AGENT, id, { summary: "Seven adds", items });
    const proposal = getProposal(db, OWNER, pid)!;
    assert.equal(proposal.items.length, 7);
    assert.equal(proposal.author, "test-agent");
    assert.equal(getDeck(db, OWNER, id).computed.pending.projected_count, 9);
    assert.throws(
      () => createProposal(db, AGENT, id, { items: [{ action: "add", oracle_id: "id-ball", reason: " " }] }),
      /needs a reason/,
    );
    assert.throws(() => createProposal(db, AGENT, id, { items: [] }), /at least one item/);
  });
  test("items must make sense against the deck", () => {
    const id = seeded();
    assert.throws(
      () => createProposal(db, AGENT, id, { items: [{ action: "cut", oracle_id: "id-solring", reason: "x" }] }),
      /not in the deck/,
    );
    assert.throws(
      () => createProposal(db, AGENT, id, { items: [{ action: "add", oracle_id: "id-counterspell", reason: "x" }] }),
      /already in the deck/,
    );
    // A card that allows more copies can be proposed again.
    add(db, id, "id-forest");
    createProposal(db, AGENT, id, { items: [{ action: "add", oracle_id: "id-forest", reason: "One more land." }] });
  });
  test("a second pending proposal for the same change is refused", () => {
    const id = seeded();
    const first = createProposal(db, AGENT, id, {
      items: [{ action: "add", oracle_id: "id-solring", reason: "Ramp." }],
    });
    assert.throws(
      () => createProposal(db, AGENT, id, { items: [{ action: "add", oracle_id: "id-solring", reason: "Ramp again." }] }),
      new RegExp(`Proposal #${first} already has a pending add of Sol Ring`),
    );
  });
  // Spec §10.5: nothing the app displays names a card that does not exist.
  test("a reason naming an invented card fails the whole proposal", () => {
    const id = seeded();
    assert.throws(
      () =>
        createProposal(db, AGENT, id, {
          items: [
            { action: "add", oracle_id: "id-solring", reason: "Better than [[Signet of the Adeptus]]." },
          ],
        }),
      /is not a card name/,
    );
    assert.equal(listProposals(db, OWNER, { deckId: id }).length, 0);
  });
});

describe("decision (spec §8.2)", () => {
  test("only the owner rules", () => {
    const id = seeded();
    const pid = createProposal(db, AGENT, id, {
      items: [{ action: "add", oracle_id: "id-solring", reason: "Ramp." }],
    });
    const ids = pendingItemIds(db, OWNER, pid);
    assert.throws(() => acceptItems(db, AGENT, ids), PermissionError);
    assert.throws(() => rejectItems(db, AGENT, ids), PermissionError);
  });
  test("accepting applies the change as one logged act that carries both reasons", () => {
    const id = seeded();
    const slot = createSlot(db, OWNER, id, "Ramp");
    const pid = createProposal(db, AGENT, id, {
      items: [{ action: "add", oracle_id: "id-solring", slot_id: slot, reason: "Cheap ramp." }],
    });
    const logId = acceptItems(db, OWNER, pendingItemIds(db, OWNER, pid), "Yes, obviously.");
    assert.ok(names(id).includes("Sol Ring"));
    assert.equal(getDeck(db, OWNER, id).cards.find((c) => c.oracle_id === "id-solring")!.slot_id, slot);
    const entry = getLogEntry(db, OWNER, logId)!;
    assert.deepEqual(
      [entry.kind, entry.decision_reason, entry.proposal_id, entry.cards[0].reason],
      ["accept", "Yes, obviously.", pid, "Cheap ramp."],
    );
    assert.equal(getProposal(db, OWNER, pid)!.status, "resolved");
  });
  test("a rejection changes nothing, needs no reason, and is logged against the card", () => {
    const id = seeded();
    const before = getDeck(db, OWNER, id).deck.revision;
    const pid = createProposal(db, AGENT, id, {
      items: [
        { action: "add", oracle_id: "id-solring", reason: "Ramp." },
        { action: "cut", oracle_id: "id-counterspell", reason: "Too reactive." },
      ],
    });
    const [addItem, cutItem] = pendingItemIds(db, OWNER, pid);
    rejectItems(db, OWNER, [cutItem]);
    rejectItems(db, OWNER, [addItem], "Too slow for this list.");
    assert.equal(getDeck(db, OWNER, id).deck.revision, before);
    assert.deepEqual(names(id), ["Counterspell", "Teferi, Temporal Archmage"]);

    const history = cardHistory(db, OWNER, id, "id-solring");
    assert.deepEqual(
      [history[0].kind, history[0].action, history[0].decision_reason, history[0].reason],
      ["reject", "add", "Too slow for this list.", "Ramp."],
    );
    assert.equal(cardHistory(db, OWNER, id, "id-counterspell")[0].decision_reason, "");
    assert.equal(getProposal(db, OWNER, pid)!.status, "resolved");
  });
  test("a group is ruled on as a unit", () => {
    const id = seeded();
    const pid = createProposal(db, AGENT, id, {
      items: [
        { action: "cut", oracle_id: "id-counterspell", reason: "Swap.", group: "swap" },
        { action: "add", oracle_id: "id-solring", reason: "Swap.", group: "swap" },
        { action: "add", oracle_id: "id-forest", reason: "A land." },
      ],
    });
    const [cutItem, addItem, lone] = pendingItemIds(db, OWNER, pid);
    const logId = acceptItems(db, OWNER, [addItem]);
    assert.deepEqual(names(id), ["Sol Ring", "Teferi, Temporal Archmage"]);
    assert.equal(getDeck(db, OWNER, id).deck.revision, 3);
    assert.equal(getLogEntry(db, OWNER, logId)!.cards.length, 2);
    assert.throws(() => rejectItems(db, OWNER, [cutItem]), /already accepted/);
    assert.deepEqual(pendingItemIds(db, OWNER, pid), [lone]);
    assert.equal(getProposal(db, OWNER, pid)!.status, "open");
  });
  test("accepting the whole proposal at once is one act and one revision", () => {
    const id = seeded();
    const before = getDeck(db, OWNER, id).deck.revision;
    const pid = createProposal(db, AGENT, id, {
      items: ["id-solring", "id-forest", "id-bog"].map((oracle_id) => ({
        action: "add" as const,
        oracle_id,
        reason: "Needed.",
      })),
    });
    const logId = acceptItems(db, OWNER, pendingItemIds(db, OWNER, pid));
    assert.equal(getDeck(db, OWNER, id).deck.revision, before + 1);
    assert.equal(getLogEntry(db, OWNER, logId)!.summary, "+3 cards");
    // The log entry is undone like any other.
    undo(db, OWNER, logId);
    assert.deepEqual(names(id), ["Counterspell", "Teferi, Temporal Archmage"]);
  });
  test("a stale item fails the accept without half-applying it", () => {
    const id = seeded();
    const pid = createProposal(db, AGENT, id, {
      items: [
        { action: "add", oracle_id: "id-solring", reason: "Ramp." },
        { action: "cut", oracle_id: "id-counterspell", reason: "Cut." },
      ],
    });
    // The owner cuts the card by hand before decision.
    add(db, id, "id-forest");
    db.prepare("DELETE FROM deck_cards WHERE deck_id = ? AND oracle_id = 'id-counterspell'").run(id);
    assert.throws(() => acceptItems(db, OWNER, pendingItemIds(db, OWNER, pid)), /stale/);
    assert.ok(!names(id).includes("Sol Ring"));
    assert.equal(pendingItemIds(db, OWNER, pid).length, 2);
  });
  test("a slot deleted after proposing does not fail the accept", () => {
    const id = seeded();
    const slot = createSlot(db, OWNER, id, "Ramp");
    const pid = createProposal(db, AGENT, id, {
      items: [{ action: "add", oracle_id: "id-solring", slot_id: slot, reason: "Ramp." }],
    });
    deleteSlot(db, OWNER, id, slot);
    acceptItems(db, OWNER, pendingItemIds(db, OWNER, pid));
    assert.equal(getDeck(db, OWNER, id).cards.find((c) => c.oracle_id === "id-solring")!.slot_id, null);
  });
});

describe("decisions and the review queue (spec §8.4)", () => {
  test("every decision waits for an agent to read it, with its reason", () => {
    const fresh = testDb();
    const id = newDeck(fresh, "theorycrafted");
    const pid = createProposal(fresh, AGENT, id, {
      items: [{ action: "add", oracle_id: "id-solring", reason: "Ramp." }],
    });
    assert.equal(unreviewedCount(fresh, OWNER), 0);
    rejectItems(fresh, OWNER, pendingItemIds(fresh, OWNER, pid), "I never play fast mana.");
    const queue = listLog(fresh, OWNER, { unreviewedOnly: true });
    assert.equal(queue.length, 1);
    assert.equal(queue[0].decision_reason, "I never play fast mana.");
    assert.equal(queue[0].deck_name, getDeck(fresh, OWNER, id).deck.name);
  });
  test("the inbox lists open proposals across decks", () => {
    const fresh = testDb();
    const a = newDeck(fresh);
    const b = newDeck(fresh);
    createProposal(fresh, AGENT, a, { items: [{ action: "add", oracle_id: "id-solring", reason: "x" }] });
    createProposal(fresh, AGENT, b, { items: [{ action: "add", oracle_id: "id-forest", reason: "y" }] });
    assert.equal(listProposals(fresh, OWNER, { status: "open" }).length, 2);
    assert.equal(listProposals(fresh, OWNER, { deckId: a }).length, 1);
  });
});
