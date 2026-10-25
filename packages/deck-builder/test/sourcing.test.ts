import { test } from "node:test";
import assert from "node:assert/strict";
import { OWNER, PermissionError } from "../src/actor.ts";
import { ingestCards } from "../src/ingest.ts";
import { compareDeckSources, ownershipEvidence, recordOwnership } from "../src/deck/sourcing.ts";
import { createSlot, fileCards, getDeck, setDeckCategory, deleteDeck } from "../src/deck/service.ts";
import { forkDeck } from "../src/deck/fork.ts";
import { callTool } from "../src/mcp/tools.ts";
import { FIXTURES } from "./fixtures.ts";
import { SOURCING_CARDS } from "./sourcing-fixtures.ts";
import { AGENT, add, cut, newDeck, testDb } from "./helpers.ts";

function setup() {
  const db = testDb();
  ingestCards(db, [...FIXTURES, ...SOURCING_CARDS]);
  const [moon, heroic, descent] = SOURCING_CARDS.map(c => c.oracle_id);
  const target = newDeck(db, "paper", "Jo’s Wildfire");
  const ramp = newDeck(db, "paper", "Jo’s ramp");
  const chaos = newDeck(db, "theorycrafted", "Jo’s Chaos");
  for (const id of [moon, heroic, descent, "id-solring", "id-forest"]) add(db, target, id, { quantity: id === "id-forest" ? 3 : 1 });
  for (const source of [ramp, chaos]) add(db, source, moon);
  add(db, ramp, "id-forest", { quantity: 2 });
  add(db, chaos, "id-forest", { quantity: 3 });
  fileCards(db, target, [{ oracle_id: heroic, add_tags: ["Have"] }, { oracle_id: descent, add_tags: ["Keep"] }]);
  const slot = createSlot(db, target, "Control");
  createSlot(db, target, "Disruption");
  fileCards(db, target, [{ oracle_id: moon, slot: "Control", secondary_slots: ["Disruption"], add_tags: ["Keep", "Ordered"] }]);
  const price = db.prepare("INSERT INTO card_prices (oracle_id, usd_cents, set_code, collector_number, finish) VALUES (?, ?, 'test', '1', 'nonfoil')");
  price.run(moon, 800); price.run(heroic, 2000); price.run(descent, 700); price.run("id-forest", 10);
  db.exec("INSERT INTO meta (key, value) VALUES ('prices_as_of', '2026-10-15T12:00:00Z')");
  return { db, target, ramp, chaos, moon, heroic, descent, slot };
}

test("multi-source overlaps return actual cards, both sources, list quantities and timestamped estimates without inferred physical copies", () => {
  const { db, target, ramp, chaos, moon, slot } = setup();
  try {
    const before = getDeck(db, target);
    const result = compareDeckSources(db, AGENT, target, [ramp, chaos, ramp], {
      apply_source_tags: true, expected_revision: before.deck.revision,
      expected_source_revisions: { [ramp]: getDeck(db, ramp).deck.revision, [chaos]: getDeck(db, chaos).deck.revision },
    });
    const card = result.shared_cards.find(c => c.oracle_id === moon)!;
    assert.equal(card.name, "Winter Moon");
    assert.deepEqual(card.source_decks.map(d => d.name), ["Jo’s ramp", "Jo’s Chaos"]);
    assert.deepEqual(card.source_decks.map(d => d.quantity), [1, 1]);
    assert.deepEqual(card.ownership, { status: "unknown", quantity: null, evidence: [] });
    assert.deepEqual(card.reuse, { status: "unknown", available_quantity: null });
    assert.deepEqual(card.price, { usd_cents: 800, as_of: "2026-10-15T12:00:00Z", estimated: true });
    assert.equal(result.shared_cards.find(c => c.oracle_id === "id-forest")!.quantity, 3);
    assert.equal(result.summary.shared.cards, 2);
    assert.equal(result.summary.shared.copies, 4);
    assert.deepEqual(result.summary.shared.cost, { usd_cents: 830, unpriced: 0 });
    const after = getDeck(db, target);
    const filed = after.cards.find(c => c.oracle_id === moon)!;
    assert.equal(filed.slot_id, slot);
    assert.deepEqual(filed.secondary_slot_ids, before.cards.find(c => c.oracle_id === moon)!.secondary_slot_ids);
    assert.deepEqual(filed.tag_ids.map(id => after.tags.find(t => t.id === id)!.name).sort(), ["From Jo’s Chaos", "From Jo’s ramp", "Keep", "Ordered"].sort());
    assert.equal(after.deck.revision, before.deck.revision);
    assert.deepEqual(after.computed.card_count, before.computed.card_count);
    const tagCount = after.tags.length;
    compareDeckSources(db, AGENT, target, [ramp, chaos], { apply_source_tags: true, expected_revision: before.deck.revision,
      expected_source_revisions: { [ramp]: 2, [chaos]: 2 } });
    assert.equal(getDeck(db, target).tags.length, tagCount);
  } finally { db.close(); }
});

