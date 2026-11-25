// Archive R5 RB-1: links between blocks of one face, the remaining closed
// vocabulary mappings, step events and span certificates. Card text is exact
// Oracle text from the frozen corpus (oracle-source-f8cb9da2…).
import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";
import { resolveCard, type BlockResolution, type RefEntry } from "../src/semantic/resolution.ts";
import type { BindingNode, CardBindingAnalysis, CardBindingInput } from "../src/semantic/types.ts";

function analyze(name: string, text: string, typeLine: string, manaCost: string): { input: CardBindingInput; analysis: CardBindingAnalysis } {
  const result = parseOracleText(text, name, { typeLine, layout: "normal", includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result.lines));
  const input = { identity: "fixture", source: { name, layout: "normal", type_line: typeLine, mana_cost: manaCost }, faces: [{ sourceFaceIndex: 0, result }] };
  return { input, analysis: analyzeCardBindings(input) };
}
function blocks(name: string, text: string, typeLine: string, manaCost: string): BlockResolution[] {
  const { input, analysis } = analyze(name, text, typeLine, manaCost);
  return resolveCard(input, analysis).faces[0].blocks!;
}
const refs = (block: BlockResolution) => Object.fromEntries(block.refs.map(({ path, ...rest }: RefEntry) => [path, rest]));
const complete = (all: BlockResolution[]) => assert.deepEqual(all.map(block => block.residuals), all.map(() => []));
function segments(text: string, block: BlockResolution): string[] {
  assert.ok("certificate" in block.spans, JSON.stringify(block.spans));
  const bytes = Buffer.from(text);
  return block.spans.certificate.segments.map(s => `${bytes.subarray(s.start, s.end).toString()}=${"fields" in s ? s.fields.join(",") : "•"}`);
}

const CAGED_SUN = "As this artifact enters, choose a color.\nCreatures you control of the chosen color get +1/+1.\nWhenever a land's ability causes you to add one or more mana of the chosen color, add an additional one mana of that color.";

