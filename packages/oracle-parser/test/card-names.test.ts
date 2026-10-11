import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, Effect } from "../src/ast.ts";
import { buildCardNameCatalog, canonicalizeNamePunctuation } from "../src/card-names.ts";
import { lex } from "../src/lexer.ts";
import { normalizeOracleText } from "../src/normalize.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Names/layouts and cited Oracle excerpts checked against oracle-cards.json.
// CR 111.4, 201.2, 201.4b–f and 201.5: literal names, faces, and self-reference.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const catalog = buildCardNameCatalog([
  ...["Stangg", "Icingdeath, Frost Tyrant", "Krav, the Unredeemed", "Regna, the Redeemer", "Skoa, Embermage", "Sacrifice", "Llanowar Elves", "Ajani's Pridemate", "Sol Ring", "Bebop, Skull & Crossbones"].map(name => ({ name, layout: "normal" })),
  ...["Stangg Twin", "Icingdeath, Frost Tongue", "Carnivore", "Kobolds of Kher Keep"].map(name => ({ name, layout: "token" })),
]);
const n = (value: number) => ({ amount: "fixed" as const, value });
function ability(text: string, name?: string): Ability {
  const result = parseOracleText(text, name, { cardNames: catalog });
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return result.abilities[0];
}
function spell(text: string, expected: Effect[], name?: string) {
  assert.deepEqual(ability(text, name), { kind: "spell", abilityWord: undefined, effects: [{ steps: expected }] });
}