test("Have excludes Heroic Intervention; Keep is not ownership; unknowns stay distinct from unowned additions", () => {
  const { db, target, ramp, chaos, heroic, descent } = setup();
  try {
    const result = compareDeckSources(db, AGENT, target, [ramp, chaos], { sourcing: "not_in_sources", not_marked_owned: true, sort: "price_desc" });
    assert.ok(!result.cards.some(c => c.oracle_id === heroic));
    assert.equal(result.cards[0].oracle_id, descent);
    assert.equal(result.cards[0].ownership.status, "unknown");
    assert.equal(result.cards.at(-1)!.price.usd_cents, null);
    assert.deepEqual(result.summary.remaining.cost, { usd_cents: 700, unpriced: 1 });
    assert.equal(compareDeckSources(db, AGENT, target, [ramp], { ownership: "unowned" }).cards.length, 0);
    recordOwnership(db, AGENT, target, [{ oracle_id: descent, status: "unowned", quantity: 0, basis: "Owner explicitly confirmed no copy." }]);
    assert.deepEqual(compareDeckSources(db, AGENT, target, [ramp], { ownership: "unowned" }).cards.map(c => c.oracle_id), [descent]);
    assert.equal(ownershipEvidence(db, heroic)[0].quantity, null);
  } finally { db.close(); }
});

test("ownership survives cutting, moving, untagging, forks, and deleting its source deck", () => {
  const { db, target, ramp, heroic } = setup();
  try {
    const evidence = ownershipEvidence(db, heroic);
    fileCards(db, target, [{ oracle_id: heroic, remove_tags: ["Have"] }]);
    cut(db, target, heroic);
    add(db, ramp, heroic);
    const fork = forkDeck(db, AGENT, ramp, "Other list, same physical collection");
    deleteDeck(db, OWNER, target);
    assert.deepEqual(ownershipEvidence(db, heroic), evidence);
    const card = compareDeckSources(db, AGENT, ramp, [fork]).shared_cards.find(c => c.oracle_id === heroic)!;
    assert.equal(card.ownership.status, "owned");
    assert.equal(card.ownership.quantity, null);
    assert.equal(card.reuse.status, "unknown");
  } finally { db.close(); }
});

test("explicit available quantity is separate from deck appearances and updates use evidence guards", () => {
  const { db, target, ramp, chaos, moon } = setup();
  try {
    recordOwnership(db, AGENT, target, [{ oracle_id: moon, status: "owned", quantity: 1, available_quantity: 0, basis: "Owner confirms one copy committed elsewhere.", expected_evidence_id: null }]);
    const evidence = ownershipEvidence(db, moon)[0];
    assert.throws(() => recordOwnership(db, AGENT, target, [{ oracle_id: moon, status: "owned", basis: "Stale", expected_evidence_id: null }]), /evidence changed/);
    assert.equal(compareDeckSources(db, AGENT, target, [ramp, chaos], { availability: "insufficient" }).cards[0].oracle_id, moon);
    recordOwnership(db, AGENT, target, [{ oracle_id: moon, status: "owned", quantity: 1, available_quantity: 1, basis: "Owner made the copy available.", expected_evidence_id: evidence.id }]);
    assert.equal(compareDeckSources(db, AGENT, target, [ramp, chaos], { availability: "available" }).cards[0].oracle_id, moon);
    assert.equal(ownershipEvidence(db, moon).length, 2);
    assert.throws(() => recordOwnership(db, AGENT, target, [{ oracle_id: moon, status: "owned", quantity: 1, available_quantity: 2, basis: "Invalid" }]), /contradict/);
  } finally { db.close(); }
});

test("source tagging guards all revisions, rolls back, and preserves Legacy read-only behavior", () => {
  const { db, target, ramp, chaos, moon } = setup();
  try {
    const before = getDeck(db, target);
    assert.throws(() => compareDeckSources(db, AGENT, target, [ramp], { apply_source_tags: true }), /requires expected_revision/);
    assert.throws(() => compareDeckSources(db, AGENT, target, [ramp], { apply_source_tags: true, expected_revision: before.deck.revision, expected_source_revisions: { [ramp]: 0 } }), /revision changed/);
    assert.deepEqual(getDeck(db, target), before);
    setDeckCategory(db, OWNER, target, "legacy");
    assert.equal(compareDeckSources(db, AGENT, target, [ramp, chaos]).shared_cards.length, 2);
    assert.throws(() => compareDeckSources(db, AGENT, target, [ramp], { apply_source_tags: true }), PermissionError);
    assert.throws(() => recordOwnership(db, AGENT, target, [{ oracle_id: moon, status: "owned", basis: "Owner said so" }]), PermissionError);
  } finally { db.close(); }
});

test("the coherent MCP operation returns structured records plus concise computed text", () => {
  const { db, target, ramp, chaos } = setup();
  try {
    const result = callTool({ db, actor: AGENT }, "compare_deck_sources", { deck: target, sources: [ramp, chaos], sort: "price_desc" });
    assert.match(result.text, /Winter Moon \[Jo’s ramp; Jo’s Chaos\]/);
    assert.match(result.text, /unknown ownership/i);
    assert.equal((result.structuredContent!.shared_cards as unknown[]).length, 2);
    assert.equal(result.changed, false);
  } finally { db.close(); }
});

test("reapplying Have after a move preserves more precise physical evidence", () => {
  const { db, target, ramp, heroic } = setup();
  try {
    recordOwnership(db, AGENT, target, [{ oracle_id: heroic, status: "owned", quantity: 1, available_quantity: 1, basis: "Owner confirmed the available copy." }]);
    const evidence = ownershipEvidence(db, heroic);
    cut(db, target, heroic);
    add(db, ramp, heroic);
    fileCards(db, ramp, [{ oracle_id: heroic, add_tags: ["Have"] }]);
    assert.deepEqual(ownershipEvidence(db, heroic), evidence);
  } finally { db.close(); }
});
