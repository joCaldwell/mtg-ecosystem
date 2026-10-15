import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Wordings are from the cached Scryfall
// corpus; rules are the Comprehensive Rules of 2026-09-25.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function parse(text: string, typeLine: string, name?: string) {
  return parseOracleText(text, name, { typeLine });
}
function only(text: string, typeLine: string, name?: string): Ability {
  const parsed = parse(text, typeLine, name);
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  if (!parsed.ok) throw new Error(text);
  assert.equal(parsed.abilities.length, 1);
  return JSON.parse(JSON.stringify(parsed.abilities[0]));
}
function rejects(text: string, typeLine: string, name?: string) {
  assert.equal(parse(text, typeLine, name).ok, false, text);
}

const fixed = (value: number) => ({ amount: "fixed" as const, value });
const target = (filter: object) => ({ ref: "target", filter, count: fixed(1) });

describe("unattach as an activation cost of a granted ability", () => {
  // Leonin Bola, Heartseeker. CR 701.3d, 201.5a.
  it("keeps the Equipment's name reference as the unattached permanent", () => {
    assert.deepEqual(only('Equipped creature has "{T}, Unattach Leonin Bola: Tap target creature."', "Artifact — Equipment", "Leonin Bola"), {
      kind: "static", effect: { static: "modify", what: { ref: "equipped", noun: "creature" }, modifiers: [{ modifier: "abilities", abilities: [{ gained: "quoted",
        ability: { kind: "activated", costs: [{ cost: "tap-self" }, { cost: "unattach", what: { ref: "self" } }],
          effects: [{ steps: [{ effect: "tap", what: target({ types: ["creature"] }) }] }] } }] }] } });
  });

  it("rejects unattaching anything but the ability's named or own permanent", () => {
    rejects('Equipped creature has "{T}, Unattach an Equipment you control: Tap target creature."', "Artifact — Equipment");
    rejects('Equipped creature has "{T}, Unattach target Equipment: Tap target creature."', "Artifact — Equipment");
  });
});

describe("bands with other", () => {
  // Cathedral of Serra. CR 702.22b-c: the quality is the keyword's parameter.
  it("keeps the quality of the quoted keyword", () => {
    assert.deepEqual(only('White legendary creatures you control have "bands with other legendary creatures."', "Land"), {
      kind: "static", effect: { static: "modify", what: { ref: "each", filter: { colors: ["white"], supertypes: ["legendary"], types: ["creature"], control: "you" } },
        modifiers: [{ modifier: "abilities", abilities: [{ gained: "quoted",
          ability: { kind: "keywords", keywords: [{ keyword: "bands with other", filter: { supertypes: ["legendary"], types: ["creature"] } }] } }] }] } });
  });

  it("is not banding and needs a quality", () => {
    rejects('White legendary creatures you control have "bands with other."', "Land");
    rejects('White legendary creatures you control have "bands with legendary creatures."', "Land");
  });
});

describe("no opponent/player in a comparative condition", () => {
  // Sword Coast Sailor: the negated existential, not "each opponent has less".
  const sailor = 'Commander creatures you own have "Whenever this creature attacks a player, if no opponent has more life than that player, this creature can\'t be blocked this turn."';
  it("negates the existential comparison", () => {
    assert.deepEqual(only(sailor, "Legendary Enchantment — Background"), {
      kind: "static", effect: { static: "modify", what: { ref: "each", filter: { commander: true, types: ["creature"], own: "you" } },
        modifiers: [{ modifier: "abilities", abilities: [{ gained: "quoted", ability: { kind: "triggered",
          trigger: { trigger: "attacks", what: { ref: "this", noun: "creature" }, whom: [{ player: "a-player" }] },
          condition: { condition: "not", of: { condition: "life-total", who: { player: "an-opponent" },
            comparison: { op: "gt", value: { amount: "life-total", of: { player: "that-player" } } } } },
          effects: [{ steps: [{ effect: "cant", what: { ref: "this", noun: "creature" }, action: "be-blocked", duration: { duration: "this-turn" } }] }] } }] }] } });
  });

  it("differs from the positive existential and keeps the measured quantity", () => {
    const positive = only("Whenever this creature attacks, if an opponent has more life than you, draw a card.", "Creature — Elf");
    const negative = only("Whenever this creature attacks, if no opponent has more life than you, draw a card.", "Creature — Elf");
    assert.equal(positive.kind, "triggered");
    assert.equal(negative.kind, "triggered");
    if (positive.kind !== "triggered" || negative.kind !== "triggered") return;
    assert.deepEqual(negative.condition, { condition: "not", of: positive.condition });
    assert.deepEqual(only("Whenever this creature attacks, if no player controls more lands than you, draw a card.", "Creature — Elf"), {
      kind: "triggered", trigger: { trigger: "attacks", what: { ref: "this", noun: "creature" } },
      condition: { condition: "not", of: { condition: "control-count", who: { player: "a-player" }, of: { types: ["land"] },
        comparison: { op: "gt", value: { amount: "count", of: { types: ["land"], control: "you" } } } } },
      effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: fixed(1) }] }] });
  });
});

