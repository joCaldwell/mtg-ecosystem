import assert from "node:assert/strict";
import { test } from "node:test";
import type { Ability } from "../src/ast.ts";
import { buildCardNameCatalog, parseOracleText } from "../src/index.ts";

// Oracle clauses from the frozen deck archive. Rules edition 2026-09-25.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const you = { player: "you" };
const fixed = (value: number) => ({ amount: "fixed", value });
const battlefield = { zone: "battlefield" };
const hand = { zone: "hand", owner: "your" };

function ability(text: string, typeLine: string, name = ""): Ability {
  const result = parseOracleText(text, name, {
    typeLine,
    cardNames: name ? buildCardNameCatalog([{ name, layout: "normal" }]) : undefined,
  });
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0])) as Ability;
}

function steps(text: string, typeLine = "Sorcery") {
  const parsed = ability(text, typeLine);
  assert.ok("effects" in parsed);
  return parsed.effects.flatMap(sentence => sentence.steps);
}

function rejects(text: string, typeLine = "Sorcery") {
  assert.equal(parseOracleText(text, "", { typeLine }).ok, false, text);
}

test("a copy replacement augments a spell copy and only retargets the added copy", () => {
  const text = "If you would copy a spell one or more times, instead copy it that many times plus an additional time. You may choose new targets for the additional copy.";
  assert.deepEqual(ability(text, "Artifact"), { kind: "static", effect: { static: "replacement", replacement: {
    event: { event: "copy-spell", who: you, quantity: { op: "ge", value: fixed(1) } },
    occurrence: "each", replace: { replacement: "add-spell-copy", mayRetargetAdditional: true },
  } } });
  rejects(text.replace("a spell", "a card"), "Artifact");
  rejects(text.replace("for the additional copy", "for all copies"), "Artifact");
});

test("a permanent-spell copy's ability changes are separate from its copiable values", () => {
  const text = 'Whenever you cast a spell from exile, copy it. You may choose new targets for the copy. If it\'s a permanent spell, the copy gains haste and "At the beginning of the end step, sacrifice this permanent."';
  const [copy] = steps(text, "Creature");
  assert.equal(copy.effect, "copy-stack-object");
  assert.deepEqual(copy.what, { ref: "trigger-event-object", event: "cast" });
  assert.deepEqual(copy.retarget, { who: you, optional: true });
  assert.equal(copy.copyExceptions, undefined);
  assert.equal(copy.copyGrant?.only, "permanent-spell");
  assert.deepEqual(copy.copyGrant?.abilities, [
    { gained: "keyword", keyword: { keyword: "haste" } },
    { gained: "quoted", ability: { kind: "triggered", trigger: { trigger: "phase", phase: "end-step", whose: "each-player" }, effects: [
      { steps: [{ effect: "sacrifice", who: you, what: { ref: "this", noun: "permanent" } }] },
    ] } },
  ]);
  rejects(text.replace("copy it", "you may copy it"), "Creature");
  rejects(text.replace("a permanent spell", "a permanent"), "Creature");
});

