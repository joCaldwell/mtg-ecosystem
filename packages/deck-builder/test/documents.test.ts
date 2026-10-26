import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { PermissionError } from "../src/actor.ts";
import { OWNER } from "./actors.ts";
import { getBrief, setEngine, updateBrief } from "../src/deck/brief.ts";
import { checkDeck } from "../src/deck/checks.ts";
import { listLog } from "../src/deck/log.ts";
import {
  getProfile,
  listDocumentEdits,
  proposeDocumentEdit,
  ruleDocumentEdit,
  setProfile,
} from "../src/documents.ts";
import { AGENT, add, newDeck, testDb } from "./helpers.ts";

const db = testDb();

describe("the profile (spec §6)", () => {
  test("the owner edits it directly; an agent cannot", () => {
    setProfile(db, OWNER, "I like long games and hate fast mana.");
    assert.equal(getProfile(db, OWNER).body_md, "I like long games and hate fast mana.");
    assert.equal(getProfile(db, OWNER).word_count, 8);
    assert.throws(() => setProfile(db, AGENT, "rewritten"), PermissionError);
  });
  test("over the soft cap it is flagged, never truncated", () => {
    const fresh = testDb();
    setProfile(fresh, OWNER, Array(700).fill("word").join(" "));
    const p = getProfile(fresh, OWNER);
    assert.equal(p.word_count, 700);
    assert.equal(p.over_cap, true);
  });
  test("an agent's edit waits for the owner, who sees what it was written against", () => {
    const fresh = testDb();
    setProfile(fresh, OWNER, "Likes long games.");
    const r = proposeDocumentEdit(
      fresh,
      AGENT,
      null,
      "profile",
      { content: "Likes long games. Avoids fast mana." },
      "Rejected [[Sol Ring]] in three decks for the same reason.",
    );
    assert.equal(r.applied, false);
    assert.equal(getProfile(fresh, OWNER).body_md, "Likes long games.");
    let [edit] = listDocumentEdits(fresh, OWNER, { status: "pending" });
    assert.equal(edit.payload.base, "Likes long games.");
    assert.equal(edit.stale, false);

    // The owner edits the profile in the meantime: the pending edit is now stale.
    setProfile(fresh, OWNER, "Likes long games and combo.");
    [edit] = listDocumentEdits(fresh, OWNER, { status: "pending" });
    assert.equal(edit.stale, true);

    assert.throws(() => ruleDocumentEdit(fresh, AGENT, edit.id, "accept"), PermissionError);
    assert.throws(() => ruleDocumentEdit(fresh, OWNER, edit.id, "accept"), /stale/);
    ruleDocumentEdit(fresh, OWNER, edit.id, "accept", "Fair.", { acknowledge_stale: true });
    assert.equal(getProfile(fresh, OWNER).body_md, "Likes long games. Avoids fast mana.");
    // A profile decision belongs to no deck and still lands in the review queue.
    const [entry] = listLog(fresh, OWNER, { unreviewedOnly: true });
    assert.deepEqual([entry.deck_id, entry.kind, entry.decision_reason], [null, "accept", "Fair."]);
    assert.throws(() => ruleDocumentEdit(fresh, OWNER, edit.id, "reject"), /already accepted/);
  });
  test("a proposed edit needs a reason, and its prose is linted", () => {
    assert.throws(
      () => proposeDocumentEdit(db, AGENT, null, "profile", { content: "x" }, ""),
      /needs a reason/,
    );
    assert.throws(
      () => proposeDocumentEdit(db, AGENT, null, "profile", { content: "Loves [[Seedborn Sage]]." }, "y"),
      /is not a card name/,
    );
    assert.throws(
      () => proposeDocumentEdit(db, AGENT, 1, "profile", { content: "x" }, "y"),
      /profile edit has no deck/,
    );
  });
});

