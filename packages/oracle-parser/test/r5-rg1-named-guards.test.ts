import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, Effect, Sentence, StaticEffect } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";
import { buildCardNameCatalog } from "../src/card-names.ts";

// Archive R5 RG-1: named grammar guards that now prove their scope, with the
// paired neighbours that must keep failing. Oracle text is copied exactly
// from the frozen Scryfall corpus SHA-256
// f8cb9da28e34b01dbd72504f059e012bda62db7d332acd2e3b43e75570e92738.
// CR 603.7a, 603.12, 608.2c/e/f, 611.3a, 701.23, 406.3, 607.2a:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt

const cardNames = buildCardNameCatalog([
  { name: "Bride's Gown", layout: "normal" }, { name: "Groom's Finery", layout: "normal" },
]);
function card(text: string, name: string, typeLine: string): Ability[] {
  const result = parseOracleText(text, name, { typeLine, cardNames });
  assert.ok(result.ok, `${name}: ${JSON.stringify(result.lines.filter(line => !line.ok))}`);
  return JSON.parse(JSON.stringify(result.abilities)) as Ability[];
}
function rejects(text: string, typeLine: string, name?: string): void {
  assert.equal(parseOracleText(text, name, { typeLine }).ok, false, text);
}
function effects(ability: Ability): Sentence[] {
  assert.ok("effects" in ability && Array.isArray(ability.effects), JSON.stringify(ability));
  return ability.effects as Sentence[];
}
function only<T extends Effect["effect"]>(sentence: Sentence, kind: T): Extract<Effect, { effect: T }> {
  assert.equal(sentence.steps.length, 1, JSON.stringify(sentence));
  assert.equal(sentence.steps[0].effect, kind, JSON.stringify(sentence));
  return sentence.steps[0] as Extract<Effect, { effect: T }>;
}
function staticOf(ability: Ability): StaticEffect {
  assert.equal(ability.kind, "static", JSON.stringify(ability));
  return (ability as Extract<Ability, { kind: "static" }>).effect;
}

