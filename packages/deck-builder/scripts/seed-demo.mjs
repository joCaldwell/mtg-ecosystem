#!/usr/bin/env node
// Seed a realistic demo deck so the UI has something to render.
//
// Every panel — slot deltas, audit findings, proposals, the decision log,
// hard filters, playtest notes, the context meter — is empty or degenerate
// against a one-card deck, which makes frontend work guesswork. This builds a
// deck that exercises all of them, including the states you only hit by
// accident: a slot under target and another over it, a rejected card that
// became a hard filter, an atomic swap awaiting a ruling.
//
// Scope: it only ever creates or replaces the single deck named below. It
// never touches your real decks, and it never writes to the cards table.
//
//   node scripts/seed-demo.mjs [--keep]     (--keep: fail instead of replacing)

import { openDb } from "../src/db.ts";
import {
  addCard,
  createDeck,
  createSlot,
  createTag,
  deleteDeck,
  updateCard,
} from "../src/deck/service.ts";
import { createProposal, acceptItem, rejectItem, listProposals, addCardNote } from "../src/deck/proposals.ts";
import { updateBrief, setEngine, proposeBriefEdit } from "../src/deck/brief.ts";
import { addPlaytestNote, resolveListName } from "../src/deck/interop.ts";

const DECK_NAME = "Demo — Atraxa Superfriends";
const db = openDb();

const existing = db.prepare("SELECT id FROM decks WHERE name = ?").get(DECK_NAME);
if (existing) {
  if (process.argv.includes("--keep")) {
    console.error(`"${DECK_NAME}" already exists (id ${existing.id}). Drop --keep to replace it.`);
    process.exit(1);
  }
  deleteDeck(db, existing.id);
  console.log(`Replaced the previous "${DECK_NAME}".`);
}

const deckId = createDeck(db, DECK_NAME);

// Resolve by exact name against whatever card snapshot is installed; anything
// missing is skipped and reported rather than aborting the seed.
const missing = new Set();
function oracle(name) {
  const hits = resolveListName(db, name);
  if (hits.length !== 1) {
    missing.add(name);
    return null;
  }
  return hits[0].oracle_id;
}

const slots = {
  manaBase: createSlot(db, deckId, "mana base", 36, 38),
  ramp: createSlot(db, deckId, "ramp", 10, 12),
  walkers: createSlot(db, deckId, "walkers", 8, 12),
  proliferate: createSlot(db, deckId, "proliferate", 6, 10),
  interaction: createSlot(db, deckId, "interaction", 6, 10),
  draw: createSlot(db, deckId, "draw", 5, 8),
  defense: createSlot(db, deckId, "defense", 3, 5),
};
const tags = {
  proliferate: createTag(db, deckId, "proliferate"),
  removal: createTag(db, deckId, "removal"),
  ultimate: createTag(db, deckId, "ultimate"),
  wincon: createTag(db, deckId, "wincon"),
};

function add(name, slot, opts = {}) {
  const oid = oracle(name);
  if (!oid) return null;
  addCard(db, deckId, oid, { slotId: slot ?? null, role: opts.role });
  const patch = {};
  if (opts.quantity) patch.quantity = opts.quantity;
  if (opts.owned) patch.owned = true;
  if (opts.tags) patch.tagIds = opts.tags;
  if (Object.keys(patch).length) updateCard(db, deckId, oid, patch);
  return oid;
}

add("Atraxa, Praetors' Voice", null, { role: "commander", owned: true });