test("entry and exile counters describe the movement itself", () => {
  const delay = "Counter target spell. If the spell is countered this way, exile it with three time counters on it instead of putting it into its owner's graveyard. If it doesn't have suspend, it gains suspend.";
  const [counter, suspend] = steps(delay, "Instant");
  assert.equal(counter.effect, "counter");
  assert.deepEqual(counter.countered, { to: { zone: "exile" }, counters: [{ count: fixed(3), counter: { type: "time" } }] });
  assert.equal(suspend.effect, "gain-abilities");
  rejects(delay.replace("on it instead", "on them instead"), "Instant");
  rejects(delay.replace("target spell", "two target spells"), "Instant");

  const moveText = "Return target artifact or creature card with mana value 3 or less from your graveyard to the battlefield. If a creature enters this way, it enters with an additional +1/+1 counter on it.";
  const [move] = steps(moveText);
  assert.equal(move.effect, "move-zone");
  assert.deepEqual(move.entry, { counters: [{ count: fixed(1), counter: { type: "+1/+1" }, additional: true, entrantType: "creature" }] });
  rejects(moveText.replace("on it", "on them"));
  rejects("Return target creature card to the battlefield, then return target artifact card to the battlefield."
    + " If a creature enters this way, it enters with an additional +1/+1 counter on it.");

  const [put] = steps("Put one, two, or three target creature cards from graveyards onto the battlefield under your control. Each of them enters with an additional -1/-1 counter on it.");
  assert.equal(put.effect, "move-zone");
  assert.deepEqual(put.what, { ref: "target", filter: { types: ["creature"], cls: "card", zone: { zone: "graveyard", owner: "all-players" } },
    count: { amount: "chosen", minimum: 1, maximum: fixed(3) } });
  assert.deepEqual(put.entry, { counters: [{ count: fixed(1), counter: { type: "-1/-1" }, additional: true }] });
  rejects("Put one, two, and three target creature cards from graveyards onto the battlefield.");

  const [returnCard] = steps("Return that card under your control with a finality counter on it.");
  assert.deepEqual(returnCard, { effect: "move-zone", what: { ref: "that", filter: { cls: "card" } }, to: battlefield,
    controller: "you", entry: { counters: [{ count: fixed(1), counter: { type: "finality" } }] } });
  rejects("Return that card with a finality counter on it.");
});

test("copy-entry counter exceptions distinguish the two projected types", () => {
  const text = "You may have this creature enter as a copy of a creature or planeswalker you control, except it enters with an additional +1/+1 counter on it if it's a creature, it enters with an additional loyalty counter on it if it's a planeswalker, and it isn't legendary.";
  const parsed = ability(text, "Creature");
  assert.equal(parsed.kind, "static");
  assert.equal(parsed.effect.static, "entry-copy");
  assert.deepEqual(parsed.effect.entryExceptions, [
    { exception: "counters", count: fixed(1), counter: { type: "+1/+1" }, additional: true, entrantType: "creature" },
    { exception: "counters", count: fixed(1), counter: { type: "loyalty" }, additional: true, entrantType: "planeswalker" },
  ]);
  assert.deepEqual(parsed.effect.changes, [{ change: "types", types: {
    supertypes: { operation: "remove", types: ["legendary"] },
  } }]);
  rejects(text.replace("it's a creature", "the model is a creature"), "Creature");
});

test("the name test negates a match against the union of two independent sets", () => {
  const text = "Whenever a nontoken creature you control enters, if it doesn't have the same name as another creature you control or a creature card in your graveyard, draw a card.";
  const parsed = ability(text, "Enchantment");
  assert.equal(parsed.kind, "triggered");
  assert.deepEqual(parsed.condition, { condition: "characteristic-relation", what: { ref: "it" },
    test: { relation: "not", of: { relation: "shared-value", characteristic: "name", with: {
      operand: "matching-set", quantifier: "any", filter: { anyOf: [
        { types: ["creature"], control: "you", other: true },
        { types: ["creature"], cls: "card", zone: { zone: "graveyard", owner: "your" } },
      ] },
    } } },
  });
  rejects(text.replace("or a creature card", "and a creature card"), "Enchantment");
});

test("the free casting option is evaluated as part of the original play grant", () => {
  const text = "Exile the top three cards of your library. Until the end of your next turn, you may play those cards. If this spell was cast from a graveyard, you may play cards this way without paying their mana costs.";
  const results = steps(text);
  assert.equal(results.length, 2);
  const grant = results[1];
  assert.equal(grant.effect, "grant-play-permission");
  assert.deepEqual(grant.duration, { duration: "end-of-your-next-turn" });
  assert.equal(grant.permission.action, "play");
  assert.equal(grant.permission.alternativeCost, undefined);
  assert.deepEqual(grant.permission.optionalAlternativeCost, { cost: "without-mana-cost", check: "when-granted",
    condition: { condition: "was-cast", what: { ref: "this", noun: "spell" }, from: { zone: "graveyard", owner: "any" } } });
  rejects(text.replace("you may play those cards", "you may cast those cards"));
});

