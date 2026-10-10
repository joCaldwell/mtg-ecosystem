import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { OWNER } from "../src/actor.ts";
import { SchemaError, openDb, openReadOnly } from "../src/db.ts";
import { getBrief } from "../src/deck/brief.ts";
import { cardHistory, listLog, unreviewedCount } from "../src/deck/log.ts";
import { listProposals } from "../src/deck/proposals.ts";
import { getDeck, listDecks } from "../src/deck/service.ts";
import { listDocumentEdits } from "../src/documents.ts";
import { ingestCards } from "../src/ingest.ts";
import { listMemories } from "../src/memory.ts";
import { migrate } from "../src/migrate-v1.ts";
import { callTool } from "../src/mcp/tools.ts";
import { FIXTURES } from "./fixtures.ts";
import { AGENT } from "./helpers.ts";
import { V1_SCHEMA } from "./v1-schema.ts";

const dir = mkdtempSync(join(tmpdir(), "deck-builder-migrate-"));
const v1Path = join(dir, "deck-builder.db");

// A v1 database with one of everything the migration has to carry.
function buildV1() {
  const db = new DatabaseSync(v1Path);
  db.exec(V1_SCHEMA);
  // The card tables are the same in both versions, so the v2 ingester can
  // fill a v1 file.
  ingestCards(db, FIXTURES);
  db.exec(`
    INSERT INTO decks (id, name, revision) VALUES (7, 'Jo''s Golems', 12);
    INSERT INTO slots (id, deck_id, name, target_min, target_max, position) VALUES (3, 7, 'Ramp', 8, 10, 0);
    INSERT INTO tags (id, deck_id, name) VALUES (5, 7, 'rock');
    INSERT INTO deck_cards (deck_id, oracle_id, role, owned) VALUES (7, 'id-teferi', 'commander', 1);
    INSERT INTO deck_cards (deck_id, oracle_id, slot_id, owned) VALUES (7, 'id-solring', 3, 1);
    INSERT INTO deck_cards (deck_id, oracle_id, quantity) VALUES (7, 'id-forest', 4);
    INSERT INTO deck_cards (deck_id, oracle_id, maybeboard) VALUES (7, 'id-ball', 1);
    INSERT INTO deck_card_tags (deck_id, oracle_id, tag_id) VALUES (7, 'id-solring', 5);
    INSERT INTO briefs (deck_id, thesis, constraints_md) VALUES (7, 'Artifacts matter.', 'No infinite combos.');
    INSERT INTO engines (id, deck_id, name, description) VALUES (2, 7, 'Mana', 'Fast mana.');
    INSERT INTO engine_pieces (engine_id, oracle_id, note) VALUES (2, 'id-solring', 'the best one');
    INSERT INTO decision_log (id, deck_id, revision, kind, action, oracle_id, card_name, rationale)
      VALUES (1, 7, 10, 'accept', 'add', 'id-solring', 'Sol Ring', 'Cheap ramp.');
    INSERT INTO decision_log (id, deck_id, revision, kind, action, oracle_id, card_name, rationale, rejection_type, rejection_reason)
      VALUES (2, 7, 10, 'reject', 'add', 'id-counterspell', 'Counterspell', 'Interaction.', 'hard_filter', 'No counterspells in this deck.');
    INSERT INTO decision_log (id, deck_id, revision, kind, action, rationale)
      VALUES (3, 7, 11, 'accept', 'import', 'Imported an Archidekt list at revision 10: +3/−1');
    INSERT INTO decision_log (id, deck_id, revision, kind, action, oracle_id, card_name, undo_of)
      VALUES (4, 7, 12, 'undo', 'cut', 'id-solring', 'Sol Ring', 1);
    UPDATE decision_log SET undone_by = 4 WHERE id = 1;
    INSERT INTO hard_filters (deck_id, oracle_id, card_name, reason) VALUES (7, 'id-counterspell', 'Counterspell', 'No counterspells in this deck.');
    INSERT INTO card_notes (deck_id, oracle_id, card_name, note) VALUES (7, 'id-solring', 'Sol Ring', 'Dead in three of five opens.');
    INSERT INTO playtest_notes (deck_id, revision, note) VALUES (7, 9, 'Never found the engine before turn 9.');
    INSERT INTO proposals (id, deck_id, source, note, status) VALUES (1, 7, 'agent', 'More lands', 'open');
    INSERT INTO proposal_items (proposal_id, action, oracle_id, slot_id, rationale) VALUES (1, 'add', 'id-bog', 3, 'Graveyard hate on a land.');
    INSERT INTO proposal_items (proposal_id, action, oracle_id, rationale, status) VALUES (1, 'add', 'id-rats', 'Already ruled.', 'rejected');
    INSERT INTO brief_edits (deck_id, kind, payload_json, rationale) VALUES (7, 'thesis', '{"content":"Artifact ramp."}', 'The list drifted.');
    INSERT INTO chat_messages (deck_id, role, content_json) VALUES (7, 'user', '{"role":"user","content":"hi"}');
  `);
  db.close();
}
buildV1();
test.after(() => rmSync(dir, { recursive: true, force: true }));

