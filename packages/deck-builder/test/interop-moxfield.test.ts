// Moxfield import (spec §12), pinned to real exports. The fixtures are the
// text Moxfield's Export dialog produced for six public decks on 2026-10-25;
// see test/fixtures/moxfield/README.md and the format notes in interop.ts.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { applyImport, diffImport, parseDeckList } from "../src/deck/interop.ts";
import { getDeck } from "../src/deck/service.ts";
import { OWNER } from "./actors.ts";
import { newDeck, testDb } from "./helpers.ts";

const DIR = join(import.meta.dirname, "fixtures", "moxfield");
const fixture = (deck: string, variant: string) => readFileSync(join(DIR, `${deck}.${variant}.txt`), "utf8");

// The commanders each deck's Moxfield page reports (its JSON "commanders" board).
const COMMANDERS: Record<string, string[]> = {
  "rograkh-silas": ["Rograkh, Son of Rohgahh", "Silas Renn, Seeker Adept"],
  "bombadil-jegantha": ["Tom Bombadil"],
  "stickers-attractions": ["The Most Dangerous Gamer"],
  "precon-hail-caesar": ["Caesar, Legion's Emperor"],
  najeela: ["Najeela, the Blade-Blossom"],
  "kinnan-midrange": ["Kinnan, Bonder Prodigy"],
};
const commandersOf = (text: string) =>
  parseDeckList(text).entries.filter((e) => e.categories.includes("Commander")).map((e) => e.name).sort();
const inDeck = (text: string) =>
  parseDeckList(text).entries.filter((e) => !e.categories.some((c) => /^(sideboard|maybeboard)$/i.test(c)));

describe("Moxfield's exports, as Moxfield wrote them", () => {
  for (const [deck, commanders] of Object.entries(COMMANDERS)) {
    test(`${deck}: MTGO and plain text place the commander last; every variant reads the same 100`, () => {
      for (const variant of ["copy-for-mtgo", "copy-plain-text"]) {
        assert.deepEqual(commandersOf(fixture(deck, variant)), commanders, variant);
        assert.deepEqual(parseDeckList(fixture(deck, variant)).unparsed, [], variant);
      }
      const full = parseDeckList(fixture(deck, "copy-for-moxfield"));
      assert.deepEqual(full.unparsed, []);
      // "Copy for Moxfield" marks no commander, and the preview says so instead of guessing.
      assert.deepEqual(commandersOf(fixture(deck, "copy-for-moxfield")), []);
      assert.match(full.notes.join(" "), /does not mark the commander/);
      // Set codes, collector numbers, foil and etched markers are stripped.
      // (A name may end in a number: "Overseer of Vault 76 (PIP) 19".)
      assert.ok(full.entries.every((e) => !/[()*★]/.test(e.name)), "printing details are not part of a name");
      const copies = (text: string) => inDeck(text).reduce((n, e) => n + e.quantity, 0);
      assert.equal(copies(fixture(deck, "copy-for-moxfield")), copies(fixture(deck, "copy-for-mtgo")));
      assert.ok([99, 100].includes(copies(fixture(deck, "copy-for-mtgo"))), "a Commander deck, companion set aside");
    });
  }

  test("collector numbers with letters and stars, four-letter sets and *E* are read", () => {
    const { entries } = parseDeckList(fixture("kinnan-midrange", "copy-for-moxfield"));
    const names = entries.map((e) => e.name);
    assert.ok(names.includes("Arcane Signet"), "1 Arcane Signet (P30M) 1F★ *E*");
    assert.ok(names.includes("Birds of Paradise"), "1 Birds of Paradise (PM11) 165★ *F*");
    assert.ok(parseDeckList(fixture("najeela", "copy-for-moxfield")).entries.some((e) => e.name === "Arcane Signet"), "(SLD) 1492★ *F*");
  });

  test("SIDEBOARD: holds the companion; a blank line ends it before the command zone", () => {
    const mtgo = parseDeckList(fixture("bombadil-jegantha", "copy-for-mtgo")).entries;
    assert.deepEqual(mtgo.find((e) => e.name === "Jegantha, the Wellspring")!.categories, ["Sideboard"]);
    assert.deepEqual(mtgo.find((e) => e.name === "Tom Bombadil")!.categories, ["Commander"]);
    const arena = parseDeckList(fixture("bombadil-jegantha", "copy-for-arena"));
    assert.ok(!arena.entries.some((e) => /^Name /.test(e.name)), "Arena's About block is not a card");
    assert.deepEqual(arena.entries.find((e) => e.name === "Jegantha, the Wellspring")!.categories, ["Companion"]);
    assert.deepEqual(arena.entries.find((e) => e.name === "Tom Bombadil")!.categories, ["Commander"]);
  });

  test("ATTRACTIONS: and STICKERS: are left out and reported", () => {
    const parsed = parseDeckList(fixture("stickers-attractions", "copy-for-mtgo"));
    assert.equal(parsed.ignored.length, 60);
    assert.deepEqual([...new Set(parsed.ignored.map((i) => i.section))].sort(), ["ATTRACTIONS", "STICKERS"]);
    assert.match(parsed.notes.join(" "), /60 lines are on a board that is not part of a Commander deck/);
    assert.deepEqual(commandersOf(fixture("stickers-attractions", "copy-for-mtgo")), ["The Most Dangerous Gamer"]);
  });

  test("the command-zone reading needs the export's shape and a card that can lead a deck (review S5)", () => {
    const db = testDb();
    const id = newDeck(db, "paper");
    const roles = (text: string) => Object.fromEntries(
      [...diffImport(db, OWNER, id, text).adds].map((a) => [a.name, a.role]));
    // An inline [Commander] already names the command zone: the trailing block stays in the deck.
    assert.deepEqual(roles("1 Atraxa, Praetors' Voice [Commander]\n96 Forest\n\n1 Sol Ring\n1 Counterspell"),
      { "Atraxa, Praetors' Voice": "commander", Forest: "card", "Sol Ring": "card", Counterspell: "card" });
    // A list ending in a basic land block never makes the land a commander.
    assert.ok(!Object.values(roles("1 Sol Ring\n89 Forest\n\n10 Forest")).includes("commander"));
    // A 60-card MTGO list with a two-card sideboard is not a Commander export.
    assert.ok(!Object.values(roles("4 Counterspell\n56 Forest\n\n1 Teferi, Temporal Archmage\n1 Sol Ring")).includes("commander"));
    // The export shape with an ineligible card keeps only the eligible one.
    const mixed = roles("97 Forest\n1 Counterspell\n\n1 Teferi, Temporal Archmage\n1 Sol Ring");
    assert.equal(mixed["Teferi, Temporal Archmage"], "commander");
    assert.equal(mixed["Sol Ring"], "card");
  });

  test("a paste is bounded: 5,000 lines, 300 characters a line, in linear time (review B1)", () => {
    assert.throws(() => parseDeckList("1 Forest\n".repeat(5001)), /at most 5,000 lines/);
    const long = parseDeckList(`1 Forest${" ".repeat(40_000)}x`);
    assert.equal(long.entries.length, 0);
    assert.equal(long.unparsed.length, 1);
    const boards = `1 Forest\n\nSTICKERS:\n${"1 Sticker Card\n".repeat(4990)}`;
    const started = performance.now();
    const parsed = parseDeckList(boards);
    assert.equal(parsed.ignored.length, 4990);
    assert.ok(parsed.ignored.every((i) => i.section === "STICKERS"));
    assert.ok(performance.now() - started < 500, "one pass, not one per ignored line");
  });

  test("Archidekt's own headers still decide, so a trailing block there is not a commander", () => {
    const archidekt = "Commander\n1 Atraxa, Praetors' Voice\n\nMainboard\n1 Sol Ring\n\n1 Forest";
    assert.deepEqual(commandersOf(archidekt), ["Atraxa, Praetors' Voice"]);
    assert.deepEqual(commandersOf("1 Sol Ring\n1 Forest"), [], "one block, no command zone");
  });
});