describe("linked results of another block (CR 607.2d, 601.2c, 601.2h)", () => {
  it("links an as-enters choice to the statics and triggers that use it", () => {
    const sun = blocks("Caged Sun", CAGED_SUN, "Artifact", "{6}");
    const choice = { ref: { ref: "linked-result", block: 0, step: "/effect/effects/0/steps/0", role: "choice" } };
    assert.deepEqual(refs(sun[1])["/effect/what/filter/chosenColor"], choice);
    assert.deepEqual(refs(sun[2])["/trigger/color"], choice);
    complete(sun);
    const horn = blocks("Herald's Horn", "As this artifact enters, choose a creature type.\nCreature spells you cast of the chosen type cost {1} less to cast.\nAt the beginning of your upkeep, look at the top card of your library. If it's a creature card of the chosen type, you may reveal it and put it into your hand.", "Artifact", "{3}");
    assert.deepEqual(refs(horn[1])["/effect/scope/spells/filter/chosenType"], choice);
    assert.deepEqual(refs(horn[2])["/effects/1/condition/filter/chosenType"], choice);
    complete(horn);
  });

  it("links a later paragraph of one spell, and a spell to its own paid additional cost", () => {
    const dispatch = blocks("Dispatch", "Tap target creature.\nMetalcraft — If you control three or more artifacts, exile that creature.", "Instant", "{W}");
    assert.deepEqual(refs(dispatch[1])["/effects/0/steps/0/what"],
      { ref: { ref: "linked-result", block: 0, step: "/effects/0/steps/0/what", role: "selection" } });
    complete(dispatch);
    const fall = blocks("Momentous Fall", "As an additional cost to cast this spell, sacrifice a creature.\nYou draw cards equal to the sacrificed creature's power, then you gain life equal to its toughness.", "Instant", "{2}{G}{G}");
    const paid = { ref: { ref: "linked-result", block: 0, step: "/costs/0/what", role: "selection" } };
    assert.deepEqual(refs(fall[1])["/effects/0/steps/0/amount/of"], paid);
    assert.deepEqual(refs(fall[1])["/effects/0/steps/1/amount/of"], paid);
    complete(fall);
  });

  it("reads one announced X in the spell's additional cost (CR 107.3a)", () => {
    const deluge = blocks("Toxic Deluge", "As an additional cost to cast this spell, pay X life.\nAll creatures get -X/-X until end of turn.", "Sorcery", "{2}{B}");
    assert.deepEqual(refs(deluge[0])["/costs/0/amount"], { ref: { ref: "announced-x" } });
    complete(deluge);
  });

  it("names a kicker's payer in a later block as the prospective caster", () => {
    const battlemage = blocks("Stormscape Battlemage", "Kicker {W} and/or {2}{B} (You may pay an additional {W} and/or {2}{B} as you cast this spell.)\nWhen this creature enters, if it was kicked with its {W} kicker, you gain 3 life.\nWhen this creature enters, if it was kicked with its {2}{B} kicker, destroy target nonblack creature. That creature can't be regenerated.", "Creature — Metathran Wizard", "{2}{U}");
    for (const block of battlemage.slice(1))
      assert.deepEqual(refs(block)["/condition/kickerCost/0/payer"], { ref: { ref: "controller", role: "prospective-caster" } });
    complete(battlemage);
  });

  it("keeps linked exile pools unresolved until the pool names its block", () => {
    const [, scepter] = blocks("Isochron Scepter", "Imprint — When this artifact enters, you may exile an instant card with mana value 2 or less from your hand.\n{2}, {T}: You may copy the exiled card. If you do, you may cast the copy without paying its mana cost.", "Artifact", "{2}");
    assert.deepEqual(refs(scepter)["/effects/0/steps/0/what"], { unresolved: "cross-block:linked-exile-pool" });
  });

  it("never links a choice when the face makes two of its category, or from a granted ability", () => {
    // The binder already refuses both; the resolution guards are checked on
    // graphs that bind anyway.
    const twice = blocks("Fixture Card", "As this artifact enters, choose a color.\nAs this artifact enters, choose a color.\nCreatures you control of the chosen color get +1/+1.", "Artifact", "{1}");
    assert.ok(twice[2].residuals.length && !twice[2].refs.some(e => "ref" in e && e.ref.ref === "linked-result"), JSON.stringify(twice[2]));
    const granted = blocks("Fixture Card", "As this artifact enters, choose a color.\nCreatures you control have \"{T}: Add one mana of the chosen color.\"", "Artifact", "{1}");
    assert.ok(granted[1].residuals.length && !granted[1].refs.some(e => "ref" in e && e.ref.ref === "linked-result"), JSON.stringify(granted[1]));
    // The binder binds this one; the reference sits inside the ability the
    // static grants, so it stays a residual.
    const ward = blocks("Flickering Ward", "Enchant creature\nAs this Aura enters, choose a color.\nEnchanted creature has protection from the chosen color. This effect doesn't remove this Aura.\n{W}: Return this Aura to its owner's hand.", "Enchantment — Aura", "{W}");
    assert.deepEqual(refs(ward[2])["/effect/modifiers/0/abilities/0/keyword/from/0/value"], { unresolved: "cross-block:value-choice" });

    const { input, analysis } = analyze("Caged Sun", CAGED_SUN, "Artifact", "{6}");
    const choice = analysis.nodes.find(node => node.node === "value-choice")!;
    const second = { ...choice, id: `${choice.id}-second`, path: choice.path.replace("/abilities/0/", "/abilities/2/") } as BindingNode;
    const two = resolveCard(input, { ...analysis, nodes: [...analysis.nodes, second] }).faces[0].blocks!;
    assert.deepEqual(refs(two[1])["/effect/what/filter/chosenColor"], { unresolved: "cross-block:value-choice" });
    const ability = analysis.nodes.find(node => node.node === "ability" && node.path === "/faces/0/abilities/1")!;
    const quoted = { ...ability, id: `${ability.id}-quoted`, path: "/faces/0/abilities/1/effect/what" } as BindingNode;
    const nested = resolveCard(input, { ...analysis, nodes: [...analysis.nodes, quoted] }).faces[0].blocks!;
    assert.deepEqual(refs(nested[1])["/effect/what/filter/chosenColor"], { unresolved: "cross-block:value-choice" });
    assert.ok(!refs(nested[2])["/trigger/color"] || "ref" in refs(nested[2])["/trigger/color"]);
  });
});