describe("GG-delay: a created ability owns sentences that name objects only its body produces", () => {
  it("keeps the chosen exiled card's permission inside the delayed body (End-Blaze Epiphany)", () => {
    const [spell] = card("End-Blaze Epiphany deals X damage to target creature. When that creature dies this turn, exile a number of cards from the top of your library equal to its power, then choose a card exiled this way. Until the end of your next turn, you may play that card.", "End-Blaze Epiphany", "Instant");
    const [damage, delayedSentence] = effects(spell);
    assert.equal(damage.steps[0].effect, "damage");
    const delayed = only(delayedSentence, "create-delayed-trigger");
    assert.equal(delayed.effects.length, 2);
    assert.deepEqual(delayed.effects[0].steps.map(step => step.effect), ["exile", "choose-object"]);
    assert.equal(only(delayed.effects[1], "grant-play-permission").duration.duration, "end-of-your-next-turn");
  });

  it("keeps the sacrifice of the body's tokens in the delayed body (Dalkovan Encampment)", () => {
    const abilities = card("This land enters tapped unless you control a Swamp or a Mountain.\n{T}: Add {W}.\n{2}{W}, {T}: Whenever you attack this turn, create two 1/1 red Warrior creature tokens that are tapped and attacking. Sacrifice them at the beginning of the next end step.", "Dalkovan Encampment", "Land");
    const delayed = only(effects(abilities[2])[0], "create-delayed-trigger");
    assert.equal(delayed.occurrence, "each");
    assert.equal(delayed.effects.length, 2);
    assert.equal(delayed.effects[0].steps[0].effect, "create-token");
    const sacrifice = only(only(delayed.effects[1], "create-delayed-trigger").effects[0], "sacrifice");
    assert.deepEqual(sacrifice.what, { ref: "them" });
  });

  it("continues a reflexive body through a chain of references to its target (Grishnákh, Brash Instigator)", () => {
    const [trigger] = card("When Grishnákh enters, amass Orcs 2. When you do, until end of turn, gain control of target nonlegendary creature an opponent controls with power less than or equal to the amassed Army's power. Untap that creature. It gains haste until end of turn.", "Grishnákh, Brash Instigator", "Legendary Creature — Goblin Soldier");
    const reflexive = effects(trigger).flatMap(sentence => sentence.steps).find(step => step.effect === "create-reflexive-trigger");
    assert.ok(reflexive?.effect === "create-reflexive-trigger");
    assert.deepEqual(reflexive.effects.map(sentence => sentence.steps[0].effect), ["gain-control", "untap", "gain-abilities"]);
  });

  it("keeps the exiled cards' cast in the reflexive body (Doom Reigns Supreme)", () => {
    const abilities = card("Whenever a Villain you control enters, each opponent loses 1 life and you gain 1 life. Put a plan counter on this enchantment.\nWhen the fifth plan counter is put on this enchantment, sacrifice it. When you do, target opponent exiles the top five cards of their library. You may cast up to two spells from among the exiled cards without paying their mana costs.", "Doom Reigns Supreme", "Enchantment — Plan");
    const reflexive = effects(abilities[1]).flatMap(sentence => sentence.steps).find(step => step.effect === "create-reflexive-trigger");
    assert.ok(reflexive?.effect === "create-reflexive-trigger");
    assert.deepEqual(reflexive.effects.map(sentence => sentence.steps[0].effect), ["exile", "play-card"]);
  });

  it("resolves 'that player' in 'cards that player exiled this way' to the body's exiler", () => {
    const [probe] = card("When an artifact enters this turn, target opponent exiles the top two cards of their library. You may cast a spell from among cards that player exiled this way without paying its mana cost.", "Probe", "Sorcery");
    const delayed = effects(probe).flatMap(sentence => sentence.steps).find(step => step.effect === "create-delayed-trigger");
    assert.ok(delayed?.effect === "create-delayed-trigger");
    assert.deepEqual(delayed.effects.map(sentence => sentence.steps[0].effect), ["exile", "play-card"]);
  });

  it("keeps the remaining library cards inside the delayed body (Devourer of Destiny, Codie)", () => {
    const devourer = card("You may reveal this card from your opening hand. If you do, at the beginning of your first upkeep, look at the top four cards of your library. You may put one of those cards back on top of your library. Exile the rest.\nWhen you cast this spell, exile target permanent that's one or more colors.", "Devourer of Destiny", "Creature — Eldrazi");
    assert.ok(JSON.stringify(devourer[0]).includes('"steps":[{"effect":"exile","what":{"ref":"rest"}}]'));
    const codie = card("You can't cast permanent spells.\n{4}, {T}: Add {W}{U}{B}{R}{G}. When you next cast a spell this turn, exile cards from the top of your library until you exile an instant or sorcery card with lesser mana value. Until end of turn, you may cast that card without paying its mana cost. Put each other card exiled this way on the bottom of your library in a random order.", "Codie, Vociferous Codex", "Legendary Artifact Creature — Book Construct");
    const delayed = JSON.stringify(codie[1]);
    assert.ok(delayed.includes('"create-delayed-trigger"'));
    const body = (codie[1] as { effects?: Sentence[] }).effects?.flatMap(sentence => sentence.steps).find(step => step.effect === "create-delayed-trigger");
    assert.ok(body?.effect === "create-delayed-trigger");
    assert.deepEqual(body.effects.map(sentence => sentence.steps[0].effect), ["scan-library", "grant-play-permission", "move-zone"]);
  });

  it("still rejects a fresh target, a following condition, or an intervening condition", () => {
    // Time to Feed: the fight happens now, not when the creature dies.
    rejects("Choose target creature an opponent controls. When that creature dies this turn, you gain 3 life. Target creature you control fights that creature.", "Sorcery", "Time to Feed");
    rejects("When this enchantment enters, sacrifice a creature. When you do, you gain 3 life. If the sacrificed creature was a Hamster, draw a card.", "Enchantment");
    // Earthbender Ascension: the reflexive "if" would also govern the trample grant.
    rejects("Landfall — Whenever a land you control enters, put a quest counter on this enchantment. When you do, if it has four or more quest counters on it, put a +1/+1 counter on target creature you control. It gains trample until end of turn.", "Enchantment", "Earthbender Ascension");
    // A quoted ability's references never tie a sentence to the delayed body.
    rejects("{T}: When an artifact enters this turn, create a 1/1 black Rat creature token. This artifact gains \"Whenever a creature you control attacks, untap it.\"", "Artifact");
    // "That player" must be exactly one player at the body's own level.
    rejects("When an artifact enters this turn, each opponent exiles the top two cards of their library. You may cast a spell from among cards that player exiled this way without paying its mana cost.", "Sorcery");
    rejects("When an artifact enters this turn, target creature gains \"{T}: Target opponent exiles the top two cards of their library.\" You may cast a spell from among cards that player exiled this way without paying its mana cost.", "Sorcery");
    // A reflexive body is held to the same actor check.
    rejects("You may exile the top card of your library. When you do, target opponent exiles the top two cards of their library. Put that card from among cards you exiled this way into your hand.", "Sorcery");
    // "cards you exiled this way" names your exile, not the opponent's in the body.
    rejects("Exile the top two cards of your library. When an artifact enters this turn, target opponent exiles the top two cards of their library. You may cast a spell from among cards you exiled this way without paying its mana cost.", "Sorcery");
    // A look introduces no exiled cards: "the exiled cards" are the enclosing ability's.
    rejects("Exile the top two cards of your library. When an artifact enters this turn, look at the top two cards of your library. You may cast a spell from among the exiled cards without paying its mana cost.", "Sorcery");
    // The body introduced no card, so "that card" has no body-local antecedent.
    rejects("Exile the top card of your library. When a creature dies this turn, create a 1/1 black Rat creature token. You may play that card this turn.", "Sorcery");
  });
});

