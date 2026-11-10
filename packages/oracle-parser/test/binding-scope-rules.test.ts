// Reference-binding rules for granted abilities, attachments, entry,
// implicit players, "other", last-known information, conditional sentences,
// and created triggered abilities. Each case follows a real card's wording.
import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/parser/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";
import type { BindingNode, CardBindingAnalysis } from "../src/semantic/types.ts";

function analyze(text: string, typeLine: string, manaCost?: string): CardBindingAnalysis {
  const result = parseOracleText(text, "Fixture", { includeSourceProvenance: true, typeLine });
  assert.equal(result.ok, true, `${text}: ${JSON.stringify(result.lines)}`);
  return analyzeCardBindings({ identity: "fixture", source: { name: "Fixture", layout: "normal", type_line: typeLine, ...(manaCost ? { mana_cost: manaCost } : {}) },
    faces: [{ sourceFaceIndex: 0, result }] });
}
function bound(graph: CardBindingAnalysis, suffix: string): BindingNode | string {
  const reference = graph.references.find(candidate => candidate.path.endsWith(suffix));
  assert.ok(reference && reference.binding === "bound", `${suffix}: ${JSON.stringify(reference)}`);
  if (reference.binding !== "bound") throw new Error(suffix);
  return graph.nodes.find(node => node.id === reference.target) ?? reference.target;
}
function unresolved(graph: CardBindingAnalysis, suffix: string): void {
  const reference = graph.references.find(candidate => candidate.path.endsWith(suffix));
  assert.ok(reference && reference.binding === "unresolved", `${suffix}: ${JSON.stringify(reference)}`);
}
const fully = (graph: CardBindingAnalysis) => assert.equal(graph.fullyBound, true, JSON.stringify(graph.diagnostics));
const sourceId = (graph: CardBindingAnalysis) => graph.nodes.find(node => node.node === "source")!;

describe("granted abilities and attachments (CR 113.10, 301.5f, 303.4m)", () => {
  it("makes the recipient the source of a quoted ability", () => {
    const graph = analyze("Creatures you control have \"{T}: Add {G}.\"", "Enchantment");
    fully(graph);
    const source = bound(graph, "/ability/costs/0/source");
    assert.ok(typeof source !== "string" && source.node === "recipient" && source.each);
  });
  it("binds the equipped creature to what the Equipment is attached to (Behemoth Sledge)", () => {
    const graph = analyze("Equipped creature gets +2/+2 and has lifelink and trample.", "Artifact — Equipment");
    fully(graph);
    const what = bound(graph, "/effect/what");
    assert.ok(typeof what !== "string" && what.node === "derived-object" && what.role === "attached-to");
  });
  it("leaves a quoted ability unbound when its recipient is not an object", () => {
    const graph = analyze("You have \"Whenever you cast a spell, draw a card.\"", "Enchantment");
    assert.equal(graph.fullyBound, false);
  });
});

describe("entry, implicit players and other (CR 614.12, 102.2)", () => {
  it("binds a land's own entry and its \"other lands\" to the land", () => {
    const graph = analyze("This land enters tapped unless you control two or more other lands.", "Land");
    fully(graph);
    const you = bound(graph, "/condition/who");
    assert.ok(typeof you !== "string" && you.node === "runtime-player" && you.role === "entering-controller");
    assert.equal(bound(graph, "/condition/of/other"), sourceId(graph));
  });
  it("binds \"an opponent controls\" and \"you don't control\" relative to you", () => {
    const graph = analyze("Destroy target creature you don't control.", "Instant");
    fully(graph);
    const relation = bound(graph, "/filter/control");
    assert.ok(typeof relation !== "string" && relation.node === "derived-player" && relation.role === "other-players");
  });
  it("does not take \"another\" as the source after the trigger names an object", () => {
    const graph = analyze("Whenever a creature you control dies, return another target creature card from your graveyard to your hand.", "Enchantment");
    assert.ok(graph.diagnostics.some(d => d.path.endsWith("/filter/other")));
  });
});