describe("number of times a creature has mutated", () => {
  // Porcuparrot. CR 702.140c-d.
  it("is its own history quantity of the identified permanent", () => {
    assert.deepEqual(only("{T}: This creature deals X damage to any target, where X is the number of times this creature has mutated.", "Creature — Bird Beast"), {
      kind: "activated", costs: [{ cost: "tap-self" }], effects: [{
        steps: [{ effect: "damage", source: { ref: "this", noun: "creature" }, amount: { amount: "x" }, to: { to: "any-target" } }],
        bindings: [{ variable: "x", value: { amount: "mutation-count", of: { ref: "this", noun: "creature" } } }] }] });
  });

  it("does not count mutations of unidentified creatures", () => {
    rejects("{T}: This creature deals X damage to any target, where X is the number of times a creature has mutated.", "Creature — Bird Beast");
    rejects("{T}: This creature deals X damage to any target, where X is the number of times this creature mutated.", "Creature — Bird Beast");
  });
});

describe("can't have counters put on", () => {
  // Tatterkite, Blightbeetle. CR 113.6i.
  it("prohibits every kind of counter on the object itself", () => {
    assert.deepEqual(only("This creature can't have counters put on it.", "Artifact Creature — Scarecrow"), {
      kind: "static", effect: { static: "modify", what: { ref: "this", noun: "creature" }, modifiers: [{ modifier: "cant-have-counters", counter: "any" }] } });
  });

  it("keeps the printed counter kind and the affected group", () => {
    assert.deepEqual(only("Creatures your opponents control can't have +1/+1 counters put on them.", "Creature — Insect"), {
      kind: "static", effect: { static: "modify", what: { ref: "each", filter: { types: ["creature"], control: "opponent" } },
        modifiers: [{ modifier: "cant-have-counters", counter: { type: "+1/+1" } }] } });
  });

  it("rejects counters put on another object and stays distinct from can't have abilities", () => {
    rejects("This creature can't have counters put on target creature.", "Creature — Scarecrow");
    rejects("This creature can't have counters.", "Creature — Scarecrow");
    assert.deepEqual(only("This creature can't have flying.", "Creature — Scarecrow"), {
      kind: "static", effect: { static: "modify", what: { ref: "this", noun: "creature" }, modifiers: [{ modifier: "cant-have-abilities", abilities: [{ keyword: "flying" }] }] } });
  });
});

describe("landwalk of the chosen type", () => {
  // Traveler's Cloak. CR 702.14a, 607.2d.
  it("walks lands of the linked type choice", () => {
    assert.deepEqual(only("Enchanted creature has landwalk of the chosen type.", "Enchantment — Aura"), {
      kind: "static", effect: { static: "modify", what: { ref: "enchanted", noun: "creature" }, modifiers: [{ modifier: "abilities",
        abilities: [{ gained: "keyword", keyword: { keyword: "landwalk", filter: { types: ["land"], chosenType: { choice: "chosen-type" } } } }] }] } });
  });

  it("rejects a fixed walk with a chosen type and other chosen values", () => {
    rejects("Enchanted creature has islandwalk of the chosen type.", "Enchantment — Aura");
    rejects("Enchanted creature has landwalk of the chosen color.", "Enchantment — Aura");
  });
});

