import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { openDb } from "../src/db.ts";
import { ingestCards } from "../src/ingest.ts";
import { FIXTURES } from "./fixtures.ts";
import { addCard, createDeck, createSlot, getDeck, updateCard } from "../src/deck/service.ts";
import { createProposal, rejectItem, listProposals } from "../src/deck/proposals.ts";
import { getLog, undoDecision } from "../src/deck/log.ts";
import {
  addPlaytestNote,
  applyImport,
  deletePlaytestNote,
  diffImport,
  exportDeck,
  listPlaytestNotes,
  parseArchidektList,
} from "../src/deck/interop.ts";
import { assembleDeckSections } from "../src/agent/context.ts";

const db = openDb(":memory:");
ingestCards(db, FIXTURES);

describe("Archidekt export (spec §9)", () => {
  test("emits quantity + name + [Category], grouped command zone first then slot order", () => {
    const id = createDeck(db, "Export shape");
    const ramp = createSlot(db, id, "ramp", 8, 12);
    const interaction = createSlot(db, id, "interaction", 6, null);
    addCard(db, id, "id-teferi", { role: "commander" });
    addCard(db, id, "id-solring", { slotId: ramp });
    addCard(db, id, "id-counterspell", { slotId: interaction });
    addCard(db, id, "id-goyf"); // unslotted

    const r = exportDeck(db, id);
    assert.deepEqual(r.text.trimEnd().split("\n"), [
      "1 Teferi, Temporal Archmage [Commander]",
      "1 Sol Ring [ramp]",
      "1 Counterspell [interaction]",
      "1 Tarmogoyf [Unslotted]",
    ]);
    assert.equal(r.card_count, 4);
    // No set codes, no foil markers, no "Creatures (24)" count headers — each
    // is documented to break some importers.
    assert.doesNotMatch(r.text, /\(|\*|\d+\)/);
  });

  test("quantities ride the line; categories can be turned off", () => {
    const id = createDeck(db, "Export qty");
    addCard(db, id, "id-forest");
    updateCard(db, id, "id-forest", { quantity: 12 });
    assert.match(exportDeck(db, id).text, /^12 Forest \[Unslotted\]$/m);
    assert.equal(exportDeck(db, id, { categories: false }).text.trim(), "12 Forest");
  });

  test("buy list filters to unowned cards only", () => {
    const id = createDeck(db, "Export buylist");
    addCard(db, id, "id-solring");
    addCard(db, id, "id-counterspell");
    updateCard(db, id, "id-solring", { owned: true });

    const all = exportDeck(db, id);
    assert.equal(all.line_count, 2);
    const buy = exportDeck(db, id, { onlyUnowned: true });
    assert.equal(buy.line_count, 1);
    assert.equal(buy.omitted_owned, 1);
    assert.match(buy.text, /Counterspell/);
    assert.doesNotMatch(buy.text, /Sol Ring/);
  });

  test("companion is exported but stays outside the 100", () => {
    const id = createDeck(db, "Export companion");
    addCard(db, id, "id-atraxa", { role: "commander" });
    addCard(db, id, "id-llanowar", { role: "companion" });
    assert.match(exportDeck(db, id).text, /Llanowar Elves \[Companion\]/);
  });
});

describe("Archidekt list parsing (verified format superset)", () => {
  test("accepts the full round-trip template: 1x Name (code) *F* [Category] ^Label,#hex^", () => {
    const { entries } = parseArchidektList("1x Sol Ring (cmm) 405 *F* [Ramp,Artifact] ^Buy,#000000^");
    assert.equal(entries.length, 1);
    assert.equal(entries[0].quantity, 1);
    assert.equal(entries[0].name, "Sol Ring");
    assert.deepEqual(entries[0].categories, ["Ramp", "Artifact"]);
  });

  test("accepts bare MTGO style, backtick categories, and a missing quantity", () => {
    const { entries } = parseArchidektList(
      ["2 Counterspell", "1 Sol Ring `Maybeboard`", "Tarmogoyf"].join("\n"),
    );
    assert.deepEqual(
      entries.map((e) => [e.quantity, e.name, e.categories]),
      [
        [2, "Counterspell", []],
        [1, "Sol Ring", ["Maybeboard"]],
        [1, "Tarmogoyf", []],
      ],
    );
  });

  test("skips comments, section headers and count headers; strips SB:", () => {
    const { entries, unparsed } = parseArchidektList(
      [
        "// exported from Archidekt",
        "Commander",
        "1 Teferi, Temporal Archmage",
        "",
        "Creatures (24)",
        "Deck",
        "SB: 1 Counterspell",
        "# trailing comment",
      ].join("\n"),
    );
    assert.deepEqual(
      entries.map((e) => e.name),
      ["Teferi, Temporal Archmage", "Counterspell"],
    );
    assert.equal(unparsed.length, 0);
  });

  test("split-card names survive the metadata stripping", () => {
    const { entries } = parseArchidektList("1 Witch Enchanter // Witch-Blessed Meadow [Lands]");
    assert.equal(entries[0].name, "Witch Enchanter // Witch-Blessed Meadow");
    assert.deepEqual(entries[0].categories, ["Lands"]);
  });

  test("category modifiers are stripped rather than guessed at", () => {
    const { entries } = parseArchidektList("1 Sol Ring [Commander{top}]");
    assert.deepEqual(entries[0].categories, ["Commander"]);
  });
});

