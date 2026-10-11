import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, Effect, ObjectRef, PlayerRef } from "../src/ast.ts";
import { lex } from "../src/lexer.ts";
import { parseOracleText } from "../src/index.ts";
import { Cursor } from "../src/parser/cursor.ts";
import { parseObjectActions } from "../src/parser/object-actions.ts";

// Source clauses: Scryfall Oracle corpus SHA256
// 5e97d5c4b94e8a080ec4615bbee6cc4528333ed921e47dd1d16990a28870ff60.
// CR 701.3, 701.27–28, 701.42, 702.26:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const you: PlayerRef = { player: "you" };
const one = { amount: "fixed", value: 1 } as const;
const itRef: ObjectRef = { ref: "it" };
const source: ObjectRef = { ref: "this", noun: "creature" };
const equipment: ObjectRef = { ref: "this", noun: "equipment" };
const targetCreature: ObjectRef = { ref: "target", filter: { types: ["creature"] }, count: one };

function actions(text: string, lastSubject?: ObjectRef, lastPlayer?: PlayerRef): Effect[] {
  const c = new Cursor(lex(text));
  const result = parseObjectActions(c, lastSubject, lastPlayer);
  assert.ok(result, `${text}: ${c.errorMessage()}`);
  assert.equal(c.done(), true, text);
  return result;
}
function full(text: string, name?: string): Ability[] {
  const result = parseOracleText(text, name);
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities)) as Ability[];
}

