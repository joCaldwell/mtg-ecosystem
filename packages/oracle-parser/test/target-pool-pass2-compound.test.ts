import assert from "node:assert/strict";
import { test } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Frozen target Oracle text, verified against CR 2026-09-25.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const fixed = (value: number) => ({ amount: "fixed", value });

function one(text: string, typeLine = "Sorcery"): Ability {
  const r = parseOracleText(text, "", { typeLine });
  assert.ok(r.ok, JSON.stringify(r.lines));
  assert.equal(r.abilities.length, 1);
  return JSON.parse(JSON.stringify(r.abilities[0])) as Ability;
}

function rejects(text: string, typeLine = "Sorcery") {
  assert.equal(parseOracleText(text, "", { typeLine }).ok, false, text);
}

test("a movement replacement creates the delayed return inside each application", () => {
  const text = "If a permanent you control would be put into a graveyard from the battlefield this turn, exile it instead. Return it to the battlefield under its owner's control at the beginning of the next end step.";
  const a = one(text, "Instant");
  assert.equal(a.kind, "spell");
  const [created] = a.effects[0].steps;
  assert.equal(created.effect, "create-replacement");
  assert.deepEqual(created.duration, { duration: "this-turn" });
  assert.equal(created.replacement.occurrence, "each");
  assert.equal(created.replacement.replace.replacement, "effects");
  const [exile, delayed] = created.replacement.replace.effects[0].steps;
  assert.deepEqual(exile, { effect: "exile", what: { ref: "replacement-event-object", role: "affected" } });
  assert.equal(delayed.effect, "create-delayed-trigger");
  assert.equal(delayed.trigger.trigger, "phase");
  assert.equal(delayed.trigger.phase, "end-step");
  assert.equal(delayed.occurrence, "next");
  assert.deepEqual(delayed.effects[0].steps[0], {
    effect: "move-zone", what: { ref: "related", number: "plural", filter: { cls: "card" },
      relation: { relation: "replacement-exile-results" } },
    to: { zone: "battlefield" }, controller: "owner",
  });
  rejects(text.replace("exile it instead", "return it to its owner's hand instead"), "Instant");
  rejects(text.replace("this turn", "this combat"), "Instant");
});

test("distinct fights follow the preceding creation loop and keep both cohorts", () => {
  const text = "For each creature your opponents control, create a 4/4 green Phyrexian Beast creature token. Each of those tokens fights a different one of those creatures.";
  const a = one(text);
  assert.equal(a.kind, "spell");
  assert.equal(a.effects[0].steps[0].effect, "for-each");
  assert.deepEqual(a.effects[1].steps[0], { effect: "fight",
    a: { ref: "that", plural: true, filter: { cls: "token" } },
    b: { ref: "that", plural: true, filter: { types: ["creature"] } },
    pairing: { kind: "distinct-pairs", who: { player: "you" }, inputs: "preceding-token-loop", unmatched: "omit" },
  });
  rejects(text.replace("a 4/4", "two 4/4").replace("creature token.", "creature tokens."));
  rejects("Each of those tokens fights a different one of those creatures.");
});

test("both cards names the two earlier results without imposing a second choice", () => {
  const text = "Whenever equipped creature deals combat damage to a player, exile up to one target creature you own, then search your library for a basic land card. Put both cards onto the battlefield under your control, then shuffle.";
  const a = one(text, "Artifact");
  assert.equal(a.kind, "triggered");
  assert.deepEqual(a.effects[0].steps.map(s => s.effect), ["exile", "search"]);
  const [move, shuffle] = a.effects[1].steps;
  assert.deepEqual(move, { effect: "move-zone", what: { ref: "related", number: "plural", filter: { cls: "card" },
    relation: { relation: "exile-and-search-results", from: "preceding-sentence" } },
  to: { zone: "battlefield" }, controller: "you" });
  assert.equal(shuffle.effect, "shuffle");
  rejects(text.replace("search your library for a basic land card", "draw a card"), "Artifact");
});

test("search self-replacement changes only the search upper limit", () => {
  const text = "Search your library for up to two basic Forest cards, reveal those cards, and put one onto the battlefield tapped and the rest into your hand. Then shuffle.\nSpell mastery — If there are two or more instant and/or sorcery cards in your graveyard, search your library for up to three basic Forest cards instead of two.";
  const a = one(text);
  assert.equal(a.kind, "spell");
  const [replace, reveal, place] = a.effects[0].steps;
  assert.equal(replace.effect, "self-replacement");
  assert.equal(replace.abilityWord, "Spell mastery");
  assert.ok("effect" in replace.original);
  assert.equal(replace.original.effect, "search");
  assert.equal(replace.original.for.ref, "filter");
  assert.deepEqual(replace.original.for.count, fixed(2));
  assert.ok(Array.isArray(replace.instead));
  const [search] = replace.instead;
  assert.equal(search.effect, "search");
  assert.equal(search.for.ref, "filter");
  assert.deepEqual(search.for.count, fixed(3));
  assert.deepEqual(search.for.filter, replace.original.for.filter);
  assert.equal(reveal.effect, "reveal");
  assert.equal(place.effect, "simultaneous");
  assert.equal(a.effects[1].steps[0].effect, "shuffle");
  rejects(text.replace("instead of two", "instead of one"));
  rejects(text.replace("three basic Forest", "three basic Island"));
});