describe("the brief (spec §6)", () => {
  test("on a Paper deck an agent's brief edit is gated; rejecting it changes nothing", () => {
    const id = newDeck(db, "paper");
    updateBrief(db, OWNER, id, { thesis: "Superfriends." });
    const r = proposeDocumentEdit(db, AGENT, id, "thesis", { content: "Proliferate value." }, "The list has drifted.");
    assert.equal(r.applied, false);
    assert.equal(getBrief(db, OWNER, id).thesis, "Superfriends.");
    ruleDocumentEdit(db, OWNER, r.edit_id!, "reject", "No, it is still superfriends.");
    assert.equal(getBrief(db, OWNER, id).thesis, "Superfriends.");
    assert.equal(listDocumentEdits(db, OWNER, { deckId: id, status: "pending" }).length, 0);
    const [entry] = listLog(db, OWNER, { deckId: id });
    assert.equal(entry.kind, "reject");
    assert.match(entry.summary, /Brief edit: thesis rejected — proposed because: The list has drifted\./);
  });
  test("Theorycrafted briefs still require proposals and owner approval", () => {
    const id = newDeck(db, "theorycrafted");
    assert.throws(() => updateBrief(db, AGENT, id, { thesis: "Elves." }), PermissionError);
    assert.throws(() => proposeDocumentEdit(db, AGENT, id, "thesis", { content: "Elves." }, ""), /needs a reason/);
    const r = proposeDocumentEdit(db, AGENT, id, "thesis", { content: "Elves." }, "Owner requested an elf plan.");
    assert.equal(r.applied, false);
    assert.equal(getBrief(db, OWNER, id).thesis, "");
    ruleDocumentEdit(db, OWNER, r.edit_id!, "accept");
    assert.equal(getBrief(db, OWNER, id).thesis, "Elves.");
    const engine = proposeDocumentEdit(db, AGENT, id, "engine_set",
      { engine_name: "Untap", description: "Untap everything.", pieces: [{ oracle_id: "id-seedborn" }] }, "Supports the proposed plan.");
    assert.deepEqual(getBrief(db, OWNER, id).engines, []);
    ruleDocumentEdit(db, OWNER, engine.edit_id!, "accept");
    assert.equal(getBrief(db, OWNER, id).engines[0].pieces[0].name, "Seedborn Muse");
  });
  test("engine edits are gated on a Paper deck and applied on accept", () => {
    const id = newDeck(db, "paper");
    const set = proposeDocumentEdit(
      db,
      AGENT,
      id,
      "engine_set",
      { engine_name: "Ramp", pieces: [{ oracle_id: "id-solring" }, { oracle_id: "id-llanowar" }] },
      "Named so its pieces can be checked.",
    );
    assert.equal(getBrief(db, OWNER, id).engines.length, 0);
    assert.deepEqual(listDocumentEdits(db, OWNER, { deckId: id })[0].piece_names, ["Sol Ring", "Llanowar Elves"]);
    ruleDocumentEdit(db, OWNER, set.edit_id!, "accept");
    assert.equal(getBrief(db, OWNER, id).engines[0].pieces.length, 2);

    assert.throws(
      () => proposeDocumentEdit(db, AGENT, id, "engine_remove", { engine_name: "Nope" }, "x"),
      /no engine named 'Nope'/,
    );
    const remove = proposeDocumentEdit(db, AGENT, id, "engine_remove", { engine_name: "ramp" }, "Folded into the thesis.");
    ruleDocumentEdit(db, OWNER, remove.edit_id!, "accept");
    assert.equal(getBrief(db, OWNER, id).engines.length, 0);
  });
});

describe("engines are checkable (spec §6, §9)", () => {
  test("a missing piece is a finding; a piece on the maybe list is missing", () => {
    const id = newDeck(db, "paper");
    add(db, id, "id-solring");
    setEngine(db, OWNER, id, "Mana", "Fast mana.", [{ oracle_id: "id-solring" }, { oracle_id: "id-llanowar" }]);
    const finding = checkDeck(db, OWNER, id).find((f) => f.key.startsWith("engine_incomplete"))!;
    assert.match(finding.title, /Engine "Mana" is missing 1 of 2 piece/);
    assert.match(finding.detail, /Llanowar Elves/);
    add(db, id, "id-llanowar");
    assert.ok(!checkDeck(db, OWNER, id).some((f) => f.key.startsWith("engine_incomplete")));
  });
});