describe("a named token's own name inside its abilities", () => {
  // There and Back Again, Bonny Pall. CR 201.5: the name means this token;
  // the creating card's name (~) keeps its own meaning (201.5a).
  const smaug = (ability: object) => ({ kind: "chapter", chapters: [3], effects: [{ steps: [{ effect: "create-token", count: fixed(1), token: {
    colors: ["red"], supertypes: ["legendary"], types: ["creature"], subtypes: ["Dragon"], keywords: [{ keyword: "flying" }, { keyword: "haste" }],
    power: fixed(6), toughness: fixed(6), abilities: [{ gained: "quoted", ability }], name: "Smaug" } }] }] });
  it("reads the token's name as this token", () => {
    assert.deepEqual(only('III — Create "Smaug", a legendary 6/6 red Dragon creature token with flying, haste, and "When Smaug dies, create fourteen Treasure tokens."', "Enchantment — Saga"),
      smaug({ kind: "triggered", trigger: { trigger: "dies", what: { ref: "this", noun: "token" } },
        effects: [{ steps: [{ effect: "create-token", count: fixed(14), token: { predefined: "Treasure" } }] }] }));
    // Identical to the printed "this token".
    assert.deepEqual(only('III — Create "Smaug", a legendary 6/6 red Dragon creature token with flying, haste, and "When this token dies, create fourteen Treasure tokens."', "Enchantment — Saga"),
      only('III — Create "Smaug", a legendary 6/6 red Dragon creature token with flying, haste, and "When Smaug dies, create fourteen Treasure tokens."', "Enchantment — Saga"));
  });

  it("keeps a possessive name", () => {
    const parsed = only('When ~ enters, create "Beau", a legendary blue Ox creature token with "Beau\'s power and toughness are each equal to the number of lands you control."', "Legendary Creature — Human Druid", "Bonny Pall, Clearcutter");
    assert.equal(parsed.kind, "triggered");
    if (parsed.kind !== "triggered") return;
    assert.deepEqual(parsed.effects[0].steps[0], { effect: "create-token", count: fixed(1), token: { colors: ["blue"], supertypes: ["legendary"], types: ["creature"],
      subtypes: ["Ox"], keywords: [], abilities: [{ gained: "quoted", ability: { kind: "static", effect: { static: "cda-pt", what: { ref: "this", noun: "token" },
        stat: "both", value: { amount: "count", of: { types: ["land"], control: "you" } } } } }], name: "Beau" } });
  });

  it("does not rename another token or an unnamed token's text", () => {
    rejects('III — Create "Smaug", a legendary 6/6 red Dragon creature token with flying, haste, and "When Galactus dies, create fourteen Treasure tokens."', "Enchantment — Saga");
    rejects('III — Create a legendary 6/6 red Dragon creature token named "Smaug" with flying, haste, and "When Smaug dies, create fourteen Treasure tokens."', "Enchantment — Saga");
  });
});