test("repeat mill checks the two cards from the last iteration on the same player", () => {
  const text = "{3}, {T}: Target player mills two cards. If two cards that share a color were milled this way, repeat this process.";
  const a = one(text, "Artifact");
  assert.equal(a.kind, "activated");
  assert.deepEqual(a.effects[0].steps, [{ effect: "mill", who: { player: "target-player" }, amount: fixed(2),
    repeatWhile: { from: "last-iteration", condition: "two-cards-share-color" } }]);
  rejects(text.replace("mills two", "mills three"), "Artifact");
  rejects(text.replace("share a color", "share a name"), "Artifact");
  rejects(text.replace("Target player mills", "Each player mills"), "Artifact");
  rejects(text.replace("Target player mills", "Two target players mill"), "Artifact");
  rejects(text.replace("Target player mills", "Target opponent mills two cards. Target player mills"), "Artifact");
});

test("permanent representatives may be reused across types and only their chooser keeps them", () => {
  const text = "-9: Each opponent chooses a permanent they control of each permanent type and sacrifices the rest.";
  const a = one(text, "Planeswalker");
  assert.equal(a.kind, "loyalty");
  const [choose, sacrifice] = a.effects[0].steps;
  assert.equal(choose.effect, "choose-object");
  assert.deepEqual(choose.perPermanentType, { skipMissing: true, sameObjectForMultipleTypes: true });
  assert.deepEqual(choose.who, { player: "each-opponent" });
  assert.equal(sacrifice.effect, "sacrifice");
  assert.deepEqual(sacrifice.what, { ref: "rest", of: choose.what, exclude: "own-preceding-choices" });
  rejects(text.replace("permanent type", "creature type"), "Planeswalker");
});

test("counter-kind choice is per object and placement remains optional", () => {
  const text = "II, III — For each non-Saga permanent, choose a counter on it. You may put an additional counter of that kind on that permanent.";
  const a = one(text, "Enchantment — Saga");
  assert.equal(a.kind, "chapter");
  const [loop] = a.effects[0].steps;
  assert.equal(loop.effect, "for-each");
  assert.deepEqual(loop.each, { ref: "each", filter: { cls: "permanent", nonSubtypes: ["Saga"] } });
  assert.deepEqual(loop.effects[0].steps[0], { effect: "put-counters-of",
    counters: { of: { ref: "it" }, select: "one-existing-counter-kind", placement: "optional", phaseOrder: "choose-all-then-place" },
    on: { ref: "it" },
  });
  rejects(text.replace("on that permanent", "on this Saga"), "Enchantment — Saga");
});

test("only the linked cards go to their respective owners' library bottoms", () => {
  const text = "IV — Starting with you, each player votes for innocent or guilty. If guilty gets more votes, the owner of each card exiled with this Saga puts that card on the bottom of their library.";
  const a = one(text, "Enchantment — Saga");
  assert.equal(a.kind, "chapter");
  assert.equal(a.effects[0].steps[0].effect, "vote");
  const [loop] = a.effects[1].steps;
  assert.equal(loop.effect, "for-each");
  assert.deepEqual(loop.effects[0].steps[0], { effect: "move-zone",
    what: { ref: "it" }, who: { player: "owner", of: { ref: "it" } }, to: { zone: "library", owner: "their", position: "bottom" } });
  rejects(text.replace("on the bottom", "on the top"), "Enchantment — Saga");
});

test("found cards stay paired with their searching player through placement and shuffle", () => {
  const text = "Exile target creature you don't control. For each creature exiled this way, its controller searches their library for a basic land card. Those players put those cards onto the battlefield tapped, then shuffle.";
  const a = one(text);
  assert.equal(a.kind, "spell");
  const [loop] = a.effects[1].steps;
  assert.equal(loop.effect, "for-each");
  assert.equal(loop.groupedSearch, "last-controllers-of-exiled-creatures");
  const [move, shuffle] = a.effects[2].steps;
  assert.equal(move.effect, "move-zone");
  assert.equal(move.tapped, true);
  assert.deepEqual(move.what, { ref: "related", number: "plural", filter: { cls: "card" },
    relation: { relation: "grouped-search-results", from: "preceding-search-loop", by: "searching-player" } });
  assert.equal(shuffle.effect, "shuffle");
  assert.equal(shuffle.coalesceEarlierLibrarySearches, true);
  rejects(text.replace("its controller searches their", "you search your"));
});

test("game-wide reveal and placement preserve one cohort per player and phase order", () => {
  const text = "Each player shuffles all permanents they own into their library, then reveals that many cards from the top of their library. Each player puts all artifact, creature, and land cards revealed this way onto the battlefield, then does the same for enchantment cards, then puts all cards revealed this way that weren't put onto the battlefield on the bottom of their library.";
  const a = one(text);
  assert.equal(a.kind, "spell");
  const [shuffle, reveal] = a.effects[0].steps;
  assert.equal(shuffle.effect, "shuffle-into-library");
  assert.equal(reveal.effect, "reveal");
  assert.equal(reveal.what.ref, "filter");
  assert.deepEqual(reveal.what.count, { amount: "permanents-moved-by-shuffle", scope: "for-each-shuffling-player" });
  const [first, enchantments, rest] = a.effects[1].steps;
  assert.equal(first.effect, "move-zone");
  assert.equal(first.what.ref, "all");
  assert.deepEqual(first.what.filter.anyOf, ["artifact", "creature", "land"].map(type => ({ types: [type] })));
  assert.equal(enchantments.effect, "move-zone");
  assert.equal(enchantments.what.ref, "all");
  assert.deepEqual(enchantments.what.filter.types, ["enchantment"]);
  assert.equal(rest.effect, "move-zone");
  assert.equal(rest.what.ref, "all");
  assert.deepEqual(rest.what.filter.relation, { relation: "grouped-reveal-results", by: "revealing-player",
    afterPlacement: "exclude-successful-battlefield-moves" });
  assert.deepEqual(rest.to, { zone: "library", owner: "their", position: "bottom" });
  rejects(text.replace("they own", "they control"));
});