describe("antecedents and last-known information (CR 608.2h, 113.7a)", () => {
  it("binds a destroyed object's controller and power to that object", () => {
    const graph = analyze("Destroy target creature. Its controller creates a 3/3 green Ape creature token.", "Instant");
    fully(graph);
    assert.equal(bound(graph, "/effects/1/steps/0/who/of"), bound(graph, "/effects/0/steps/0/what"));
  });
  it("lets a dying creature's last-known information answer a past-tense check", () => {
    fully(analyze("Whenever equipped creature dies, if it was a Human, create a 1/1 white Spirit creature token.", "Artifact — Equipment"));
  });
  it("acts on the card a zone-change trigger saw move to a public zone (CR 400.7e)", () => {
    const graph = analyze("Whenever a creature dies, if it was a Human, exile it.", "Enchantment");
    fully(graph);
    const human = bound(graph, "/condition/what"), exiled = bound(graph, "/steps/0/what");
    assert.ok(typeof human !== "string" && human.node === "event-result" && human.identity === "same-objects");
    assert.ok(typeof exiled !== "string" && exiled.node === "event-result" && exiled.identity === "zone-successors");
    const card = analyze("When this creature dies, return this card to its owner's hand.", "Creature");
    fully(card);
    assert.notEqual(bound(card, "/steps/0/what"), sourceId(card));
  });
  it("binds the source's damage after it was sacrificed (Aeolipile)", () => {
    const graph = analyze("{1}, {T}, Sacrifice this artifact: It deals 2 damage to any target.", "Artifact");
    fully(graph);
    assert.equal(bound(graph, "/steps/0/source"), sourceId(graph));
  });
  it("uses an instruction's own subject for its pronouns (Misleading Motes)", () => {
    const graph = analyze("Target creature's owner puts it on the top or bottom of their library.", "Instant");
    fully(graph);
    assert.equal(bound(graph, "/options/0/what"), bound(graph, "/options/0/who/of"));
  });
  it("binds the cards looked at and the player damaged by the trigger", () => {
    fully(analyze("Look at the top three cards of your library, then put them back in any order.", "Sorcery"));
    fully(analyze("Whenever this creature deals combat damage to a player, that player discards a card.", "Creature"));
  });
  it("binds a possessive to its instruction's subject (Palazzo Archers)", () => {
    const graph = analyze("Whenever a creature with flying attacks you, this creature deals damage equal to its power to that creature.", "Creature");
    fully(graph);
    assert.equal(bound(graph, "/amount/of"), sourceId(graph));
    assert.notEqual(bound(graph, "/to/ref"), sourceId(graph));
  });
  it("makes an intervening if clause's subject an antecedent (Volition Reins, Guardian of the Ages)", () => {
    const reins = analyze("When this Aura enters, if enchanted permanent is tapped, untap it.", "Enchantment — Aura");
    fully(reins);
    assert.equal(bound(reins, "/steps/0/what"), bound(reins, "/condition/what"));
    // "It" in the effect is the condition's subject, not the attacker.
    const guardian = analyze("Whenever a creature attacks you or a planeswalker you control, if this creature has defender, it loses defender and gains trample until end of turn.", "Creature");
    fully(guardian);
    assert.equal(bound(guardian, "/steps/0/what"), sourceId(guardian));
  });
  it("prefers an introduced object over the source for \"it\" (Llanowar Empath)", () => {
    const graph = analyze("When this creature enters, scry 2, then reveal the top card of your library. If it's a creature card, put it into your hand.", "Creature");
    const it = bound(graph, "/effects/1/steps/0/what");
    assert.notEqual(it, sourceId(graph));
  });
});

