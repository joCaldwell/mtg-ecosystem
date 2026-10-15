import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, ParsedLine } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Card wordings from the cached Scryfall Oracle corpus. CR 707.12, 707.12a,
// 112.2a (copying a card and casting the copy), 707.10 (stack copies).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function line(text: string, typeLine = "Sorcery"): ParsedLine {
  const result = parseOracleText(text, "Test Card", { typeLine });
  assert.equal(result.lines.length, 1, text);
  return result.lines[0];
}
function ability(text: string, typeLine?: string): Ability {
  const parsed = line(text, typeLine);
  assert.ok(parsed.ok, `${text}: ${parsed.ok ? "" : parsed.error}`);
  return JSON.parse(JSON.stringify(parsed.ability)) as Ability;
}
function fails(text: string, typeLine?: string): void {
  assert.equal(line(text, typeLine).ok, false, text);
}
const you = { player: "you" } as const;
const one = { amount: "fixed", value: 1 } as const;
const instantOrSorcery = { allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "card" } as const;
const theCopy = { ref: "copy-result", number: "singular" } as const;
const theCopies = { ref: "copy-result", number: "plural" } as const;
const free = { cost: "without-mana-cost" } as const;

describe("copying a card and casting the copy (CR 707.12)", () => {
  it("copies the exiled card and offers a cast of that one copy", () => {
    // Demilich.
    assert.deepEqual(ability("Whenever this creature attacks, exile up to one target instant or sorcery card from your graveyard. Copy it. You may cast the copy.", "Creature — Skeleton Wizard"), {
      kind: "triggered", trigger: { trigger: "attacks", what: { ref: "this", noun: "creature" } },
      effects: [
        { steps: [{ effect: "exile", what: { ref: "target", filter: { ...instantOrSorcery, zone: { zone: "graveyard", owner: "your" } }, count: one, upTo: true } }] },
        { steps: [{ effect: "copy-card", who: you, what: { ref: "it" }, count: one }] },
        { steps: [{ effect: "play-card", optional: true, instruction: { who: you, action: "cast", what: theCopy } }] },
      ],
    });
  });

  it("keeps the free cast and a consequence of casting", () => {
    // Kaervek, the Punisher.
    assert.deepEqual(ability("Whenever you commit a crime, exile up to one target black card from your graveyard and copy it. You may cast the copy. If you do, you lose 2 life.", "Legendary Creature — Human Warlock"), {
      kind: "triggered", trigger: { trigger: "commits-crime", who: you },
      effects: [
        { steps: [
          { effect: "exile", what: { ref: "target", filter: { colors: ["black"], cls: "card", zone: { zone: "graveyard", owner: "your" } }, count: one, upTo: true } },
          { effect: "copy-card", who: you, what: { ref: "it" }, count: one },
        ] },
        { steps: [{ effect: "play-card", optional: true, instruction: { who: you, action: "cast", what: theCopy } }] },
        { condition: { condition: "you-do" }, conditionKind: "if", steps: [{ effect: "lose-life", who: you, amount: { amount: "fixed", value: 2 } }] },
      ],
    });
    // Shiko, Paragon of the Way: one sentence.
    assert.deepEqual(ability("Copy it, then you may cast the copy without paying its mana cost."), {
      kind: "spell", effects: [{ steps: [
        { effect: "copy-card", who: you, what: { ref: "it" }, count: one },
        { effect: "play-card", optional: true, instruction: { who: you, action: "cast", what: theCopy, alternativeCost: free } },
      ] }],
    });
  });

  it("keeps an optional copy separate from the optional cast", () => {
    // Isochron Scepter.
    assert.deepEqual(ability("{2}, {T}: You may copy the exiled card. If you do, you may cast the copy without paying its mana cost.", "Artifact"), {
      kind: "activated", costs: [{ cost: "mana", symbols: ["2"] }, { cost: "tap-self" }],
      effects: [
        { steps: [{ effect: "copy-card", who: you, optional: true, count: one,
          what: { ref: "related", number: "singular", filter: { cls: "card" }, relation: { relation: "event-result", event: "exile", scope: "contextual" } } }] },
        { condition: { condition: "you-do" }, conditionKind: "if",
          steps: [{ effect: "play-card", optional: true, instruction: { who: you, action: "cast", what: theCopy, alternativeCost: free } }] },
      ],
    });
  });

  it("makes several copies, each cast or not individually (707.12a)", () => {
    // Mnemonic Deluge.
    assert.deepEqual(ability("Copy that card three times. You may cast the copies without paying their mana costs."), {
      kind: "spell", effects: [
        { steps: [{ effect: "copy-card", who: you, what: { ref: "that", filter: { cls: "card" } }, count: { amount: "fixed", value: 3 } }] },
        { steps: [{ effect: "play-card", optional: true, instruction: { who: you, action: "cast", what: theCopies, alternativeCost: free } }] },
      ],
    });
    // Arcane Bombardment.
    assert.deepEqual(ability("Copy each card exiled with this enchantment. You may cast any number of the copies without paying their mana costs."), {
      kind: "spell", effects: [
        { steps: [{ effect: "copy-card", who: you, count: one,
          what: { ref: "each", filter: { cls: "card", relation: { relation: "linked-exile", source: { ref: "this", noun: "enchantment" } } } } }] },
        { steps: [{ effect: "play-card", optional: true, instruction: { who: you, action: "cast", alternativeCost: free,
          what: { ref: "from-pool", pool: theCopies, count: { amount: "chosen", minimum: 0 } } } }] },
      ],
    });
  });

  it("copies each card of a for-each separately", () => {
    // Mizzix's Mastery.
    assert.deepEqual(ability("For each card exiled this way, copy it, and you may cast the copy without paying its mana cost."), {
      kind: "spell", effects: [{ steps: [{ effect: "for-each", processing: "instruction-wise",
        each: { ref: "each", filter: { cls: "card", relation: { relation: "event-result", event: "exile", scope: "this-way" } } },
        effects: [{ steps: [
          { effect: "copy-card", who: you, what: { ref: "it" }, count: one },
          { effect: "play-card", optional: true, instruction: { who: you, action: "cast", what: theCopy, alternativeCost: free } },
        ] }] }] }],
    });
  });

  it("keeps the copy and its cast together in a delayed triggered ability", () => {
    // Surge to Victory.
    assert.deepEqual(ability("Whenever a creature you control deals combat damage to a player this turn, copy the exiled card. You may cast the copy without paying its mana cost."), {
      kind: "spell", effects: [{ steps: [{ effect: "create-delayed-trigger", occurrence: "each", duration: { duration: "this-turn" }, targets: "delayed-ability",
        trigger: { trigger: "deals-damage", combat: true, to: "any", what: { ref: "filter", filter: { types: ["creature"], control: "you" }, count: one } },
        effects: [
          { steps: [{ effect: "copy-card", who: you, count: one,
            what: { ref: "related", number: "singular", filter: { cls: "card" }, relation: { relation: "event-result", event: "exile", scope: "contextual" } } }] },
          { steps: [{ effect: "play-card", optional: true, instruction: { who: you, action: "cast", what: theCopy, alternativeCost: free } }] },
        ] }] }],
    });
    // An unrelated second sentence is not part of the delayed body.
    fails("Whenever a creature you control deals combat damage to a player this turn, copy the exiled card. Draw a card.");
  });

  it("rejects a copy reference whose number is not the number of copies made", () => {
    fails("Copy that card three times. You may cast the copy without paying its mana cost.");
    fails("Copy that card. You may cast the copies without paying their mana costs.");
    fails("Copy each card exiled with this enchantment. You may cast the copy.");
  });

  it("rejects casting a copy that no instruction made", () => {
    fails("You may cast the copy.");
    fails("Exile target instant card from your graveyard. You may cast the copy without paying its mana cost.");
    // A copy of a spell is put onto the stack, never cast (CR 707.10).
    fails("Copy target instant spell. You may cast the copy.");
  });

  it("does not take an untyped pronoun for a card unless the copy is cast", () => {
    fails("Exile target instant card from your graveyard. Copy it.");
    fails("Exile target instant card from your graveyard. Copy it. Draw a card.");
    // A land can't be cast, and a copy isn't played (CR 707.12).
    fails("Copy that card. You may play the copy.");
  });

  it("still copies a spell on the stack as a stack copy", () => {
    assert.deepEqual(ability("Copy target instant or sorcery spell. You may choose new targets for the copy."), {
      kind: "spell", effects: [{ steps: [{ effect: "copy-stack-object", who: you, count: one, retarget: { who: you, optional: true },
        what: { ref: "target", filter: { allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "spell" }, count: one } }] }],
    });
  });
});