describe("literal source-name catalog", () => {
  it("uses printed planeswalker types to resolve non-comma shortened names without erasing subtype filters", () => {
    const result = parseOracleText('+1: Until your next turn, whenever a creature deals combat damage to Vraska, destroy that creature.', 'Vraska the Unseen', {
      typeLine: 'Legendary Planeswalker — Vraska',
    });
    assert.ok(result.ok, JSON.stringify(result.lines));
    assert.deepEqual(JSON.parse(JSON.stringify(result.abilities)), [{
      kind: 'loyalty', cost: { sign: 1, amount: 1 }, effects: [{ steps: [{
        effect: 'create-delayed-trigger', trigger: {
          trigger: 'deals-damage', what: { ref: 'filter', filter: { types: ['creature'] }, count: n(1) },
          combat: true, to: { ref: 'self' },
        }, occurrence: 'each', effects: [{ steps: [{ effect: 'destroy', what: { ref: 'that', filter: { types: ['creature'] } } }] }],
        targets: 'delayed-ability', duration: { duration: 'your-next-turn' },
      }] }],
    }]);
    assert.deepEqual(normalizeOracleText('Gideon becomes a creature. You control a Gideon planeswalker. Gideon spells cost {1} less to cast.', 'Gideon of the Trials', undefined, 'Legendary Planeswalker — Gideon'), [
      '~ becomes a creature. You control a Gideon planeswalker. Gideon spells cost {1} less to cast.',
    ]);
    assert.deepEqual(normalizeOracleText('Nissa gets a counter. Search your library for a card named Nissa\'s Chosen.', 'Nissa Revane', undefined, 'Legendary Planeswalker — Nissa'), [
      '~ gets a counter. Search your library for a card named Nissa\'s Chosen.',
    ]);
    assert.deepEqual(normalizeOracleText('Goblin creatures get +1/+1.', 'Goblin King', undefined, 'Creature — Goblin'), ['Goblin creatures get +1/+1.']);
    assert.deepEqual(normalizeOracleText('Vraska spells cost {1} less to cast.', 'Vraska, Golgari Queen', undefined, 'Legendary Planeswalker — Vraska'), ['Vraska spells cost {1} less to cast.']);
    assert.deepEqual(normalizeOracleText('Spells your opponents cast that target Charix cost {2} more to cast.', 'Charix, the Raging Isle'), ['Spells your opponents cast that target ~ cost {2} more to cast.']);
  });

  it("retains exact source identities and distinguishes face names from combined labels", () => {
    const names = buildCardNameCatalog([
      { name: "Fire // Ice", layout: "split", oracle_id: "split-oracle", card_faces: [{ name: "Fire" }, { name: "Ice" }] },
      { name: "Soldier", layout: "token", oracle_id: "token-oracle" },
      { name: "Fire", layout: "token", oracle_id: "token-fire" },
    ]);
    assert.deepEqual(names.exact("Fire")?.sources, [
      { sourceIndex: 0, sourceFaceIndex: 0, kind: "card", oracleId: "split-oracle" },
      { sourceIndex: 2, kind: "token", oracleId: "token-fire" },
    ]);
    assert.deepEqual(names.exact("Fire", ["card"])?.sources, [{ sourceIndex: 0, sourceFaceIndex: 0, kind: "card", oracleId: "split-oracle" }]);
    assert.equal(names.exact("Fire // Ice", ["card"]), undefined);
    assert.equal(names.exact("Fire // Ice")?.sources[0].kind, "combined-face-label");
    assert.equal(names.exact("Soldier", ["card"]), undefined);
    assert.equal(names.size, 4);
    assert.ok(Object.isFrozen(names.exact("Fire")?.sources));
    assert.equal(parseOracleText("Search your library for a card named Fire // Ice.", undefined, { cardNames: names }).ok, false);
    assert.equal(parseOracleText('Search your library for a card named "Fire // Ice".', undefined, { cardNames: names }).ok, false);
  });

  it("matches longest literal spans without fuzzy case or word-boundary guesses", () => {
    assert.deepEqual(catalog.matches("Stangg Twin, a legendary", 0).map(({ name, end }) => ({ name, end })), [
      { name: "Stangg Twin", end: 11 }, { name: "Stangg", end: 6 },
    ]);
    for (const text of ["stangg", "Stanggs", "Stangg_", "Stangg𐐀"]) assert.equal(catalog.matches(text, 0).length, 0, text);
    assert.equal(catalog.matches("𐐀Stangg", 2).length, 0);
    assert.equal(catalog.exact("Ajani’s Pridemate")?.name, "Ajani's Pridemate");
    assert.equal(canonicalizeNamePunctuation("‘A’–−“B”"), "‘A'—-\"B\"");
  });

  it("rejects ambiguous normalization collisions rather than choosing a spelling", () => {
    const names = buildCardNameCatalog([{ name: "A’s", layout: "normal" }, { name: "A's", layout: "normal" }]);
    assert.equal(names.exact("A’s"), undefined);
    assert.equal(parseOracleText("Partner with A’s", undefined, { cardNames: names }).ok, false);
  });

  it("protects literal names before full and shortened source self-reference rewriting", () => {
    assert.deepEqual(normalizeOracleText("When Stangg enters, create Stangg Twin, a legendary 3/4 red and green Human Warrior creature token.", "Stangg", catalog), [
      'When ~ enters, create "Stangg Twin", a legendary 3/4 red and green Human Warrior creature token.',
    ]);
    assert.deepEqual(normalizeOracleText("Icingdeath dies. Create Icingdeath, Frost Tongue, a legendary white Equipment artifact token.", "Icingdeath, Frost Tyrant", catalog), [
      '~ dies. Create "Icingdeath, Frost Tongue", a legendary white Equipment artifact token.',
    ]);
    // Synthetic punctuation fixture exercises a literal that would otherwise
    // be erased as reminder text, without treating arbitrary prose as a name.
    const names = buildCardNameCatalog([{ name: "Test (One)", layout: "token" }]);
    assert.deepEqual(normalizeOracleText("Create a creature token named Test (One). (Reminder.)", undefined, names), ['Create a creature token named "Test (One)".']);
  });

  it("does not mistake arbitrary catalog words for names outside explicit slots", () => {
    assert.deepEqual(normalizeOracleText("Sacrifice a creature. Create a creature token.", undefined, catalog), ["Sacrifice a creature. Create a creature token."]);
    assert.equal(parseOracleText("Partner with Unknown Partner", undefined, { cardNames: catalog }).ok, false);
    assert.equal(parseOracleText('Partner with "Unknown Partner"', undefined, { cardNames: catalog }).ok, false);
    assert.equal(parseOracleText("Partner with Stangg Twin", undefined, { cardNames: catalog }).ok, false);
    assert.equal(parseOracleText("Partner with Krav, the Unredeemed and unsupported prose", undefined, { cardNames: catalog }).ok, false);
    assert.equal(parseOracleText("Search your library for a card named Sol Ring or Llanowar Elves.", undefined, { cardNames: catalog }).ok, false);
  });

  it("stores the exact partner and literal named filter, not source identity", () => {
    assert.deepEqual(ability("Partner with Krav, the Unredeemed"), { kind: "keywords", keywords: [{ keyword: "partner with", pairedWith: "Krav, the Unredeemed" }] });
    assert.deepEqual(ability("Partner with Bebop, Skull & Crossbones"), { kind: "keywords", keywords: [{ keyword: "partner with", pairedWith: "Bebop, Skull & Crossbones" }] });
    spell("Search your library for a card named Llanowar Elves.", [{ effect: "search", who: { player: "you" }, zones: { selection: "all", of: [{ zone: "library", owner: "your" }] },
      for: { ref: "filter", filter: { cls: "card", name: { name: "literal", value: "Llanowar Elves" } }, count: n(1) } }]);
  });

  it("stops a comma-bearing name before the next cost, even when its verb is a card name", () => {
    assert.deepEqual(ability("Discard another card named Skoa, Embermage, Sacrifice two Mountains: Skoa deals 4 damage to any target.", "Skoa, Embermage"), {
      kind: "activated", abilityWord: undefined, restriction: undefined,
      costs: [{ cost: "discard", what: { ref: "filter", filter: { cls: "card", name: { name: "literal", value: "Skoa, Embermage" }, other: true }, count: n(1) } },
        { cost: "sacrifice", what: { ref: "filter", filter: { subtypes: ["Mountain"] }, count: n(2) } }],
      effects: [{ steps: [{ effect: "damage", source: { ref: "self" }, amount: n(4), to: { to: "any-target" } }] }],
    });
  });

  it("retains named token characteristics with a name prefix or suffix", () => {
    spell("Create Stangg Twin, a legendary 3/4 red and green Human Warrior creature token.", [{ effect: "create-token", count: n(1), token: {
      name: "Stangg Twin", power: n(3), toughness: n(4), colors: ["red", "green"], supertypes: ["legendary"], types: ["creature"], subtypes: ["Human", "Warrior"], keywords: [],
    } }], "Stangg");
    spell("Create a 3/1 red Beast creature token named Carnivore.", [{ effect: "create-token", count: n(1), token: {
      name: "Carnivore", power: n(3), toughness: n(1), colors: ["red"], supertypes: [], types: ["creature"], subtypes: ["Beast"], keywords: [],
    } }]);
    spell("Create a 0/1 red Kobold creature token named Kobolds of Kher Keep.", [{ effect: "create-token", count: n(1), token: {
      name: "Kobolds of Kher Keep", power: n(0), toughness: n(1), colors: ["red"], supertypes: [], types: ["creature"], subtypes: ["Kobold"], keywords: [],
    } }]);
  });

  it("keeps literal names outside quoted abilities and supports both ability/name clause orders", () => {
    const expected: Effect[] = [{ effect: "create-token", count: n(1), token: {
      name: "Llanowar Elves", power: n(1), toughness: n(1), colors: ["green"], supertypes: [], types: ["creature"], subtypes: ["Elf", "Druid"], keywords: [],
      abilities: [{ gained: "quoted", ability: { kind: "activated", abilityWord: undefined, restriction: undefined, costs: [{ cost: "tap-self" }],
        effects: [{ steps: [{ effect: "add-mana", who: { player: "you" }, mana: { mana: "fixed", symbols: ["G"] } }] }] } }],
    } }];
    spell('Create a 1/1 green Elf Druid creature token named Llanowar Elves with "{T}: Add {G}."', expected);
    spell('Create a 1/1 green Elf Druid creature token with "{T}: Add {G}." named Llanowar Elves.', expected);
  });

  it("rejects unknown names, conflicting names, and misplaced or duplicated characteristics", () => {
    for (const text of [
      "Create Unknown Token, a 1/1 creature token.", "Create a 1/1 creature token named Unknown Token.",
      "Create Stangg Twin, a 1/1 creature token named Stangg Twin.", "Create a creature token named Carnivore named Stangg Twin.",
      "Create a creature token named Carnivore with flying named Stangg Twin.", "Create a 1/1 2/2 creature token named Carnivore.",
      "Create a creature 1/1 token named Carnivore.",
    ]) assert.equal(parseOracleText(text, undefined, { cardNames: catalog }).ok, false, text);
  });

  it("retains stable token provenance for targets around literal names", () => {
    const line = normalizeOracleText("Exile target creature named Llanowar Elves. Tap target creature.", undefined, catalog)[0];
    const tokens = lex(line, "line:0", catalog);
    assert.deepEqual(tokens.find(token => token.kind === "card-name"), { kind: "card-name", value: "Llanowar Elves", provenance: { sourceId: "line:0", index: 4 } });
    const result = parseOracleText("Exile target creature named Llanowar Elves. Tap target creature.", undefined, { cardNames: catalog, includeSourceProvenance: true });
    assert.ok(result.ok);
    const parsed = result.abilities[0]; assert.ok(parsed.kind === "spell");
    assert.deepEqual(parsed.effects, [
      { steps: [{ effect: "exile", what: { ref: "target", selectionId: "line:0:1", filter: { types: ["creature"], name: { name: "literal", value: "Llanowar Elves" } }, count: n(1) } }] },
      { steps: [{ effect: "tap", what: { ref: "target", selectionId: "line:0:7", filter: { types: ["creature"] }, count: n(1) } }] },
    ]);
  });

  it("threads the catalog and original line provenance through modal options", () => {
    const result = parseOracleText("Choose one —\n• Exile target creature named Llanowar Elves.\n• Draw a card.", undefined,
      { cardNames: catalog, includeSourceProvenance: true });
    assert.ok(result.ok);
    assert.deepEqual(result.abilities, [{ kind: "spell", abilityWord: undefined, effects: [{ steps: [{ effect: "modal", count: { exactly: 1 }, options: [
      { effects: [{ steps: [{ effect: "exile", what: { ref: "target", selectionId: "line:1:2", filter: { types: ["creature"], name: { name: "literal", value: "Llanowar Elves" } }, count: n(1) } }] }] },
      { effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: n(1) }] }] },
    ] }] }] }]);
  });
});