test("cast eligibility compares the spell being proposed with the triggering spell", () => {
  const text = "Whenever an opponent casts a spell, you may reveal the top card of your library. If you do, you may cast that card without paying its mana cost if the two spells have the same mana value.";
  const [reveal, cast] = steps(text, "Enchantment");
  assert.equal(reveal.effect, "reveal");
  assert.equal(reveal.optional, true);
  assert.equal(cast.effect, "play-card");
  assert.deepEqual(cast.instruction.condition, { condition: "proposed-spell-matches-trigger-mana-value",
    triggerSpell: { ref: "trigger-event-object", event: "cast" } });
  rejects(text.replace("Whenever an opponent casts a spell", "At the beginning of your upkeep"), "Enchantment");
});

test("each opponent's search decision and the later shuffle have different scopes", () => {
  const text = "Search your library for a land card and put it onto the battlefield. Each opponent may search their library for a land card and put it onto the battlefield. For each opponent who searches a library this way, search your library for a land card and put it onto the battlefield. Then each player who searched a library this way shuffles.";
  const [, , optional, additional, shuffle] = steps(text);
  assert.equal(optional.effect, "optional-group");
  assert.deepEqual(optional.who, { player: "each-opponent" });
  assert.equal(additional.effect, "for-each");
  const filter = { searched: { zone: { zone: "library", owner: "any" }, scope: "this-way" } };
  assert.deepEqual(additional.each, { player: "filtered", base: { player: "each-opponent" }, filter });
  assert.equal(shuffle.effect, "shuffle");
  assert.deepEqual(shuffle.who, { player: "filtered", base: { player: "each-player" }, filter });
  assert.deepEqual(shuffle.library.owner, shuffle.who);
  assert.equal(shuffle.coalesceEarlierLibrarySearches, true);
  rejects(text.replace("Then each player who searched a library this way shuffles", "Then that player shuffles"));
});

test("the active player shares one use across playing a land and casting a spell", () => {
  const text = "During each player's turn, that player may play a land or cast a spell from among cards exiled with this enchantment, and they may spend mana as though it were mana of any color to cast that spell. When they do, exile the top card of their library.";
  const parsed = ability(text, "Enchantment");
  assert.equal(parsed.kind, "static");
  assert.equal(parsed.effect.static, "play-permission-choice");
  assert.deepEqual(parsed.effect.useLimit, { maximum: 1, period: "turn", during: "active-player" });
  const [play, cast] = parsed.effect.permissions;
  assert.deepEqual([play.who, cast.who], [{ player: "active-player" }, { player: "active-player" }]);
  assert.deepEqual(play.fromAmong, cast.fromAmong);
  assert.equal(play.manaPayment, undefined);
  assert.equal(cast.manaPayment, "any-color");
  assert.deepEqual(play.riders?.map(rider => rider.rider), ["use-trigger"]);
  assert.deepEqual(cast.riders?.map(rider => rider.rider), ["use-trigger"]);
  rejects(text.replace("When they do, exile", "When they cast a spell, exile"), "Enchantment");
});

test("upkeep-only beginning phases and time travel retain the independent events", () => {
  const [extra] = steps("Whenever ~ deals combat damage to a player, you get that many additional upkeep steps after this phase.", "Creature");
  assert.deepEqual(extra, { effect: "add-phases", after: { phase: "current" }, sequence: [
    { phase: "beginning", count: { amount: "that-much" }, onlyStep: "upkeep", who: you, requiresControllerTurn: true },
  ] });
  assert.deepEqual(steps("{7}: Time travel three times. Activate only as a sorcery.", "Creature"),
    [{ effect: "time-travel", who: you, times: fixed(3) }]);
  rejects("Time travel a times.");
  rejects("You get three additional upkeep steps after this step.");
  rejects("Each player gets three additional upkeep steps after this phase.");
});

test("flavor punctuation cannot absorb earlier keywords", () => {
  const text = "I. AM. TALKING! — When this creature enters, draw a card.";
  const parsed = ability(text, "Creature");
  assert.equal(parsed.kind, "triggered");
  assert.equal(parsed.abilityWord, "I. AM. TALKING!");
  rejects(text.replace("I. AM. TALKING!", "FLYING. AM. TALKING!"), "Creature");
});