describe("conditional sentences and created abilities (CR 608.2c-d, 603.7, 603.12)", () => {
  it("links \"If you do\" to the one optional instruction before it", () => {
    const graph = analyze("You may sacrifice a creature. If you do, draw two cards.", "Sorcery");
    fully(graph);
    const link = graph.nodes.find(node => node.node === "decision-link");
    assert.ok(link && link.node === "decision-link" && link.decision.endsWith("/effects/0/steps/0"));
  });
  it("binds an unless payment to its payer after the instruction it follows", () => {
    const graph = analyze("Counter target spell unless its controller pays {3}.", "Instant");
    fully(graph);
    assert.equal(bound(graph, "/condition/who/of"), bound(graph, "/steps/0/what"));
  });
  it("lets a delayed trigger act on the object the spell exiled", () => {
    const graph = analyze("Exile target creature. Return it to the battlefield under its owner's control at the beginning of the next end step.", "Instant");
    fully(graph);
    const delayed = graph.nodes.find(node => node.node === "ability" && node.abilityKind === "delayed-triggered");
    assert.ok(delayed);
  });
  it("links a reflexive trigger to the decision it follows", () => {
    const graph = analyze("When this creature enters, you may discard a card. When you do, destroy target creature.", "Creature");
    fully(graph);
    assert.ok(graph.nodes.some(node => node.node === "decision-link"));
  });
  it("binds a vote count only to the one vote held before it", () => {
    fully(analyze("Each player votes for evidence or bribery. For each evidence vote, draw a card.", "Sorcery"));
  });
});

describe("zone successors, spell paragraphs and replaced instructions (CR 400.7e, 608.2c, 614.15)", () => {
  it("asks about a dead creature's counters through its last-known information", () => {
    const graph = analyze("When this creature dies, put X +1/+1 counters on target creature you control, where X is the number of +1/+1 counters on this creature.", "Creature");
    fully(graph);
    assert.equal(bound(graph, "/bindings/0/value/on"), sourceId(graph));
  });
  it("follows the card after it moves again (Lamplight Phoenix)", () => {
    const graph = analyze("When this creature dies, you may exile it and collect evidence 4. If you do, return this card to the battlefield tapped.", "Creature");
    fully(graph);
    assert.equal(bound(graph, "/effects/1/steps/0/what"), graph.nodes.find(node => node.path.endsWith("/effects/0/steps/0/steps/0") && node.node === "event-result"));
  });
  it("reads an instant's paragraphs as one spell (Dispatch)", () => {
    const graph = analyze("Tap target creature.\nMetalcraft — If you control three or more artifacts, exile that creature.", "Instant");
    fully(graph);
    assert.equal(bound(graph, "/abilities/1/effects/0/steps/0/what"), bound(graph, "/abilities/0/effects/0/steps/0/what"));
  });
  it("binds a replacement's pronoun to the participant in the same role (Flare of Faith, Frost Bite)", () => {
    const graph = analyze("Target creature gets +2/+2 until end of turn. If it's a Human, instead it gets +3/+3 and gains indestructible until end of turn.", "Instant");
    fully(graph);
    assert.equal(bound(graph, "/instead/0/what"), bound(graph, "/original/what"));
  });
  it("binds a static condition's subject and a spell's own cost condition", () => {
    const aura = analyze("Enchant creature\nEnchanted creature gets +1/+2 as long as it's white. Otherwise, it gets -2/-1.", "Enchantment — Aura");
    fully(aura);
    assert.equal(bound(aura, "/effect/otherwise/what"), bound(aura, "/effect/effect/what"));
    const spell = analyze("This spell costs {2} less to cast if it targets an attacking creature.\nReturn target creature to its owner's hand.", "Instant");
    fully(spell);
    assert.equal(bound(spell, "/effect/condition/what"), sourceId(spell));
    // "It" is each spell whose cost changes, not the permanent.
    unresolved(analyze("Each creature spell you cast costs {1} less to cast if it has mutate.", "Creature"), "/effect/condition/what");
  });
  it("binds that player to an owner or controller named through an object", () => {
    const recoil = analyze("Return target permanent to its owner's hand. Then that player discards a card.", "Instant");
    fully(recoil);
    assert.equal(bound(recoil, "/effects/1/steps/0/who"), bound(recoil, "/effects/0/steps/0/to"));
    fully(analyze("Whenever a land an opponent controls enters, that player loses 2 life and you gain 2 life.", "Enchantment"));
    fully(analyze("Counter target spell unless its controller pays {1}. That player discards a card.", "Instant"));
    // A player named outright is preferred over one named through an object.
    const actor = analyze("Target player returns target creature to its owner's hand. That player draws a card.", "Sorcery");
    assert.equal(bound(actor, "/effects/1/steps/0/who"), bound(actor, "/effects/0/steps/0/who"));
  });
});

