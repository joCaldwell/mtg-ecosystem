import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, PlayerRef, Sentence } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Clauses are Oracle text from the frozen Scryfall corpus; card names are
// given beside each. Rules text (2026-09-25):
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const you: PlayerRef = { player: "you" };

function abilities(text: string, typeLine = "Instant"): Ability[] {
  const result = parseOracleText(text, undefined, { typeLine });
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities)) as Ability[];
}
/** Sentences of a one-line instant. */
function spell(text: string): Sentence[] {
  const [ability] = abilities(text);
  assert.equal(ability.kind, "spell");
  return (ability as Extract<Ability, { kind: "spell" }>).effects;
}
function rejected(text: string, typeLine = "Instant"): void {
  assert.equal(parseOracleText(text, undefined, { typeLine }).ok, false, text);
}

describe("hidden agenda and double agenda (CR 702.106)", () => {
  const chosenName = { choice: "chosen-value", category: "card-name", which: "chosen" };
  it("keeps the keyword beside the linked chosen-name ability", () => {
    // Brago's Favor.
    assert.deepEqual(abilities("Hidden agenda\nSpells with the chosen name you cast cost {1} less to cast.", "Conspiracy"), [
      { kind: "keywords", keywords: [{ keyword: "hidden agenda" }] },
      { kind: "static", effect: {
        static: "cost-modification",
        scope: { action: "cast", spells: { ref: "each", filter: { cls: "spell", chosenName } }, caster: you },
        change: { direction: "reduce", symbols: ["1"] },
      } },
    ]);
  });
  it("distinguishes the two-name variant", () => {
    assert.deepEqual(abilities("Double agenda", "Conspiracy"), [{ kind: "keywords", keywords: [{ keyword: "double agenda" }] }]);
  });
  it("rejects agenda wordings that are not the keyword", () => {
    rejected("Hidden agenda 2", "Conspiracy");
    rejected("Secret agenda", "Conspiracy");
  });
});

describe("destination-first battlefield moves (CR 608.2c)", () => {
  it("is the same zone move as the object-first wording", () => {
    // Thrilling Encore, Myojin of Grim Betrayal.
    const inverted = spell("Put onto the battlefield under your control all creature cards in all graveyards that were put there from the battlefield this turn.");
    assert.deepEqual(inverted, spell("Put all creature cards in all graveyards that were put there from the battlefield this turn onto the battlefield under your control."));
    const graveyards = { zone: "graveyard", owner: "all-players" };
    assert.deepEqual(inverted, [{ steps: [{
      effect: "move-zone",
      what: { ref: "all", filter: { types: ["creature"], cls: "card", zone: graveyards,
        zoneChangeHistory: { event: "put-into-zone", to: graveyards, from: { zone: "battlefield" }, during: "this-turn" } } },
      to: { zone: "battlefield" }, controller: "you",
    }] }]);
  });
  it("keeps the entry state stated before the objects and each target", () => {
    // Gerrard's Hourglass Pendant; Moira, Urborg Haunt.
    assert.deepEqual(spell("Return to the battlefield tapped all artifact and creature cards in your graveyard that were put there from the battlefield this turn."),
      spell("Return all artifact and creature cards in your graveyard that were put there from the battlefield this turn to the battlefield tapped."));
    const moved = spell("Return to the battlefield target creature card in your graveyard.");
    assert.deepEqual(moved, spell("Return target creature card in your graveyard to the battlefield."));
    assert.deepEqual(moved, [{ steps: [{
      effect: "move-zone",
      what: { ref: "target", filter: { types: ["creature"], cls: "card", zone: { zone: "graveyard", owner: "your" } }, count: { amount: "fixed", value: 1 } },
      to: { zone: "battlefield" },
    }] }]);
  });
  it("coordinates with a preceding instruction", () => {
    // Gerrard, Weatherlight Hero.
    const [dies] = abilities("When ~ dies, exile it and return to the battlefield all creature cards in your graveyard.", "Creature — Human");
    assert.deepEqual((dies as Extract<Ability, { kind: "triggered" }>).effects, [{ steps: [
      { effect: "exile", what: { ref: "it" } },
      { effect: "move-zone", what: { ref: "all", filter: { types: ["creature"], cls: "card", zone: { zone: "graveyard", owner: "your" } } }, to: { zone: "battlefield" } },
    ] }]);
  });
  it("rejects a destination-first move that does not end with its objects or names another zone", () => {
    rejected("Put onto the battlefield under your control all creature cards in all graveyards, then shuffle.");
    // The destination may precede the objects, as for the battlefield.
    assert.deepEqual(spell("Return to your hand all creature cards in your graveyard."),
      spell("Return all creature cards in your graveyard to your hand."));
    rejected("Put onto the battlefield attacking all creature cards in your graveyard.");
    rejected("Put onto the battlefield.");
  });
});