test("count expressions keep their own rule-defined source and scope", () => {
  const [loss] = steps("When this creature enters, you lose all but 1 life.", "Creature");
  assert.deepEqual(loss, { effect: "lose-life", who: you,
    amount: { amount: "minus", a: { amount: "life-total", of: you }, b: fixed(1) } });
  const [gain] = steps("When this creature leaves the battlefield, you gain life equal to the life you lost when it entered.", "Creature");
  assert.deepEqual(gain, { effect: "gain-life", who: you,
    amount: { amount: "life-lost-on-entry", who: you, what: { ref: "it" } } });
  const cost = ability("{3}, {T}, Pay life equal to the number of colors in your commanders' color identity: Draw a card.", "Land");
  assert.equal(cost.kind, "activated");
  assert.deepEqual(cost.costs[2], { cost: "pay-life", amount: { amount: "commander-color-identity-count", whose: you } });
  const token = ability("{3}{G}{G}, Sacrifice this creature: Create an X/X colorless Golem artifact creature token, where X is three times this creature's power.", "Creature");
  assert.equal(token.kind, "activated");
  assert.deepEqual(token.effects[0].bindings, [{ variable: "x", value: { amount: "product",
    factors: [fixed(3), { amount: "attribute", of: { ref: "this", noun: "creature" }, attribute: "power" }] } }]);
  rejects("Draw X cards, where X is a times seven.");
});

test("single-stat and linked-exile rules keep their exact effect and event", () => {
  assert.deepEqual(ability("Creatures your opponents control have base toughness 1.", "Creature"), {
    kind: "static", effect: { static: "modify", what: { ref: "each", filter: { types: ["creature"], control: "opponent" } },
      modifiers: [{ modifier: "base-pt", toughness: fixed(1) }] },
  });
  const [exile] = steps("When this creature enters, exile target creature an opponent controls until an opponent becomes the monarch.", "Creature");
  assert.equal(exile.effect, "exile-until");
  assert.deepEqual(exile.until, { event: "becomes-monarch", who: { player: "an-opponent" } });
  assert.deepEqual(exile.return, { to: "previous-zone", battlefieldController: "owner" });
  rejects("Exile target creature until an opponent is the monarch.");
  rejects("Creatures your opponents control have base toughness 1/1.", "Creature");
});

test("Ward suppression and cleanup damage retention affect only their stated rules", () => {
  const text = "Creatures your opponents control can be the targets of spells and abilities as though they didn't have hexproof. Ward abilities of those creatures don't trigger.";
  const parsed = ability(text, "Enchantment");
  assert.equal(parsed.kind, "static");
  assert.equal(parsed.effect.static, "group");
  const [targeting, ward] = parsed.effect.effects;
  assert.equal(targeting.static, "targeting-as-though");
  assert.equal(ward.static, "trigger-suppression");
  assert.deepEqual(ward.abilities, { cls: "ability", abilityKind: "triggered", abilityKeyword: "ward", abilitySource: targeting.what[0] });
  assert.deepEqual(ability("Damage isn't removed from this creature during cleanup steps.", "Creature"), {
    kind: "static", effect: { static: "retain-damage-during-cleanup", what: { ref: "this", noun: "creature" } },
  });
  rejects(text.replace("hexproof. Ward", "hexproof Ward"), "Enchantment");
  rejects("Damage isn't removed from this creature when it regenerates.", "Creature");
});

test("selection uses the event player's actual opponents and the initiative holder", () => {
  const [draw] = steps("Whenever a player casts a noncreature spell, each of that player's opponents may draw a card.", "Creature");
  const who = { player: "each-opponent-of", of: { player: "that-player" } };
  assert.deepEqual(draw, { effect: "draw", who, amount: fixed(1), optional: true, decidedBy: who });
  const attack = ability("Whenever you attack the player who has the initiative, create a Treasure token.", "Enchantment");
  assert.equal(attack.kind, "triggered");
  assert.deepEqual(attack.trigger, { trigger: "player-attacks", who: you, whom: { player: "the-initiative-holder" }, per: "declaration" });
  const [mana] = steps("A player of your choice adds {C}.");
  assert.deepEqual(mana, { effect: "add-mana", who: { player: "player-of-choice", chooser: you }, mana: { mana: "fixed", symbols: ["C"] } });
  rejects("A player of your choice add {C}.");
  rejects("Whenever you attack the player who has the other initiative, draw a card.", "Creature");
});