describe("closed vocabulary for binder nodes", () => {
  it("names a declaration cost's payer as the declaring event player (CR 508.1)", () => {
    const [propaganda] = blocks("Propaganda", "Creatures can't attack you unless their controller pays {2} for each creature they control that's attacking you.", "Enchantment", "{2}{U}");
    const declaring = { ref: { ref: "event-player", role: "declaring-player" } };
    assert.deepEqual(refs(propaganda)["/effect/rule/who"], declaring);
    assert.deepEqual(refs(propaganda)["/effect/rule/costs/0/payer"], declaring);
    assert.deepEqual(propaganda.residuals, []);
  });

  it("names alternatives only when every alternative event denotes the same reference (CR 603.2)", () => {
    const [hammer] = blocks("Hammer of Nazahn", "Whenever Hammer of Nazahn or another Equipment you control enters, you may attach that Equipment to target creature you control.\nEquipped creature gets +2/+0 and has indestructible.\nEquip {4}", "Legendary Artifact — Equipment", "{4}");
    assert.deepEqual(refs(hammer)["/effects/0/steps/0/what"], { ref: { ref: "event-object", role: "enters" } });
    assert.deepEqual(hammer.residuals, []);
    // A dies or exile trigger, and a blocks or becomes-blocked trigger, name different events.
    const bontu = blocks("God-Eternal Bontu", "Menace\nWhen God-Eternal Bontu enters, sacrifice any number of other permanents, then draw that many cards.\nWhen God-Eternal Bontu dies or is put into exile from the battlefield, you may put her into her owner's library third from the top.", "Legendary Creature — Zombie God", "{3}{B}{B}");
    assert.deepEqual(refs(bontu[2])["/effects/0/steps/0/what"], { unresolved: "alternatives" });
    const [wurm] = blocks("Engulfing Slagwurm", "Whenever this creature blocks or becomes blocked by a creature, destroy that creature. You gain life equal to that creature's toughness.", "Creature — Wurm", "{5}{G}{G}");
    assert.deepEqual(refs(wurm)["/effects/0/steps/0/what"], { unresolved: "alternatives" });
  });

  it("binds a per-recipient damage amount's player to that member", () => {
    const [, , lynx] = blocks("Sunspine Lynx", "Players can't gain life.\nDamage can't be prevented.\nWhen this creature enters, it deals damage to each player equal to the number of nonbasic lands that player controls.", "Creature — Elemental Cat", "{2}{R}{R}");
    assert.deepEqual(refs(lynx)["/effects/0/steps/0/amount/of/control"], { ref: { ref: "bound-var", id: "/effects/0/steps/0/amount" } });
    assert.deepEqual(lynx.residuals, []);
  });

  it("derives a player inside a granted ability from an operand of that same scope", () => {
    const [, burgeoning] = blocks("Urban Burgeoning", "Enchant land\nEnchanted land has \"Untap this land during each other player's untap step.\"", "Enchantment — Aura", "{G}");
    assert.deepEqual(refs(burgeoning)["/effect/modifiers/0/abilities/0/ability/effect/step/owner"], {
      derived: { relation: "other-players", of: { ref: "controller", role: "static-controller" } }, scope: "/effect/modifiers/0/abilities/0/ability" });
    assert.deepEqual(burgeoning.residuals, []);
  });
});

