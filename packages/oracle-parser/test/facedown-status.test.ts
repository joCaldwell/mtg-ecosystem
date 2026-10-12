// Status-changing and face-related actions: transformed/face-down entry,
// manifest/cloak, turning face up/down, detain, suspect, flip, day/night,
// exert, phasing, and regeneration riders.
// CR 701.35, 701.40, 701.43, 701.58, 701.60, 701.62, 702.26, 708, 712.14, 731:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, Effect, ObjectRef, Sentence } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

function only(text: string): Ability {
  const parsed = parseOracleText(text);
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  if (!parsed.ok) throw new Error(text);
  assert.equal(parsed.abilities.length, 1);
  // Drop explicit-undefined keys so expectations list only meaningful fields.
  return JSON.parse(JSON.stringify(parsed.abilities[0])) as Ability;
}
function fails(text: string): void {
  assert.equal(parseOracleText(text).ok, false, text);
}
/** Steps of a spell line's sentences, flattened. */
function steps(text: string): Effect[] {
  const ability = only(text);
  assert.equal(ability.kind, "spell", text);
  return (ability as Extract<Ability, { kind: "spell" }>).effects.flatMap(s => s.steps);
}

const fixed = (value: number) => ({ amount: "fixed" as const, value });
const it_: ObjectRef = { ref: "it" };
const yourLibrary = { zone: "library" as const, owner: "your" as const };
const topOfLibrary = (count: number) => ({ ref: "library-cards" as const, position: "top" as const, count: fixed(count), of: yourLibrary });
const you = { player: "you" as const };
const targetCreature = { ref: "target" as const, filter: { types: ["creature"] }, count: fixed(1) };

describe("battlefield entry: transformed and face down (CR 712.14a, 708.2-3)", () => {
  it("records transformed on a return to the battlefield, and the unchanged control", () => {
    assert.deepEqual(only("III — Exile this Saga, then return it to the battlefield transformed under your control."), {
      kind: "chapter", chapters: [3], effects: [{ steps: [
        { effect: "exile", what: { ref: "this", noun: "saga" } },
        { effect: "move-zone", what: it_, to: { zone: "battlefield" }, controller: "you", entry: { face: "transformed" } },
      ] }],
    });
    assert.deepEqual(steps("Return it to the battlefield under your control."), [
      { effect: "move-zone", what: it_, to: { zone: "battlefield" }, controller: "you" },
    ]);
  });

  it("combines transformed with tapped, owner control, counters, and attachment", () => {
    assert.deepEqual(steps("Return it to the battlefield tapped and transformed under its owner's control with three time counters on it."), [
      { effect: "move-zone", what: it_, to: { zone: "battlefield" }, tapped: true, controller: "owner",
        entry: { face: "transformed", counters: [{ count: fixed(3), counter: { type: "time" } }] } },
    ]);
    assert.deepEqual(steps("Return it to the battlefield transformed under your control attached to target opponent."), [
      { effect: "move-zone", what: it_, to: { zone: "battlefield" }, controller: "you",
        entry: { face: "transformed", attachedTo: { player: "target-opponent" } } },
    ]);
  });

  it("records converted separately from transformed (CR 701.28)", () => {
    assert.deepEqual(steps("Return it to the battlefield converted under its owner's control."), [
      { effect: "move-zone", what: it_, to: { zone: "battlefield" }, controller: "owner", entry: { face: "converted" } },
    ]);
  });

  it("records face down, including when put onto the battlefield", () => {
    assert.deepEqual(steps("Return it to the battlefield face down under your control."), [
      { effect: "move-zone", what: it_, to: { zone: "battlefield" }, controller: "you", entry: { faceDown: {} } },
    ]);
    assert.deepEqual(steps("Put it onto the battlefield face down under your control."), [
      { effect: "move-zone", what: it_, to: { zone: "battlefield" }, controller: "you", entry: { faceDown: {} } },
    ]);
  });

  it("completes the face-down characteristics from the next sentence (CR 708.2)", () => {
    assert.deepEqual(steps("Put all creature cards exiled with this enchantment onto the battlefield face down under your control. They're 2/2 Cyberman artifact creatures."), [
      { effect: "move-zone",
        what: { ref: "all", filter: { types: ["creature"], cls: "card", relation: { relation: "linked-exile", source: { ref: "this", noun: "enchantment" } } } },
        to: { zone: "battlefield" }, controller: "you",
        entry: { faceDown: { characteristics: { types: ["artifact", "creature"], subtypes: ["Cyberman"], power: fixed(2), toughness: fixed(2) } } } },
    ]);
    // Lands have no power or toughness to list.
    const trigger = only("Whenever another nontoken creature you control dies, you may return it to the battlefield face down under its owner's control. It's a Forest land.");
    assert.equal(trigger.kind, "triggered");
    assert.deepEqual((trigger as Extract<Ability, { kind: "triggered" }>).effects[0].steps, [
      { effect: "move-zone", what: it_, to: { zone: "battlefield" }, controller: "owner", optional: true,
        entry: { faceDown: { characteristics: { types: ["land"], subtypes: ["Forest"] } } } },
    ]);
  });

  it("rejects impossible or unsupported entry combinations", () => {
    fails("Return it to the battlefield transformed and face down under your control.");
    fails("Return it to the battlefield face up under your control.");
    fails("Return it to the battlefield transformed transformed under your control.");
    fails("Return it to the battlefield flipped.");
  });
});