describe("put a counter or remove one (CR 608.2d)", () => {
  const one = { amount: "fixed", value: 1 };
  it("takes the elided counter kind from the first alternative", () => {
    // Jinxed Choker.
    const [choker] = abilities("{3}: Put a charge counter on this artifact or remove one from it.", "Artifact");
    const effects = (choker as Extract<Ability, { kind: "activated" }>).effects;
    assert.deepEqual(effects, [{ steps: [{ effect: "choice", who: you, options: [
      { effect: "put-counters", on: { ref: "this", noun: "artifact" }, counter: { type: "charge" }, count: one },
      { effect: "remove-counters", counter: { type: "charge" }, count: one, from: { ref: "it" } },
    ] }] }]);
    const [stated] = abilities("{3}: Put a charge counter on this artifact or remove a charge counter from it.", "Artifact");
    assert.deepEqual(effects, (stated as Extract<Ability, { kind: "activated" }>).effects);
  });
  it("keeps the single target of the first alternative", () => {
    // Sigurd, Jarl of Ravensthorpe.
    assert.deepEqual(spell("Put a lore counter on target Saga you control or remove one from it."), [{ steps: [{ effect: "choice", who: you, options: [
      { effect: "put-counters", on: { ref: "target", filter: { subtypes: ["Saga"], control: "you" }, count: one }, counter: { type: "lore" }, count: one },
      { effect: "remove-counters", counter: { type: "lore" }, count: one, from: { ref: "it" } },
    ] }] }]);
  });
  it("rejects an elided kind without exactly one counter named before it", () => {
    rejected("Put two charge counters on target artifact or remove one from it.");
    rejected("Tap target artifact or remove one from it.");
    rejected("Remove one from target artifact.");
  });
});

describe("a following sentence stating how the moved or created object enters (CR 508.4, 608.2c)", () => {
  const one = { amount: "fixed", value: 1 };
  const triggered = (text: string) => (abilities(text, "Creature — Human")[0] as Extract<Ability, { kind: "triggered" }>).effects;
  it("modifies the created token's own entry, like the inline wording", () => {
    // Thousand-Faced Shadow.
    const effects = triggered("Whenever ~ attacks, create a token that's a copy of another target attacking creature. The token enters tapped and attacking.");
    assert.deepEqual(effects, triggered("Whenever ~ attacks, create a tapped and attacking token that's a copy of another target attacking creature."));
    assert.deepEqual(effects, [{ steps: [{ effect: "create-token", count: one, token: {
      copyOf: { ref: "target", filter: { status: ["attacking"], types: ["creature"], other: true }, count: one }, tapped: true, attacking: true,
    } }] }]);
  });
  it("accepts the created token's name as the subject", () => {
    // Kari Zev, Skyship Raider.
    assert.deepEqual(triggered("Whenever ~ attacks, create \"Ragavan\", a legendary 2/1 red Monkey creature token. Ragavan enters tapped and attacking."),
      [{ steps: [{ effect: "create-token", count: one, token: {
        colors: ["red"], supertypes: ["legendary"], types: ["creature"], subtypes: ["Monkey"], keywords: [],
        power: { amount: "fixed", value: 2 }, toughness: { amount: "fixed", value: 1 }, name: "Ragavan", tapped: true, attacking: true,
      } }] }]);
  });
  it("modifies a put-onto-the-battlefield move, keeping its named defender", () => {
    // Shark Shredder, Killer Clone.
    const effects = triggered("Whenever ~ deals combat damage to a player, put up to one target creature card from that player's graveyard onto the battlefield under your control. It enters tapped and attacking that player.");
    assert.deepEqual(effects, triggered("Whenever ~ deals combat damage to a player, put up to one target creature card from that player's graveyard onto the battlefield under your control tapped and attacking that player."));
    assert.deepEqual(effects, [{ steps: [{ effect: "move-zone",
      what: { ref: "target", filter: { types: ["creature"], cls: "card", zone: { zone: "graveyard", owner: "that-player" } }, count: one, upTo: true },
      to: { zone: "battlefield" }, controller: "you", tapped: true, entry: { attacking: { defender: { player: "that-player" } } },
    }] }]);
  });
  it("rejects mismatched subjects, repeated state, other instructions and unrepresented token defenders", () => {
    // A created token has no named-defender field (as for the inline wording).
    rejected("Create a 1/1 white Soldier creature token. The token enters tapped and attacking that player.");
    rejected("Create two 1/1 white Soldier creature tokens. The token enters tapped.");
    rejected("Create a 1/1 white Soldier creature token. They enter tapped.");
    rejected("Create a tapped 1/1 white Soldier creature token. It enters tapped.");
    rejected("Create a 1/1 white Soldier creature token named Bob. Ragavan enters tapped.");
    rejected("Draw a card. It enters tapped.");
    rejected("Return target creature card from your graveyard to your hand. It enters tapped.");
    rejected("Create a 1/1 white Soldier creature token. It enters tapped and attacking and gains haste.");
  });
});