describe("step events", () => {
  const event = (block: BlockResolution, path: string) => block.steps.find(step => step.path === path);
  it("records a choice between instructions as the choice step's event (CR 608.2d)", () => {
    const [gust] = blocks("Aether Gust", "Choose target spell or permanent that's red or green. Its owner puts it on their choice of the top or bottom of their library.", "Instant", "{1}{U}");
    assert.deepEqual(event(gust, "/effects/1/steps/0"), { path: "/effects/1/steps/0", astEffect: "choice", event: "choice" });
    assert.deepEqual(gust.residuals, []);
  });

  it("records text changes and copy retargeting as their own steps", () => {
    const [alter] = blocks("Alter Reality", "Change the text of target spell or permanent by replacing all instances of one color word with another. (This effect lasts indefinitely.)\nFlashback {1}{U} (You may cast this card from your graveyard for its flashback cost. Then exile it.)", "Instant", "{1}{U}");
    assert.deepEqual(event(alter, "/effects/0/steps/0"), { path: "/effects/0/steps/0", astEffect: "change-text", event: "change-text" });
    assert.deepEqual(alter.residuals, []);
    const [vengeance] = blocks("Increasing Vengeance", "Copy target instant or sorcery spell you control. If this spell was cast from a graveyard, copy that spell twice instead. You may choose new targets for the copies.\nFlashback {3}{R}{R} (You may cast this card from your graveyard for its flashback cost. Then exile it.)", "Instant", "{R}{R}");
    assert.ok(vengeance.steps.some(step => step.astEffect === "choose-new-targets" && "event" in step), JSON.stringify(vengeance.steps));
    assert.deepEqual(vengeance.residuals, []);
  });
});

describe("span certificates", () => {
  it("attributes an unrecorded single sentence with no other clause field to that sentence", () => {
    const text = "End the turn. (Exile all spells and abilities from the stack, including this card. The player whose turn it is discards down to their maximum hand size. Damage wears off, and \"this turn\" and \"until end of turn\" effects end.)\nAt the beginning of your next end step, you lose the game.";
    const [, delayed] = blocks("Glorious End", text, "Instant", "{2}{R}");
    const line = "At the beginning of your next end step, you lose the game.";
    assert.deepEqual(segments(line, delayed), [`${line}=/effects/0`]);
  });

  it("keeps the unrecorded case unsupported when a trigger shares the line", () => {
    const [, yidris] = blocks("Yidris, Maelstrom Wielder", "Trample\nWhenever Yidris deals combat damage to a player, as you cast spells from your hand this turn, they gain cascade. (When you cast the spell, exile cards from the top of your library until you exile a nonland card that costs less. You may cast it without paying its mana cost. Put the exiled cards on the bottom in a random order.)", "Legendary Creature — Ogre Wizard", "{W}{U}{B}{R}{G}");
    assert.deepEqual(yidris.spans, { unsupported: "sentences were not recorded clause by clause" });
  });

  it("assigns bullets to the outer modal options, not a choice inside one bullet", () => {
    const text = "Choose one or both —\n• The owner of target nonland permanent puts it on their choice of the top or bottom of their library.\n• Return target nonland permanent to its owner's hand.";
    const [ashore] = blocks("Run Ashore", text, "Instant", "{4}{U}{U}");
    assert.deepEqual(segments(text, ashore), ["Choose one or both —=/effects/0", "•=•",
      "The owner of target nonland permanent puts it on their choice of the top or bottom of their library.=/effects/0/steps/0/options/0",
      "•=•", "Return target nonland permanent to its owner's hand.=/effects/0/steps/0/options/1"]);
    assert.deepEqual(ashore.residuals, []);
  });

  it("never certifies an unspaced keyword-cost dash as an ability word", () => {
    // CR 702.47a: the opponent gaining 5 life is Roar of Jukai's splice cost,
    // not an instruction of the spell.
    const text = "If you control a Forest, each blocked creature gets +2/+2 until end of turn.\nSplice onto Arcane—An opponent gains 5 life. (As you cast an Arcane spell, you may reveal this card from your hand and pay its splice cost. If you do, add this card's effects to that spell.)";
    const [, splice] = blocks("Roar of Jukai", text, "Instant — Arcane", "{2}{G}");
    assert.ok(!("certificate" in splice.spans), JSON.stringify(splice.spans));
  });
});