describe("turning objects face up and face down (CR 708)", () => {
  it("turn-face keeps direction, object, and a following characteristic definition", () => {
    assert.deepEqual(steps("Turn target creature face down."), [{ effect: "turn-face", what: targetCreature, face: "down" }]);
    assert.deepEqual(steps("Turn target face-down creature an opponent controls face up."), [
      { effect: "turn-face", face: "up",
        what: { ref: "target", filter: { status: ["face-down"], types: ["creature"], control: "opponent" }, count: fixed(1) } },
    ]);
    assert.deepEqual(steps("Turn target creature face down. It's a 2/2 Cyberman artifact creature."), [
      { effect: "turn-face", what: targetCreature, face: "down",
        characteristics: { types: ["artifact", "creature"], subtypes: ["Cyberman"], power: fixed(2), toughness: fixed(2) } },
    ]);
  });

  it("a face-down definition must agree in number and follow a face-down instruction", () => {
    fails("Turn target creature face down. They're 2/2 Cyberman artifact creatures.");
    fails("Turn target creature face up. It's a 2/2 Cyberman artifact creature.");
    // A bare "It's a …" sentence is the separate resolving characteristic statement
    // (modify-characteristics, statedAs "is"); it never becomes a face-down definition.
    assert.equal(steps("It's a 2/2 Cyberman artifact creature.")[0].effect, "modify-characteristics");
    fails("Turn target creature face down. It's a 2/2 Cyberman artifact creature with flying.");
    // Face down is distinct from face up: different node field.
    assert.notDeepEqual(steps("Turn target creature face up."), steps("Turn target creature face down."));
  });

  it("selects creatures that have a morph ability by keyword, not by text", () => {
    assert.deepEqual(steps("Turn target creature with a morph ability face down."), [
      { effect: "turn-face", face: "down", what: { ref: "target", filter: { types: ["creature"], withAbility: ["morph"] }, count: fixed(1) } },
    ]);
  });
});

describe("exile face down (CR 406.3)", () => {
  it("marks the exile move as face down, and only when printed", () => {
    assert.deepEqual(steps("Exile the top card of your library face down."), [{ effect: "exile", what: topOfLibrary(1), faceDown: true }]);
    assert.deepEqual(steps("Exile the top card of your library."), [{ effect: "exile", what: topOfLibrary(1) }]);
    assert.deepEqual(steps("Exile a card from your hand face down."), [
      { effect: "exile", what: { ref: "filter", filter: { cls: "card", zone: { zone: "hand", owner: "your" } }, count: fixed(1) }, faceDown: true },
    ]);
  });

  it("keeps the exiling player and the replacement context", () => {
    assert.deepEqual(steps("Each player exiles all cards from their hand face down and draws seven cards."), [
      { effect: "exile", who: { player: "each-player" }, faceDown: true,
        what: { ref: "all", filter: { cls: "card", zone: { zone: "hand", owner: "their" } } } },
      { effect: "draw", who: { player: "each-player" }, amount: fixed(7) },
    ]);
    fails("Exile the top card of your library face up.");
  });
});