describe("import diff (spec §9)", () => {
  function seed(name: string) {
    const id = createDeck(db, name);
    addCard(db, id, "id-teferi", { role: "commander" });
    addCard(db, id, "id-solring");
    addCard(db, id, "id-counterspell");
    return id;
  }

  test("round-trips its own export with zero changes", () => {
    const id = seed("Import roundtrip");
    const diff = diffImport(db, id, exportDeck(db, id).text);
    assert.deepEqual(diff.adds, []);
    assert.deepEqual(diff.cuts, []);
    assert.equal(diff.unchanged, 3);
    assert.equal(diff.unresolved.length, 0);
  });

  test("computes adds, cuts and quantity changes against the current deck", () => {
    const id = seed("Import diff");
    const diff = diffImport(
      db,
      id,
      ["1 Teferi, Temporal Archmage", "3 Sol Ring", "1 Tarmogoyf"].join("\n"),
    );
    assert.deepEqual(
      diff.adds.map((a) => a.name),
      ["Tarmogoyf"],
    );
    assert.deepEqual(
      diff.cuts.map((c) => c.name),
      ["Counterspell"],
    );
    assert.deepEqual(diff.quantity_changes, [
      { oracle_id: "id-solring", name: "Sol Ring", from: 1, to: 3 },
    ]);
  });

  test("a category matching a slot name assigns the slot", () => {
    const id = seed("Import slots");
    const ramp = createSlot(db, id, "ramp", null, null);
    const diff = diffImport(db, id, "1 Llanowar Elves [ramp]");
    const add = diff.adds.find((a) => a.name === "Llanowar Elves")!;
    assert.equal(add.slot_id, ramp);
    assert.equal(add.slot_name, "ramp");
  });

  test("names are resolved exactly — near misses are reported, never guessed", () => {
    const id = seed("Import unresolved");
    const diff = diffImport(db, id, "1 Sol Rings\n1 Mana Drainn");
    assert.equal(diff.adds.length, 0);
    assert.deepEqual(
      diff.unresolved.map((u) => u.name),
      ["Sol Rings", "Mana Drainn"],
    );
    assert.ok(diff.unresolved[0].suggestions.includes("Sol Ring"));
  });

  test("a name shared with another card's face resolves to the card actually named that", () => {
    // "Witch Enchanter // Witch-Blessed Meadow" has a face named
    // "Witch Enchanter"; a hypothetical standalone card of that name must win.
    const id = seed("Import face collision");
    const diff = diffImport(db, id, "1 Witch Enchanter // Witch-Blessed Meadow");
    assert.equal(diff.ambiguous.length, 0);
    assert.deepEqual(diff.adds.map((a) => a.name), ["Witch Enchanter // Witch-Blessed Meadow"]);
  });

  test("hard-filtered cards are reported as blocked, not proposed", () => {
    const id = seed("Import blocked");
    const pid = createProposal(db, id, [
      { action: "add", oracle_id: "id-ball", rationale: "test" },
    ]);
    const item = listProposals(db, id, "open").find((p) => p.id === pid)!.items[0];
    rejectItem(db, item.id, "hard_filter", "too fragile");

    const diff = diffImport(db, id, "1 Ball Lightning");
    assert.equal(diff.adds.length, 0);
    assert.deepEqual(
      diff.blocked.map((b) => b.name),
      ["Ball Lightning"],
    );
    // …and the import still applies, rather than throwing on the filter.
    const r = applyImport(db, id, "1 Ball Lightning\n1 Sol Ring\n1 Counterspell\n1 Teferi, Temporal Archmage");
    assert.equal(r.log_id, null); // nothing left to change
  });
});