describe("you may have the base power and toughness of an object become", () => {
  // Exuberant Wolfbear, Sita Varma. CR 613.4b, 611.2d; you decide (608.2d).
  it("is an optional base P/T change of the named object", () => {
    assert.deepEqual(only("Whenever this creature attacks, you may have the base power and toughness of target Human you control become equal to this creature's power and toughness until end of turn.", "Creature — Wolf Bear"), {
      kind: "triggered", trigger: { trigger: "attacks", what: { ref: "this", noun: "creature" } }, effects: [{ steps: [{ effect: "modify-characteristics",
        what: { ref: "target", filter: { subtypes: ["Human"], control: "you" }, count: fixed(1) },
        modifiers: [{ modifier: "base-pt", power: { amount: "attribute", of: { ref: "this", noun: "creature" }, attribute: "power" },
          toughness: { amount: "attribute", of: { ref: "this", noun: "creature" }, attribute: "toughness" }, valueScope: "each-affected-object" }],
        snapshot: "effect-created", duration: { duration: "end-of-turn" }, optional: true }] }] });
  });

  it("sets both to one stated value and matches the possessive form", () => {
    const ofForm = only("Whenever this creature attacks, the base power and toughness of each other creature you control become equal to this creature's power until end of turn.", "Creature — Wolf Bear");
    assert.deepEqual(only("Whenever this creature attacks, the base power and toughness of target creature become 1/1 until end of turn.", "Creature — Wolf Bear"),
      only("Whenever this creature attacks, target creature's base power and toughness become 1/1 until end of turn.", "Creature — Wolf Bear"));
    assert.equal(ofForm.kind, "triggered");
    if (ofForm.kind !== "triggered") return;
    const power = { amount: "attribute", of: { ref: "this", noun: "creature" }, attribute: "power" };
    assert.deepEqual(ofForm.effects[0].steps[0], { effect: "modify-characteristics", what: { ref: "each", filter: { other: true, types: ["creature"], control: "you" } },
      modifiers: [{ modifier: "base-pt", power, toughness: power, valueScope: "each-affected-object" }], snapshot: "effect-created", duration: { duration: "end-of-turn" } });
  });

  it("rejects a causative with an unrelated characteristic", () => {
    rejects("Whenever this creature attacks, you may have the base power of target creature become blue until end of turn.", "Creature — Wolf Bear");
  });
});

describe("loses all land types and abilities", () => {
  // Lithoform Blight. CR 205.3m, 613.1d/f: both removals, then the grant.
  it("removes every land type and every ability before the printed grants", () => {
    assert.deepEqual(only('Enchanted land loses all land types and abilities and has "{T}: Add {C}."', "Enchantment — Aura"), {
      kind: "static", effect: { static: "modify", what: { ref: "enchanted", noun: "land" }, modifiers: [
        { modifier: "types", change: { subtypes: [{ family: "land", operation: "remove", types: { all: true } }] } },
        { modifier: "lose-abilities", removal: { removal: "all" } },
        { modifier: "abilities", abilities: [{ gained: "quoted", ability: { kind: "activated", costs: [{ cost: "tap-self" }],
          effects: [{ steps: [{ effect: "add-mana", who: { player: "you" }, mana: { mana: "fixed", symbols: ["C"] } }] }] } }] }] } });
  });

  it("rejects a qualifier that could reach only one of the removals, and other type families", () => {
    rejects("Enchanted land loses all land types and abilities as long as it's tapped.", "Enchantment — Aura");
    rejects("Enchanted land loses all artifact types and abilities.", "Enchantment — Aura");
    rejects("Enchanted land gains all land types and abilities.", "Enchantment — Aura");
  });
});

describe("loses all unspent mana", () => {
  // Mana Short, Worldpurge. CR 106.4.
  it("empties the named player's mana pool", () => {
    assert.deepEqual(only("Tap all lands target player controls and that player loses all unspent mana.", "Instant"), {
      kind: "spell", effects: [{ steps: [
        { effect: "tap", what: { ref: "all", filter: { types: ["land"], controlledBy: { player: "target-player" } } } },
        { effect: "lose-unspent-mana", who: { player: "that-player" } }] }] });
  });

  it("is not gaining mana or losing part of it", () => {
    rejects("Each player gains all unspent mana.", "Sorcery");
    rejects("Each player loses all unspent green mana.", "Sorcery");
  });
});

describe("bare Players as the recipient of a player grant", () => {
  // Everybody Lives!. CR 702.11d: players may have hexproof.
  it("grants the keyword to every player", () => {
    assert.deepEqual(only("Players gain hexproof until end of turn.", "Instant"), {
      kind: "spell", effects: [{ steps: [{ effect: "player-gain-abilities", who: { player: "each-player" },
        abilities: [{ gained: "keyword", keyword: { keyword: "hexproof" } }], duration: { duration: "end-of-turn" } }] }] });
  });

  it("requires plural agreement", () => {
    rejects("Players gains hexproof until end of turn.", "Instant");
  });
});