describe("manifest, manifest dread, and cloak (CR 701.40, 701.58, 701.62)", () => {
  it("manifests and cloaks cards from a stated library, hand, or established pool", () => {
    assert.deepEqual(steps("Manifest the top card of your library."), [{ effect: "manifest", who: you, what: topOfLibrary(1) }]);
    assert.deepEqual(steps("Cloak the top two cards of your library."), [{ effect: "cloak", who: you, what: topOfLibrary(2) }]);
    assert.deepEqual(steps("Manifest a card from your hand."), [
      { effect: "manifest", who: you,
        what: { ref: "filter", filter: { cls: "card", zone: { zone: "hand", owner: "your" } }, count: fixed(1) } },
    ]);
    assert.deepEqual(steps("Manifest your hand."), [{ effect: "manifest", who: you, what: { ref: "zone-cards", zone: "hand", owner: you } }]);
    assert.deepEqual(steps("Manifest those cards."), [
      { effect: "manifest", who: you, what: { ref: "that", filter: { cls: "card" }, plural: true } },
    ]);
  });

  it("retains the acting player and another player's library", () => {
    assert.deepEqual(steps("Target player manifests the top card of their library."), [
      { effect: "manifest", who: { player: "target-player" },
        what: { ref: "library-cards", position: "top", count: fixed(1), of: { zone: "library", owner: "their" } } },
    ]);
    assert.deepEqual(steps("Destroy target creature. Its controller manifests dread."), [
      { effect: "destroy", what: targetCreature },
      { effect: "manifest-dread", who: { player: "controller", of: it_ } },
    ]);
  });

  it("represents a variable number of cards from the top of the library", () => {
    assert.deepEqual(steps("Manifest a number of cards from the top of your library equal to the number of counters on it."), [
      { effect: "manifest", who: you,
        what: { ref: "library-cards", position: "top", count: { amount: "counter-count", counter: "any", on: it_ }, of: yourLibrary } },
    ]);
  });

  it("manifest dread is its own procedure, with an optional repetition count", () => {
    assert.deepEqual(steps("Manifest dread."), [{ effect: "manifest-dread", who: you }]);
    assert.deepEqual(steps("Manifest dread twice."), [{ effect: "manifest-dread", who: you, times: fixed(2) }]);
    assert.deepEqual(steps("Manifest dread X times."), [{ effect: "manifest-dread", who: you, times: { amount: "x" } }]);
    assert.notDeepEqual(steps("Manifest dread."), steps("Manifest the top card of your library."));
  });

  it("selects a subset of looked-at cards to cloak", () => {
    assert.deepEqual(steps("Look at the top five cards of your library, cloak two of them, and put the rest on the bottom of your library in a random order."), [
      { effect: "look-at", who: you, what: { top: fixed(5), of: yourLibrary } },
      { effect: "cloak", who: you, what: { ref: "from-pool", pool: { ref: "them" }, count: fixed(2) } },
      { effect: "move-zone", what: { ref: "rest" }, to: { zone: "library", owner: "your", position: "bottom" }, order: "random" },
    ]);
  });

  it("only cards can be manifested; permanents and misspellings fail", () => {
    fails("Manifest target creature.");
    fails("Manifest all creatures you control.");
    fails("Its controller manifests bread.");
    fails("Manifest dread thrice.");
    fails("Manifest the top card.");
  });
});