describe("that many: measured quantities (CR 608.2c, 608.2h)", () => {
  const quantityOf = (graph: CardBindingAnalysis, suffix: string) => {
    const node = bound(graph, suffix);
    assert.ok(typeof node !== "string" && node.node === "event-result" && node.result === "quantity");
    return node.path;
  };
  it("counts what the instruction it follows did (Shocker, Book Devourer)", () => {
    const shocker = analyze("Whenever this creature deals damage to a player, that player discards all the cards in their hand, then draws that many cards.", "Creature");
    fully(shocker);
    assert.ok(quantityOf(shocker, "/steps/1/amount").endsWith("/effects/0/steps/0"));
    const devourer = analyze("Whenever this creature deals combat damage to a player, you may discard all the cards in your hand. If you do, draw that many cards.", "Creature");
    fully(devourer);
    assert.ok(quantityOf(devourer, "/effects/1/steps/0/amount").endsWith("/effects/0/steps/0"));
  });
  it("does not count a single object an instruction acted on (RMS Titanic)", () => {
    const graph = analyze("Whenever this creature deals combat damage to a player, sacrifice it and create that many Treasure tokens.", "Artifact — Vehicle");
    fully(graph);
    assert.ok(quantityOf(graph, "/steps/1/count").endsWith("/trigger"));
  });
  it("does not take a repeated that many as a new antecedent (Requiem Monolith)", () => {
    const graph = analyze("Whenever this creature is dealt damage, you draw that many cards and lose that much life.", "Creature");
    fully(graph);
    assert.ok(quantityOf(graph, "/steps/1/amount").endsWith("/trigger"));
  });
  it("measures one-or-more events, damage to a player, and costs", () => {
    fully(analyze("Whenever one or more Dragons you control attack, draw that many cards.", "Creature"));
    fully(analyze("Whenever you're dealt damage, put that many charge counters on this artifact.", "Artifact"));
    const amulet = analyze("{2}, {T}, Remove all charge counters from this artifact: It deals that much damage to target creature.", "Artifact");
    fully(amulet);
    assert.ok(quantityOf(amulet, "/steps/0/amount").endsWith("/costs/2"));
  });
});

describe("announced X (CR 107.3)", () => {
  const announcement = (graph: CardBindingAnalysis, suffix: string) => {
    const node = bound(graph, suffix);
    assert.ok(typeof node !== "string" && node.node === "announced-number", JSON.stringify(node));
    return node.announcement;
  };
  it("binds X to the value announced for the spell, the activation, or the entering spell", () => {
    assert.equal(announcement(analyze("Target player draws X cards.", "Sorcery", "{X}{U}{U}"), "/steps/0/amount"), "cast");
    assert.equal(announcement(analyze("{X}{R}, {T}: This creature deals X damage to target creature without flying.", "Creature", "{2}{R}"), "/steps/0/amount"), "activation");
    assert.equal(announcement(analyze("This creature enters with X +1/+1 counters on it.", "Creature", "{X}{G}{G}"), "/effect/count"), "entering-spell");
  });
  it("leaves X undefined on a permanent's other abilities (CR 107.3g) and uses a printed definition (CR 107.3c)", () => {
    unresolved(analyze("This creature gets +X/+X.", "Creature", "{X}{G}"), "/power/amount");
    const ghalta = analyze("This spell costs {X} less to cast, where X is the greatest toughness among creatures you control.", "Creature", "{10}{G}{G}");
    fully(ghalta);
    const defined = bound(ghalta, "/change/symbols/0");
    assert.ok(typeof defined !== "string" && defined.node === "defined-number");
  });
});