// Deliberately leaves `walkers` under target and `ramp` over it, so the slot
// deltas and the audit have something real to report.
const groups = [
  [slots.ramp, ["Sol Ring", "Arcane Signet", "Fellwar Stone", "Chromatic Lantern", "Cultivate",
    "Nature's Lore", "Rampant Growth", "Farseek", "Birds of Paradise", "Llanowar Elves",
    "Kodama's Reach", "Three Visits", "Mind Stone"]],
  [slots.walkers, ["Teferi, Temporal Archmage", "Vraska, Golgari Queen", "Karn Liberated",
    "Ajani Steadfast", "Narset Transcendent"]],
  [slots.proliferate, ["Flux Channeler", "Evolution Sage", "Inexorable Tide", "Contagion Engine",
    "Contagion Clasp", "Thrummingbird", "Tekuthal, Inquiry Dominus"]],
  [slots.interaction, ["Swords to Plowshares", "Path to Exile", "Anguished Unmaking",
    "Assassin's Trophy", "Beast Within", "Cyclonic Rift", "Vindicate"]],
  [slots.draw, ["Rhystic Study", "Sylvan Library", "Phyrexian Arena", "Night's Whisper",
    "Painful Truths"]],
  [slots.defense, ["Ghostly Prison", "Propaganda", "Norn's Annex"]],
  [slots.manaBase, ["Command Tower", "Exotic Orchard", "Breeding Pool", "Godless Shrine",
    "Hallowed Fountain", "Overgrown Tomb", "Temple Garden", "Watery Grave", "Yavimaya Coast",
    "Reflecting Pool", "Path of Ancestry", "Bojuka Bog", "Karn's Bastion"]],
];
for (const [slot, names] of groups) for (const n of names) add(n, slot);

// Basics fill the mana base slot toward its target and exercise the quantity path.
add("Forest", slots.manaBase, { quantity: 6, owned: true });
add("Plains", slots.manaBase, { quantity: 5, owned: true });
add("Island", slots.manaBase, { quantity: 5, owned: true });
add("Swamp", slots.manaBase, { quantity: 5, owned: true });

for (const [name, tagIds] of [
  ["Flux Channeler", [tags.proliferate]],
  ["Inexorable Tide", [tags.proliferate, tags.wincon]],
  ["Contagion Engine", [tags.proliferate]],
  ["Karn Liberated", [tags.ultimate, tags.wincon]],
  ["Swords to Plowshares", [tags.removal]],
  ["Assassin's Trophy", [tags.removal]],
]) {
  const oid = oracle(name);
  if (oid) updateCard(db, deckId, oid, { tagIds });
}

updateBrief(db, deckId, {
  thesis:
    "Atraxa proliferates planeswalker loyalty every end step until an ultimate ends the game. " +
    "The deck is a control shell first: survive, stick a walker, then let the proliferate engine " +
    "outpace what any one opponent can remove.",
  constraints_md:
    "- Casual power level; no infinite combos.\n" +
    "- The playgroup dislikes counterspells — do not suggest them.\n" +
    "- Roughly $20 per card ceiling.\n" +
    "- Keep the curve under 4 outside of the walkers.",
});

const enginePieces = ["Flux Channeler", "Evolution Sage", "Inexorable Tide", "Contagion Engine"]
  .map(oracle)
  .filter(Boolean)
  .map((oracle_id) => ({ oracle_id }));
if (enginePieces.length)
  setEngine(db, deckId, "proliferate loop", "Cheap proliferate triggers that stack with Atraxa's end step.", enginePieces);

// --- history: the decision log is the version history, so give it one ---

function propose(items, note, source = "agent") {
  return createProposal(db, deckId, items, { source, note });
}
function itemsOf(proposalId) {
  return listProposals(db, deckId, "open").find((p) => p.id === proposalId)?.items ?? [];
}

// An accepted add, so the log has an accept in it.
const oGoyf = oracle("Doubling Season");
if (oGoyf) {
  const pid = propose(
    [{ action: "add", oracle_id: oGoyf, slot_id: slots.walkers, rationale: "Doubles loyalty counters — every walker arrives one activation from its ultimate." }],
    "Loyalty doubling",
  );
  const item = itemsOf(pid)[0];
  if (item) acceptItem(db, item.id);
}