describe("detain, suspect, flip, and day/night", () => {
  it("detain keeps its targets and up-to count (CR 701.35)", () => {
    assert.deepEqual(steps("Detain up to two target creatures your opponents control."), [
      { effect: "detain", what: { ref: "target", filter: { types: ["creature"], control: "opponent" }, count: fixed(2), upTo: true } },
    ]);
    assert.deepEqual(steps("Detain target nonland permanent an opponent controls."), [
      { effect: "detain", what: { ref: "target", filter: { nonTypes: ["land"], cls: "permanent", control: "opponent" }, count: fixed(1) } },
    ]);
  });

  it("suspect and no-longer-suspected are opposite designations (CR 701.60)", () => {
    assert.deepEqual(steps("Suspect up to one other target creature you control."), [
      { effect: "suspect", what: { ref: "target", filter: { types: ["creature"], control: "you", other: true }, count: fixed(1), upTo: true } },
    ]);
    assert.deepEqual(steps("Suspect it. You may have it become no longer suspected."), [
      { effect: "suspect", what: it_ },
      { effect: "unsuspect", what: it_, optional: true },
    ]);
    assert.deepEqual(steps("All suspected creatures are no longer suspected."), [
      { effect: "unsuspect", what: { ref: "all", filter: { status: ["suspected"], types: ["creature"] } } },
    ]);
  });

  it("flip is a status change on a permanent, never a coin flip (CR 710.4)", () => {
    assert.deepEqual(only("Whenever this creature deals damage to an opponent, flip it."), {
      kind: "triggered",
      trigger: { trigger: "deals-damage", what: { ref: "this", noun: "creature" }, to: { player: "an-opponent" } },
      effects: [{ steps: [{ effect: "flip", what: it_ }] }],
    });
    // "Flip a coin." is the separate coin-flip action (CR 705), never this status change.
    assert.equal(steps("Flip a coin.")[0].effect, "flip-coin");
    fails("Flip target creature.");
    fails("Flip it onto the battlefield from a height of at least one foot.");
  });

  it("it becomes day or night (CR 731.1)", () => {
    assert.deepEqual(steps("It becomes night."), [{ effect: "set-day-night", to: "night" }]);
    assert.deepEqual(steps("It becomes day."), [{ effect: "set-day-night", to: "day" }]);
    fails("It becomes dusk.");
    fails("It becomes night and day.");
  });
});

describe("exert as an attack cost (CR 701.43d)", () => {
  it("links the optional exert cost to its reflexive 'when you do' ability", () => {
    const ability = only("You may exert this creature as it attacks. When you do, it gets +1/+1 and gains menace until end of turn.");
    assert.equal(ability.kind, "exert-on-attack");
    const exert = ability as Extract<Ability, { kind: "exert-on-attack" }>;
    assert.deepEqual(exert.what, { ref: "this", noun: "creature" });
    assert.deepEqual(exert.reflexive, [{ steps: [
      { effect: "pump", what: it_, power: { sign: 1, amount: fixed(1) }, toughness: { sign: 1, amount: fixed(1) }, duration: { duration: "end-of-turn" } },
      { effect: "gain-abilities", what: it_, abilities: [{ gained: "keyword", keyword: { keyword: "menace" } }], duration: { duration: "end-of-turn" } },
    ] }] as Sentence[]);
  });

  it("allows the exert cost alone, and keeps targets of the reflexive ability", () => {
    assert.deepEqual(only("You may exert this creature as it attacks."), { kind: "exert-on-attack", what: { ref: "this", noun: "creature" } });
    const ability = only("You may exert this creature as it attacks. When you do, target creature can't block this turn.") as Extract<Ability, { kind: "exert-on-attack" }>;
    assert.equal(ability.reflexive?.[0].steps[0].effect, "cant");
  });

  it("does not accept other exert wordings", () => {
    fails("You may exert target creature as it attacks.");
    fails("You may exert this creature as it blocks.");
    fails("You may exert this creature as it attacks. Draw a card.");
    fails("If this creature hasn't been exerted this turn, you may exert it as it attacks.");
  });
});