describe("GG-quant: text after a loop that provably stands outside it", () => {
  it("names all searched cards together (Doubling Chant, Clarion Ultimatum, Jace, Architect of Thought)", () => {
    const [chant] = card("For each creature you control, you may search your library for a creature card with the same name as that creature. Put those cards onto the battlefield, then shuffle.", "Doubling Chant", "Sorcery");
    const [loopSentence, put] = effects(chant);
    const loop = only(loopSentence, "for-each");
    assert.equal(loop.effects[0].steps[0].effect, "search");
    assert.deepEqual(put.steps.map(step => step.effect), ["move-zone", "shuffle"]);
    // CR 701.23h: the members' searches before the one shuffle are one search.
    assert.equal(only({ ...put, steps: put.steps.slice(1) }, "shuffle").coalesceEarlierLibrarySearches, true);
    card("Choose five permanents you control. For each of those permanents, you may search your library for a card with the same name as that permanent. Put those cards onto the battlefield tapped, then shuffle.", "Clarion Ultimatum", "Sorcery");
    const jace = card("+1: Until your next turn, whenever a creature an opponent controls attacks, it gets -1/-0 until end of turn.\n−2: Reveal the top three cards of your library. An opponent separates those cards into two piles. Put one pile into your hand and the other on the bottom of your library in any order.\n−8: For each player, search that player's library for a nonland card and exile it, then that player shuffles. You may cast those cards without paying their mana costs.", "Jace, Architect of Thought", "Legendary Planeswalker — Jace");
    const [ultimateLoop, cast] = effects(jace[2]);
    assert.deepEqual(only(ultimateLoop, "for-each").effects[0].steps.map(step => step.effect), ["search", "exile", "shuffle"]);
    assert.equal(only(cast, "play-card").instruction.action, "cast");
  });

  it("shuffles the enclosing player's searched library once (Haunting Echoes, Dichotomancy)", () => {
    const [echoes] = card("Exile all cards from target player's graveyard other than basic land cards. For each card exiled this way, search that player's library for all cards with the same name as that card and exile them. Then that player shuffles.", "Haunting Echoes", "Sorcery");
    const sentences = effects(echoes);
    assert.equal(sentences.length, 3);
    assert.deepEqual(only(sentences[2], "shuffle").who, { player: "that-player" });
    assert.equal(only(sentences[2], "shuffle").coalesceEarlierLibrarySearches, true);
    const [dichotomancy] = card("For each tapped nonland permanent target opponent controls, search that player's library for a card with the same name as that permanent. Put those cards onto the battlefield under your control, then that player shuffles.\nSuspend 3—{1}{U}{U} (Rather than cast this card from your hand, you may pay {1}{U}{U} and exile it with three time counters on it. At the beginning of your upkeep, remove a time counter. When the last is removed, you may cast it without paying its mana cost.)", "Dichotomancy", "Sorcery");
    assert.deepEqual(effects(dichotomancy)[1].steps.map(step => step.effect), ["move-zone", "shuffle"]);
  });

  it("marks a standalone 'Then shuffle.' after member searches as their one shuffle", () => {
    const [probe] = card("For each creature you control, search your library for a creature card with the same name as that creature and put it into your hand. Then shuffle.", "Probe", "Sorcery");
    assert.equal(only(effects(probe)[1], "shuffle").coalesceEarlierLibrarySearches, true);
    // No library was searched: the shuffle is independent and stays unflagged.
    const [graveyard] = card("For each creature you control, search your graveyard for a creature card and put it into your hand. Then shuffle.", "Probe", "Sorcery");
    assert.equal(only(effects(graveyard)[1], "shuffle").coalesceEarlierLibrarySearches, undefined);
    // One search per member with no shuffle wording has nothing to record.
    const [lands] = card("For each opponent, that player searches their library for a basic land card and puts it into their hand.", "Probe", "Sorcery");
    assert.equal(effects(lands).length, 1);
  });

  it("shuffles every player who searched this way once (From the Ashes)", () => {
    const [ashes] = card("Destroy all nonbasic lands. For each land destroyed this way, its controller may search their library for a basic land card and put it onto the battlefield. Then each player who searched their library this way shuffles.", "From the Ashes", "Sorcery");
    const shuffle = only(effects(ashes)[2], "shuffle");
    assert.ok("player" in shuffle.who && shuffle.who.player === "filtered");
    assert.equal(shuffle.coalesceEarlierLibrarySearches, true);
  });

  it("creates one cast-results replacement after the casting loop (Diluvian Primordial)", () => {
    const abilities = card("Flying\nWhen this creature enters, for each opponent, you may cast up to one target instant or sorcery card from that player's graveyard without paying its mana cost. If a spell cast this way would be put into a graveyard, exile it instead.", "Diluvian Primordial", "Creature — Avatar");
    const [loop, replacement] = effects(abilities[1]);
    assert.equal(only(loop, "for-each").effects[0].steps[0].effect, "play-card");
    assert.equal(only(replacement, "create-replacement").replacement.event.event, "move-zone");
  });

  it("reads a later 'revealed/exiled this way' as the earlier event the loop never performs (Culmination of Studies, Phabine, Haunting Imitation)", () => {
    const [culmination] = card("Exile the top X cards of your library. For each land card exiled this way, create a Treasure token. For each blue card exiled this way, draw a card. For each red card exiled this way, Culmination of Studies deals 1 damage to each opponent.", "Culmination of Studies", "Sorcery");
    assert.deepEqual(effects(culmination).map(sentence => sentence.steps[0].effect), ["exile", "for-each", "for-each", "for-each"]);
    const phabine = card("Creature tokens you control have haste.\nParley — At the beginning of combat on your turn, each player reveals the top card of their library. For each land card revealed this way, you create a 1/1 green and white Citizen creature token. Then creatures you control get +1/+1 until end of turn for each nonland card revealed this way. Then each player draws a card.", "Phabine, Boss's Confidant", "Legendary Creature — Cat Advisor");
    assert.deepEqual(effects(phabine[1]).map(sentence => sentence.steps[0].effect), ["reveal", "for-each", "pump", "draw"]);
    const [imitation] = card("Each player reveals the top card of their library. For each creature card revealed this way, create a token that's a copy of that card, except it's 1/1, it's a Spirit in addition to its other types, and it has flying. If no creature cards were revealed this way, return Haunting Imitation to its owner's hand.", "Haunting Imitation", "Sorcery");
    assert.equal(effects(imitation).length, 3);
  });

  it("still rejects per-member continuations and unanchored plural actors", () => {
    // Tolarian Contempt: "That creature" is each opponent's own choice.
    rejects("At the beginning of your end step, for each opponent, choose up to one target creature they control with a rejection counter on it. That creature's owner puts it on their choice of the top or bottom of their library.", "Enchantment");
    // Tidal Flats: a per-member optional payment and its outcome.
    rejects("{U}{U}: For each attacking creature without flying, its controller may pay {1}. If that player doesn't, creatures you control blocking that creature gain first strike until end of turn.", "Enchantment");
    // "Those players" has no antecedent when you made every search.
    rejects("Destroy X target artifacts and/or creatures. For each permanent destroyed this way, you reveal cards from the top of your library until an artifact or creature card is revealed and exile that card. Those players put the exiled cards onto the battlefield, then shuffle.", "Sorcery");
    // The loop body itself reveals, so "revealed this way" could name it.
    rejects("Each player reveals the top card of their library. For each land card revealed this way, reveal the top two cards of your library. Then each player draws a card for each nonland card revealed this way.", "Sorcery");
    // Indomitable Creativity: instruction-wise, every scan would run before
    // any exile, so two scans could find the same card (CR 608.2f).
    rejects("Destroy X target artifacts and/or creatures. For each permanent destroyed this way, its controller reveals cards from the top of their library until an artifact or creature card is revealed and exiles that card. Those players put the exiled cards onto the battlefield, then shuffle.", "Sorcery", "Indomitable Creativity");
    // A search inside a delayed body happens later; no shuffle now can be its shuffle.
    rejects("Destroy all nonbasic lands. For each land destroyed this way, at the beginning of the next end step, its controller may search their library for a basic land card and put it onto the battlefield. Then each player who searched their library this way shuffles.", "Sorcery");
    // Members may share a library: instruction-wise, all reveals would precede any exile.
    rejects("Destroy X target artifacts and/or creatures. For each permanent destroyed this way, its controller reveals cards from the top of their library until an artifact or creature card is revealed and exiles that card.", "Sorcery");
    // Chaos Mutation: the AST has only instruction-wise processing, but the
    // reveal/put sequence must run member by member (CR 608.2f), even with
    // different controllers.
    rejects("Exile any number of target creatures controlled by different players. For each creature exiled this way, its controller reveals cards from the top of their library until they reveal a creature card, puts that card onto the battlefield, then puts the rest on the bottom of their library in a random order.", "Sorcery", "Chaos Mutation");
    // "their" names the target player's library, shared by every member.
    rejects("For each opponent, target player reveals cards from the top of their library until they reveal a creature card, then exiles that card.", "Sorcery");
    // Scrying then drawing reorders and draws from the same library each time.
    rejects("For each creature you control, scry 1, then draw a card.", "Sorcery");
    // Several players may search one library: not one searcher's repeated search (CR 701.23h).
    rejects("Target opponent reveals their hand. For each creature, its controller searches that player's library for a basic land card and puts it into their hand. Then that player shuffles.", "Sorcery");
    // Shuffling a card into the library also shuffles it (CR 701.24c), so it
    // must be an allowlisted CR 701.23h shuffle too.
    rejects("For each creature you control, search your library for a basic land card. Put those cards onto the battlefield, then shuffle this spell into your library.", "Sorcery");
    // A shuffle two sentences after the loop is never checked as the members' shuffle.
    rejects("For each creature you control, you may search your library for a creature card with the same name as that creature. Put those cards onto the battlefield. Then shuffle.", "Sorcery");
    // The member's scry and your draw touch different libraries in every iteration.
    rejects("For each player, that player scries 1, then you draw a card.", "Sorcery");
    // The searchers' shuffle must include every searcher (you may be one of them).
    rejects("Destroy all nonbasic lands. For each land destroyed this way, its controller may search their library for a basic land card and put it onto the battlefield. Then each opponent who searched their library this way shuffles.", "Sorcery");
    // Without "controlled by different players" two members may share a library.
    rejects("Exile any number of target creatures. For each creature exiled this way, its controller reveals cards from the top of their library until they reveal a creature card, puts that card onto the battlefield, then puts the rest on the bottom of their library in a random order.", "Sorcery");
    // Discover exiles cards (CR 701.57), so "exiled this way" could name it.
    rejects("Exile the top two cards of your library. For each land card exiled this way, discover 3. For each creature card exiled this way, draw a card.", "Sorcery");
    // An object loop's body never named "that player", so it may be a member's.
    rejects("For each creature target opponent controls, search your library for a card with the same name as that creature. Then that player shuffles.", "Sorcery");
  });
});