describe("Moxfield and Archidekt import to the same deck", () => {
  // Fixture cards only, padded to the 100 an export has; the shapes are the
  // real exports' (the full owner-data check is recorded in the phase 4a PR).
  const archidekt = [
    "Commander", "1 Teferi, Temporal Archmage", "",
    "Mainboard", "1 Counterspell", "1 Sol Ring", "1 Witch Enchanter // Witch-Blessed Meadow", "96 Forest", "",
    "Sideboard", "1 Ball Lightning",
  ].join("\n");
  const moxfieldMtgo = [
    "1 Counterspell", "96 Forest", "1 Sol Ring", "1 Witch Enchanter/Witch-Blessed Meadow", "",
    "SIDEBOARD:", "1 Ball Lightning", "",
    "1 Teferi, Temporal Archmage",
  ].join("\n");
  const moxfieldPlain = moxfieldMtgo.replace("Witch Enchanter/Witch-Blessed Meadow", "Witch Enchanter / Witch-Blessed Meadow");
  const shape = (db: ReturnType<typeof testDb>, id: number) =>
    getDeck(db, OWNER, id).cards.map((c) => [c.name, c.quantity, c.role, c.maybe]).sort();

  for (const [label, text] of [["MTGO", moxfieldMtgo], ["plain text", moxfieldPlain]] as const)
    test(`Moxfield ${label} matches the Archidekt text: commander, cards, one-slash names and the sideboard`, () => {
      const db = testDb();
      const a = newDeck(db, "paper"), m = newDeck(db, "paper");
      const viaArchidekt = applyImport(db, OWNER, a, archidekt), viaMoxfield = applyImport(db, OWNER, m, text);
      assert.deepEqual(shape(db, m), shape(db, a));
      assert.deepEqual(viaMoxfield.applied, viaArchidekt.applied);
      assert.equal(viaMoxfield.diff.unresolved.length, 0);
      assert.ok(viaMoxfield.log_id, "one log entry, so one undo");
      assert.deepEqual(diffImport(db, OWNER, m, text).adds, [], "importing it again changes nothing");
    });

  test("a one-slash name is tried as written first, so it never shadows a real card", () => {
    const db = testDb();
    const id = newDeck(db, "paper");
    const diff = diffImport(db, OWNER, id, "1 Witch Enchanter / Witch-Blessed Meadow (MH3) 239 *F*");
    assert.deepEqual(diff.adds.map((c) => c.name), ["Witch Enchanter // Witch-Blessed Meadow"]);
    assert.deepEqual(diffImport(db, OWNER, id, "1 Forest / Island").unresolved.map((u) => u.name), ["Forest / Island"]);
  });
});