describe("as a permanent is turned face up (CR 708.11)", () => {
  it("is applied while turning face up, not a trigger", () => {
    assert.deepEqual(only("As this creature is turned face up, put four +1/+1 counters on it."), {
      kind: "static",
      effect: { static: "as-turned-face-up", what: { ref: "this", noun: "creature" },
        effects: [{ steps: [{ effect: "put-counters", on: it_, counter: { type: "+1/+1" }, count: fixed(4) }] }] },
    });
  });

  it("keeps 'enters or is turned face up' distinct from 'is turned face up'", () => {
    const both = only("As this creature enters or is turned face up, put two +1/+1 counters on it.");
    const faceUp = only("As this creature is turned face up, put two +1/+1 counters on it.");
    assert.equal((both as Extract<Ability, { kind: "static" }>).effect.static, "as-turned-face-up");
    assert.deepEqual((both as { effect: { alsoEnters?: true } }).effect.alsoEnters, true);
    assert.equal((faceUp as { effect: { alsoEnters?: true } }).effect.alsoEnters, undefined);
    fails("As this creature is turned face down, put two +1/+1 counters on it.");
  });
});

describe("phasing and transformation prohibitions", () => {
  it("can't phase in/out and can't transform are distinct prohibited actions", () => {
    assert.deepEqual(only("Permanents can't phase in."), {
      kind: "static", effect: { static: "cant", what: { ref: "each", filter: { cls: "permanent" } }, action: "phase-in" },
    });
    assert.deepEqual(only("Non-Human Werewolves you control can't transform."), {
      kind: "static", effect: { static: "cant", action: "transform",
        what: { ref: "each", filter: { nonSubtypes: ["Human"], subtypes: ["Werewolf"], control: "you" } } },
    });
    assert.deepEqual(steps("Until your next upkeep, target permanent can't phase out."), [
      { effect: "cant", action: "phase-out", duration: { duration: "your-next-upkeep" },
        what: { ref: "target", filter: { cls: "permanent" }, count: fixed(1) } },
    ]);
    fails("Permanents can't phase sideways.");
  });

  it("a coordinated subject phases out simultaneously", () => {
    assert.deepEqual(steps("This creature and that creature phase out."), [
      { effect: "simultaneous", steps: [
        { effect: "phase-out", what: { ref: "this", noun: "creature" } },
        { effect: "phase-out", what: { ref: "that", filter: { types: ["creature"] } } },
      ] },
    ]);
    assert.deepEqual(steps("This creature phases out."), [{ effect: "phase-out", what: { ref: "this", noun: "creature" } }]);
  });

  it("transformed is a status of the permanent", () => {
    const ability = only("This creature gets +1/+0 for each transformed permanent you control.");
    assert.equal(ability.kind, "static");
    assert.deepEqual((ability as { effect: { modifiers: { forEach: unknown }[] } }).effect.modifiers[0].forEach,
      { status: ["transformed"], cls: "permanent", control: "you" });
  });
});

describe("regeneration riders (CR 608.2c, 701.19c)", () => {
  const destroyAll = (extra: object) => [{ effect: "destroy", what: { ref: "all", filter: { types: ["creature"] } }, ...extra }];

  it("'destroyed this way' bans regeneration on the destruction itself", () => {
    assert.deepEqual(steps("Destroy target creature. A creature destroyed this way can't be regenerated."), [
      { effect: "destroy", what: targetCreature, regenerationForbidden: true },
    ]);
    assert.deepEqual(steps("Destroy all creatures. Creatures destroyed this way can't be regenerated."), destroyAll({ regenerationForbidden: true }));
  });

  it("the same-sentence 'and it can't be regenerated' is the same rider", () => {
    assert.deepEqual(steps("Destroy target creature and it can't be regenerated."), [
      { effect: "destroy", what: targetCreature, regenerationForbidden: true },
    ]);
  });

  it("a rider about a different object kind than the destruction is rejected", () => {
    fails("Destroy all lands. Creatures destroyed this way can't be regenerated.");
    fails("Destroy target artifact. A creature destroyed this way can't be regenerated.");
    fails("Destroy all lands or all creatures. Creatures destroyed this way can't be regenerated.");
  });
});
