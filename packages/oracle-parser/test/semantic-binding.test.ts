import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, ObjectRef, ParseCardResult } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";
import type { CardBindingAnalysis, CardBindingInput, ReferenceBinding } from "../src/semantic/types.ts";

const fixed = (value: number) => ({ amount: "fixed" as const, value });
function input(text: string, name = "Binding fixture"): CardBindingInput {
  const result = parseOracleText(text, name, { includeSourceProvenance: true });
  assert.equal(result.ok, true, JSON.stringify(result.lines));
  return { identity: "fixture-id", source: { name, layout: "normal", type_line: "Sorcery" },
    faces: [{ sourceFaceIndex: 0, result }] };
}
function fromAbilities(abilities: Ability[]): CardBindingInput {
  const name = "Binding fixture";
  const result: ParseCardResult = { name, ok: true, abilities,
    lines: abilities.map(ability => ({ text: "structural fixture", ok: true, ability })) };
  return { identity: "fixture-id", source: { name, layout: "normal" }, faces: [{ sourceFaceIndex: 0, result }] };
}
function target(selectionId?: string): ObjectRef {
  return { ref: "target", filter: { types: ["creature"] }, count: fixed(1),
    ...(selectionId ? { selectionId } : {}) };
}
function binding(graph: CardBindingAnalysis, suffix: string): ReferenceBinding {
  const ref = graph.references.find(reference => reference.path.endsWith(suffix));
  assert.ok(ref, suffix);
  return ref;
}
function targetOf(graph: CardBindingAnalysis, suffix: string): string {
  const ref = binding(graph, suffix);
  assert.equal(ref.binding, "bound", JSON.stringify(ref));
  if (ref.binding !== "bound") throw new Error(suffix);
  return ref.target;
}
function freeze(value: unknown): void {
  if (!value || typeof value !== "object") return;
  Object.freeze(value);
  for (const entry of Object.values(value)) freeze(entry);
}