describe("flavor-word labels before modes (CR 207.2d)", () => {
  const one = { amount: "fixed", value: 1 };
  it("keeps labels with numbers, commas, exclamation marks and five words as labels only", () => {
    // Aerith Rescue Mission; Dathon and Picard at El-Adrel; Summon: Magus Sisters.
    const text = "Choose one —\n• Take 59 Flights of Stairs — Draw a card.\n• Temba, His Arms Wide — Scry 1.\n• Combine Powers! — You gain 3 life.";
    assert.deepEqual(spell(text), [{ steps: [{ effect: "modal", count: { exactly: 1 }, options: [
      { effects: [{ steps: [{ effect: "draw", who: you, amount: one }] }], label: "Take 59 Flights of Stairs" },
      { effects: [{ steps: [{ effect: "scry", amount: one }] }], label: "Temba, His Arms Wide" },
      { effects: [{ steps: [{ effect: "gain-life", who: you, amount: { amount: "fixed", value: 3 } }] }], label: "Combine Powers!" },
    ] }] }]);
  });
  it("still requires a complete body and refuses keywords or sentences as labels", () => {
    rejected("Choose one —\n• That Could Actually Be Dangerous — Draw a card. Ponder deeply.\n• Combine Powers! — Scry 1.");
    rejected("Choose one —\n• Kicker — Draw a card.\n• Combine Powers! — Scry 1.");
    rejected("Choose one —\n• Draw a card. Then — Scry 1.\n• Combine Powers! — Scry 1.");
    rejected("Choose one —\n• — Draw a card.\n• Combine Powers! — Scry 1.");
  });
});

describe("modal headers: another chooser, any number, up to X (CR 700.2a-e)", () => {
  const one = { amount: "fixed", value: 1 };
  const draw = { effects: [{ steps: [{ effect: "draw", who: you, amount: one }] }] };
  const gain = { effects: [{ steps: [{ effect: "gain-life", who: you, amount: one }] }] };
  const bullets = "\n• Draw a card.\n• You gain 1 life.";
  it("records an opponent who chooses the modes", () => {
    // Library of Lat-Nam, Misfortune, Fatal Lore.
    assert.deepEqual(spell(`An opponent chooses one —${bullets}`), [{ steps: [{
      effect: "modal", count: { exactly: 1 }, options: [draw, gain], chooser: { player: "an-opponent" },
    }] }]);
    assert.deepEqual(spell(`Target opponent chooses one —${bullets}`), [{ steps: [{
      effect: "modal", count: { exactly: 1 }, options: [draw, gain], chooser: { player: "target-opponent" },
    }] }]);
    assert.deepEqual(spell(`Choose one —${bullets}`), [{ steps: [{ effect: "modal", count: { exactly: 1 }, options: [draw, gain] }] }]);
  });
  it("distinguishes any number, exactly X and up to X", () => {
    // Rankle, Master of Pranks; Mindslaver Toolkit; The Ruinous Wrecking Crew.
    assert.deepEqual(spell(`Choose any number —${bullets}`)[0].steps[0], { effect: "modal", count: { atLeast: 0 }, options: [draw, gain] });
    assert.deepEqual(spell(`Choose X —${bullets}`)[0].steps[0], { effect: "modal", count: { variable: "x" }, options: [draw, gain] });
    assert.deepEqual(spell(`Choose up to X —${bullets}`)[0].steps[0], { effect: "modal", count: { variable: "x", upTo: true }, options: [draw, gain] });
  });
  it("upgrades to any number under a condition, with the dash after 'instead'", () => {
    // Catch-Up Mechanic's header.
    const [modal] = spell(`Choose one. If an opponent has at least 5 more life than you, choose any number instead—${bullets}`)[0].steps;
    assert.equal(modal.effect, "modal");
    const m = modal as Extract<typeof modal, { effect: "modal" }>;
    assert.deepEqual([m.count, m.conditionalCount?.count], [{ exactly: 1 }, { atLeast: 0 }]);
  });
  it("rejects other choosers and malformed counts", () => {
    rejected(`You choose one —${bullets}`);
    rejected(`Each opponent chooses one —${bullets}`);
    rejected(`An opponent choose one —${bullets}`);
    rejected(`Choose any —${bullets}`);
    rejected(`Choose up to that —${bullets}`);
    rejected(`Choose one. If you control a creature, you may choose any number instead—${bullets}`);
  });
});

describe("a spell that can't be copied (CR 113.6g)", () => {
  it("is a static prohibition distinct from can't be countered", () => {
    // Display of Power, See Double.
    assert.deepEqual(abilities("This spell can't be copied."), [{ kind: "static", effect: { static: "cant", what: { ref: "this", noun: "spell" }, action: "be-copied" } }]);
    assert.deepEqual(abilities("This spell can't be countered."), [{ kind: "static", effect: { static: "cant", what: { ref: "this", noun: "spell" }, action: "be-countered" } }]);
  });
  it("rejects incomplete wordings", () => {
    rejected("This spell can't be copied or.");
    rejected("This spell can't copied.");
  });
});