test("alternative casting costs and timing guards remain independent of casting origin", () => {
  const cost = ability("You may pay {B} rather than pay this spell's mana cost if there are thirteen or more creatures on the battlefield.", "Sorcery");
  assert.equal(cost.kind, "static");
  assert.equal(cost.effect.static, "alternative-cost");
  assert.deepEqual(cost.effect.condition, { condition: "count", of: { types: ["creature"], zone: battlefield },
    comparison: { op: "ge", value: fixed(13) } });
  const bound = ability("Once each turn, you may pay {0} rather than pay the mana cost for a spell you cast with mana value X or less, where X is the number of time counters on this enchantment.", "Enchantment");
  assert.equal(bound.kind, "static");
  assert.equal(bound.effect.static, "alternative-cost");
  assert.deepEqual(bound.effect.bindings, [{ variable: "x",
    value: { amount: "counter-count", counter: { type: "time" }, on: { ref: "this", noun: "enchantment" } } }]);
  assert.deepEqual(bound.effect.useLimit, { maximum: 1, period: "turn", during: "any" });
  assert.deepEqual(ability("Plotting cards from your hand costs {2} less.", "Creature"), { kind: "static",
    effect: { static: "cost-modification", scope: { action: "pay-cost", cost: "plot", payer: you, from: hand },
      change: { direction: "reduce", symbols: ["2"] } } });
  const combined = ability("The first creature spell you cast each turn costs {2} less to cast and can be cast as though it had flash.", "Creature");
  assert.equal(combined.kind, "static");
  assert.equal(combined.effect.static, "group");
  const [reduction, timing] = combined.effect.effects;
  assert.equal(reduction.static, "cost-modification");
  assert.equal(timing.static, "cast-timing");
  assert.deepEqual(timing.scope, reduction.scope);
  assert.deepEqual(timing.scope.occurrence, { ordinal: 1, period: "turn" });
  rejects("Plotting cards from your hand costs {2} less to cast.", "Creature");
  rejects("You may pay {B} rather than pay this spell's mana cost if there are thirteen or more.", "Sorcery");
});

test("a resolution payment chooses X under an event-specific upper bound", () => {
  const text = "Whenever you gain life, you may pay {X}, where X is less than or equal to the amount of life you gained. If you do, draw X cards.";
  const [payment, draw] = steps(text, "Artifact");
  assert.equal(payment.effect, "pay-cost");
  assert.equal(payment.optional, true);
  assert.deepEqual(payment.variableConstraints, [{ variable: "x", maximum: {
    amount: "event-quantity", event: "life-change", scope: "contextual", change: "gained", who: you,
  } }]);
  assert.deepEqual(draw, { effect: "draw", who: you, amount: { amount: "x" } });
  rejects(text.replace("the amount of life you gained", "X"), "Artifact");
  rejects(text.replace("{X}", "{3}"), "Artifact");
});