describe("linked abilities and this-way results (CR 607, 702.33e, 608.2c)", () => {
  it("links if it was kicked to the one kicker ability printed on the card", () => {
    const graph = analyze("Kicker {2}{R}\nWhen this creature enters, if it was kicked, it deals 3 damage to any target.", "Creature");
    fully(graph);
    assert.ok(graph.nodes.some(node => node.node === "linked-ability" && node.ability.endsWith("/keywords/0#keyword")));
    const specific = analyze("Kicker {1}{G} and/or {2}{U}\nWhen this creature enters, if it was kicked with its {1}{G} kicker, put two +1/+1 counters on it.", "Creature");
    fully(specific);
    const payer = bound(specific, "/condition/kickerCost/0/payer");
    assert.ok(typeof payer !== "string" && payer.node === "runtime-player" && payer.role === "prospective-caster");
  });
  it("links destroyed this way to the one earlier destroy instruction", () => {
    const graph = analyze("Destroy all creatures. You gain 1 life for each creature destroyed this way.", "Sorcery");
    fully(graph);
    const link = graph.nodes.find(node => node.node === "event-link");
    assert.ok(link && link.node === "event-link" && link.event.includes("/effects/0/steps/0#event"));
  });
  it("does not link to an instruction in another mode", () => {
    const graph = analyze("Choose one —\n• Discard a card.\n• Draw cards equal to the number of cards discarded this way.", "Sorcery");
    assert.equal(graph.fullyBound, false);
  });
});

describe("the rest of a pool and an affected object's controller", () => {
  it("binds the rest to the looked-at cards less the chosen one", () => {
    const graph = analyze("Look at the top three cards of your library. Put one of them into your hand and the rest into your graveyard.", "Sorcery");
    fully(graph);
    const rest = bound(graph, "/steps/1/what");
    assert.ok(typeof rest !== "string" && rest.node === "remainder");
  });
  it("binds its controller's untap step and an exiled card's owner (CR 502.3, 610.3c)", () => {
    const tapped = analyze("Tap target creature. It doesn't untap during its controller's next untap step.", "Instant");
    fully(tapped);
    const controller = bound(tapped, "/step/owner");
    assert.ok(typeof controller !== "string" && controller.node === "derived-player" && controller.role === "controller");
    fully(analyze("When this creature enters, exile target artifact or creature an opponent controls until this creature leaves the battlefield.", "Creature"));
  });
});

describe("type agreement and governing conditions", () => {
  it("binds that land to the antecedent established as a land (Akoum Hellkite)", () => {
    const graph = analyze("Landfall — Whenever a land you control enters, this creature deals 1 damage to any target. If that land is a Mountain, this creature deals 2 damage instead.", "Creature");
    fully(graph);
  });
  it("binds it to the subject of the condition governing it (Strider, Ranger of the North)", () => {
    const graph = analyze("Landfall — Whenever a land you control enters, target creature gets +1/+1 until end of turn. Then if that creature has power 4 or greater, it gains first strike until end of turn.", "Creature");
    fully(graph);
    assert.equal(bound(graph, "/effects/1/steps/0/what"), bound(graph, "/effects/0/steps/0/what"));
  });
});

describe("block triggers and alternative events (CR 509, 603.2)", () => {
  it("binds that creature to the creature blocked, not the blocker (Wall of Frost)", () => {
    const graph = analyze("Whenever this creature blocks a creature, that creature doesn't untap during its controller's next untap step.", "Creature — Wall");
    fully(graph);
    assert.notEqual(bound(graph, "/steps/0/what"), sourceId(graph));
    assert.ok(graph.references.every(reference => reference.binding !== "bound" || !reference.path.endsWith("/steps/0/what") || reference.target.endsWith("/trigger/blocking#query")));
  });
  it("binds that creature to whichever alternative event's other creature (Inferno Elemental)", () => {
    const graph = analyze("Whenever this creature blocks or becomes blocked by a creature, this creature deals 3 damage to that creature.", "Creature");
    fully(graph);
    const other = bound(graph, "/to/ref");
    assert.ok(typeof other !== "string" && other.node === "alternatives" && other.of.length === 2);
  });
  it("binds the payer's count in the ability's own voice (Rhystic-style unless)", () => {
    const graph = analyze("Counter target spell unless its controller pays {1} for each creature you control.", "Instant");
    fully(graph);
    const you = bound(graph, "/multiplier/of/control");
    assert.ok(typeof you !== "string" && you.node === "runtime-player" && you.role === "spell-controller");
  });
});

