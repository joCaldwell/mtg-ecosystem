import { test } from "node:test";
import assert from "node:assert/strict";
import { analyzeMana, type ManaCard } from "../src/deck/mana.ts";
import { MANA_FIXTURES } from "./fixtures/mana-analysis.ts";
import { add, newDeck, testDb, AGENT } from "./helpers.ts";
import { OWNER } from "./actors.ts";
import { getDeck, shortlistCard } from "../src/deck/service.ts";
import { publicDeckState } from "../src/state.ts";
import { createProposal } from "../src/deck/proposals.ts";
import { setVisibility } from "../src/sharing.ts";
import { stateLines } from "../src/mcp/render.ts";

function card(name: string, over: Partial<ManaCard> = {}): ManaCard {
  const row = MANA_FIXTURES.find(row => row.name === name || row.name.split(" // ")[0] === name);
  assert(row, `fixture verified by exact DB name: ${name}`);
  return { ...row, quantity: 1, role: "card", maybe: 0, ...over };
}
const single = (name: string) => analyzeMana([card(name)]);

test("fixed symbols, flexible alternatives, generic, snow and variables stay distinct", () => {
  const toluz = single("Toluz, Clever Conductor").first_face.costs;
  assert.equal(toluz.fixed_symbols.U, 1);
  assert.deepEqual(toluz.flexible_symbols, { "{W/U}": 1, "{U/B}": 1 });
  const two = single("Kin-Tree Severance").first_face.costs;
  assert.equal(Object.values(two.fixed_symbols).reduce((a, b) => a + b), 0);
  assert.deepEqual(two.flexible_symbols, { "{2/W}": 1, "{2/B}": 1, "{2/G}": 1 });
  const phyrexian = single("Tamiyo, Compleated Sage").first_face.costs;
  assert.equal(phyrexian.fixed_symbols.G, 1); assert.equal(phyrexian.fixed_symbols.U, 1);
  assert.deepEqual(phyrexian.flexible_symbols, { "{G/U/P}": 1 });
  assert.equal(Object.keys(single("Ulalek, Fused Atrocity").first_face.costs.flexible_symbols).length, 5);
  const variable = single("Lattice Library").first_face.costs;
  assert.equal(variable.fixed_symbols.G, 2); assert.equal(variable.variable_symbols.X, 1);
  assert.equal(single("Icehide Golem").first_face.costs.snow_symbols, 1);
  const colourless = single("Eldrazi Confluence").first_face.costs;
  assert.equal(colourless.fixed_symbols.C, 2); assert.equal(colourless.generic_units, 2);
});

test("intrinsic and explicit source counts weight copies, deduplicate outputs and never use identity", () => {
  const result = analyzeMana([card("Forest", { quantity: 5 }), card("Sol Ring", { quantity: 2 }), card("Savai Triome"), card("Boreal Druid"), card("Glacial Fortress")]);
  assert.equal(result.first_face.sources.land.by_type.G, 5);
  assert.equal(result.first_face.sources.nonland.by_type.C, 3, "two Ring copies and one Druid, not mana units");
  assert.equal(result.first_face.sources.land.by_type.R, 1); assert.equal(result.first_face.sources.land.by_type.B, 1);
  assert.equal(result.first_face.sources.land.by_type.W, 2); assert.equal(result.first_face.sources.land.by_type.U, 1);
  assert.equal(result.coverage.recognized_source_copies, 10);
  const tower = single("Command Tower");
  assert.equal(tower.coverage.unresolved_source_copies, 1); assert.equal(tower.coverage.recognized_source_copies, 0);
  assert.deepEqual(analyzeMana([{ ...card("Command Tower"), ...{ color_identity: "WUBRG", colors: "WUBRG" } }]), tower);
});

test("any-colour, partial, restricted, spell and indirect source text remains honest", () => {
  const any = single("Birds of Paradise");
  assert.equal(any.first_face.sources.nonland.any_color_copies, 1);
  assert.equal(Object.values(any.first_face.sources.nonland.by_type).reduce((a, b) => a + b), 0);
  const partial = single("Tainted Isle");
  assert.equal(partial.first_face.sources.land.by_type.C, 1);
  assert.equal(partial.first_face.sources.land.by_type.U, 0); assert.equal(partial.first_face.sources.land.by_type.B, 0);
  assert.equal(partial.coverage.partial_source_copies, 1);
  for (const name of ["Arcane Signet", "Selesnya Signet", "Fertile Ground", "Evolving Wilds", "Rampant Growth", "Ruthless Knave"]) {
    const result = single(name); assert.equal(result.coverage.recognized_source_copies, 0, name); assert.equal(result.coverage.unresolved_source_copies, 1, name);
  }
  assert(single("Fertile Ground").issues.some(i => i.reason === "unsupported_mana_text"));
  for (const name of ["Ruthless Knave", "Eldrazi Confluence"]) {
    assert(single(name).issues.some(i => i.reason === "quoted_or_granted_text"), name);
  }
});

test("mana-value, mana-cost, cost reduction and unrelated tokens do not imply source warnings", () => {
  for (const name of ["Kin-Tree Severance", "Tamiyo, Compleated Sage", "Lattice Library"]) {
    const result = single(name);
    assert.equal(result.coverage.unresolved_source_copies, 0, name);
    assert.equal(result.coverage.no_supported_source_pattern_copies, 1, name);
    assert.equal(result.issues.filter(i => i.domain === "source").length, 0, name);
  }
});

