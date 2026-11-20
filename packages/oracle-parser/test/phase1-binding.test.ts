import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { parseOracleText } from "../src/parser/index.ts";
import { analyzeCardBindings } from "../src/semantic/bindings.ts";
import type { CardBindingAnalysis } from "../src/semantic/types.ts";

function analyze(oracleText: string, typeLine: string): CardBindingAnalysis {
  const name = "Fixture";
  const result = parseOracleText(oracleText, name, { typeLine, includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return analyzeCardBindings({
    identity: "fixture", source: { name, type_line: typeLine, layout: "normal" },
    faces: [{ sourceFaceIndex: 0, result }],
  });
}

function bound(graph: CardBindingAnalysis, suffix: string): string {
  const reference = graph.references.find(entry => entry.path.endsWith(suffix));
  assert.ok(reference?.binding === "bound", JSON.stringify(reference ?? graph.diagnostics));
  return reference.target;
}

function boundCard(graph: CardBindingAnalysis): void {
  assert.equal(graph.referencesBound, true, JSON.stringify(graph.diagnostics));
}

describe("counter-change matches", () => {
  it("binds the measured change separately from the actor and recipient", () => {
    const graph = analyze("Whenever you put one or more -1/-1 counters on a creature, "
      + "create that many 1/1 black Insect creature tokens.", "Enchantment");
    boundCard(graph);
    const match = graph.nodes.find(node => node.node === "counter-change-match");
    assert.ok(match && match.actor);
    assert.deepEqual(match.pattern.match, { counters: "one-or-more", recipients: "each" });
    assert.equal(match.actor, bound(graph, "/trigger/by"));
    assert.deepEqual(match.recipients, [bound(graph, "/trigger/on/what")]);
    const count = graph.nodes.find(node => node.id === bound(graph, "/steps/0/count"));
    assert.ok(count?.node === "event-result" && count.identity === "measured-quantity" && count.event === match.event);
    assert.equal(graph.engineFeatures[0]?.feature, "counter-change-event");
  });

  it("does not turn any-of recipients into one guaranteed creature", () => {
    const graph = analyze("Whenever you put one or more counters on a permanent or player, "
      + "put a +1/+1 counter on that creature.", "Enchantment");
    assert.ok(graph.diagnostics.some(diagnostic => diagnostic.code === "missing-antecedent"));
    assert.ok(!graph.nodes.some(node => node.node === "event-result" && node.result === "objects"));
  });

  it("keeps a threshold transition outside ordinary per-recipient counting", () => {
    const graph = analyze("Whenever the last time counter is removed from this creature, draw a card.", "Creature");
    assert.ok(graph.diagnostics.some(diagnostic => diagnostic.code === "unsupported-scope"));
    assert.ok(!graph.nodes.some(node => node.node === "counter-change-match"));
  });
});

describe("payment scope and announced X", () => {
  it("binds an unambiguous activation exclusion and reads the sacrificed object's last known power", () => {
    const graph = analyze("{B}, {T}, Sacrifice another creature: Create X 2/2 black Zombie creature tokens, "
      + "where X is the sacrificed creature's power.", "Creature");
    boundCard(graph);
    assert.equal(bound(graph, "/costs/2/what/filter/other"), graph.nodes.find(node => node.node === "source")?.id);
    assert.equal(bound(graph, "/bindings/0/value/of"), bound(graph, "/costs/2/what"));
  });

  it("rejects an ambiguous exclusion when another object was selected by a cost", () => {
    const graph = analyze("Tap an untapped creature you control, Sacrifice another creature: Draw a card.", "Creature");
    assert.ok(graph.diagnostics.some(diagnostic => diagnostic.code === "unsupported-reference" && diagnostic.path.endsWith("/filter/other")));
  });

  it("does not follow a contextual sacrifice reference into the graveyard as an action", () => {
    const graph = analyze("Sacrifice a creature: Exile the sacrificed creature.", "Artifact");
    assert.ok(graph.diagnostics.some(diagnostic => diagnostic.path.endsWith("/steps/0/what")));
  });

  it("shares the spell's announced X with the spell's additional payment", () => {
    const graph = analyze("As an additional cost to cast this spell, pay X life.\nAll creatures get -X/-X until end of turn.", "Sorcery");
    boundCard(graph);
    const cost = bound(graph, "/costs/0/amount");
    assert.equal(bound(graph, "/power/amount"), cost);
    assert.equal(bound(graph, "/toughness/amount"), cost);
    assert.equal(graph.nodes.filter(node => node.node === "announced-number").length, 1);
  });

  it("keeps a nonzero constraint on the announced activation amount", () => {
    const graph = analyze("{2}{B}, Sacrifice X artifacts: Return target creature card with power X or less "
      + "from your graveyard to the battlefield. X can't be 0.", "Creature");
    boundCard(graph);
    const number = graph.nodes.find(node => node.node === "announced-number");
    assert.ok(number);
    assert.deepEqual(number.constraints, [{ variable: "x", not: 0 }]);
    assert.equal(bound(graph, "/costs/1/what/count"), number.id);
    assert.equal(bound(graph, "/comparison/value"), number.id);
  });

  it("binds a local X definition used by the payment modifier", () => {
    const graph = analyze("{7}{G}, {T}: Create a 4/4 green Wurm creature token. "
      + "This ability costs {X} less to activate, where X is the greatest power among Wurms you control.", "Creature");
    boundCard(graph);
    const number = graph.nodes.find(node => node.node === "defined-number");
    assert.ok(number && number.path.endsWith("/costModifications/0/bindings/0"));
    assert.equal(bound(graph, "/costModifications/0/change/symbols/0"), number.id);
  });
});

describe("manifest result identity", () => {
  it("captures the one possible face-down creature, including when there was no card", () => {
    const graph = analyze("When this Equipment enters, manifest dread, then attach this Equipment to that creature.", "Artifact — Equipment");
    boundCard(graph);
    const result = graph.nodes.find(node => node.id === bound(graph, "/steps/1/to"));
    assert.ok(result?.node === "event-result" && result.identity === "new-objects");
    assert.equal(result.cardinality, "one");
    assert.equal(result.availability, "if-produced");
    assert.deepEqual(result.description, { cls: "permanent", types: ["creature"] });
  });

  it("does not make one manifest result available outside a conditional branch", () => {
    const graph = analyze("When this Equipment enters, you gain 1 life. If you control a creature, manifest dread. Attach this Equipment to that creature.", "Artifact — Equipment");
    assert.ok(graph.diagnostics.some(diagnostic => diagnostic.path.endsWith("/to")));
  });
});

describe("plural pronouns in ability grants", () => {
  it("uses the sole established object group", () => {
    const graph = analyze("Untap all creatures and gain control of them until end of turn. They gain haste until end of turn.", "Sorcery");
    boundCard(graph);
    assert.equal(bound(graph, "/effects/1/steps/0/what"), bound(graph, "/effects/0/steps/0/what"));
  });

  it("rejects competing groups", () => {
    const graph = analyze("Untap all creatures. Tap all artifacts. They gain haste until end of turn.", "Sorcery");
    assert.ok(graph.diagnostics.some(diagnostic => diagnostic.code === "unsupported-reference" && diagnostic.path.endsWith("/what")));
  });
});

describe("stack and text operations", () => {
  it("distinguishes changing the old target from optionally rechoosing targets", () => {
    const change = analyze("Change the target of target spell or ability with a single target.", "Instant");
    const choose = analyze("You may choose new targets for target spell or ability.", "Instant");
    boundCard(change);
    boundCard(choose);
    assert.deepEqual(change.nodes.find(node => node.node === "stack-instruction")?.action,
      { kind: "change-targets", scope: "all" });
    assert.deepEqual(choose.nodes.find(node => node.node === "stack-instruction")?.action,
      { kind: "choose-new-targets", timing: "resolution" });
  });

  it("does not borrow a copy created in a preceding conditional branch", () => {
    const graph = analyze("If you control a creature, copy target spell. You may choose new targets for the copy.", "Instant");
    assert.equal(graph.referencesBound, false);
    assert.ok(graph.references.some(reference => "ref" in reference.expression &&
      reference.expression.ref === "copy-result" && reference.binding === "unresolved"));
  });

  it("does not resolve a bare original-or-copy pronoun by recency", () => {
    const graph = analyze("Copy target instant or sorcery spell, then return it to its owner's hand.", "Instant");
    assert.ok(graph.diagnostics.some(diagnostic => diagnostic.code === "ambiguous-antecedent"));
  });

  it("binds the target and the contextual word choices separately", () => {
    const graph = analyze("Change the text of target permanent by replacing all instances of one color word with another "
      + "or one basic land type with another.", "Instant");
    boundCard(graph);
    const choice = graph.nodes.find(node => node.node === "text-choice");
    assert.ok(choice);
    assert.equal(choice.subject, bound(graph, "/steps/0/what"));
    assert.deepEqual(choice.replacements.map(option => option.category), ["color-word", "basic-land-type"]);
    assert.equal(graph.engineFeatures[0]?.feature, "text-change");
  });
});

describe("private information and player roles", () => {
  it("binds the viewer and library owner, and defers refreshing the private look", () => {
    const graph = analyze("You may look at the top card of your library any time.", "Artifact");
    boundCard(graph);
    const look = graph.nodes.find(node => node.node === "private-library-look");
    assert.ok(look);
    assert.equal(look.viewer, look.libraryOwner);
    assert.equal(look.viewer, bound(graph, "/permission/who"));
    assert.equal(look.refresh, "after-announcement-or-special-action");
    assert.equal(graph.engineFeatures[0]?.feature, "private-library-look");
  });

  it("binds a cast trigger's possessive zone to the matching caster", () => {
    const graph = analyze("Whenever a player casts a spell from their hand, that player draws a card.", "Artifact");
    boundCard(graph);
    assert.equal(bound(graph, "/trigger/from/owner"), bound(graph, "/trigger/who"));
  });

  it("retains your library or hand as a separate role from another caster", () => {
    const graph = analyze("Whenever a player casts a spell from your hand, that player draws a card.", "Artifact");
    boundCard(graph);
    assert.notEqual(bound(graph, "/trigger/from/owner"), bound(graph, "/trigger/who"));
  });

  it("recognizes repeated ownership of the same announced object", () => {
    const graph = analyze("The owner of target permanent shuffles it into their library, then reveals the top card "
      + "of their library. If it's a permanent card, they put it onto the battlefield.", "Instant");
    boundCard(graph);
    const player = graph.nodes.find(node => node.id === bound(graph, "/effects/1/steps/0/who"));
    assert.ok(player?.node === "selection" && "player" in player.expression && player.expression.player === "owner");
    assert.equal(bound(graph, player.path + "/of"), bound(graph, "/effects/0/steps/0/who/of"));
  });

  it("does not identify owners of independently chosen objects", () => {
    const graph = analyze("The owner of target permanent draws a card. The owner of target artifact loses 1 life. "
      + "They gain 1 life.", "Instant");
    assert.ok(graph.diagnostics.some(diagnostic => diagnostic.code === "ambiguous-antecedent"));
  });
});

describe("counts and public-zone successor captures", () => {
  it("keeps a resolution choice separate from the number actually discarded", () => {
    const graph = analyze("Discard any number of cards, then draw that many cards.", "Sorcery");
    boundCard(graph);
    const choice = graph.nodes.find(node => node.node === "resolution-count");
    assert.ok(choice && choice.operation === "discard" && choice.minimum === 0);
    const measured = graph.nodes.find(node => node.id === bound(graph, "/steps/1/amount"));
    assert.ok(measured?.node === "event-result" && measured.identity === "measured-quantity");
    assert.notEqual(measured.id, choice.id);
  });

  it("announces bounded ordinary targets without supporting divided distribution", () => {
    const graph = analyze("Put one, two, or three target creature cards from graveyards onto the battlefield under your control.", "Sorcery");
    boundCard(graph);
    const count = graph.nodes.find(node => node.node === "target-count");
    assert.deepEqual([count?.minimum, count?.maximum], [1, 3]);
    const divided = analyze("Distribute two +1/+1 counters among one or two target creatures.", "Instant");
    assert.ok(!divided.nodes.some(node => node.node === "target-count"));
    assert.ok(divided.diagnostics.some(diagnostic => diagnostic.code === "unsupported-number-binding"));
  });

  it("uses the actual exile successors in a later permission", () => {
    const graph = analyze("Exile the top five cards of your library. You may play cards exiled this way until the end of your next turn.", "Sorcery");
    boundCard(graph);
    const successor = graph.nodes.find(node => node.id === bound(graph, "/permission/what"));
    assert.ok(successor?.node === "event-result" && successor.identity === "zone-successors");
    assert.notEqual(successor.id, bound(graph, "/effects/0/steps/0/what"));
  });

  it("does not choose a producer when another exile might have supplied the card", () => {
    const graph = analyze("Exile the top card of your library. If you control a creature, exile the top card of your library. "
      + "You may play the exiled card this turn.", "Sorcery");
    assert.ok(graph.references.some(reference => reference.path.endsWith("/permission/what") && reference.binding === "unresolved"));
  });

  it("finds the public successor of the card that paid an exile cost", () => {
    const graph = analyze("{2}, Exile a nonland card from your hand: Put four time counters on the exiled card.", "Creature");
    boundCard(graph);
    const successor = graph.nodes.find(node => node.id === bound(graph, "/steps/0/on"));
    assert.ok(successor?.node === "event-result" && successor.identity === "zone-successors");
    assert.notEqual(successor.id, bound(graph, "/costs/1/what"));
  });
});

describe("recipient-dependent damage amounts", () => {
  it("uses each recipient's own count for the simultaneous event", () => {
    const graph = analyze("This spell deals damage to each player equal to twice the number of nonbasic lands that player controls.", "Instant");
    boundCard(graph);
    const member = graph.nodes.find(node => node.node === "recipient-member");
    assert.ok(member);
    assert.equal(member.group, bound(graph, "/to/ref"));
    assert.equal(member.id, bound(graph, "/of/control"));
    assert.equal(graph.engineFeatures[0]?.feature, "recipient-dependent-damage");
  });

  it("does not turn an object-or-player damage recipient into a player", () => {
    const graph = analyze("This spell deals damage to any target equal to the number of artifacts that player controls.", "Instant");
    assert.ok(!graph.nodes.some(node => node.node === "recipient-member"));
    assert.ok(graph.diagnostics.some(diagnostic => diagnostic.code === "missing-antecedent"));
  });
});

describe("characteristic comparisons and local continuous rules", () => {
  it("tests a named target against a current matching set", () => {
    const graph = analyze("Counter target spell if it shares a color with a creature you control.", "Instant");
    boundCard(graph);
    const predicate = graph.nodes.find(node => node.node === "characteristic-predicate");
    assert.ok(predicate?.operand.kind === "matching-set");
    assert.equal(predicate.subject, bound(graph, "/steps/0/what"));
    assert.equal(predicate.operand.quantifier, "any");
    assert.equal(predicate.timing, "resolution");
  });

  it("leaves a comparison with a missing subject unresolved", () => {
    const graph = analyze("If it shares a color with a creature you control, draw a card.", "Instant");
    assert.ok(!graph.nodes.some(node => node.node === "characteristic-predicate"));
    assert.ok(graph.diagnostics.some(diagnostic => diagnostic.code === "missing-antecedent"));
  });

  it("tests a common name without selecting any of the lands", () => {
    const graph = analyze("{2}, {T}: Draw a card. Activate only if you control three or more lands with the same name.", "Artifact");
    boundCard(graph);
    const predicate = graph.nodes.find(node => node.node === "common-name-subset");
    assert.ok(predicate && predicate.minimum === 3);
    assert.ok(!graph.nodes.some(node => node.node === "selection" && node.path.startsWith(predicate.path)));
  });

  it("binds a static counter multiplier to its individual affected object", () => {
    const graph = analyze("This creature gets +1/+1 for each time counter on it.", "Creature");
    boundCard(graph);
    const member = graph.nodes.find(node => node.node === "characteristic-subject" && node.role === "affected-member");
    assert.ok(member?.node === "characteristic-subject");
    assert.equal(member.id, bound(graph, "/multiplier/on"));
    assert.equal(member.of, bound(graph, "/effect/what"));
  });

  it("binds untap of an enchanted land relative to that land's controller", () => {
    const graph = analyze("Enchant land\nEnchanted land has \"Untap this land during each other player's untap step.\"", "Enchantment — Aura");
    boundCard(graph);
    const owners = graph.nodes.find(node => node.id === bound(graph, "/step/owner"));
    assert.ok(owners?.node === "derived-player" && owners.role === "other-players");
    const scope = graph.nodes.find(node => node.id === owners.scope);
    assert.ok(scope?.node === "ability" && scope.source !== graph.nodes.find(node => node.node === "source")?.id);
    assert.equal(scope.you, owners.of);
  });

  it("binds a global prevention prohibition without making it a shield", () => {
    const graph = analyze("Damage can't be prevented.", "Enchantment");
    boundCard(graph);
    assert.ok(graph.nodes.some(node => node.node === "static-rule" && node.rule.static === "prohibit"));
    assert.deepEqual(graph.engineFeatures.map(entry => entry.feature), ["damage-prevention"]);
  });

  it("models retained mana as a change of type for the bound player", () => {
    const graph = analyze("If you would lose unspent mana, that mana becomes black instead.", "Creature");
    boundCard(graph);
    const rule = graph.nodes.find(node => node.node === "keep-unspent-mana");
    assert.ok(rule);
    assert.equal(rule.player, bound(graph, "/replacement/event/who"));
    assert.equal(rule.symbol, "B");
    assert.equal(graph.fullyBound, false);
  });
});

describe("sequential library scans and temporary card copies", () => {
  it("records an ordered scan's pool and matching card as different results", () => {
    const graph = analyze("Exile cards from the top of your library until you exile a nonland card. "
      + "You may cast that card without paying its mana cost.", "Sorcery");
    boundCard(graph);
    const scan = graph.nodes.find(node => node.node === "library-scan");
    assert.ok(scan && scan.operation === "exile");
    const match = graph.nodes.find(node => node.id === scan.matches);
    assert.ok(match?.node === "event-result" && match.subsetOf === scan.pool && match.identity === "zone-successors");
    assert.equal(match.id, bound(graph, "/instruction/what"));
    assert.notEqual(scan.pool, scan.matches);
  });

  it("keeps the rest of a reveal scan in the original library pool", () => {
    const graph = analyze("Reveal cards from the top of your library until you reveal a land card. "
      + "Put that card onto the battlefield and the rest on the bottom of your library in a random order.", "Sorcery");
    boundCard(graph);
    const scan = graph.nodes.find(node => node.node === "library-scan");
    const rest = graph.nodes.find(node => node.node === "remainder");
    assert.ok(scan && rest);
    assert.equal(scan.operation, "reveal");
    assert.equal(rest.pool, scan.pool);
    assert.equal(rest.excluding, scan.matches);
  });

  it("rejects a scan result outside the branch that produced it", () => {
    const graph = analyze("If you control a creature, exile cards from the top of your library until you exile a nonland card. "
      + "You may cast that card without paying its mana cost.", "Sorcery");
    assert.ok(graph.references.some(reference => reference.path.endsWith("/instruction/what") && reference.binding === "unresolved"));
  });

  it("copies an exiled card before any of the resulting copies is cast", () => {
    const graph = analyze("Exile target instant or sorcery card from a graveyard. Copy that card three times. "
      + "You may cast the copies without paying their mana costs.", "Sorcery");
    boundCard(graph);
    const copy = graph.nodes.find(node => node.node === "card-copy");
    assert.ok(copy && copy.count === 3);
    assert.equal(copy.card, bound(graph, "/effects/1/steps/0/what"));
    assert.equal(copy.result, bound(graph, "/instruction/what"));
    assert.notEqual(copy.result, copy.card);
  });

  it("does not equate a copied card with a missing prior exile", () => {
    const graph = analyze("{2}, {T}: You may copy the exiled card. If you do, you may cast the copy without paying its mana cost.", "Artifact");
    assert.equal(graph.referencesBound, false);
    assert.ok(!graph.nodes.some(node => node.node === "card-copy"));
  });
});

describe("group instructions and event quantities", () => {
  it("draws for each printed player in APNAP order", () => {
    const graph = analyze("{T}: You and target opponent each draw a card.", "Creature");
    boundCard(graph);
    const group = graph.nodes.find(node => node.node === "group-instruction");
    assert.ok(group?.execution.kind === "draw");
    assert.deepEqual(group.execution.players, [bound(graph, "/members/0"), bound(graph, "/members/1")]);
    assert.equal(group.execution.order, "apnap");
  });

  it("associates each announced target with its own player member", () => {
    const graph = analyze("When this enchantment enters, for each opponent, exile up to one target nonland permanent "
      + "that player controls until this enchantment leaves the battlefield.", "Enchantment");
    boundCard(graph);
    const group = graph.nodes.find(node => node.node === "group-instruction");
    const target = graph.nodes.find(node => node.node === "selection" && node.selection === "target");
    assert.ok(group?.execution.kind === "for-each" && target?.node === "selection");
    assert.equal(group.execution.membership, "announcement");
    assert.equal(target.perMember, group.execution.member);
    assert.equal(bound(graph, "/filter/control"), group.execution.member);
  });

  it("does not export an individual copied token from a group", () => {
    const name = "Fixture";
    const source = { name, layout: "normal", type_line: "Instant" };
    const parse = (text: string) => parseOracleText(text, name, { typeLine: source.type_line, includeSourceProvenance: true });
    const loop = parse("For each token you control, create a token that's a copy of that permanent.");
    const after = parse("It gains haste until end of turn.");
    assert.ok(loop.ok && after.ok);
    const [spell] = loop.abilities;
    const [continuation] = after.abilities;
    assert.ok(spell.kind === "spell" && continuation.kind === "spell");
    spell.effects.push(...continuation.effects);
    const graph = analyzeCardBindings({ identity: "fixture", source, faces: [{ sourceFaceIndex: 0, result: loop }] });
    assert.ok(graph.nodes.some(node => node.node === "group-instruction"));
    assert.ok(graph.diagnostics.some(diagnostic => diagnostic.code === "missing-antecedent"));
  });

  it("sums actual damage from the bound event across its recipients", () => {
    const graph = analyze("This spell deals 1 damage to each opponent. You gain life equal to the damage dealt this way.", "Instant");
    boundCard(graph);
    const measure = graph.nodes.find(node => node.node === "event-aggregate");
    assert.ok(measure && measure.measure === "damage-dealt");
    assert.ok(graph.nodes.some(node => node.id === measure.event && node.node === "event" && node.operation === "damage"));
  });

  it("rejects competing damage events instead of choosing by proximity", () => {
    const graph = analyze("This spell deals 1 damage to each opponent. This spell deals 2 damage to each opponent. "
      + "You gain life equal to the damage dealt this way.", "Instant");
    assert.ok(!graph.nodes.some(node => node.node === "event-aggregate"));
    assert.ok(graph.diagnostics.some(diagnostic => diagnostic.code === "unsupported-scope"));
  });

  it("uses the recorded monstrosity amount for X", () => {
    const graph = analyze("{X}{X}{R}: Monstrosity X.\nWhen this creature becomes monstrous, goad up to X target creatures your opponents control.", "Creature");
    boundCard(graph);
    const target = bound(graph, "/steps/0/what/count");
    assert.ok(graph.nodes.some(node => node.id === target && node.node === "event-result" && node.identity === "measured-quantity"));
    assert.equal(graph.engineFeatures[0]?.feature, "monstrosity-event");
  });

  it("keeps the draw-step actor for both branches of a later instruction", () => {
    const graph = analyze("At the beginning of each player's draw step, that player exiles the top card of their library. "
      + "If it's a land card, the player puts it onto the battlefield. Otherwise, the player casts it without paying its mana cost if able.", "Artifact");
    boundCard(graph);
    assert.equal(bound(graph, "/effects/1/steps/0/who"), bound(graph, "/otherwise/steps/0/instruction/who"));
    assert.equal(bound(graph, "/effects/0/steps/0/who"), bound(graph, "/effects/1/steps/0/who"));
  });
});

describe("entry choices, linked pools, and emblem sources", () => {
  it("chooses a battlefield model before entry without announcing a target", () => {
    const graph = analyze("You may have this artifact enter as a copy of any artifact on the battlefield.", "Artifact");
    boundCard(graph);
    const choice = graph.nodes.find(node => node.node === "entry-copy-model-choice");
    assert.ok(choice && choice.selection === "non-target" && choice.timing === "before-entry");
    const player = graph.nodes.find(node => node.id === choice.chooser);
    assert.ok(player?.node === "runtime-player" && player.role === "entering-controller");
    assert.equal(graph.engineFeatures[0]?.feature, "entry-copy-replacement");
  });

  it("keeps the stated copy duration and haste exception separate from the model", () => {
    const graph = analyze("You may have this artifact enter as a copy of any creature on the battlefield until end of turn, "
      + "except it has haste.", "Artifact");
    boundCard(graph);
    const copy = graph.nodes.find(node => node.node === "entry-copy-replacement");
    assert.ok(copy);
    assert.deepEqual(copy.duration, { duration: "end-of-turn" });
    assert.equal(copy.changes?.[0]?.change, "abilities");
  });

  it("matches the name of any card retained by the one printed imprint producer", () => {
    const graph = analyze("Imprint — When this artifact enters, you may exile target land you control.\n"
      + "Whenever a land with the same name as the exiled card is tapped for mana, its controller adds one mana of any type that land produced.", "Artifact");
    boundCard(graph);
    const pool = graph.nodes.find(node => node.node === "linked-exile-pool");
    const comparison = graph.nodes.find(node => node.node === "characteristic-predicate");
    assert.ok(pool && comparison?.operand.kind === "linked-exile-names");
    assert.equal(pool.identity, "source-incarnation");
    assert.equal(comparison.operand.pool, pool.id);
    assert.equal(comparison.operand.quantifier, "any");
  });

  it("refuses to assign a linked name to either of two eligible producers", () => {
    const graph = analyze("Imprint — When this artifact enters, you may exile target land you control.\n"
      + "{2}: Exile target land.\nWhenever a land with the same name as the exiled card is tapped for mana, draw a card.", "Artifact");
    assert.ok(!graph.nodes.some(node => node.node === "linked-exile-pool"));
    assert.equal(graph.referencesBound, false);
  });

  it("assigns a quoted ability to the emblem controlled by the recipient", () => {
    const graph = analyze("+1: You get an emblem with \"Whenever you cast a spell, draw a card.\"", "Planeswalker");
    boundCard(graph);
    const emblem = graph.nodes.find(node => node.node === "recipient" && node.emblem);
    assert.ok(emblem?.node === "recipient" && emblem.emblem);
    assert.equal(emblem.emblem.zone, "command");
    assert.equal(emblem.emblem.controller, bound(graph, "/steps/0/who"));
    assert.ok(graph.nodes.some(node => node.node === "ability" && node.source === emblem.id));
  });

  it("keeps a fixed blight payment in the payer's scope", () => {
    const graph = analyze("Blight 2: Draw a card.", "Creature");
    boundCard(graph);
    const payment = graph.nodes.find(node => node.node === "blight-payment");
    assert.ok(payment);
    assert.equal(payment.payer, bound(graph, "/costs/0/payer"));
    assert.equal(payment.count, 2);
  });
});