// Examples checked against the cached Scryfall records: Lay of the Land,
// Mourning Thrull, and Cloudshift. Reference rules are from the 2026-09-25 CR:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
describe("whole-card reference binding", () => {
  it("binds you differently for resolving spells, activations, triggers, and static abilities", () => {
    const graph = analyzeCardBindings(input("Draw a card.\n{T}: Draw a card.\nWhenever this creature deals damage, you gain that much life.\nCreatures you control get +1/+1."));
    assert.equal(graph.fullyBound, true, JSON.stringify(graph.diagnostics));
    assert.equal(graph.executable, false);
    assert.deepEqual(graph.nodes.filter(node => node.node === "runtime-player").map(node => node.role),
      ["spell-controller", "activator", "trigger-controller", "static-source-controller"]);
    const tapSource = targetOf(graph, "/abilities/1/costs/0/source");
    assert.equal(tapSource, "card:fixture-id/source#object");
    assert.equal(targetOf(graph, "/abilities/1/effects/0/steps/0/who"), "card:fixture-id/faces/0/abilities/1#you");
  });

  it("binds search, reveal, and move references to the same selected card (Lay of the Land)", () => {
    const graph = analyzeCardBindings(input("Search your library for a basic land card, reveal it, put it into your hand, then shuffle.", "Lay of the Land"));
    assert.equal(graph.fullyBound, true, JSON.stringify(graph.diagnostics));
    const selected = targetOf(graph, "/effects/0/steps/0/for");
    assert.equal(targetOf(graph, "/effects/0/steps/1/what"), selected);
    assert.equal(targetOf(graph, "/effects/0/steps/2/what"), selected);
    assert.equal(graph.nodes.filter(node => node.node === "event").length, 4);
    assert.ok(graph.references.every(ref => ref.rules.length > 0));
  });

  it("binds that much to the actual matching damage event, not printed damage text", () => {
    const graph = analyzeCardBindings(input("Whenever this creature deals damage, you gain that much life.", "Mourning Thrull"));
    assert.equal(graph.fullyBound, true, JSON.stringify(graph.diagnostics));
    const amount = targetOf(graph, "/effects/0/steps/0/amount");
    const quantity = graph.nodes.find(node => node.id === amount);
    assert.deepEqual(quantity, { id: "card:fixture-id/faces/0/abilities/0/trigger#quantity", path: "/faces/0/abilities/0/trigger",
      node: "event-result", event: "card:fixture-id/faces/0/abilities/0/trigger#event", result: "quantity",
      cardinality: "one", availability: "if-produced", identity: "measured-quantity", rules: ["608.2h"] });
  });

  it("uses explicit target identities and tracks a public-zone successor", () => {
    const graph = analyzeCardBindings(fromAbilities([{ kind: "spell", effects: [{ steps: [
      { effect: "exile", what: target("line:0:1") },
      { effect: "move-zone", what: { ref: "that", filter: { cls: "card" } }, to: { zone: "battlefield" }, controller: "owner" },
    ] }] }]));
    assert.equal(graph.fullyBound, true, JSON.stringify(graph.diagnostics));
    assert.equal(targetOf(graph, "/effects/0/steps/0/what"), "card:fixture-id/faces/0/abilities/0#ability/targets/line%3A0%3A1");
    const returned = targetOf(graph, "/effects/0/steps/1/what");
    assert.equal(returned, "card:fixture-id/faces/0/abilities/0/effects/0/steps/0#objects");
    const owner = graph.nodes.find(node => node.id === targetOf(graph, "/effects/0/steps/1/controller"));
    assert.ok(owner?.node === "derived-player" && owner.of === returned && owner.role === "owner");
  });

  it("consumes actual parser provenance for separate and elided target occurrences", () => {
    const separate = analyzeCardBindings(input("Tap target creature, then untap target creature."));
    assert.equal(separate.fullyBound, true, JSON.stringify(separate.diagnostics));
    assert.notEqual(targetOf(separate, "/steps/0/what"), targetOf(separate, "/steps/1/what"));
    const elided = analyzeCardBindings(input("Target creature gets +1/+1 and gains flying until end of turn."));
    assert.equal(targetOf(elided, "/steps/0/what"), targetOf(elided, "/steps/1/what"));
    assert.equal(elided.nodes.filter(node => node.node === "selection").length, 1);
    const cloudshift = analyzeCardBindings(input("Exile target creature you control, then return that card to the battlefield under your control.", "Cloudshift"));
    assert.equal(cloudshift.fullyBound, true, JSON.stringify(cloudshift.diagnostics));
    assert.equal(targetOf(cloudshift, "/steps/1/what"), "card:fixture-id/faces/0/abilities/0/effects/0/steps/0#objects");
  });

  it("reuses an elided target ID but preserves separate identical target declarations", () => {
    const make = (second: ObjectRef) => fromAbilities([{ kind: "spell", effects: [{ steps: [
      { effect: "tap", what: target("line:0:1") }, { effect: "untap", what: second },
    ] }] }]);
    const shared = analyzeCardBindings(make(target("line:0:1")));
    assert.equal(shared.fullyBound, true, JSON.stringify(shared.diagnostics));
    assert.equal(shared.nodes.filter(node => node.node === "selection").length, 1);
    assert.equal(targetOf(shared, "/steps/0/what"), targetOf(shared, "/steps/1/what"));
    const separate = analyzeCardBindings(make(target("line:0:5")));
    assert.equal(separate.fullyBound, true, JSON.stringify(separate.diagnostics));
    assert.equal(separate.nodes.filter(node => node.node === "selection").length, 2);
    assert.notEqual(targetOf(separate, "/steps/0/what"), targetOf(separate, "/steps/1/what"));
  });

  it("rejects missing or conflicting target identity without guessing from equality", () => {
    const noIdentity = analyzeCardBindings(fromAbilities([{ kind: "spell", effects: [{ steps: [{ effect: "tap", what: target() }] }] }]));
    assert.equal(noIdentity.fullyBound, false);
    assert.equal(noIdentity.counts.unresolved, 1);
    assert.equal(noIdentity.diagnostics[0].code, "missing-target-identity");
    const conflict = { ...target("same"), filter: { types: ["artifact"] } };
    const contradictory = analyzeCardBindings(fromAbilities([{ kind: "spell", effects: [{ steps: [
      { effect: "tap", what: target("same") }, { effect: "untap", what: conflict },
    ] }] }]));
    assert.equal(contradictory.fullyBound, false);
    assert.equal(contradictory.counts.unresolved, 2);
    assert.ok(contradictory.references.every(ref => ref.binding === "unresolved"));
  });

  it("keeps distinct participants and diagnoses ambiguous pronouns instead of taking the nearest noun", () => {
    const graph = analyzeCardBindings(fromAbilities([{ kind: "spell", effects: [{ steps: [
      { effect: "tap", what: target("first") }, { effect: "tap", what: target("second") },
      { effect: "untap", what: { ref: "it" } },
    ] }] }]));
    const pronoun = binding(graph, "/steps/2/what");
    assert.equal(graph.fullyBound, false);
    assert.ok(pronoun.binding === "unresolved" && pronoun.code === "ambiguous-antecedent" && pronoun.candidates.length === 2);
  });

  it("retains plural results and diagnoses a singular reference to a group", () => {
    const text = "Draw two cards. Reveal them.";
    const plural = analyzeCardBindings(input(text));
    assert.equal(plural.fullyBound, true, JSON.stringify(plural.diagnostics));
    assert.equal(targetOf(plural, "/effects/1/steps/0/what"), "card:fixture-id/faces/0/abilities/0/effects/0/steps/0#objects");
    const singular = analyzeCardBindings(input("Draw two cards. Reveal it."));
    assert.equal(singular.fullyBound, false);
    assert.equal(binding(singular, "/effects/1/steps/0/what").binding, "unresolved");
    assert.ok(singular.diagnostics.some(d => d.code === "cardinality-mismatch"));
  });

  it("does not leak source or antecedent bindings between abilities or faces", () => {
    const isolated = analyzeCardBindings(input("{T}: Draw a card.\n{1}: Reveal it."));
    assert.equal(isolated.fullyBound, false);
    assert.ok(isolated.diagnostics.some(d => d.code === "missing-antecedent"));
    // An instant's or sorcery's paragraphs are followed in order as one spell (CR 113.3a, 608.2c).
    assert.equal(analyzeCardBindings(input("Draw a card.\nReveal it.")).fullyBound, true);
    const front = parseOracleText("Draw a card.", "Front");
    const back = parseOracleText("Reveal it.", "Back");
    const graph = analyzeCardBindings({ identity: "two-faced", source: { name: "Front // Back", layout: "transform",
      card_faces: [{ name: "Front" }, { name: "Back" }] }, faces: [
      { sourceFaceIndex: 0, result: front }, { sourceFaceIndex: 1, result: back },
    ] });
    assert.equal(graph.fullyBound, false);
    assert.equal(graph.nodes.filter(node => node.node === "source").length, 1);
    assert.equal(graph.nodes.filter(node => node.node === "face").length, 2);
    assert.ok(graph.diagnostics.some(d => d.path.startsWith("/faces/1/") && d.code === "missing-antecedent"));
  });

  it("reports every symbolic reference behind an unsupported nested scope", () => {
    const graph = analyzeCardBindings(input("Choose two. You may choose the same mode more than once.\n• Draw a card.\n• Target player mills a card."));
    assert.equal(graph.fullyBound, false);
    assert.ok(graph.diagnostics.some(d => d.code === "unsupported-scope"));
    assert.equal(graph.counts.obligations, 2);
    assert.equal(graph.counts.unresolved, 2);
    const unknownX = analyzeCardBindings(input("Draw X cards."));
    assert.equal(unknownX.fullyBound, false);
    assert.ok(unknownX.diagnostics.some(d => d.code === "unsupported-number-binding"));
  });

  it("validates consumed source fields and requires all faces to have matching successful parses", () => {
    const base = input("Draw a card.");
    const failed = analyzeCardBindings({ ...base, faces: [] });
    assert.equal(failed.fullyBound, false);
    assert.equal(failed.diagnostics[0].code, "missing-face");
    const mismatch = analyzeCardBindings({ ...base, source: { name: "Wrong source", layout: "normal" } });
    assert.equal(mismatch.diagnostics[0].code, "face-mismatch");
    const invalid = structuredClone(base);
    Object.assign(invalid.source, { mana_cost: 23 });
    assert.equal(analyzeCardBindings(invalid).diagnostics[0].code, "invalid-source");
    const invalidIdentity = structuredClone(base);
    Object.assign(invalidIdentity, { identity: 23 });
    assert.equal(analyzeCardBindings(invalidIdentity).diagnostics[0].code, "invalid-source");
    assert.equal(analyzeCardBindings({ ...base, source: { ...base.source, layout: "transform" } }).diagnostics[0].code, "invalid-source");
    const unsupported = analyzeCardBindings({ ...base, faces: [{ sourceFaceIndex: 0, result: parseOracleText("Invented text.", base.source.name) }] });
    assert.equal(unsupported.diagnostics[0].code, "unparsed-face");
  });

  it("does not call unexpanded intrinsic payment scopes fully bound", () => {
    // Actual costs include Tenth District Hero and Ruthless Waterbender.
    for (const text of ["Collect evidence 6: Draw a card.", "Waterbend {2}: Draw a card.", "Forage: Draw a card.", "Blight 1: Draw a card."]) {
      const graph = analyzeCardBindings(input(text));
      assert.equal(graph.fullyBound, false, text);
      assert.ok(graph.diagnostics.some(d => d.code === "unsupported-scope"), text);
    }
  });

  it("diagnoses unbound other/affected-object exclusions without crashing", () => {
    for (const text of ["Whenever another creature dies, draw a card.",
      "Creatures you control get +1/+1 for each other creature you control."]) {
      const graph = analyzeCardBindings(input(text));
      assert.equal(graph.fullyBound, false, text);
      assert.ok(graph.diagnostics.some(d => d.code === "unsupported-reference"));
    }
  });

  it("does not equate a moved source with its old object or assume a qualified that", () => {
    for (const text of ["Exile this creature, then return this card to the battlefield.",
      "Sacrifice this creature: Return this card to your hand."]) {
      const graph = analyzeCardBindings(input(text));
      assert.equal(graph.fullyBound, false, text);
      assert.ok(graph.references.some(ref => ref.binding === "unresolved" && "ref" in ref.expression && ref.expression.ref === "this"));
    }
    for (const text of ["Search your library for a land card. Reveal that creature.",
      "Search your library for a noncreature card. Reveal that creature."]) {
      const graph = analyzeCardBindings(input(text));
      assert.equal(graph.fullyBound, false, text);
      assert.equal(binding(graph, "/effects/1/steps/0/what").binding, "unresolved");
    }
  });

  it("does not reduce distributed or repeated results to one object", () => {
    for (const text of ["Each player draws a card. Reveal it.", "Each opponent mills a card. Exile it.",
      "Each player creates a 1/1 white Soldier creature token. Sacrifice it.",
      "Create a 1/1 white Soldier creature token for each creature you control. Sacrifice it."]) {
      const graph = analyzeCardBindings(input(text));
      assert.equal(graph.fullyBound, false, text);
      assert.equal(binding(graph, "/effects/1/steps/0/what").binding, "unresolved");
    }
  });

  it("does not resolve a pronoun past an action that creates new objects", () => {
    // Populate and discover produce objects of their own, so "it" has two candidates.
    for (const text of ["Create a 1/1 white Soldier creature token. Populate. Sacrifice it.",
      "Tap target creature. Discover 3. Untap it."]) {
      const graph = analyzeCardBindings(input(text));
      assert.equal(graph.fullyBound, false, text);
      const it = graph.references.find(ref => "ref" in ref.expression && ref.expression.ref === "it");
      assert.ok(it && it.binding === "unresolved" && it.code === "ambiguous-antecedent", `${text}: ${JSON.stringify(it)}`);
    }
  });

  it("keeps an action's participant as the antecedent when it creates nothing", () => {
    const graph = analyzeCardBindings(input("Tap target creature. Scry 2. Untap it."));
    assert.equal(graph.fullyBound, true, JSON.stringify(graph.diagnostics));
    assert.equal(targetOf(graph, "/steps/0/what"), targetOf(graph, "/effects/2/steps/0/what"));
  });

  it("binds another player's optional payment to that player, not the ability controller (CR 118.12a)", () => {
    const graph = analyzeCardBindings(input("Draw a card unless target opponent pays {2}."));
    assert.equal(graph.fullyBound, true, JSON.stringify(graph.diagnostics));
    assert.equal(targetOf(graph, "/cost/0/payer"), targetOf(graph, "/condition/who"));
    assert.notEqual(targetOf(graph, "/cost/0/payer"), targetOf(graph, "/steps/0/who"));
    // "Unless its controller pays": the controller of the spell the instruction targets.
    const counter = analyzeCardBindings(input("Counter target spell unless its controller pays {3}."));
    assert.equal(counter.fullyBound, true, JSON.stringify(counter.diagnostics));
    assert.equal(targetOf(counter, "/condition/who/of"), targetOf(counter, "/steps/0/what"));
  });

  it("retains implicit actor, turn-owner, payer, and zone-owner obligations", () => {
    const token = analyzeCardBindings(input("Whenever you attack, create a 1/1 white Soldier creature token."));
    assert.equal(token.fullyBound, true, JSON.stringify(token.diagnostics));
    assert.equal(targetOf(token, "/trigger/who"), targetOf(token, "/effects/0/steps/0/who"));
    const activation = analyzeCardBindings(input("{T}: Draw a card. Activate only during your turn."));
    assert.equal(targetOf(activation, "/restriction/turn"), targetOf(activation, "/costs/0/payer"));
    for (const text of ["Discard your hand: Draw a card.", "Mill two cards: Draw a card."]) {
      const graph = analyzeCardBindings(input(text));
      assert.equal(targetOf(graph, "/costs/0/payer"), targetOf(graph, "/costs/0/zone-owner"));
    }
    const permission = analyzeCardBindings(input("{2}: Draw a card. Any player may activate this ability."));
    assert.equal(permission.fullyBound, false);
    assert.ok(permission.diagnostics.some(d => d.path.endsWith("/permission")));
  });

  it("propagates unresolved target inputs into event results and later pronouns", () => {
    const original = input("Target player draws a card. Reveal it.");
    original.faces[0].result = parseOracleText("Target player draws a card. Reveal it.", original.source.name);
    const graph = analyzeCardBindings(original);
    assert.equal(graph.fullyBound, false);
    const pronoun = binding(graph, "/effects/1/steps/0/what");
    assert.ok(pronoun.binding === "unresolved" && pronoun.code === "unresolved-dependency");
    const direct = analyzeCardBindings(input("This creature deals 1 damage to any target."));
    assert.equal(direct.fullyBound, true, JSON.stringify(direct.diagnostics));
    assert.ok(direct.nodes.some(node => node.node === "selection" && "ref" in node.expression && node.expression.ref === "any-target"));
    const union = analyzeCardBindings(fromAbilities([{ kind: "spell", effects: [{ steps: [
      { effect: "tap", what: { ref: "any-target", selectionId: "line:0:1" } },
      { effect: "untap", what: { ref: "it" } },
    ] }] }]));
    assert.equal(binding(union, "/steps/1/what").binding, "unresolved");
  });

  it("is deterministic across serialization and never changes the parser result", () => {
    const original = input("Search your library for a basic land card, reveal it, put it into your hand, then shuffle.");
    const before = JSON.stringify(original);
    freeze(original);
    const graph = analyzeCardBindings(original);
    assert.deepEqual(graph, analyzeCardBindings(JSON.parse(before) as CardBindingInput));
    assert.equal(JSON.stringify(original), before);
    assert.equal(graph.counts.obligations, graph.counts.bound + graph.counts.unresolved);
    assert.equal(new Set(graph.nodes.map(node => node.id)).size, graph.nodes.length);
    assert.ok(graph.references.every(ref => ref.binding !== "bound" || graph.nodes.some(node => node.id === ref.target)));
    assert.equal("certified" in graph, false);
    assert.equal("validated" in graph, false);
  });
});
