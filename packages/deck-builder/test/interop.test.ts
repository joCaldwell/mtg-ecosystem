import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { PermissionError } from "../src/actor.ts";
import { OWNER } from "./actors.ts";
import { CardResolutionError } from "../src/cards.ts";
import { createDeck, createSlot, getDeck, shortlistCard } from "../src/deck/service.ts";
import { listLog } from "../src/deck/log.ts";
import { setQuantity, undo } from "../src/deck/membership.ts";
import {
  applyImport,
  diffImport,
  exportDeck,
  fillFromList,
  parseArchidektList,
} from "../src/deck/interop.ts";
import { AGENT, add, newDeck, testDb } from "./helpers.ts";

const db = testDb();

describe("Archidekt export (spec §12)", () => {
  test("emits quantity + name + [Category], grouped command zone first then slot order", () => {
    const id = createDeck(db, OWNER, "Export shape");
    const ramp = createSlot(db, OWNER, id, "ramp", 8, 12);
    const interaction = createSlot(db, OWNER, id, "interaction", 6, null);
    add(db, id, "id-teferi", { role: "commander" });
    add(db, id, "id-solring", { slotId: ramp });
    add(db, id, "id-counterspell", { slotId: interaction });
    add(db, id, "id-goyf"); // unslotted

    const r = exportDeck(db, OWNER, id);
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
    const id = createDeck(db, OWNER, "Export qty");
    add(db, id, "id-forest");
    setQuantity(db, OWNER, id, "id-forest", 12);
    assert.match(exportDeck(db, OWNER, id).text, /^12 Forest \[Unslotted\]$/m);
    assert.equal(exportDeck(db, OWNER, id, { categories: false }).text.trim(), "12 Forest");
  });



  test("companion is exported but stays outside the 100", () => {
    const id = createDeck(db, OWNER, "Export companion");
    add(db, id, "id-atraxa", { role: "commander" });
    add(db, id, "id-llanowar", { role: "companion" });
    assert.match(exportDeck(db, OWNER, id).text, /Llanowar Elves \[Companion\]/);
  });

  test("a selection exports only matching deck cards, preserves quantities, and never falls back to the full deck", () => {
    const id = createDeck(db, OWNER, "Filtered export");
    add(db, id, "id-teferi", { role: "commander" });
    add(db, id, "id-forest");
    setQuantity(db, OWNER, id, "id-forest", 12);
    add(db, id, "id-solring");
    const before = getDeck(db, OWNER, id);
    const r = exportDeck(db, OWNER, id, { categories: false, oracleIds: ["id-teferi", "id-forest", "id-forest", "not-in-this-deck"] });
    const names = new Map(before.cards.map(c => [c.oracle_id, c.name]));
    assert.equal(r.text, `1 ${names.get("id-teferi")}\n12 ${names.get("id-forest")}\n`);
    assert.equal(r.card_count, 13);
    assert.equal(r.line_count, 2);
    assert.equal(r.revision, before.deck.revision);
    for (const oracleIds of [[], ["not-in-this-deck"]]) {
      const empty = exportDeck(db, OWNER, id, { oracleIds, includeMaybe: true });
      assert.equal(empty.text, "");
      assert.equal(empty.card_count, 0);
      assert.equal(empty.line_count, 0);
    }
    assert.deepEqual(getDeck(db, OWNER, id), before);
  });

  test("maybe-list inclusion is independent of the selection and round-trips as Maybeboard", () => {
    const id = createDeck(db, OWNER, "Optional maybe export");
    add(db, id, "id-solring");
    shortlistCard(db, OWNER, id, "id-forest");
    const before = getDeck(db, OWNER, id);
    const maybeName = before.cards.find(c => c.oracle_id === "id-forest")!.name;
    assert.equal(exportDeck(db, OWNER, id).card_count, 1);
    const full = exportDeck(db, OWNER, id, { includeMaybe: true });
    assert.equal(full.card_count, 2);
    assert.equal(full.line_count, 2);
    assert.ok(full.text.endsWith(`1 ${maybeName} [Maybeboard]\n`));
    const oracleIds = ["id-forest"];
    assert.equal(exportDeck(db, OWNER, id, { oracleIds }).text, "");
    assert.equal(exportDeck(db, OWNER, id, { oracleIds, includeMaybe: true, categories: false }).text, `1 ${maybeName}\n`);
    const imported = createDeck(db, OWNER, "Maybe export round trip");
    applyImport(db, OWNER, imported, full.text);
    const parked = getDeck(db, OWNER, imported).cards.find(c => c.oracle_id === "id-forest")!;
    assert.equal(parked.maybe, 1);
    assert.equal(parked.quantity, 1);
    assert.deepEqual(getDeck(db, OWNER, id), before);
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

  test("skips comments, section headers and count headers; reads SB: as the sideboard", () => {
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
      entries.map((e) => [e.name, e.categories]),
      [
        ["Teferi, Temporal Archmage", ["Commander"]],
        ["Counterspell", ["Sideboard"]],
      ],
    );
    assert.equal(unparsed.length, 0);
  });

  test("a section header is the category of every line beneath it, until the next header", () => {
    // The shape of Archidekt's plain text export: no [Category] on any line.
    const { entries } = parseArchidektList(
      [
        "Commander",
        "1 Teferi, Temporal Archmage",
        "",
        "Mainboard",
        "1 Sol Ring",
        "",
        "10 Forest",
        "",
        "Maybeboard",
        "1 Ball Lightning",
        "",
        "1 Tarmogoyf [ramp]",
      ].join("\n"),
    );
    assert.deepEqual(
      entries.map((e) => [e.name, e.categories]),
      [
        ["Teferi, Temporal Archmage", ["Commander"]],
        ["Sol Ring", []],
        ["Forest", []],
        ["Ball Lightning", ["Maybeboard"]],
        ["Tarmogoyf", ["ramp", "Maybeboard"]],
      ],
    );
  });

  test("a count header names its section too, and an unknown one returns to the deck", () => {
    const { entries } = parseArchidektList(
      ["Maybeboard (1)", "1 Ball Lightning", "Creatures (1)", "1 Tarmogoyf"].join("\n"),
    );
    assert.deepEqual(
      entries.map((e) => [e.name, e.categories]),
      [
        ["Ball Lightning", ["Maybeboard"]],
        ["Tarmogoyf", []],
      ],
    );
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

describe("import diff (spec §12)", () => {
  function seed(name: string) {
    const id = createDeck(db, OWNER, name);
    add(db, id, "id-teferi", { role: "commander" });
    add(db, id, "id-solring");
    add(db, id, "id-counterspell");
    return id;
  }

  test("round-trips its own export with zero changes", () => {
    const id = seed("Import roundtrip");
    const diff = diffImport(db, OWNER, id, exportDeck(db, OWNER, id).text);
    assert.deepEqual(diff.adds, []);
    assert.deepEqual(diff.cuts, []);
    assert.equal(diff.unchanged, 3);
    assert.equal(diff.unresolved.length, 0);
  });

  test("computes adds, cuts and quantity changes against the current deck", () => {
    const id = seed("Import diff");
    const diff = diffImport(
      db, OWNER,
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
    assert.equal(diff.add_count, 3); // Tarmogoyf + two additional Sol Rings
    assert.equal(diff.cut_count, 1);
    assert.equal(diff.unchanged, 2); // Teferi + the original Sol Ring
  });

  test("counts every copy of a newly imported basic land", () => {
    const id = createDeck(db, OWNER, "Import copy count");
    const diff = diffImport(db, OWNER, id, "30 Forest\n1 Sol Ring");
    assert.equal(diff.adds.length, 2); // two distinct card rows
    assert.equal(diff.add_count, 31); // but 31 physical cards
    assert.equal(diff.cut_count, 0);
  });

  test("a category matching a slot name assigns the slot", () => {
    const id = seed("Import slots");
    const ramp = createSlot(db, OWNER, id, "ramp", null, null);
    const diff = diffImport(db, OWNER, id, "1 Llanowar Elves [ramp]");
    const add = diff.adds.find((a) => a.name === "Llanowar Elves")!;
    assert.equal(add.slot_id, ramp);
    assert.equal(add.slot_name, "ramp");
  });

  test("names are resolved exactly — near misses are reported, never guessed", () => {
    const id = seed("Import unresolved");
    const diff = diffImport(db, OWNER, id, "1 Sol Rings\n1 Mana Drainn");
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
    const diff = diffImport(db, OWNER, id, "1 Witch Enchanter // Witch-Blessed Meadow");
    assert.equal(diff.ambiguous.length, 0);
    assert.deepEqual(diff.adds.map((a) => a.name), ["Witch Enchanter // Witch-Blessed Meadow"]);
  });
});

describe("import applies in full and at once (spec §12)", () => {
  test("adds, cuts and quantity changes all land in one call — no per-card gate", () => {
    const id = createDeck(db, OWNER, "Import applies");
    add(db, id, "id-solring"); // absent from the list below → cut
    add(db, id, "id-rats");
    setQuantity(db, OWNER, id, "id-rats", 1);

    const beforeRevision = getDeck(db, OWNER, id).deck.revision;
    const r = applyImport(db, OWNER, id, "1 Counterspell\n1 Tarmogoyf\n4 Typhoid Rats");
    assert.deepEqual(r.applied, { added: 5, cut: 1, quantity_changed: 1, role_changed: 0 });

    const names = getDeck(db, OWNER, id).cards.map((c) => c.name).sort();
    assert.deepEqual(names, ["Counterspell", "Tarmogoyf", "Typhoid Rats"]);
    assert.equal(getDeck(db, OWNER, id).cards.find((c) => c.name === "Typhoid Rats")!.quantity, 4);
    assert.equal(getDeck(db, OWNER, id).deck.revision, beforeRevision + 1);
    assert.equal(listLog(db, OWNER, { deckId: id })[0].revision, beforeRevision + 1);
    // Nothing waiting on the owner: the paste was the ruling.
    assert.equal(getDeck(db, OWNER, id).computed.pending.adds, 0);
  });

  test("a 100-card list is one log entry, not one per card ", () => {
    const id = createDeck(db, OWNER, "Import log volume");
    const list = [
      "94 Forest",
      "1 Llanowar Elves",
      "1 Seedborn Muse",
      "1 Counterspell",
      "1 Sol Ring",
      "1 Tarmogoyf",
      "1 Typhoid Rats",
    ].join("\n");
    applyImport(db, OWNER, id, list);
    const log = listLog(db, OWNER, { deckId: id });
    assert.equal(log.length, 1);
    assert.equal(log[0].kind, "import");
    assert.match(log[0].summary, /\+100\/−0/);
    assert.equal(log[0].cards.length, 7);
    assert.equal(getDeck(db, OWNER, id).deck.revision, 1);
    assert.equal(log[0].revision, 1);
  });

  test("undoing the single log entry restores the entire pre-import list", () => {
    const id = createDeck(db, OWNER, "Import undo");
    const ramp = createSlot(db, OWNER, id, "ramp", 8, 12);
    add(db, id, "id-teferi", { role: "commander" });
    add(db, id, "id-solring", { slotId: ramp });

    applyImport(db, OWNER, id, "1 Counterspell\n1 Tarmogoyf");
    assert.deepEqual(getDeck(db, OWNER, id).cards.map((c) => c.name).sort(), ["Counterspell", "Tarmogoyf"]);

    const entry = listLog(db, OWNER, { deckId: id })[0];
    const importedRevision = getDeck(db, OWNER, id).deck.revision;
    undo(db, OWNER, entry.id);

    const after = getDeck(db, OWNER, id);
    assert.deepEqual(after.cards.map((c) => c.name).sort(), ["Sol Ring", "Teferi, Temporal Archmage"]);
    const solring = after.cards.find((c) => c.name === "Sol Ring")!;
    assert.equal(solring.slot_id, ramp); // slot survives the round trip
    assert.equal(after.cards.find((c) => c.name === "Teferi, Temporal Archmage")!.role, "commander");
    assert.equal(after.deck.revision, importedRevision + 1);
    // The undo is itself logged, and the import row is marked as undone.
    assert.equal(listLog(db, OWNER, { deckId: id })[0].kind, "undo");
    assert.ok(listLog(db, OWNER, { deckId: id }).find((e) => e.id === entry.id)!.undone_by);
  });

  test("a [Commander] category puts the card in the command zone", () => {
    const id = createDeck(db, OWNER, "Import commander");
    applyImport(db, OWNER, id, "1 Teferi, Temporal Archmage [Commander]\n1 Counterspell");
    const state = getDeck(db, OWNER, id);
    assert.equal(state.cards.find((c) => c.name === "Teferi, Temporal Archmage")!.role, "commander");
    assert.equal(state.deck.color_identity, "U"); // identity recomputed from it
  });

  test("an identical list changes nothing and writes no log entry", () => {
    const id = createDeck(db, OWNER, "Import noop");
    add(db, id, "id-solring");
    const r = applyImport(db, OWNER, id, "1 Sol Ring");
    assert.equal(r.log_id, null);
    assert.deepEqual(r.applied, { added: 0, cut: 0, quantity_changed: 0, role_changed: 0 });
    // Only the owner's own add is in the log.
    assert.equal(listLog(db, OWNER, { deckId: id }).length, 1);
  });

  test("a Maybeboard category is shortlisted, not added, and the preview says so", () => {
    const id = createDeck(db, OWNER, "Import maybeboard");
    const text = "1 Sol Ring\n1 Ball Lightning [Maybeboard]";
    assert.deepEqual(diffImport(db, OWNER, id, text).set_aside.map((c) => c.name), ["Ball Lightning"]);
    applyImport(db, OWNER, id, text);
    const view = getDeck(db, OWNER, id);
    assert.equal(view.computed.card_count, 1);
    assert.equal(view.cards.find((c) => c.name === "Ball Lightning")!.maybe, 1);
  });

  test("Archidekt's sectioned export sets the commander and keeps the maybeboard out of the deck", () => {
    const id = createDeck(db, OWNER, "Import sections");
    const text = [
      "Commander",
      "1 Teferi, Temporal Archmage",
      "",
      "Mainboard",
      "1 Sol Ring",
      "5 Forest",
      "",
      "Maybeboard",
      "1 Ball Lightning",
      "1 Tarmogoyf",
    ].join("\n");
    applyImport(db, OWNER, id, text);
    const view = getDeck(db, OWNER, id);
    assert.equal(view.cards.find((c) => c.name === "Teferi, Temporal Archmage")!.role, "commander");
    assert.equal(view.deck.color_identity, "U");
    assert.equal(view.computed.card_count, 7);
    assert.deepEqual(
      view.cards.filter((c) => c.maybe).map((c) => c.name).sort(),
      ["Ball Lightning", "Tarmogoyf"],
    );
  });

  test("pasting the sectioned list over a deck that took it all in flat repairs it, in one undoable entry", () => {
    const id = createDeck(db, OWNER, "Import repair");
    // What the same list produced while the headers were being skipped.
    applyImport(db, OWNER, id, "1 Teferi, Temporal Archmage\n1 Sol Ring\n1 Ball Lightning");
    const text = "Commander\n1 Teferi, Temporal Archmage\n\nMainboard\n1 Sol Ring\n\nMaybeboard\n1 Ball Lightning";

    const diff = diffImport(db, OWNER, id, text);
    assert.deepEqual(diff.role_changes, [
      { oracle_id: "id-teferi", name: "Teferi, Temporal Archmage", from: "card", to: "commander" },
    ]);
    assert.deepEqual(diff.cuts.map((c) => c.name), ["Ball Lightning"]);
    assert.deepEqual(diff.set_aside.map((c) => c.name), ["Ball Lightning"]);

    const r = applyImport(db, OWNER, id, text);
    assert.deepEqual(r.applied, { added: 0, cut: 1, quantity_changed: 0, role_changed: 1 });
    let view = getDeck(db, OWNER, id);
    assert.equal(view.cards.find((c) => c.name === "Teferi, Temporal Archmage")!.role, "commander");
    assert.equal(view.cards.find((c) => c.name === "Ball Lightning")!.maybe, 1);
    assert.equal(view.computed.card_count, 2);
    // Already as the list says: nothing left to do.
    assert.equal(applyImport(db, OWNER, id, text).log_id, null);

    undo(db, OWNER, r.log_id!);
    view = getDeck(db, OWNER, id);
    assert.equal(view.cards.find((c) => c.name === "Teferi, Temporal Archmage")!.role, "card");
    assert.equal(view.computed.card_count, 3);
  });

  test("a list never demotes the commander, and never seats a third", () => {
    const id = createDeck(db, OWNER, "Import keeps commander");
    add(db, id, "id-teferi", { role: "commander" });
    add(db, id, "id-kenrith", { role: "commander" });
    add(db, id, "id-golos");
    const diff = diffImport(
      db, OWNER,
      id,
      "Commander\n1 Golos, Tireless Pilgrim\n\nMainboard\n1 Teferi, Temporal Archmage\n1 Kenrith, the Returned King",
    );
    assert.deepEqual(diff.role_changes, []);
    assert.deepEqual(diff.cuts, []);
  });

  test("the maybe list is not exported and a shortlisted card in the paste is an add", () => {
    const id = createDeck(db, OWNER, "Import promotes");
    applyImport(db, OWNER, id, "1 Sol Ring\n1 Ball Lightning [Maybeboard]");
    assert.equal(exportDeck(db, OWNER, id).text, "1 Sol Ring [Unslotted]\n");
    applyImport(db, OWNER, id, "1 Sol Ring\n1 Ball Lightning");
    assert.equal(getDeck(db, OWNER, id).computed.card_count, 2);
  });
});

describe("who may paste a list (spec §10.3)", () => {
  test("importing over a deck is the owner's; an agent fills a draft, all or nothing", () => {
    const active = newDeck(db, "paper");
    assert.throws(() => applyImport(db, AGENT, active, "1 Sol Ring"), PermissionError);
    assert.throws(() => fillFromList(db, AGENT, active, "1 Sol Ring"), PermissionError);

    const draft = newDeck(db, "theorycrafted");
    assert.throws(
      () => fillFromList(db, AGENT, draft, "1 Sol Ring\n1 Sol Ringz"),
      (e: Error) => e instanceof CardResolutionError && /nothing was changed/.test(e.message),
    );
    assert.equal(getDeck(db, OWNER, draft).computed.card_count, 0);

    fillFromList(db, AGENT, draft, "1 Teferi, Temporal Archmage [Commander]\n1 Sol Ring\n5 Forest");
    const view = getDeck(db, OWNER, draft);
    assert.equal(view.computed.card_count, 7);
    assert.equal(view.deck.color_identity, "U");
    assert.equal(listLog(db, OWNER, { deckId: draft })[0].author, "test-agent");
  });
});