test("first and other faces are separate alternatives with no flattened cost double count", () => {
  const witch = single("Witch Enchanter");
  assert.equal(witch.first_face.costs.fixed_symbols.W, 1);
  assert.equal(witch.first_face.sources.land.by_type.W, 0); assert.equal(witch.other_faces.sources.land.by_type.W, 1);
  const alrund = single("Alrund, God of the Cosmos");
  assert.equal(alrund.first_face.costs.fixed_symbols.U, 2); assert.equal(alrund.other_faces.costs.fixed_symbols.U, 1);
  const captive = single("Ulvenwald Captive");
  assert.equal(captive.first_face.sources.nonland.by_type.G, 1); assert.equal(captive.other_faces.sources.nonland.by_type.C, 1);
  const pathway = single("Hengegate Pathway");
  assert.equal(pathway.first_face.sources.land.by_type.W, 1); assert.equal(pathway.other_faces.sources.land.by_type.U, 1);
  assert.equal(pathway.coverage.recognized_source_copies, 1);
});

test("unsupported cost/face text gives stable issues; quoted/granted patterns never become parent sources", () => {
  // Synthetic negative syntax, not invented facts stored in the card database.
  const malformed = analyzeMana([card("Counterspell", { mana_cost: "{U}{HW}{C/P}oops" }), card("Witch Enchanter", { faces_json: "[{}]" })]);
  assert.equal(malformed.first_face.costs.fixed_symbols.U, 1);
  assert.equal(malformed.coverage.cost_issue_copies, 2);
  assert(malformed.issues.some(i => i.reason === "unsupported_symbol" && i.excerpt === "{C/P}"));
  for (const text of ['Creatures have "{T}: Add {W}."', '(Reminder ({T}: Add {W}.))', '{T}: Add {W}. Spend this mana only to cast creatures.', '{T}: Add {W}. (']) {
    const result = analyzeMana([card("Sol Ring", { oracle_text: text })]);
    assert.equal(result.coverage.recognized_source_copies, 0, text);
  }
  assert.equal(analyzeMana([card("Counterspell", { mana_cost: "{0}" })]).first_face.costs.no_printed_cost_nonland_copies, 0);
  assert.equal(analyzeMana([card("Counterspell", { mana_cost: null })]).first_face.costs.no_printed_cost_nonland_copies, 1);
});

test("quantity, commander, companion/maybe scope and coverage partitions remain consistent", () => {
  const result = analyzeMana([card("Counterspell", { quantity: 3, role: "commander" }), card("Forest", { quantity: 4 }), card("Sol Ring", { maybe: 1 }), card("Birds of Paradise", { role: "companion" })]);
  assert.equal(result.first_face.costs.fixed_symbols.U, 6); assert.equal(result.first_face.sources.land.by_type.G, 4);
  const c = result.coverage;
  assert.equal(c.included_copies, 7); assert.equal(c.recognized_source_copies + c.unresolved_source_copies + c.no_supported_source_pattern_copies, c.included_copies);
  assert(c.partial_source_copies <= c.recognized_source_copies);
});

test("owner/public capability and agent text agree without exposing internal face fields or pending cards", () => {
  const db = testDb();
  try {
    const id = newDeck(db); add(db, id, "id-atraxa", { role: "commander" }); add(db, id, "id-counterspell", { quantity: 3 }); add(db, id, "id-forest", { quantity: 4 }); add(db, id, "id-goyf", { role: "companion" }); shortlistCard(db, OWNER, id, "id-solring", "Parked");
    createProposal(db, AGENT, id, { summary: "Uncommitted source", items: [{ action: "add", oracle_id: "id-llanowar", reason: "Possible additional source." }] });
    const owner = getDeck(db, OWNER, id); const { slug } = setVisibility(db, OWNER, id, "public"); assert(slug);
    const publicDeck = publicDeckState(db, null, slug);
    assert.deepEqual(publicDeck.computed.mana, owner.computed.mana);
    assert.equal(owner.computed.mana.first_face.costs.fixed_symbols.U, 7);
    assert.equal(owner.computed.mana.first_face.sources.land.by_type.G, 4);
    assert.equal(owner.computed.type_counts.Creature, 1); assert.equal(owner.computed.type_counts.Land, 4);
    assert(!("layout" in owner.cards[0])); assert(!("faces_json" in publicDeck.cards[0]));
    const text = stateLines(owner);
    assert.match(text, /fixed cost symbols \/ recognized land copies \/ other permanent copies/);
    assert.match(text, /not available mana or casting odds/); assert.match(text, /any-colour sources/);
    assert.deepEqual(getDeck(db, AGENT, id).computed.mana, owner.computed.mana);
  } finally { db.close(); }
});

test("agent transcript labels legacy all-face alternatives separately from fixed first-face demand", () => {
  const db = testDb();
  try {
    const view = getDeck(db, OWNER, newDeck(db));
    const mana = analyzeMana([card("Toluz, Clever Conductor"), card("Alrund, God of the Cosmos")]);
    const text = stateLines({ ...view, computed: { ...view.computed, pips: { W: 1, U: 6, B: 1, R: 0, G: 0 }, mana } });
    assert.match(text, /Legacy raw colour-letter count, all printed faces including flexible alternatives: W 1, U 6, B 1/);
    assert.match(text, /Mana comparison, first printed face.*W 0\/0\/0; U 3\/0\/0; B 0\/0\/0/);
    assert.match(text, /flexible costs \{W\/U\} ×1, \{U\/B\} ×1/);
    assert.match(text, /Other printed faces, separately \(not simultaneous\): W 0\/0\/0; U 1\/0\/0/);
    assert.match(text, /The mana comparison counts.*It keeps flexible costs, any-colour output and other faces separate/);
  } finally { db.close(); }
});