describe("GG-cond: a trailing condition after ', then' governs only the final instruction", () => {
  it("splits the sentence at 'then' (Swallowed by Leviathan, Phantasmal Sphere, Retraced Image)", () => {
    // A fixed single library card is not a group.
    const [top] = card("Draw a card, then exile the top card of your library if you control a Swamp.", "Probe", "Sorcery");
    assert.deepEqual(effects(top).map(sentence => [sentence.steps.map(step => step.effect), sentence.conditionKind ?? null]),
      [[["draw"], null], [["exile"], "if"]]);
    // Only the instruction after the last ", then" is conditional.
    const [several] = card("Draw a card, gain 2 life, then lose 1 life if you control a Swamp.", "Probe", "Sorcery");
    assert.deepEqual(effects(several).map(sentence => [sentence.steps.map(step => step.effect), sentence.conditionKind ?? null]),
      [[["draw", "gain-life"], null], [["lose-life"], "if"]]);
    const [swallowed] = card("Choose target spell. Surveil 2, then counter the chosen spell unless its controller pays {1} for each card in your graveyard. (To surveil 2, look at the top two cards of your library, then put any number of them into your graveyard and the rest on top of your library in any order.)", "Swallowed by Leviathan", "Instant");
    const [, surveil, counter] = effects(swallowed);
    assert.deepEqual(surveil.steps.map(step => step.effect as string), ["surveil"]);
    assert.equal(surveil.condition, undefined);
    assert.equal(only(counter, "counter").effect, "counter");
    assert.equal(counter.conditionKind, "unless");
    assert.equal(counter.condition?.condition, "pays");
    const sphere = card("Flying\nAt the beginning of your upkeep, put a +1/+1 counter on this creature, then sacrifice this creature unless you pay {1} for each +1/+1 counter on it.\nWhen this creature leaves the battlefield, target opponent creates an X/X blue Orb creature token with flying, where X is the number of +1/+1 counters on this creature.", "Phantasmal Sphere", "Creature — Illusion");
    const [counters, sacrifice] = effects(sphere[1]);
    assert.equal(only(counters, "put-counters").effect, "put-counters");
    assert.equal(counters.condition, undefined);
    assert.equal(only(sacrifice, "sacrifice").effect, "sacrifice");
    assert.equal(sacrifice.conditionKind, "unless");
    const [image] = card("Reveal a card in your hand, then put that card onto the battlefield if it has the same name as a permanent.", "Retraced Image", "Sorcery");
    const [reveal, put] = effects(image);
    assert.equal(reveal.condition, undefined);
    assert.equal(put.conditionKind, "if");
    assert.equal(put.conditionPosition, "after");
    assert.equal(put.steps.length, 1);
  });

  it("still rejects 'and' coordination, a leading condition, and per-step conditions", () => {
    // The condition's pronoun must have the row's one antecedent: here "it"
    // could be the drawn card, and "that card" has no singular antecedent.
    rejects("{T}: Draw a card, then sacrifice this creature if it's tapped.", "Creature — Human");
    rejects("Draw two cards, then put that card onto the battlefield if it's a creature card.", "Sorcery");
    // Only allowlisted instruction shapes split; here the top card of each player's library.
    rejects("Draw a card, then exile the top card of each player's library if it's a creature card.", "Sorcery");
    rejects("Counter target activated ability from an artifact source and destroy that artifact if it's on the battlefield.", "Instant");
    // Volatile Stormdrake: "If you do" governs both instructions.
    rejects("When this creature enters, exchange control of this creature and target creature an opponent controls. If you do, you get {E}{E}{E}{E}, then sacrifice that creature unless you pay an amount of {E} equal to its mana value.", "Creature — Drake");
    rejects("At the beginning of combat on your turn, choose up to one target creature you control. Until end of turn, that creature gets +2/+0, gains trample if it's red, and gains vigilance if it's white.", "Legendary Creature — Angel");
    // A player group acts per member; one sentence condition loses that partition.
    rejects("Each player reveals the top card of their library, then puts it onto the battlefield if it's a permanent card.", "Sorcery");
    rejects("Whenever Vaevictis Asmadi attacks, for each player, choose target permanent that player controls. Those players sacrifice those permanents. Each player who sacrificed a permanent this way reveals the top card of their library, then puts it onto the battlefield if it's a permanent card.", "Legendary Creature — Elder Dragon", "Vaevictis Asmadi, the Dire");
    // A group anywhere in the instruction, here the damage source, may make
    // the condition per member.
    rejects("Each creature you control deals 1 damage to you, then gain 2 life if that creature is a Goblin.", "Sorcery");
    // A group in the conditional instruction: the condition may be per member.
    rejects("Draw a card, then this spell deals 2 damage to each opponent if that player controls a Swamp.", "Sorcery");
    // The split prefix is checked like any sentence: "one pile" needs a separation.
    rejects("Put one pile into your hand, then gain 2 life if you control a Swamp.", "Sorcery");
    // A trailing definition could serve either half of the split.
    rejects("Draw X cards, then gain 2 life unless an opponent pays {X}, where X is the number of creatures you control.", "Sorcery");
  });
});