describe("import applies in full and at once (spec §9)", () => {
  test("adds, cuts and quantity changes all land in one call — no per-card gate", () => {
    const id = createDeck(db, "Import applies");
    addCard(db, id, "id-solring"); // absent from the list below → cut
    addCard(db, id, "id-rats");
    updateCard(db, id, "id-rats", { quantity: 1 });

    const r = applyImport(db, id, "1 Counterspell\n1 Tarmogoyf\n4 Typhoid Rats");
    assert.deepEqual(r.applied, { added: 2, cut: 1, quantity_changed: 1 });

    const names = getDeck(db, id).cards.map((c) => c.name).sort();
    assert.deepEqual(names, ["Counterspell", "Tarmogoyf", "Typhoid Rats"]);
    assert.equal(getDeck(db, id).cards.find((c) => c.name === "Typhoid Rats")!.quantity, 4);
    // Nothing waiting on the owner: the paste was the ruling.
    assert.equal(listProposals(db, id, "open").length, 0);
  });

  test("a 100-card list is one log entry, not one per card (§12 retention)", () => {
    const id = createDeck(db, "Import log volume");
    const list = [
      "Llanowar Elves",
      "Seedborn Muse",
      "Counterspell",
      "Sol Ring",
      "Tarmogoyf",
      "Typhoid Rats",
      "Bojuka Bog",
    ]
      .map((n) => `1 ${n}`)
      .join("\n");
    applyImport(db, id, list);
    const log = getLog(db, id) as any[];
    assert.equal(log.length, 1);
    assert.equal(log[0].kind, "accept");
    assert.equal(log[0].action, "import");
    assert.match(log[0].rationale, /\+7\/−0/);
  });

  test("undoing the single log entry restores the entire pre-import list", () => {
    const id = createDeck(db, "Import undo");
    const ramp = createSlot(db, id, "ramp", 8, 12);
    addCard(db, id, "id-teferi", { role: "commander" });
    addCard(db, id, "id-solring", { slotId: ramp });
    updateCard(db, id, "id-solring", { owned: true });

    applyImport(db, id, "1 Counterspell\n1 Tarmogoyf");
    assert.deepEqual(getDeck(db, id).cards.map((c) => c.name).sort(), ["Counterspell", "Tarmogoyf"]);

    const entry = (getLog(db, id) as any[])[0];
    undoDecision(db, id, entry.id);

    const after = getDeck(db, id);
    assert.deepEqual(after.cards.map((c) => c.name).sort(), ["Sol Ring", "Teferi, Temporal Archmage"]);
    const solring = after.cards.find((c) => c.name === "Sol Ring")!;
    assert.equal(solring.slot_id, ramp); // slot survives the round trip
    assert.ok(solring.owned); //             …and so does owned
    assert.equal(after.cards.find((c) => c.name === "Teferi, Temporal Archmage")!.role, "commander");
    // The undo is itself logged, and the import row is marked as undone.
    assert.equal((getLog(db, id) as any[])[0].kind, "undo");
    assert.ok((getLog(db, id) as any[]).find((e) => e.id === entry.id)!.undone_by);
  });

  test("a [Commander] category puts the card in the command zone", () => {
    const id = createDeck(db, "Import commander");
    applyImport(db, id, "1 Teferi, Temporal Archmage [Commander]\n1 Counterspell");
    const state = getDeck(db, id);
    assert.equal(state.cards.find((c) => c.name === "Teferi, Temporal Archmage")!.role, "commander");
    assert.equal(state.deck.color_identity, "U"); // identity recomputed from it
  });

  test("an identical list changes nothing and writes no log entry", () => {
    const id = createDeck(db, "Import noop");
    addCard(db, id, "id-solring");
    const r = applyImport(db, id, "1 Sol Ring");
    assert.equal(r.log_id, null);
    assert.deepEqual(r.applied, { added: 0, cut: 0, quantity_changed: 0 });
    assert.equal((getLog(db, id) as any[]).length, 0);
  });
});

describe("playtest notes (spec §9)", () => {
  test("a note is stamped to the deck revision and pins the list it describes", () => {
    const id = createDeck(db, "Playtest stamp");
    addCard(db, id, "id-solring");
    addCard(db, id, "id-counterspell");
    addPlaytestNote(db, id, "Never found the engine before turn 9");

    const before = listPlaytestNotes(db, id)[0];
    assert.equal(before.revision, 2);
    assert.deepEqual(before.cards, ["Counterspell", "Sol Ring"]);

    // Deck moves on; the note still describes the old list.
    addCard(db, id, "id-goyf");
    const after = listPlaytestNotes(db, id)[0];
    assert.deepEqual(after.cards, ["Counterspell", "Sol Ring"]);
    assert.equal(after.revision, 2);
  });

  test("notes reach the agent's context (kept forever, spec §12)", () => {
    const id = createDeck(db, "Playtest context");
    addCard(db, id, "id-solring");
    addPlaytestNote(db, id, "Static Orb was dead in three of five opens");
    const { sections } = assembleDeckSections(db, id, 30);
    assert.match(sections, /Static Orb was dead in three of five opens/);
    assert.match(sections, /Playtest notes from goldfishing/);
  });

  test("empty notes are rejected; notes can be deleted", () => {
    const id = createDeck(db, "Playtest delete");
    assert.throws(() => addPlaytestNote(db, id, "  "), /cannot be empty/);
    const noteId = addPlaytestNote(db, id, "real note");
    deletePlaytestNote(db, id, noteId);
    assert.equal(listPlaytestNotes(db, id).length, 0);
    assert.throws(() => deletePlaytestNote(db, id, noteId), /not found/);
  });
});