describe("migrating a v1 database", () => {
  const before = readFileSync(v1Path);
  const v1 = openReadOnly(v1Path);
  const v2 = openDb(":memory:");
  const report = migrate(v1, v2);
  v1.close();

  test("the v1 file is not written to, and cannot be", () => {
    assert.deepEqual(readFileSync(v1Path), before);
    const ro = openReadOnly(v1Path);
    assert.throws(() => ro.exec("DELETE FROM decks"), /readonly/);
    ro.close();
  });
  test("v2 refuses to open a v1 file as its own database", () => {
    assert.throws(() => openDb(v1Path), (e: Error) => e instanceof SchemaError && /migrate-v1/.test(e.message));
    assert.deepEqual(readFileSync(v1Path), before);
  });
  test("the report counts what moved and what stayed", () => {
    assert.deepEqual(
      { ...report, cards: undefined },
      {
        cards: undefined, decks: 1, deck_cards: 4, slots: 1, tags: 1, briefs: 1, engines: 1,
        log_entries: 4, open_proposals: 1, pending_brief_edits: 1, memories_from_hard_filters: 1,
        memories_from_card_notes: 1, memories_from_playtest_notes: 1,
        left_behind: { chat_messages: 1, consolidations: 0, audit_runs: 0 },
        skipped_cards: [],
      },
    );
    assert.equal(report.cards, FIXTURES.length - 1); // the token fixture is not ingested
  });
  test("a deck arrives active, with its list, organization, paper flags and brief", () => {
    const [row] = listDecks(v2);
    assert.deepEqual([row.name, row.status, row.revision, row.color_identity], ["Jo's Golems", "active", 12, "U"]);
    const view = getDeck(v2, row.id);
    assert.equal(view.computed.card_count, 6);
    assert.equal(view.computed.maybe_count, 1);
    const ring = view.cards.find((c) => c.name === "Sol Ring")!;
    assert.equal(view.slots.find((s) => s.id === ring.slot_id)!.name, "Ramp");
    assert.deepEqual(ring.tag_ids, [view.tags[0].id]);
    assert.deepEqual(
      view.cards.map((c) => [c.name, c.in_paper]).sort(),
      [["Ball Lightning", 0], ["Forest", 0], ["Sol Ring", 1], ["Teferi, Temporal Archmage", 1]],
    );
    const brief = getBrief(v2, row.id);
    assert.deepEqual([brief.thesis, brief.constraints_md], ["Artifacts matter.", "No infinite combos."]);
    assert.deepEqual(brief.engines[0].pieces.map((p) => [p.name, p.note, p.in_deck]), [["Sol Ring", "the best one", true]]);
  });
  test("the decision log arrives as history, already reviewed, with reasons intact", () => {
    const [{ id }] = listDecks(v2);
    assert.equal(unreviewedCount(v2), 0);
    const log = listLog(v2, { deckId: id });
    assert.deepEqual(log.map((e) => e.kind), ["undo", "import", "reject", "accept"]);
    const rejection = log[2];
    assert.equal(rejection.ruling_reason, "No counterspells in this deck.");
    assert.equal(rejection.summary, "Rejected adding Counterspell (v1 reason type: a hard no)");
    assert.equal(log[3].undone_by, log[0].id);
    assert.equal(cardHistory(v2, id, "id-counterspell")[0].ruling_reason, "No counterspells in this deck.");
  });
  test("what v2 has no table for becomes a deck memory, word for word", () => {
    const [{ id }] = listDecks(v2);
    const memories = listMemories(v2, { deckId: id }).map((m) => [m.text, m.basis, m.deck_revision, m.cards.map((c) => c.name)]);
    assert.deepEqual(memories.sort(), [
      ["Playtest finding on [[Sol Ring]]: Dead in three of five opens.", "stated", null, ["Sol Ring"]],
      ["Playtest note: Never found the engine before turn 9.", "stated", 9, []],
      ["Ruled out [[Counterspell]] for this deck as a hard no: No counterspells in this deck.", "stated", null, ["Counterspell"]],
    ]);
  });
  test("what was waiting on the owner still is", () => {
    const [{ id }] = listDecks(v2);
    const [proposal] = listProposals(v2, { deckId: id, status: "open" });
    assert.deepEqual(
      [proposal.author, proposal.summary, proposal.items.map((i) => [i.card_name, i.slot_name, i.reason])],
      ["the v1 agent", "More lands", [["Bojuka Bog", "Ramp", "Graveyard hate on a land."]]],
    );
    const [edit] = listDocumentEdits(v2, { deckId: id, status: "pending" });
    assert.deepEqual([edit.kind, edit.payload.content, edit.reason], ["thesis", "Artifact ramp.", "The list drifted."]);
  });
  test("an agent reading the migrated deck sees the old ruling beside the card", () => {
    const [{ id }] = listDecks(v2);
    const out = callTool({ db: v2, actor: AGENT }, "get_cards", { cards: ["Counterspell"], deck: id }).text;
    assert.match(out, /the owner rejected adding it — the owner's reason: "No counterspells in this deck\."/);
    assert.match(out, /memory #\d+ \(deck "Jo's Golems", stated\): Ruled out \[\[Counterspell\]\]/);
  });
  test("it is a first-time copy: a target that already has decks is refused", () => {
    const again = openReadOnly(v1Path);
    assert.throws(() => migrate(again, v2), /already has decks/);
    again.close();
    assert.equal(listDecks(v2).length, 1);
    void OWNER;
  });
});