test("same-effect type changes keep the granted ability", () => {
  const text = "Enchanted creature is an Insect artifact creature with base power and toughness 0/1 and has indestructible, and it loses all other abilities, card types, and creature types.";
  const aura = ability(text, "Enchantment — Aura");
  assert.equal(aura.kind, "static");
  assert.equal(aura.effect.static, "modify");
  const [types, stats, grant, removal] = aura.effect.modifiers;
  assert.deepEqual(types, { modifier: "types", change: {
    cardTypes: { operation: "set", types: ["artifact", "creature"] },
    subtypes: [{ family: "creature", operation: "set", types: ["Insect"] }],
  } });
  assert.deepEqual(stats, { modifier: "base-pt", power: fixed(0), toughness: fixed(1) });
  assert.deepEqual(grant, { modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "indestructible" } }] });
  assert.deepEqual(removal, { modifier: "lose-abilities", removal: { removal: "all-other", than: "abilities-granted-by-this-effect" } });
  rejects(text.replace(" and has indestructible", ""), "Enchantment — Aura");
  const [treasure] = steps('-2: Target creature becomes a Treasure artifact with "{T}, Sacrifice this artifact: Add one mana of any color" and loses all other card types and abilities.', "Planeswalker");
  assert.equal(treasure.effect, "modify-characteristics");
  assert.deepEqual(treasure.modifiers.at(-1), removal);
  const [poison] = steps("-9: If target player has fewer than nine poison counters, they get a number of poison counters equal to the difference.", "Planeswalker");
  assert.equal(poison.effect, "get-counters");
  assert.deepEqual(poison.count, { amount: "minus", a: fixed(9),
    b: { amount: "counter-count", counter: { type: "poison" }, on: { player: "target-player" } } });
});

test("protection queries the complement of commander identity and not live colors", () => {
  const text = "Equipped creature gets +3/+3 and has protection from each color that's not in your commander's color identity.";
  const gear = ability(text, "Artifact — Equipment");
  assert.equal(gear.kind, "static");
  assert.equal(gear.effect.static, "modify");
  assert.deepEqual(gear.effect.modifiers[1], { modifier: "abilities", abilities: [{ gained: "keyword",
    keyword: { keyword: "protection", from: [{ scope: "colors-outside-commander-identity", whose: you }] } }] });
  rejects(text.replace("color identity", "colors"), "Artifact — Equipment");
});

test("a token's dynamic base stats use the entire death-event group", () => {
  const text = "Whenever one or more nontoken creatures you control die, create a green Fungus Dinosaur creature token with base power and toughness each equal to the total power of those creatures.";
  const [create] = steps(text, "Artifact");
  assert.equal(create.effect, "create-token");
  assert.ok("types" in create.token);
  assert.deepEqual(create.token.power, { amount: "attribute-sum", attribute: "power",
    among: { ref: "that", plural: true, filter: { types: ["creature"] } } });
  assert.deepEqual(create.token.toughness, create.token.power);
  rejects(text.replace("create a green", "create a 1/1 green"), "Artifact");
});

test("historical casting origin and period remain properties of the completed casts", () => {
  const text = "This spell deals X damage to each opponent, where X is 1 plus the number of spells you've cast from anywhere other than your hand this turn.";
  const spell = ability(text, "Sorcery");
  assert.equal(spell.kind, "spell");
  assert.deepEqual(spell.effects[0].bindings, [{ variable: "x", value: { amount: "plus", a: fixed(1),
    b: { amount: "cast-count", who: you, what: { cls: "spell" }, during: "this-turn", notFrom: hand },
  } }]);
  const name = "Approach of the Second Sun";
  const approach = ability('If this spell was cast from your hand and you\'ve cast another spell named "Approach of the Second Sun" this game, you win the game. Otherwise, put Approach of the Second Sun into its owner\'s library seventh from the top and you gain 7 life.',
    "Sorcery", name);
  assert.equal(approach.kind, "spell");
  const condition = approach.effects[0].condition;
  assert.equal(condition?.condition, "all-of");
  const previous = condition.conditions[1];
  assert.equal(previous.condition, "cast-count");
  assert.equal(previous.during, "this-game");
  assert.equal(previous.what.other, true);
  rejects(text.replace("this turn", "when you drew it"));
});

test("mana value alternatives describe exactly the stated values", () => {
  const text = "I — You may cast an instant or sorcery spell with mana value 1 or 3 from your hand without paying its mana cost.";
  const [cast] = steps(text, "Enchantment — Saga");
  assert.equal(cast.effect, "play-card");
  assert.equal(cast.instruction.what.ref, "filter");
  const values = cast.instruction.what.filter.allOf?.[1]?.allOf?.[0]?.anyOf;
  assert.deepEqual(values, [1, 3].map(value => ({
    properties: [{ property: "mana-value", comparison: { op: "eq", value: fixed(value) } }],
  })));
  rejects(text.replace("1 or 3", "1 or 3 or greater"), "Enchantment — Saga");
});