// A rejection that routes to a hard filter, and one that routes to a card note.
const oCounter = oracle("Counterspell");
if (oCounter) {
  const pid = propose(
    [{ action: "add", oracle_id: oCounter, slot_id: slots.interaction, rationale: "Cheapest possible answer to a combo turn." }],
    "Interaction top-up",
  );
  const item = itemsOf(pid)[0];
  if (item) rejectItem(db, item.id, "hard_filter", "The playgroup hates counterspells. Never suggest one again.");
}
const oOrb = oracle("Static Orb");
if (oOrb) {
  const pid = propose(
    [{ action: "add", oracle_id: oOrb, slot_id: slots.defense, rationale: "Slows the table down while walkers tick up." }],
    "Stax angle",
  );
  const item = itemsOf(pid)[0];
  if (item) rejectItem(db, item.id, "playtest_finding", "Tried it — symmetrical enough that it cost me the games I was ahead in.");
}

// Left pending on purpose: an atomic swap, so the grouped-ruling UI has a
// case to render.
const oCut = oracle("Mind Stone") ?? oracle("Chromatic Lantern");
const oAdd = oracle("Oath of Teferi");
if (oCut && oAdd) {
  propose(
    [
      { action: "cut", oracle_id: oCut, rationale: "Weakest rock in an already-over-target ramp slot.", group_id: "swap-1" },
      { action: "add", oracle_id: oAdd, slot_id: slots.walkers, rationale: "Extra activation every turn is worth more here than the fourth mana rock.", group_id: "swap-1" },
    ],
    "Swap a rock for an extra walker activation",
  );
}

const oArena = oracle("Phyrexian Arena");
if (oArena)
  proposeBriefEdit(
    db,
    deckId,
    "constraints",
    { content: "- Casual power level; no infinite combos.\n- No counterspells.\n- Roughly $20 per card ceiling.\n- Life total is a resource; symmetrical taxes are not." },
    "Every stax piece suggested so far has been cut for symmetry. The constraint should say so.",
  );

if (oOrb) addCardNote(db, deckId, oOrb, "Dead in three of five opening hands.");
addPlaytestNote(db, deckId, "Goldfished ten games: engine online turn 6 on the play, turn 8 on the draw. Never lost to running out of gas — lost to not interacting.");
addPlaytestNote(db, deckId, "Two games ended with a walker one activation short of an ultimate. More proliferate, not more walkers.");

// A short transcript, so the context meter and consolidate have input
// without needing a live model call.
const chat = db.prepare("INSERT INTO chat_messages (deck_id, role, content_json) VALUES (?, ?, ?)");
for (const [role, content] of [
  ["user", "I want this to be an Atraxa proliferate deck that wins with planeswalker ultimates. Casual, no infinite combos."],
  ["assistant", "Understood. The shell is a control deck that happens to win with [[Atraxa, Praetors' Voice]] ultimates rather than a superfriends pile — that changes what goes in the interaction slot."],
  ["user", "My playgroup hates counterspells, never suggest them."],
  ["assistant", "Noted as a durable constraint. I'll lean on unconditional removal like [[Swords to Plowshares]] and [[Assassin's Trophy]] instead."],
  ["user", "What's the ramp package look like?"],
  ["assistant", "The ramp slot is currently over target. I'd cut the weakest rock before adding anything else to it."],
]) {
  chat.run(deckId, role, JSON.stringify({ role, content }));
}

const state = db
  .prepare("SELECT COALESCE(SUM(quantity), 0) n FROM deck_cards WHERE deck_id = ? AND role != 'companion'")
  .get(deckId);
console.log(`Seeded "${DECK_NAME}" (deck #${deckId}) — ${state.n}/100 cards.`);
if (missing.size) {
  console.log(`\n${missing.size} name(s) not in this card snapshot, skipped:`);
  console.log("  " + [...missing].join(", "));
}
console.log(`\nOpen it at http://localhost:${process.env.DECKBUILDER_PORT ?? 8787}/#/deck/${deckId}`);