describe("rules-defined object actions", () => {
  it("keeps transform and convert distinct from zone changes or face-up actions", () => {
    // Ulvenwald Captive 0272ca81-e727-4f4b-b06e-072d70bb5558.
    assert.deepEqual(actions("Transform this creature"), [{ effect: "transform", what: source, who: you }]);
    // Jetfire, Air Guardian 38de7183-81ef-4a79-b231-7195ba89a281.
    assert.deepEqual(actions("Convert ~"), [{ effect: "convert", what: { ref: "self" }, who: you }]);
    assert.deepEqual(actions("transform it"), [{ effect: "transform", what: itRef, who: you }]);
    assert.deepEqual(actions("This creature transforms"), [{ effect: "transform", what: source }]);
    for (const text of ["Transform", "Convert", "Transform it into a Dragon", "Turn this creature face up", "Turn this creature face down"]) {
      const c = new Cursor(lex(text));
      assert.equal(parseObjectActions(c), null, text);
      assert.equal(c.pos, 0, text);
    }
  });

  it("preserves attachment object and endpoint without changing either controller", () => {
    // Maul of the Skyclaves 088a1f56-198c-41a1-b244-ec1c7a771041.
    assert.deepEqual(actions("Attach it to target creature you control"), [{
      effect: "attach", who: you, what: itRef,
      to: { ref: "target", filter: { types: ["creature"], control: "you" }, count: one },
    }]);
    // Curse of Leeches uses the nontargeted player endpoint.
    assert.deepEqual(actions("attach it to a player"), [{ effect: "attach", who: you, what: itRef, to: { player: "a-player" } }]);
    assert.deepEqual(actions("attach this Aura to that player"), [{
      effect: "attach", who: you, what: { ref: "this", noun: "aura" }, to: { player: "that-player" },
    }]);
    assert.deepEqual(actions("Attach target Aura to target player"), [{ effect: "attach", who: you,
      what: { ref: "target", filter: { subtypes: ["Aura"] }, count: one }, to: { player: "target-player" },
    }]);
  });

  it("keeps unattaching from an endpoint distinct from selecting objects from a pool", () => {
    // Disarm 44d54814-2eeb-4df8-b8d3-789f0bcb307a.
    assert.deepEqual(actions("Unattach all Equipment from target creature"), [{
      effect: "unattach", who: you, what: { ref: "all", filter: { subtypes: ["Equipment"] } }, from: targetCreature,
    }]);
    assert.deepEqual(actions("Unattach an Equipment from it"), [{
      effect: "unattach", who: you, what: { ref: "filter", filter: { subtypes: ["Equipment"] }, count: one }, from: itRef,
    }]);
    assert.deepEqual(actions("Unattach all Equipment from them"), [{
      effect: "unattach", who: you, what: { ref: "all", filter: { subtypes: ["Equipment"] } }, from: { ref: "them" },
    }]);
    // Carry Away 913b57de-f921-40ea-9124-0da35a269181.
    assert.deepEqual(actions("unattach enchanted Equipment"), [{
      effect: "unattach", who: you, what: { ref: "enchanted", noun: "equipment" },
    }]);
  });

  it("represents phasing as status change and preserves selected quantity", () => {
    // Blink Dog 02a7d95a-0a45-4aa3-b5b8-01c0b36462d8.
    assert.deepEqual(actions("This creature phases out"), [{ effect: "phase-out", what: source }]);
    // Clever Concealment 42bb7ea9-f6e4-4551-8d93-3b1eae84b865.
    assert.deepEqual(actions("Any number of target nonland permanents you control phase out"), [{
      effect: "phase-out", what: { ref: "target", count: { amount: "chosen", minimum: 0 }, filter: { nonTypes: ["land"], cls: "permanent", control: "you" } },
    }]);
    // The Pandorica's eventual phase-in instruction is a different status.
    assert.deepEqual(actions("that permanent phases in"), [{ effect: "phase-in", what: { ref: "that", filter: { cls: "permanent" } } }]);
    assert.deepEqual(actions("equipped creature phases out"), [{ effect: "phase-out", what: { ref: "equipped", noun: "creature" } }]);
  });

  it("retains stated actors and permits elision only with established context", () => {
    const actor: PlayerRef = { player: "target-player" };
    assert.deepEqual(actions("Target player attaches this Equipment to it"), [{ effect: "attach", who: actor, what: equipment, to: itRef }]);
    assert.deepEqual(actions("attaches this Equipment to it", undefined, actor), [{ effect: "attach", who: actor, what: equipment, to: itRef }]);
    assert.deepEqual(actions("transform it", undefined, actor), [{ effect: "transform", who: you, what: itRef }]);
    assert.deepEqual(actions("transform it", undefined, { player: "they" }), [{ effect: "transform", who: { player: "they" }, what: itRef }]);
    assert.deepEqual(actions("phases out", targetCreature), [{ effect: "phase-out", what: targetCreature }]);
    for (const text of ["attaches this Equipment to it", "phases out", "transforms"]) {
      const c = new Cursor(lex(text));
      assert.equal(parseObjectActions(c), null, text);
      assert.equal(c.pos, 0, text);
    }
  });

  it("rolls back missing endpoints and refuses unrepresented mechanics", () => {
    for (const text of ["Attach it", "Attach it to", "Unattach it from", "This creature phases", "This creature phases sideways", "Target player phases out", "Detach it", "Meld them into Chittering Host"]) {
      const c = new Cursor(lex(text));
      assert.equal(parseObjectActions(c), null, text);
      assert.equal(c.pos, 0, text);
    }
    // Unsupported qualification must not vanish in a complete ability.
    for (const text of [
      "Attach it to another permanent it can enchant.",
      "Attach it to target permanent or player.",
      "This creature phases out and can't phase in during its controller's next untap step.",
      "Permanents can't phase in.",
    ]) assert.equal(parseOracleText(text).ok, false, text);
  });

  it("retains an attachment's old endpoint as the selection filter, and the new destination", () => {
    assert.deepEqual(full("Attach all Equipment attached to that creature to this creature."), [{
      kind: "spell", effects: [{ steps: [{ effect: "attach", who: you,
        what: { ref: "all", filter: { subtypes: ["Equipment"], attachedTo: { ref: "that", filter: { types: ["creature"] } } } },
        to: source,
      }] }],
    }]);
  });
});