describe("conditions printed after their instruction (CR 608.2c)", () => {
  it("checks a trailing if on the instruction's participants (Sage-Eye Avengers)", () => {
    const graph = analyze("Whenever this creature attacks, you may return target creature to its owner's hand if its power is less than this creature's power.", "Creature");
    fully(graph);
    assert.equal(bound(graph, "/condition/value/of"), bound(graph, "/steps/0/what"));
  });
  it("does not let the instruction's own result answer its condition (Garna, Bloodfist of Keld)", () => {
    const graph = analyze("Whenever another creature you control dies, draw a card if it was attacking. Otherwise, this creature deals 1 damage to each opponent.", "Creature");
    fully(graph);
    const it = bound(graph, "/condition/what");
    assert.ok(typeof it !== "string" && it.node === "event-result" && it.identity === "same-objects");
  });
  it("finds a discarded card and asks a dead creature's counters as it last existed", () => {
    const reveler = analyze("When this creature enters, draw a card, then discard a card. If you discard a land card this way, put it from your graveyard onto the battlefield tapped.", "Creature");
    fully(reveler);
    assert.ok(bound(reveler, "/effects/1/steps/0/what") !== bound(reveler, "/effects/0/steps/0/who"));
    const squad = analyze("When this creature dies, put its counters on target creature you control.", "Creature");
    fully(squad);
    const counters = bound(squad, "/counters/of");
    assert.ok(typeof counters !== "string" && counters.node === "event-result" && counters.identity === "same-objects");
  });
});

describe("would-be controllers and owners (CR 109.5)", () => {
  const role = (graph: CardBindingAnalysis, suffix: string) => {
    const node = bound(graph, suffix);
    return typeof node !== "string" && node.node === "runtime-player" ? node.role : undefined;
  };
  it("makes a spell's own alternative cost the caster's", () => {
    const graph = analyze("You may sacrifice a Mountain rather than pay this spell's mana cost.\nThis spell deals 3 damage to target creature.", "Instant");
    fully(graph);
    assert.equal(role(graph, "/effect/costs/0/payer"), "prospective-caster");
  });
  it("makes a card's own permission its owner's, and a permanent's permission its controller's", () => {
    const own = analyze("You may cast this card from your graveyard as long as you control a Zombie.", "Creature");
    fully(own);
    assert.equal(role(own, "/permission/condition/who"), "card-owner");
    const crucible = analyze("You may play lands from your graveyard.", "Artifact");
    fully(crucible);
    assert.equal(role(crucible, "/permission/who"), "static-source-controller");
  });
});

describe("defending player (CR 508.5)", () => {
  it("is the player the one attacking creature attacks", () => {
    const graph = analyze("Whenever this creature attacks, defending player loses 1 life and you gain 1 life.", "Creature");
    fully(graph);
    const defending = bound(graph, "/steps/0/who");
    assert.ok(typeof defending !== "string" && defending.node === "derived-player" && defending.role === "defending");
    fully(analyze("This creature can't attack unless defending player controls an Island.", "Creature"));
  });
  it("stays unresolved for a group of attacking creatures (CR 508.5a)", () => {
    unresolved(analyze("Each creature you control can't be blocked as long as defending player controls an artifact land.", "Creature"), "/condition/who");
  });
});