test("reassembled counter choices and amounts preserve the recipient", () => {
  const [choice] = steps("Put a +1/+1 counter or a loyalty counter on it.");
  assert.equal(choice.effect, "choice");
  assert.deepEqual(choice.options, ["+1/+1", "loyalty"].map(type => ({
    effect: "put-counters", on: { ref: "it" }, counter: { type }, count: fixed(1),
  })));
  rejects("Put +1/+1 counter or loyalty counter on it.");
  const [pump] = steps("Creatures you control get +1/+0 until end of turn for each lore counter among Sagas you control.");
  assert.equal(pump.effect, "pump");
  assert.deepEqual(pump.power.amount, { amount: "product", factors: [fixed(1), { amount: "counter-count",
    counter: { type: "lore" }, on: { ref: "each", filter: { subtypes: ["Saga"], control: "you" } }, total: true }] });
  const [distinct] = steps("Creatures you control get +1/+0 until end of turn for each kind of counter among Sagas you control.");
  assert.equal(distinct.effect, "pump");
  assert.deepEqual(distinct.power.amount, { amount: "product", factors: [fixed(1), { amount: "distinct-count",
    characteristic: "counter-kind", among: { ref: "each", filter: { subtypes: ["Saga"], control: "you" } } }] });
});

test("suspend counters and the suspend casting payment remain distinct", () => {
  const [exile] = steps("You may exile a card from your hand with a number of time counters on it equal to its mana value.");
  assert.equal(exile.effect, "exile");
  assert.deepEqual(exile.what, { ref: "filter", filter: { cls: "card", zone: hand }, count: fixed(1) });
  assert.deepEqual(exile.counters, [{ counter: { type: "time" },
    count: { amount: "attribute", attribute: "mana-value", of: { ref: "it" } } }]);
  rejects("You may exile a card from your graveyard with a number of time counters on it equal to its mana value.");
  const text = "{T}: You may cast a spell with suspend from your hand. If you do, pay its suspend cost rather than its mana cost. Activate only as a sorcery.";
  const castAbility = ability(text, "Creature");
  assert.equal(castAbility.kind, "activated");
  assert.deepEqual(castAbility.restriction, { restriction: "sorcery" });
  const [cast] = castAbility.effects[0].steps;
  assert.equal(cast.effect, "play-card");
  assert.equal(cast.optional, true);
  assert.deepEqual(cast.instruction.alternativeCost, { cost: "keyword-cost", keyword: "suspend" });
  rejects(text.replace("with suspend", "with flashback"), "Creature");

  const copyText = "Whenever you cast your second spell each turn, copy it, then exile the spell you cast with four time counters on it.";
  const [copy, original] = steps(copyText, "Creature");
  assert.equal(copy.effect, "copy-stack-object");
  assert.equal(original.effect, "exile");
  assert.deepEqual(original.what, copy.what);
  assert.deepEqual(original.counters, [{ counter: { type: "time" }, count: fixed(4) }]);
  rejects(copyText.replace("you cast your", "an opponent casts their"), "Creature");
  rejects(copyText.replace("the spell you cast with", "the spell you cast from your hand with"), "Creature");
});

test("the two suspended-card and permanent branches retain independent predicates", () => {
  const [target] = steps("{T}: Choose target permanent you control or suspended card you own.", "Artifact");
  assert.deepEqual(target, { effect: "declare-target", what: { ref: "target", count: fixed(1), filter: { anyOf: [
    { cls: "permanent", control: "you" }, { cls: "card", status: ["suspended"], own: "you" },
  ] } } });
  rejects("{T}: Choose target permanent you control or card you own.", "Artifact");
  rejects("{T}: Choose target permanent you control or suspended card you own in your graveyard.", "Artifact");
  rejects("{T}: You may choose target permanent you control or suspended card you own.", "Artifact");
  const [put] = steps("Put two time counters on each permanent with a time counter on it and each suspended card.");
  assert.equal(put.effect, "put-counters");
  assert.deepEqual(put.on, { ref: "each", filter: { anyOf: [
    { cls: "permanent", counterConstraints: [{ counter: { type: "time" }, comparison: { op: "ge", value: fixed(1) } }] },
    { cls: "card", status: ["suspended"] },
  ] } });
  rejects("Put two time counters on each permanent with a time counter on it and each tapped permanent.");
});