describe("object actions in complete abilities", () => {
  it("retains activation costs and restrictions around transformation", () => {
    assert.deepEqual(full("{5}{G}{G}: Transform this creature."), [{ kind: "activated",
      costs: [{ cost: "mana", symbols: ["5", "G", "G"] }], effects: [{ steps: [{ effect: "transform", what: source, who: you }] }],
    }]);
    assert.deepEqual(full("{4}{B/P}: Transform this creature. Activate only as a sorcery."), [{ kind: "activated",
      costs: [{ cost: "mana", symbols: ["4", "B/P"] }], restriction: { restriction: "sorcery" }, effects: [{ steps: [{ effect: "transform", what: source, who: you }] }],
    }]);
  });

  it("keeps unattachment then transformation as separate ordered actions", () => {
    // Elbrus, the Binding Blade da5881ca-b86e-4582-ae0e-1570c59773ae.
    assert.deepEqual(full("When equipped creature deals combat damage to a player, unattach ~, then transform it."), [{
      kind: "triggered", trigger: { trigger: "deals-damage", what: { ref: "equipped", noun: "creature" }, combat: true, to: "any" },
      effects: [{ steps: [{ effect: "unattach", what: { ref: "self" }, who: you }, { effect: "transform", what: itRef, who: you }] }],
    }]);
    assert.deepEqual(full("{U}{U}{U}: Convert ~, then adapt 3."), [{ kind: "activated",
      costs: [{ cost: "mana", symbols: ["U", "U", "U"] }], effects: [{ steps: [
        { effect: "convert", what: { ref: "self" }, who: you }, { effect: "adapt", amount: { amount: "fixed", value: 3 } },
      ] }],
    }]);
  });

  it("preserves one optional choice across movement and attachment", () => {
    // Deathrender 05ac6e5c-b773-45ed-9c74-3cd4b05bc287: declining the
    // creature move also declines its coordinated attachment instruction.
    assert.deepEqual(full("Whenever equipped creature dies, you may put a creature card from your hand onto the battlefield and attach this Equipment to it."), [{
      kind: "triggered", trigger: { trigger: "dies", what: { ref: "equipped", noun: "creature" } },
      effects: [{ steps: [{ effect: "optional-group", who: you, steps: [
        { effect: "move-zone", what: { ref: "filter", filter: { types: ["creature"], cls: "card", zone: { zone: "hand", owner: "your" } }, count: one }, to: { zone: "battlefield" } },
        { effect: "attach", what: equipment, to: itRef, who: you },
      ] }] }],
    }]);
  });

  it("retains conjugated actor continuation while resetting context between sentences", () => {
    const who: PlayerRef = { player: "target-player" };
    assert.deepEqual(full("Target player attaches this Equipment to it and transforms that creature."), [{
      kind: "spell", effects: [{ steps: [
        { effect: "attach", what: equipment, to: itRef, who },
        { effect: "transform", what: { ref: "that", filter: { types: ["creature"] } }, who },
      ] }],
    }]);
    for (const text of [
      "Target player attaches this Equipment to it. Transforms that creature.",
      "Target creature gets +1/+1 until end of turn. Phases out.",
    ]) assert.equal(parseOracleText(text).ok, false, text);
  });

  it("keeps attach targets and restrictions within the activated ability", () => {
    // Bloodthirsty Blade b2747459-05ec-42a6-81be-e97e182bc778.
    assert.deepEqual(full("{1}: Attach this Equipment to target creature an opponent controls. Activate only as a sorcery."), [{
      kind: "activated", costs: [{ cost: "mana", symbols: ["1"] }], restriction: { restriction: "sorcery" },
      effects: [{ steps: [{ effect: "attach", who: you, what: equipment,
        to: { ref: "target", filter: { types: ["creature"], control: "opponent" }, count: one } }] }],
    }]);
    assert.deepEqual(full("Unattach all Equipment from target creature."), [{
      kind: "spell", effects: [{ steps: [{ effect: "unattach", who: you, what: { ref: "all", filter: { subtypes: ["Equipment"] } }, from: targetCreature }] }],
    }]);
  });

  it("does not turn one phasing action into recurring phasing or a zone change", () => {
    assert.deepEqual(full("Teleport — {3}{W}: This creature phases out."), [{ kind: "activated", abilityWord: "Teleport",
      costs: [{ cost: "mana", symbols: ["3", "W"] }], effects: [{ steps: [{ effect: "phase-out", what: source }] }],
    }]);
    assert.deepEqual(full("Target creature phases out."), [{ kind: "spell", effects: [{ steps: [{ effect: "phase-out", what: targetCreature }] }] }]);
    assert.deepEqual(full("Phasing"), [{ kind: "keywords", keywords: [{ keyword: "phasing" }] }]);
  });

  it("keeps unsupported meld result names out of transformation nodes", () => {
    for (const [name, text] of [
      ["Mishra, Claimed by Gix", "Exile them, then meld them into Mishra, Lost to Phyrexia. It enters tapped and attacking."],
      ["Titania, Voice of Gaea", "Exile them, then meld them into Titania, Gaea Incarnate."],
      ["Urza, Lord Protector", "Exile them, then meld them into Urza, Planeswalker."],
    ]) assert.equal(parseOracleText(text, name).ok, false, name);
  });
});