describe("players and objects a trigger or instruction names", () => {
  it("never takes the spell that targeted it for a bare it (Phantasmal Bear), but does for that spell (Frost Titan)", () => {
    const bear = analyze("When this creature becomes the target of a spell or ability, sacrifice it.", "Creature");
    fully(bear);
    assert.notEqual(bound(bear, "/steps/0/what"), graphTargetOf(bear));
    const glasskite = analyze("Whenever this creature becomes the target of a spell or ability for the first time each turn, counter that spell or ability.", "Creature");
    fully(glasskite);
    assert.equal(bound(glasskite, "/steps/0/what"), graphTargetOf(glasskite));
  });
  it("binds that player to a targeted player named inside an instruction (Extract)", () => {
    const graph = analyze("Search target player's library for a card and exile it. Then that player shuffles.", "Sorcery");
    fully(graph);
  });
  it("binds that player to the player a trigger names (Curse of the Bloody Tome, Liability)", () => {
    fully(analyze("Enchant player\nAt the beginning of enchanted player's upkeep, that player mills two cards.", "Enchantment — Aura Curse"));
    fully(analyze("Whenever a nontoken permanent is put into a player's graveyard from the battlefield, that player loses 1 life.", "Enchantment"));
  });
  it("counters the spell a cast trigger is about (Hesitation)", () => {
    fully(analyze("When a player casts a spell, sacrifice this enchantment and counter that spell.", "Enchantment"));
  });
});
function graphTargetOf(graph: CardBindingAnalysis) {
  return graph.nodes.find(node => node.node === "event-result" && node.path.endsWith("/trigger/of"));
}

describe("established descriptions (CR 110.1, 111.1)", () => {
  it("knows looked-at cards are cards, tokens are tokens, and creatures are permanents", () => {
    fully(analyze("Look at the top card of your library. You may exile that card.", "Sorcery"));
    fully(analyze("Create a token that's a copy of target artifact. That token gains haste. Exile it at the beginning of the next end step.", "Sorcery"));
    fully(analyze("This spell deals 5 damage to target creature or planeswalker. That permanent loses indestructible until end of turn.", "Instant"));
  });
  it("keeps the exiled card, not a declined return, for if you don't (Gilraen)", () => {
    const graph = analyze("{2}, {T}: Exile another target creature you control. You may return that card to the battlefield under its owner's control. If you don't, at the beginning of the next end step, return that card to the battlefield under its owner's control with a vigilance counter and a lifelink counter on it.", "Creature");
    fully(graph);
    const card = bound(graph, "/effects/2/steps/0/effects/0/steps/0/what");
    assert.ok(typeof card !== "string" && card.node === "event-result" && card.path.endsWith("/effects/0/steps/0"));
  });
});

describe("groups of unknown size", () => {
  it("binds a plural reference to up to X found cards but not a singular one", () => {
    fully(analyze("Search your library for up to X basic land cards, where X is the greatest power among creatures you control. Put those cards onto the battlefield tapped, then shuffle.", "Sorcery"));
    fully(analyze("{X}: Look at the top X cards of your library, then put them back in any order.", "Enchantment"));
  });
});

describe("engine features beyond reference binding", () => {
  it("binds references and records the engine feature each construct needs", () => {
    for (const [text, typeLine, feature] of [
      ["When this creature enters, if you attacked this turn, draw a card.", "Creature", "event-history"],
      ["Prevent the next 3 damage that would be dealt to any target this turn.", "Instant", "damage-prevention"],
      ["Target land becomes a 3/3 creature with haste until end of turn. It's still a land.", "Instant", "characteristic-layers"],
    ] as const) {
      const graph = analyze(text, typeLine);
      assert.equal(graph.referencesBound, true, `${text}: ${JSON.stringify(graph.diagnostics)}`);
      assert.equal(graph.fullyBound, false, text);
      assert.deepEqual([...new Set(graph.engineFeatures.map(need => need.feature))], [feature], text);
    }
  });
  it("keeps a condition about an earlier instruction of the ability a binding obligation", () => {
    const graph = analyze("Clash with an opponent. If you win, draw a card.", "Sorcery");
    assert.equal(graph.referencesBound, false);
  });
});
