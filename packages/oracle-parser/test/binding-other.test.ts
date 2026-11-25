import assert from "node:assert/strict";
import { isDeepStrictEqual } from "node:util";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";
import { resolveCard } from "../src/semantic/resolution.ts";

function analyze(text: string, typeLine: string, name = "Fixture") {
  const result = parseOracleText(text, name, { typeLine, includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result.lines));
  const input = { identity: "fixture", source: { name, layout: "normal", type_line: typeLine }, faces: [{ sourceFaceIndex: 0, result }] };
  const graph = analyzeCardBindings(input);
  return { graph, resolution: resolveCard(input, graph) };
}

function assertUnresolved(text: string, typeLine: string, path: string) {
  const { graph } = analyze(text, typeLine);
  assert.ok(graph.diagnostics.some(diagnostic => diagnostic.path === path &&
    diagnostic.message === "The excluded antecedent of other/another is unresolved."), JSON.stringify(graph.diagnostics));
  assert.ok(!graph.references.some(reference => reference.path === path && reference.binding === "bound"));
}

describe("other/another exclusions", () => {
  it("excludes Chandra's Ignition's announced damage source from its recipients", () => {
    const { resolution } = analyze("Target creature you control deals damage equal to its power to each other creature and each opponent.",
      "Sorcery", "Chandra's Ignition");
    assert.deepEqual(resolution.faces[0].blocks?.[0].refs.find(ref => ref.path.endsWith("/other")), {
      path: "/effects/0/steps/0/to/refs/0/filter/other", ref: { ref: "target", slot: 0 },
    });
    assert.equal(resolution.complete, true);
  });

  it("excludes Urborg Repossession's first target from its kicked target", () => {
    const { resolution } = analyze(
      "Kicker {1}{G} (You may pay an additional {1}{G} as you cast this spell.)\nReturn target creature card from your graveyard to your hand. You gain 2 life. If this spell was kicked, return another target permanent card from your graveyard to your hand.",
      "Sorcery", "Urborg Repossession");
    const refs = resolution.faces[0].blocks?.[1].refs;
    assert.deepEqual(refs?.find(ref => ref.path.endsWith("/other")), {
      path: "/effects/2/steps/0/what/filter/other", ref: { ref: "target", slot: 0 },
    });
    assert.deepEqual(refs?.find(ref => ref.path === "/effects/2/steps/0/what"), {
      path: "/effects/2/steps/0/what", ref: { ref: "target", slot: 1 },
    });
    assert.equal(resolution.complete, true);
  });

  it("binds Junk Diver's graveyard successor but exports it as a residual", () => {
    const { graph, resolution } = analyze("Flying\nWhen this creature dies, return another target artifact card from your graveyard to your hand.",
      "Artifact Creature — Bird", "Junk Diver");
    const exclusion = graph.references.find(ref => ref.path.endsWith("/other"));
    assert.ok(exclusion?.binding === "bound");
    const node = graph.nodes.find(node => node.id === exclusion.target);
    assert.ok(node?.node === "event-result" && node.identity === "zone-successors" && node.cardinality === "one");
    assert.equal(node.id, "card:fixture/faces/0/abilities/1/trigger#successor");
    // The export has no reference for the successor distinct from the dying
    // object (Goblin Arsonist's "it"), so the exclusion stays a residual.
    assert.deepEqual(resolution.faces[0].blocks?.[1].refs.find(ref => ref.path.endsWith("/other")), {
      path: "/effects/0/steps/0/what/filter/other", unresolved: "event-result:successor-exclusion",
    });
    assert.ok(resolution.faces[0].blocks?.[1].residuals.includes("ref:event-result:successor-exclusion"));
    assert.equal(resolution.complete, false);
    const { resolution: arsonist } = analyze("When this creature dies, you may have it deal 1 damage to any target.",
      "Creature — Goblin Shaman", "Goblin Arsonist");
    assert.ok(arsonist.faces[0].blocks?.[0].refs.some(ref => "ref" in ref && isDeepStrictEqual(ref.ref, { ref: "event-object", role: "dies" })));
  });

  it("excludes Shadow Stinger from its tap payment", () => {
    const { resolution } = analyze(
      "Tap another untapped Rogue you control: This creature gains deathtouch until end of turn.\nWhenever this creature deals combat damage to a player, that player mills three cards. (They put the top three cards of their library into their graveyard.)",
      "Creature — Vampire Rogue", "Shadow Stinger");
    assert.deepEqual(resolution.faces[0].blocks?.[0].refs.find(ref => ref.path.endsWith("/other")), {
      path: "/costs/0/what/filter/other", ref: { ref: "source" },
    });
    assert.equal(resolution.complete, true);
  });

  it("excludes the first target of a fight, counter move or separate instruction", () => {
    const cases = [
      ["Clash of Titans", "Target creature fights another target creature. (Each deals damage equal to its power to the other.)", "/effects/0/steps/0/b"],
      ["Fate Transfer", "Move all counters from target creature onto another target creature.", "/effects/0/steps/0/to"],
      ["Leeching Bite", "Target creature gets +1/+1 until end of turn. Another target creature gets -1/-1 until end of turn.", "/effects/1/steps/0/what"],
    ];
    for (const [name, text, operand] of cases) {
      const { resolution } = analyze(text, "Instant", name);
      assert.deepEqual(resolution.faces[0].blocks?.[0].refs.find(ref => ref.path.endsWith("/other")), {
        path: `${operand}/filter/other`, ref: { ref: "target", slot: 0 },
      });
      assert.equal(resolution.complete, true, name);
    }
  });

  it("keeps Fate Transfer's exclusion unresolved for an untargeted or grouped counter source", () => {
    for (const from of ["a creature you control", "two target creatures"])
      assertUnresolved(`Move all counters from ${from} onto another target creature.`, "Instant",
        "/faces/0/abilities/0/effects/0/steps/0/to/filter/other");
  });

  it("excludes Xavier Sal from its single-counter payment, but not after an earlier object payment", () => {
    const { resolution } = analyze(
      "{T}, Remove a counter from another permanent you control: Populate. Activate only as a sorcery. (Create a token that's a copy of a creature token you control.)\n{T}, Sacrifice another creature: Proliferate. Activate only as a sorcery. (Choose any number of permanents and/or players, then give each another counter of each kind already there.)",
      "Legendary Creature — Human Fungus Pirate", "Xavier Sal, Infested Captain");
    assert.deepEqual(resolution.faces[0].blocks?.[0].refs.find(ref => ref.path.endsWith("/other")), {
      path: "/costs/1/from/filter/other", ref: { ref: "source" },
    });
    assert.equal(resolution.complete, true);
    assertUnresolved("{T}, Sacrifice a creature, Remove a counter from another permanent you control: Populate. Activate only as a sorcery.",
      "Legendary Creature — Human Fungus Pirate", "/faces/0/abilities/0/costs/2/from/filter/other");
  });

  it("excludes the source from graveyard exile and distributed counter payments", () => {
    const { resolution: boneDragon } = analyze(
      "Flying\n{3}{B}{B}, Exile seven other cards from your graveyard: Return this card from your graveyard to the battlefield tapped.",
      "Creature — Dragon Skeleton", "Bone Dragon");
    assert.deepEqual(boneDragon.faces[0].blocks?.[1].refs.find(ref => ref.path.endsWith("/other")), {
      path: "/costs/1/what/filter/other", ref: { ref: "source" },
    });
    const { resolution: tekuthal } = analyze(
      "Flying\nIf you would proliferate, proliferate twice instead.\n{1}{U/P}{U/P}, Remove three counters from among other artifacts, creatures, and planeswalkers you control: Put an indestructible counter on Tekuthal. ({U/P} can be paid with either {U} or 2 life.)",
      "Legendary Creature — Phyrexian Horror", "Tekuthal, Inquiry Dominus");
    assert.deepEqual(tekuthal.faces[0].blocks?.[2].refs.find(ref => ref.path.endsWith("/other")), {
      path: "/costs/1/from/filter/other", ref: { ref: "source" },
    });
  });

  it("keeps ambiguous, grouped, untargeted and cross-block spell operands unresolved", () => {
    const cases = [
      ["Destroy target creature. Destroy target creature. Destroy another target creature.", "0/effects/2/steps/0"],
      ["Destroy two target creatures. Destroy another target creature.", "0/effects/1/steps/0"],
      ["Choose a creature, then destroy another target creature.", "0/effects/0/steps/1"],
      ["Choose one —\n• Destroy target creature.\n• Destroy another target creature.", "0/effects/0/steps/0/options/1/effects/0/steps/0"],
      ["Destroy target creature.\nDestroy another target creature.", "1/effects/0/steps/0"],
    ];
    for (const [text, step] of cases) assertUnresolved(text, "Sorcery", `/faces/0/abilities/${step}/what/filter/other`);
  });

  it("keeps a graveyard exclusion unresolved for a different or alternative dying object", () => {
    for (const subject of ["a creature you control", "this creature or another creature you control"])
      assertUnresolved(`When ${subject} dies, return another target artifact card from your graveyard to your hand.`, "Creature",
        "/faces/0/abilities/0/effects/0/steps/0/what/filter/other");
  });

  it("keeps a quoted exclusion, a cost with two object payments and a static with two objects unresolved", () => {
    assertUnresolved('Creatures you control have "Tap another untapped creature you control: Draw a card."', "Enchantment",
      "/faces/0/abilities/0/effect/modifiers/0/abilities/0/ability/costs/0/what/filter/other");
    assertUnresolved("Sacrifice a creature, Tap another untapped creature you control: Draw a card.", "Creature",
      "/faces/0/abilities/0/costs/1/what/filter/other");
    assertUnresolved("This creature and enchanted creature get +1/+1. Other creatures you control get -1/-1.", "Enchantment Creature — Aura",
      "/faces/0/abilities/0/effect/effects/1/what/filter/other");
  });
});