test("comma-separated zone triggers retain the origin qualifier of one event", () => {
  const text = "Whenever another creature dies, or a creature card is put into a graveyard from anywhere other than the battlefield, or a creature card leaves your graveyard, draw a card.";
  const parsed = ability(text, "Creature");
  assert.equal(parsed.kind, "triggered");
  assert.equal(parsed.trigger.trigger, "any-of");
  assert.deepEqual(parsed.trigger.events.map(event => event.trigger), ["dies", "put-into-graveyard", "leaves-zone"]);
  const [, move, leaves] = parsed.trigger.events;
  assert.equal(move.trigger, "put-into-graveyard");
  assert.equal(move.from, "anywhere");
  assert.deepEqual(move.notFrom, battlefield);
  assert.equal(leaves.trigger, "leaves-zone");
  assert.deepEqual(leaves.from, { zone: "graveyard", owner: "your" });
  rejects(text.replace("or a creature card leaves your graveyard", "or whenever an unknown event occurs"), "Creature");
  rejects(text.replace("or a creature card leaves your graveyard", "or it leaves your graveyard"), "Creature");
});

test("an opponent or an opposing permanent shares one damage target selection", () => {
  const text = "This spell deals 3 damage to target opponent, creature an opponent controls, or planeswalker an opponent controls.";
  const result = parseOracleText(text, "", { typeLine: "Instant", includeSourceProvenance: true });
  assert.ok(result.ok);
  const spell = result.abilities[0];
  assert.equal(spell.kind, "spell");
  const [damage] = spell.effects[0].steps;
  assert.equal(damage.effect, "damage");
  assert.equal(damage.to.to, "object");
  const target = damage.to.ref;
  assert.equal(target.ref, "target");
  assert.ok(target.selectionId);
  assert.deepEqual(target.count, fixed(1));
  assert.deepEqual(target.filter, { orOpponent: true, anyOf: [
    { types: ["creature"], control: "opponent" }, { types: ["planeswalker"], control: "opponent" },
  ] });
  rejects(text.replace("or planeswalker an opponent controls", "and planeswalker an opponent controls"), "Instant");
});

test("greatest power is evaluated for each sacrificing opponent", () => {
  const text = "Each opponent sacrifices a creature they control with the greatest power.";
  const [sacrifice] = steps(text);
  assert.equal(sacrifice.effect, "sacrifice");
  assert.deepEqual(sacrifice.who, { player: "each-opponent" });
  assert.equal(sacrifice.what.ref, "filter");
  assert.deepEqual(sacrifice.what.filter.properties?.[0].comparison, { op: "eq", value: {
    amount: "attribute-extremum", attribute: "power", extremum: "maximum",
    among: { types: ["creature"], controlledBy: { player: "they" } },
  } });
  rejects(text.replace("they control", "you control"));
});

test("the fallback cast is mandatory when legal and occurs during resolution", () => {
  const text = "At the beginning of each player's draw step, that player exiles the top card of their library. If it's a land card, the player puts it onto the battlefield. Otherwise, the player casts it without paying its mana cost if able.";
  const parsed = ability(text, "Artifact");
  assert.equal(parsed.kind, "triggered");
  const fallback = parsed.effects[1].otherwise;
  assert.equal(fallback?.on, "condition-false");
  assert.deepEqual(fallback.steps, [{ effect: "play-card", instruction: {
    who: { player: "the-player" }, action: "cast", what: { ref: "it" },
    alternativeCost: { cost: "without-mana-cost" }, requiredIfAble: true,
  } }]);
  rejects(text.replace("if able", "if tempted"), "Artifact");
});