describe("GG-perm: static abilities that were only readable as resolving instructions", () => {
  it("reads 'has <keyword> unless <state>' as a negated static condition (Tadeas, Juniper Ascendant)", () => {
    const abilities = card("Reach\nTadeas has hexproof unless it's attacking.\nWhenever a creature you control with reach attacks, untap it and it can't be blocked by creatures with greater power this combat.\nWhenever one or more creatures you control deal combat damage to a player, draw a card.", "Tadeas, Juniper Ascendant", "Legendary Creature — Human Monk");
    const effect = staticOf(abilities[1]);
    assert.equal(effect.static, "modify");
    assert.deepEqual(effect.static === "modify" && effect.condition, { condition: "not", of: { condition: "status", what: { ref: "it" }, status: "attacking" } });
    // A payment or an action outcome is not a continuously checked state.
    rejects("This creature has hexproof unless an opponent pays {2}.", "Creature — Human");
    rejects("This creature has hexproof unless you do.", "Creature — Human");
    rejects("This creature has hexproof unless you don't cast a spell this way.", "Creature — Human");
    rejects("This creature has hexproof unless you rolled 6.", "Creature — Human");
    rejects("This creature has hexproof unless the amount of mana spent to cast it is greater than 2.", "Creature — Human");
    // "it" after a group subject could be any member or the source.
    rejects("Each creature you control has flying unless this creature is tapped or it's attacking.", "Creature — Human");
    rejects("Creatures you control have hexproof unless it's attacking.", "Enchantment");
    // The current phase is a state.
    const [phase] = card("This creature has hexproof unless it's your main phase.", "Probe", "Creature — Human");
    assert.equal(staticOf(phase).static, "modify");
  });

  it("grants the linked-exile look and play permissions (Bane Alley Broker, Kheru Mind-Eater, Colfenor's Plans)", () => {
    const broker = card("{T}: Draw a card, then exile a card from your hand face down.\nYou may look at cards exiled with this creature.\n{U}{B}, {T}: Return a card exiled with this creature to its owner's hand.", "Bane Alley Broker", "Creature — Human Rogue");
    const look = staticOf(broker[1]);
    assert.equal(look.static, "look-permission");
    const linked = { ref: "related", number: "plural", filter: { cls: "card" }, relation: { relation: "linked-exile", source: { ref: "this", noun: "creature" } } };
    assert.deepEqual(look.static === "look-permission" && look.permission.what, [linked]);
    const kheru = card("Menace\nWhenever this creature deals combat damage to a player, that player exiles a card from their hand face down.\nYou may look at cards exiled with this creature, and you may play lands and cast spells from among those cards.", "Kheru Mind-Eater", "Creature — Vampire");
    const group = staticOf(kheru[2]);
    assert.equal(group.static, "group");
    // "those cards" is replaced by the same linked-exile pool, not left as an anaphor.
    assert.ok(!JSON.stringify(group).includes('"ref":"that"'));
    card("When this enchantment enters, exile the top seven cards of your library face down.\nYou may look at the cards exiled with this enchantment, and you may play lands and cast spells from among those cards.\nSkip your draw step.\nYou can't cast more than one spell each turn.", "Colfenor's Plans", "Enchantment");
    // Another player's look, or cards exiled by some other object, stay unsupported.
    rejects("Each player may look at cards they exiled with this enchantment.", "Enchantment");
  });

  it("closes a static condition at its sentence boundary (Tenza, Godo's Maul; Bride's Gown; Radiant Destiny)", () => {
    const tenza = card("Equipped creature gets +1/+1. As long as it's legendary, it gets an additional +2/+2. As long as it's red, it has trample.\nEquip {1} ({1}: Attach to target creature you control. Equip only as a sorcery.)", "Tenza, Godo's Maul", "Legendary Artifact — Equipment");
    const group = staticOf(tenza[0]);
    assert.ok(group.static === "group");
    const equipped = { ref: "equipped", noun: "creature" };
    assert.deepEqual(group.effects.map(effect => effect.static === "modify" ? effect.condition ?? null : effect.static), [
      null,
      { condition: "is-filter", what: equipped, filter: { supertypes: ["legendary"] } },
      { condition: "is-filter", what: equipped, filter: { colors: ["red"] } },
    ]);
    const gown = card("Equipped creature gets +2/+0. It gets an additional +0/+2 and has first strike as long as an Equipment named Groom's Finery is attached to a creature you control.\nEquip {2} ({2}: Attach to target creature you control. Equip only as a sorcery.)", "Bride's Gown", "Artifact — Equipment");
    const gownGroup = staticOf(gown[0]);
    assert.ok(gownGroup.static === "group" && gownGroup.effects.length === 2);
    assert.ok(gownGroup.effects[0].static === "modify" && !gownGroup.effects[0].condition);
    assert.ok(gownGroup.effects[1].static === "modify" && gownGroup.effects[1].condition && gownGroup.effects[1].modifiers.length === 2);
    const destiny = card("Ascend (If you control ten or more permanents, you get the city's blessing for the rest of the game.)\nAs this enchantment enters, choose a creature type.\nCreatures you control of the chosen type get +1/+1. As long as you have the city's blessing, they also have vigilance.", "Radiant Destiny", "Enchantment");
    const destinyGroup = staticOf(destiny[2]);
    assert.ok(destinyGroup.static === "group" && destinyGroup.effects[1].static === "modify");
    assert.deepEqual(destinyGroup.effects[1].condition, { condition: "designation", who: { player: "you" }, designation: "city's-blessing" });
  });

  it("still rejects a pronoun that disagrees with the preceding subject", () => {
    rejects("Creatures you control get +1/+1. As long as you control a Swamp, it has vigilance.", "Enchantment");
    rejects("Equipped creature gets +1/+1. As long as it's red, they have trample.", "Artifact — Equipment");
  });
});

describe("guards whose remaining cases stay rejected", () => {
  it("keeps an unrepresented mana-type anaphor and emerge-sacrifice gate unsupported", () => {
    // Incubation Druid: "three mana of that type" needs one shared type, which
    // a could-produce production cannot state.
    rejects("{T}: Add one mana of any type that a land you control could produce. If this creature has a +1/+1 counter on it, add three mana of that type instead.", "Creature — Elf Druid", "Incubation Druid");
    rejects("When you sacrifice this creature while casting a spell with emerge, create a 3/2 colorless Eldrazi Horror creature token.", "Creature — Human Horror", "Foul Emissary");
  });
});
