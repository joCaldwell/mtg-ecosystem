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

  it("keeps retargeting in the immediately associated copy instruction", () => {
    const adjacent = analyze("Copy target spell. You may choose new targets for the copy.", "Instant");
    boundCard(adjacent);
    assert.ok(adjacent.nodes.some(node => node.node === "stack-instruction" &&
      node.action.kind === "copy" && node.action.retarget));

    const intervening = analyze("Copy target spell. Draw a card. You may choose new targets for the copy.", "Instant");
    assert.equal(intervening.referencesBound, false);
    assert.ok(!intervening.nodes.some(node => node.node === "stack-instruction" &&
      node.action.kind === "choose-new-targets" && node.action.timing === "copy-creation"));
  });

  it("rejects a text choice whose enclosing optional decision would be lost", () => {
    const graph = analyze("You may change the text of target permanent by replacing all instances of one color word "
      + "with another and draw a card.", "Instant");
    assert.equal(graph.referencesBound, false);
    assert.ok(!graph.nodes.some(node => node.node === "text-choice"));
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

describe("target eligibility at announcement and resolution", () => {
  it("uses source information even after that source is sacrificed to pay for the ability", () => {
    const graph = analyze("{T}, Sacrifice this creature: Destroy target creature that shares a color with this creature.", "Creature");
    boundCard(graph);
    const predicate = graph.nodes.find(node => node.node === "characteristic-predicate");
    assert.ok(predicate?.timing === "target-legality" && predicate.operand.kind === "object");
    assert.equal(predicate.operand.information, "current-or-last-known");
    assert.ok(predicate.rules.includes("608.2b"));
  });

  it("excludes the evaluated target from a comparison with another creature", () => {
    const graph = analyze("{T}: Destroy target creature with the same name as another creature you control.", "Creature");
    boundCard(graph);
    const predicate = graph.nodes.find(node => node.node === "characteristic-predicate");
    assert.ok(predicate);
    assert.equal(bound(graph, "/with/filter/other"), predicate.subject);
    assert.ok(graph.nodes.some(node => node.id === predicate.subject && node.node === "characteristic-subject"));
  });

  it("excludes the entering creature from the battlefield alternative at both intervening-if checks", () => {
    const graph = analyze("Whenever a nontoken creature you control enters, if it doesn't have the same name "
      + "as another creature you control or a creature card in your graveyard, draw a card.", "Enchantment");
    boundCard(graph);
    const predicate = graph.nodes.find(node => node.node === "characteristic-predicate");
    assert.ok(predicate?.timing === "intervening-if");
    assert.equal(bound(graph, "/with/filter/anyOf/0/other"), predicate.subject);
    assert.ok(!graph.references.some(reference => reference.path.endsWith("/with/filter/anyOf/1/other")));
  });

  it("reevaluates a matching set without snapshotting it at announcement", () => {
    const graph = analyze("Destroy target creature that shares a color with a creature you control.", "Instant");
    boundCard(graph);
    const predicate = graph.nodes.find(node => node.node === "characteristic-predicate");
    assert.ok(predicate?.timing === "target-legality" && predicate.operand.kind === "matching-set");
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

  it("rejects a scan whose enclosing optional decision would be lost", () => {
    const graph = analyze("You may reveal cards from the top of your library until you reveal a land card "
      + "and put it into your hand.", "Sorcery");
    assert.equal(graph.referencesBound, false);
    assert.ok(!graph.nodes.some(node => node.node === "library-scan"));
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

  it("does not ignore a qualifier on the linked name comparison", () => {
    const graph = analyze("Imprint — When this artifact enters, you may exile target land you control.\n"
      + "Whenever a land with the same name as the exiled creature card is tapped for mana, draw a card.", "Artifact");
    assert.equal(graph.referencesBound, false);
    assert.ok(!graph.nodes.some(node => node.node === "characteristic-predicate" && node.operand.kind === "linked-exile-names"));
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

describe("explicit timing, exclusions, and special instruction operands", () => {
  it("anchors a bounded draw to the next turn's upkeep and its own acting player", () => {
    const graph = analyze("Counter target spell. Its controller may draw up to two cards at the beginning of the next turn's upkeep.\n"
      + "You draw a card at the beginning of the next turn's upkeep.", "Instant");
    boundCard(graph);
    const phase = graph.nodes.filter(node => node.node === "scheduled-phase");
    assert.equal(phase.length, 2);
    assert.ok(phase.every(node => node.turn === "next" && node.phase === "upkeep"));
    const draw = graph.nodes.find(node => node.node === "resolution-count");
    assert.ok(draw?.operation === "draw");
    assert.equal(draw.chooser, bound(graph, "/effects/0/steps/0/who"));
    assert.equal(draw.maximum, 2);
  });

  it("acts on a successfully countered card's new exile object", () => {
    const graph = analyze("Counter target spell. If the spell is countered this way, exile it with three time counters on it "
      + "instead of putting it into its owner's graveyard. If it doesn't have suspend, it gains suspend.", "Instant");
    boundCard(graph);
    const successor = graph.nodes.find(node => node.id === bound(graph, "/effects/1/steps/0/what"));
    assert.ok(successor?.node === "event-result" && successor.identity === "zone-successors");
    assert.notEqual(successor.id, bound(graph, "/effects/0/steps/0/what"));
    assert.equal(graph.engineFeatures[0]?.feature, "counter-destination");
  });

  it("keeps the announced cards distinct from the later sacrifice condition", () => {
    const graph = analyze("Choose two target creature cards in your graveyard. Sacrifice a creature. "
      + "If you do, return the chosen cards to the battlefield tapped.", "Sorcery");
    boundCard(graph);
    const gate = graph.nodes.find(node => node.node === "action-cost-condition");
    assert.ok(gate);
    assert.ok(graph.nodes.some(node => node.id === gate.event && node.node === "event" && node.operation === "sacrifice"));
    assert.equal(bound(graph, "/effects/2/steps/0/what"), bound(graph, "/effects/0/steps/0/what"));
  });

  it("preserves competing resolution choices instead of assuming the announced target", () => {
    const graph = analyze("Choose target creature. Choose a creature you control. Destroy the chosen creature.", "Sorcery");
    assert.ok(graph.references.some(reference => "ref" in reference.expression &&
      reference.expression.ref === "choice-result" && reference.binding === "unresolved"));
  });

  it("evaluates hexproof exceptions when targets are checked", () => {
    const graph = analyze("{1}, {T}: Until end of turn, your opponents and creatures your opponents control with hexproof "
      + "can be the targets of spells and abilities you control as though they didn't have hexproof.", "Land");
    boundCard(graph);
    const permission = graph.nodes.find(node => node.node === "targeting-permission");
    assert.ok(permission && permission.evaluation === "each-target-legality-check");
    assert.ok(graph.nodes.some(node => permission.affected.includes(node.id) && node.node === "selection" &&
      node.timing === "target-legality" && "ref" in node.expression && node.expression.ref === "each"));
  });

  it("does not link a condition to an unrelated earlier sacrifice", () => {
    const graph = analyze("Sacrifice a creature. Draw a card. If you do, gain 1 life.", "Sorcery");
    assert.ok(!graph.nodes.some(node => node.node === "action-cost-condition"));
    assert.ok(graph.diagnostics.some(diagnostic => diagnostic.code === "unsupported-reference"));
  });

  it("uses the player targeted by the explicit counter condition", () => {
    const graph = analyze("If target player has fewer than nine poison counters, they get a number of poison counters equal to the difference.", "Sorcery");
    boundCard(graph);
    const target = bound(graph, "/condition/who");
    assert.equal(bound(graph, "/steps/0/who"), target);
    assert.equal(bound(graph, "/count/b/on"), target);
  });

  it("excludes only the exact copy model from the resolving group", () => {
    const graph = analyze("Choose target artifact you control. Each other artifact you control becomes a copy of that artifact until end of turn.", "Instant");
    boundCard(graph);
    assert.equal(bound(graph, "/what/filter/other"), bound(graph, "/spec/copyOf"));
    assert.equal(bound(graph, "/spec/copyOf"), bound(graph, "/effects/0/steps/0/what"));
  });

  it("binds a value exchange to one current player and one creature", () => {
    const graph = analyze("{T}: Exchange target opponent's life total with this creature's toughness.", "Creature");
    boundCard(graph);
    const exchange = graph.nodes.find(node => node.node === "value-exchange");
    assert.ok(exchange);
    assert.deepEqual(exchange.subjects, [bound(graph, "/a/of"), bound(graph, "/b/of")]);
  });
});

describe("activation and declaration actors", () => {
  it("charges the actual activator for the chosen payment alternative", () => {
    const graph = analyze("Pay 2 life or {2}: Counter target red or green spell. Any player may activate this ability.", "Enchantment");
    boundCard(graph);
    const payment = graph.nodes.find(node => node.node === "payment-alternatives");
    const permission = graph.nodes.find(node => node.node === "activation-permission");
    assert.ok(payment && permission);
    assert.equal(permission.actors, "any-player");
    assert.equal(bound(graph, "/options/0/0/payer"), payment.payer);
    assert.equal(bound(graph, "/options/1/0/payer"), payment.payer);
    assert.notEqual(permission.source, payment.payer);
  });

  it("binds the attack tax to the proposed declaring player", () => {
    const graph = analyze("Nonblack creatures can't attack you unless their controller pays {2} for each creature they control "
      + "that's attacking you.", "Enchantment");
    boundCard(graph);
    const payment = graph.nodes.find(node => node.node === "attack-payment");
    assert.ok(payment);
    assert.equal(bound(graph, "/rule/who"), payment.payer);
    assert.equal(bound(graph, "/rule/multiplier/of/controlledBy"), payment.payer);
    assert.notEqual(bound(graph, "/rule/defenders/0/who"), payment.payer);
  });

  it("binds intrinsic Class payment and the level-specific trigger separately", () => {
    const graph = analyze("Whenever one or more tokens you control enter, draw a card. This ability triggers only once each turn.\n"
      + "{W}: Level 2\nWhen this Class becomes level 2, create a token that's a copy of target token you control.\n"
      + "{3}{W}: Level 3\nCreature tokens you control get +2/+2.", "Enchantment — Class");
    boundCard(graph);
    const levels = graph.nodes.filter(node => node.node === "class-level");
    assert.deepEqual(levels.map(node => [node.level, node.requiresLevel, node.timing]), [[2, 1, "sorcery"], [3, 2, "sorcery"]]);
    assert.ok(graph.nodes.some(node => node.node === "event" && node.operation === "trigger:becomes-class-level" && levels[0].abilities.includes(node.scope)));
  });

  it("does not invent an intrinsic Class level for an unrelated trigger", () => {
    const graph = analyze("When this Class becomes level 2, draw a card.", "Enchantment — Class");
    assert.equal(graph.referencesBound, false);
    assert.ok(!graph.nodes.some(node => node.node === "class-level"));
  });

  it("tests the whole attack declaration against the stated defender", () => {
    const graph = analyze("Whenever you attack the player who has the initiative, create a Treasure token.", "Enchantment");
    boundCard(graph);
    const attack = graph.nodes.find(node => node.node === "attack-declaration");
    assert.ok(attack);
    assert.equal(attack.actor, bound(graph, "/trigger/who"));
    assert.equal(attack.defender, bound(graph, "/trigger/whom"));
  });

  it("proves an explicit union qualification only when every alternative qualifies", () => {
    const prefix = "{T}: Choose target permanent you control or suspended card you own.";
    const accepted = analyze(`${prefix} If that permanent or card has a time counter on it, you may remove a time counter from it or put another time counter on it.`,
      "Artifact Creature");
    boundCard(accepted);
    const rejected = analyze(`${prefix} If that permanent has a time counter on it, draw a card.`, "Artifact Creature");
    assert.ok(rejected.diagnostics.some(diagnostic => diagnostic.code === "unsupported-reference"));
  });
});

describe("per-event replacements and prevention", () => {
  it("links an exile replacement to the eligible move and only the cards exiled by it", () => {
    const graph = analyze("If a nontoken creature an opponent controls would die, instead exile it and you gain 2 life.\n"
      + "{4}{B}{B}, {T}: Put target creature card exiled with this artifact onto the battlefield tapped under your control "
      + "with two additional +1/+1 counters on it.", "Artifact");
    boundCard(graph);
    const replacement = graph.nodes.find(node => node.node === "exile-replacement");
    const pool = graph.nodes.find(node => node.node === "linked-exile-pool");
    assert.ok(replacement && pool);
    assert.equal(replacement.object, bound(graph, "/replace/effects/0/steps/0/what"));
    assert.equal(pool.event, replacement.exileEvent);
    assert.equal(pool.identity, "source-incarnation");
    assert.ok(pool.rules.includes("607.2b"));
  });

  it("fires a reflexive token trigger only for the exile that actually happened", () => {
    const graph = analyze("If a creature an opponent controls would die, exile it instead. "
      + "When you do, create a 1/1 green Saproling creature token.", "Creature");
    boundCard(graph);
    const replacement = graph.nodes.find(node => node.node === "exile-replacement");
    const trigger = graph.nodes.find(node => node.node === "performed-action-trigger");
    assert.ok(replacement?.reflexiveTrigger && trigger);
    assert.equal(trigger.event, replacement.exileEvent);
    assert.equal(trigger.outcome, "actually-exiled");
  });

  it("leaves distinct exile producers unlinked", () => {
    const graph = analyze("If a nontoken creature an opponent controls would die, instead exile it and you gain 2 life.\n"
      + "{2}: Exile target creature.\n{T}: Put target creature card exiled with this artifact onto the battlefield.",
      "Artifact");
    assert.equal(graph.referencesBound, false);
    assert.ok(!graph.nodes.some(node => node.node === "linked-exile-pool"));
  });

  it("requires a unique eligible object recipient for a source exclusion", () => {
    const name = "Fixture";
    const typeLine = "Artifact — Equipment";
    const result = parseOracleText("If another creature would deal combat damage to equipped creature, "
      + "it deals double that damage to equipped creature instead.", name, { typeLine, includeSourceProvenance: true });
    assert.ok(result.ok);
    const [ability] = result.abilities;
    assert.ok(ability.kind === "static" && ability.effect.static === "replacement");
    const event = ability.effect.replacement.event;
    assert.ok(event.event === "damage" && event.to);
    event.to.push({ player: "you" });
    const graph = analyzeCardBindings({ identity: "fixture", source: { name, type_line: typeLine, layout: "normal" },
      faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(graph.referencesBound, false);
    assert.ok(!graph.nodes.some(node => node.node === "event-participant"));
  });

  it("treats an explicit move to exile and airbend as competing linked producers", () => {
    for (const second of ["Put target creature card from your graveyard into exile.", "Airbend target creature."]) {
      const graph = analyze("When this artifact enters, exile target creature.\n"
        + "{T}: " + second + "\n{2}: Put target creature card exiled with this artifact onto the battlefield.", "Artifact");
      assert.equal(graph.referencesBound, false);
      assert.ok(!graph.nodes.some(node => node.node === "linked-exile-pool"));
    }
  });

  it("applies the counter follow-up to the same recipient and the actual prevention amount", () => {
    const graph = analyze("If damage would be dealt to another creature you control, prevent that damage. "
      + "Put a +1/+1 counter on that creature for each 1 damage prevented this way.", "Creature");
    boundCard(graph);
    const prevention = graph.nodes.find(node => node.node === "prevention-outcome");
    assert.ok(prevention);
    assert.equal(prevention.recipient, bound(graph, "/after/0/steps/0/on"));
    assert.equal(prevention.prevented, bound(graph, "/after/0/steps/0/count"));
    assert.ok(graph.engineFeatures.some(feature => feature.feature === "damage-prevention" && feature.path.endsWith("/count")));
  });

  it("leaves prevention references unresolved outside the prevention follow-up", () => {
    const graph = analyze("Prevent all damage that would be dealt to target creature this turn. "
      + "Put a +1/+1 counter on that creature for each 1 damage prevented this way.", "Instant");
    assert.equal(graph.referencesBound, false);
    assert.ok(!graph.nodes.some(node => node.node === "prevention-outcome"));
  });
});

describe("exact choice, payment, and zone-trigger identities", () => {
  it("binds a resolution choice limited to permanent types", () => {
    const graph = analyze("Choose a permanent type. Return all cards of the chosen type from your graveyard to your hand.", "Sorcery");
    boundCard(graph);
    const choice = graph.nodes.find(node => node.node === "value-choice");
    assert.ok(choice?.within === "permanent-types");
    assert.equal(bound(graph, "/chosenType"), choice.id);
  });

  it("does not let a branch-local permanent type supply the choice afterward", () => {
    const graph = analyze("If you control a creature, choose a permanent type. Return all cards of the chosen type "
      + "from your graveyard to your hand.", "Sorcery");
    assert.equal(graph.referencesBound, false);
  });

  it("requires the exact earlier discard for an if-you-do condition", () => {
    const graph = analyze("Discard a card. If you do, draw a card.", "Sorcery");
    boundCard(graph);
    const gate = graph.nodes.find(node => node.node === "action-cost-condition");
    assert.ok(gate?.action === "discard");
    assert.ok(graph.nodes.some(node => node.node === "event" && node.id === gate.event && node.operation === "discard"));

    const wrong = analyze("Discard a card. Gain 1 life. If you do, draw a card.", "Sorcery");
    assert.equal(wrong.referencesBound, false);
    assert.ok(!wrong.nodes.some(node => node.node === "action-cost-condition"));
  });

  it("reads the cast trigger's actual mana payment", () => {
    const graph = analyze("Whenever you cast a red spell, this enchantment deals X damage to any target, "
      + "where X is the amount of mana spent to cast that spell.", "Enchantment");
    boundCard(graph);
    const paid = graph.nodes.find(node => node.node === "cast-mana-paid");
    assert.ok(paid);
    assert.equal(paid.occurrence, bound(graph, "/bindings/0/value/what"));
    assert.ok(graph.nodes.some(node => node.id === paid.occurrence && node.node === "event-result"));

    const wrong = analyze("This enchantment deals X damage to any target, "
      + "where X is the amount of mana spent to cast that spell.", "Sorcery");
    assert.equal(wrong.referencesBound, false);
    assert.ok(!wrong.nodes.some(node => node.node === "cast-mana-paid"));
  });

  it("does not assign contextual cast payment past a competing spell", () => {
    const graph = analyze("Whenever you cast a red spell, counter target spell. This enchantment deals X damage to any target, "
      + "where X is the amount of mana spent to cast that spell.", "Enchantment");
    assert.equal(graph.referencesBound, false);
    assert.ok(!graph.nodes.some(node => node.node === "cast-mana-paid"));
  });

  it("finds the successor for the specific battlefield departure that triggered", () => {
    const graph = analyze("When this creature dies or is put into exile from the battlefield, "
      + "you may put it into its owner's library third from the top.", "Creature");
    boundCard(graph);
    const successor = graph.nodes.find(node => node.node === "alternatives" && node.rules.includes("400.7e"));
    assert.ok(successor?.node === "alternatives" && successor.of.length === 2);
    assert.equal(bound(graph, "/effects/0/steps/0/what"), successor.id);
    assert.ok(successor.of.every(id => graph.nodes.some(node => node.id === id &&
      node.node === "event-result" && node.identity === "zone-successors")));

    const wrong = analyze("When this creature enters or dies, you may put it into its owner's library third from the top.", "Creature");
    assert.ok(!wrong.nodes.some(node => node.node === "alternatives" && node.rules.includes("400.7e")));
  });
});

describe("announced copy models", () => {
  it("retains the announced model for the immediate linked effects", () => {
    const graph = analyze("At the beginning of combat on your turn, you may have this creature become a copy of another "
      + "target creature you control until end of turn, except its name is Fixture. If you do, this creature and that creature "
      + "each get +2/+0 and gain haste until end of turn and attack this turn if able.", "Creature");
    boundCard(graph);
    const model = bound(graph, "/spec/copyOf");
    const followup = graph.references.filter(reference => "ref" in reference.expression && reference.expression.ref === "that");
    assert.equal(followup.length, 3);
    assert.ok(followup.every(reference => reference.binding === "bound" && reference.target === model));
  });

  it("rejects a model reference with two independently announced creature targets", () => {
    const graph = analyze("{T}: Tap target creature. You may have this creature become a copy of target creature you control "
      + "until end of turn. Tap that creature.", "Creature");
    assert.ok(graph.diagnostics.some(diagnostic => diagnostic.code === "ambiguous-antecedent" &&
      diagnostic.path.endsWith("/effects/2/steps/0/what")));
  });
});

describe("quantified selections and simultaneous damage", () => {
  it("uses each creature for both sides of its component of damage", () => {
    const graph = analyze("Each creature deals damage to itself equal to its power.", "Sorcery");
    boundCard(graph);
    const damage = graph.nodes.find(node => node.node === "source-dependent-damage");
    assert.ok(damage?.timing === "simultaneous");
    assert.equal(damage.sourceAndRecipient, bound(graph, "/to/ref"));
    assert.equal(damage.sourceAndRecipient, bound(graph, "/amount/of"));
    assert.notEqual(damage.group, damage.sourceAndRecipient);
  });

  it("does not guess a recipient when a different creature has already been mentioned", () => {
    const graph = analyze("Tap target creature. Each creature deals damage to it equal to its power.", "Sorcery");
    assert.ok(!graph.nodes.some(node => node.node === "source-dependent-damage"));
  });

  it("records the legal union of announcement targets for a later instruction", () => {
    const graph = analyze("I, II — For each opponent, tap up to one target creature that player controls. Goad those creatures. "
      + "Until your next turn, whenever one of those creatures deals combat damage, draw a card.", "Enchantment — Saga");
    boundCard(graph);
    const pool = graph.nodes.find(node => node.node === "eligible-target-group");
    assert.ok(pool);
    assert.equal(pool.id, bound(graph, "/effects/1/steps/0/what"));
    assert.equal(pool.id, bound(graph, "/trigger/what/pool"));
    assert.ok(graph.nodes.some(node => node.id === pool.template && node.node === "selection" &&
      node.perMember === pool.member && node.timing === "stack-announcement"));
  });

  it("binds one target slot per opponent for destruction", () => {
    const graph = analyze("III — For each opponent, destroy up to one target artifact that player controls.", "Enchantment — Saga");
    boundCard(graph);
    const group = graph.nodes.find(node => node.node === "group-instruction");
    assert.equal(group?.execution.kind === "for-each" ? group.execution.membership : undefined, "announcement");
    assert.ok(!graph.nodes.some(node => node.node === "eligible-target-group"));
  });

  it("gives each actual nonland exile successor its own counters and suspend test", () => {
    const graph = analyze("Exile the top five cards of your library. For each nonland card exiled this way, "
      + "put a number of time counters on that card equal to its mana value. If it doesn't have suspend, it gains suspend.", "Sorcery");
    boundCard(graph);
    const pool = graph.nodes.find(node => node.node === "filtered-result-group");
    assert.ok(pool);
    assert.ok(graph.nodes.some(node => node.id === pool.pool && node.node === "event-result" && node.identity === "zone-successors"));
    assert.equal(bound(graph, "/count/of"), bound(graph, "/condition/of/what"));
    assert.equal(bound(graph, "/count/of"), bound(graph, "/effects/0/steps/0/on"));
  });

  it("does not treat two exile instructions as the one producer of a nonland exile loop", () => {
    const graph = analyze("Exile the top card of your library. Exile the top card of your library. "
      + "For each nonland card exiled this way, put a number of time counters on that card equal to its mana value. "
      + "If it doesn't have suspend, it gains suspend.", "Sorcery");
    assert.equal(graph.referencesBound, false);
    assert.ok(!graph.nodes.some(node => node.node === "filtered-result-group"));
  });

  it("permits zero or more casts in sequence from the actual exile successors", () => {
    const graph = analyze("Exile the top card of each player's library, then you may cast any number of spells "
      + "from among those cards without paying their mana costs.", "Sorcery");
    boundCard(graph);
    const casts = graph.nodes.find(node => node.node === "sequential-cast");
    assert.ok(casts);
    assert.equal(casts.pool, bound(graph, "/instruction/fromAmong"));
    assert.ok(graph.nodes.some(node => node.id === casts.pool && node.node === "event-result" &&
      node.identity === "zone-successors" && node.cardinality === "many"));
  });

  it("keeps a reveal pool separate from the immediately castable exile group", () => {
    const graph = analyze("Reveal the top card of each player's library, then you may cast any number of spells "
      + "from among those cards without paying their mana costs.", "Sorcery");
    assert.equal(graph.referencesBound, false);
    assert.ok(!graph.nodes.some(node => node.node === "sequential-cast"));
  });
});

describe("event-bound player sets and private access", () => {
  it("binds each opposing attacker relative to the player that was attacked", () => {
    const graph = analyze("Enchant player\nWhenever enchanted player is attacked, create a Gold token. "
      + "Each opponent attacking that player does the same.", "Enchantment — Aura");
    boundCard(graph);
    const group = graph.nodes.find(node => node.node === "relative-player-group");
    assert.ok(group?.filter.kind === "attacking");
    assert.equal(group.filter.player, bound(graph, "/filter/attackingPlayer"));
    assert.ok(graph.nodes.some(node => node.id === group.filter.player && node.node === "event-result" && node.result === "player"));
  });

  it("does not borrow an attacked player from a different ability", () => {
    const graph = analyze("Enchant player\nWhenever enchanted player is attacked, draw a card.\n"
      + "At the beginning of your upkeep, each opponent attacking that player creates a Gold token.", "Enchantment — Aura");
    assert.equal(graph.referencesBound, false);
    assert.ok(!graph.nodes.some(node => node.node === "relative-player-group"));
  });

  it("binds each other player as the chooser of their own sacrifice", () => {
    const graph = analyze("Whenever a creature you control dies, each other player sacrifices a creature of their choice.", "Enchantment");
    boundCard(graph);
    const group = graph.nodes.find(node => node.node === "relative-player-group");
    assert.equal(group?.filter.kind, "other-than");
    assert.equal(bound(graph, "/steps/0/who"), bound(graph, "/steps/0/chosenBy"));
    assert.equal(group?.filter.player, bound(graph, "/who/filter/other"));
  });

  it("uses the recipient's exile incarnation at counter removal time", () => {
    const graph = analyze("Whenever a time counter is removed from this card while it's exiled, draw a card.", "Creature");
    boundCard(graph);
    const event = graph.nodes.find(node => node.node === "counter-change-match");
    assert.deepEqual(event?.gate, { zone: "exile", timing: "matching-event" });
    assert.ok(graph.nodes.some(node => node.id === bound(graph, "/gate/condition/what") &&
      node.node === "event-result" && node.identity === "same-objects"));
  });

  it("keeps an intervening-if separate from the removal event's gate", () => {
    const name = "Fixture";
    const typeLine = "Creature";
    const result = parseOracleText("Whenever a time counter is removed from this card while it's exiled, draw a card.",
      name, { typeLine, includeSourceProvenance: true });
    assert.ok(result.ok);
    const [ability] = result.abilities;
    assert.ok(ability.kind === "triggered" && ability.trigger.trigger === "event-gated" && ability.trigger.gate.gate === "condition");
    ability.condition = ability.trigger.gate.condition;
    ability.trigger = ability.trigger.event;
    const graph = analyzeCardBindings({ identity: "fixture", source: { name, type_line: typeLine, layout: "normal" },
      faces: [{ sourceFaceIndex: 0, result }] });
    assert.ok(!graph.nodes.some(node => node.node === "counter-change-match" && node.gate));
  });

  it("retains private inspection of a face-down exile successor independently of play duration", () => {
    const graph = analyze("Exile the top card of your library face down. You may look at and play that card this turn.", "Sorcery");
    boundCard(graph);
    const look = graph.nodes.find(node => node.node === "face-down-exile-look");
    assert.equal(look?.continuedUntil, "leaves-exile-or-shuffled");
    assert.equal(look?.card, bound(graph, "/permission/what/0"));
    assert.equal(look?.card, bound(graph, "/permission/what"));
  });

  it("does not infer a face-down exile successor from an ordinary exile", () => {
    const graph = analyze("Exile the top card of your library. You may look at and play that card this turn.", "Sorcery");
    assert.ok(!graph.nodes.some(node => node.node === "face-down-exile-look"));
  });
});

describe("scope and visibility fences", () => {
  it("does not use an exile result in the branch that declines it", () => {
    const graph = analyze("You may exile target instant card from a graveyard. If you don't, copy the exiled card.", "Sorcery");
    assert.equal(graph.referencesBound, false);
    assert.ok(!graph.nodes.some(node => node.node === "card-copy"));
  });

  it("will not assign the attack filter to the trigger when another player is eligible", () => {
    const graph = analyze("Enchant player\nWhenever enchanted player is attacked, target player draws a card. "
      + "Each opponent attacking that player creates a Treasure token.", "Enchantment — Aura");
    assert.equal(graph.referencesBound, false);
    assert.ok(!graph.nodes.some(node => node.node === "relative-player-group"));
  });

  it("does not evaluate characteristics of a face-down exile card without permission", () => {
    const graph = analyze("Exile the top three cards of your library face down. For each nonland card exiled this way, "
      + "put a number of time counters on that card equal to its mana value. If it doesn't have suspend, it gains suspend.", "Sorcery");
    assert.equal(graph.referencesBound, false);
    assert.ok(!graph.nodes.some(node => node.node === "filtered-result-group"));
  });

  it("requires authorization before casting a face-down exile card", () => {
    const graph = analyze("Exile the top card of each player's library face down, then you may cast any number of spells "
      + "from among those cards without paying their mana costs.", "Sorcery");
    assert.equal(graph.referencesBound, false);
    assert.ok(!graph.nodes.some(node => node.node === "sequential-cast"));
  });

  it("retains the condition on a private inspection grant", () => {
    const graph = analyze("Exile the top card of your library face down. If you control a creature, "
      + "you may look at and play that card this turn.", "Sorcery");
    boundCard(graph);
    const look = graph.nodes.find(node => node.node === "face-down-exile-look");
    assert.ok(graph.nodes.some(node => node.id === look?.event && node.node === "event" && node.conditional));
  });

  it("does not identify revealed cards with an unrelated earlier scan", () => {
    const graph = analyze("Reveal cards from the top of your library until you reveal a land card. "
      + "Reveal the top card of your library. Put the revealed card into your hand.", "Sorcery");
    assert.equal(graph.referencesBound, false);
    assert.ok(!graph.nodes.some(node => node.node === "event-link" && node.action === "reveal"));
  });
});

describe("search results and shared land types", () => {
  it("uses only the actual exile successor and the actual search result", () => {
    const graph = analyze("Exile up to one target creature you own, then search your library for a basic land card. "
      + "Put both cards onto the battlefield under your control, then shuffle.", "Sorcery");
    boundCard(graph);
    const union = graph.nodes.find(node => node.node === "result-union");
    assert.ok(union);
    assert.equal(union.id, bound(graph, "/effects/1/steps/0/what"));
    const [exile, search] = union.from.map(id => graph.nodes.find(node => node.id === id));
    assert.ok(exile?.node === "event-result" && exile.identity === "zone-successors");
    assert.ok(search?.node === "event-result" && search.identity === "same-objects");
    assert.ok(graph.nodes.some(node => node.id === search.event && node.node === "event" && node.operation === "search"));
  });

  it("does not alias a second exile instruction to the library search", () => {
    const name = "Fixture";
    const typeLine = "Sorcery";
    const result = parseOracleText("Exile up to one target creature you own, then search your library for a basic land card. "
      + "Put both cards onto the battlefield under your control, then shuffle.", name, { typeLine, includeSourceProvenance: true });
    assert.ok(result.ok && result.abilities[0].kind === "spell");
    result.abilities[0].effects[0].steps[1] = {
      effect: "exile", what: { ref: "library-cards", position: "top", of: { zone: "library", owner: "your" },
        count: { amount: "fixed", value: 1 } },
    };
    const graph = analyzeCardBindings({ identity: "fixture", source: { name, type_line: typeLine, layout: "normal" },
      faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(graph.referencesBound, false);
    assert.ok(!graph.nodes.some(node => node.node === "result-union"));
  });

  it("chooses up to two basic lands whose types intersect", () => {
    const graph = analyze("Search your library for up to two basic land cards that share a land type, "
      + "put them onto the battlefield tapped, then shuffle.", "Sorcery");
    boundCard(graph);
    const shared = graph.nodes.find(node => node.node === "shared-land-type-search");
    assert.ok(shared?.maximum === 2 && shared.characteristic === "land-type");
    const moved = bound(graph, "/steps/1/what");
    assert.ok(graph.nodes.some(node => node.node === "event-result" && node.id === moved && node.event === shared.event));
  });

  it("does not apply the shared land type rule to other collective predicates", () => {
    const graph = analyze("Search your library for up to two basic land cards with different names, "
      + "put them onto the battlefield tapped, then shuffle.", "Sorcery");
    assert.equal(graph.referencesBound, false);
    assert.ok(!graph.nodes.some(node => node.node === "shared-land-type-search"));
  });
});

describe("exile qualification for card copies", () => {
  it("applies the stated type filter to the actual exile set", () => {
    const graph = analyze("Exile the top three cards of your library. For each instant card exiled this way, "
      + "copy that card, then you may cast the copy without paying its mana cost.", "Sorcery");
    boundCard(graph);
    const pool = graph.nodes.find(node => node.node === "filtered-result-group");
    assert.deepEqual(pool?.filter, { cls: "card", types: ["instant"] });
  });

  it("does not inspect face-down cards to choose instant copies", () => {
    const graph = analyze("Exile the top three cards of your library face down. For each instant card exiled this way, "
      + "copy that card, then you may cast the copy without paying its mana cost.", "Sorcery");
    assert.equal(graph.referencesBound, false);
    assert.ok(!graph.nodes.some(node => node.node === "card-copy"));
  });
});
