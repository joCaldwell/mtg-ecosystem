// Single-template recognisers: library searches and reveals.
import { isDeepStrictEqual } from "node:util";
import type { Effect, ObjectRef, Sentence } from "../../../ast.ts";
import type { Analyzer } from "../analyzer.ts";
import { child, combine, objectCardinality, type Candidate, type Context } from "../context.ts";
import { isRecord } from "../predicates.ts";

export function controllerSearch(this: Analyzer, effect: Extract<Effect, { effect: "optional-group" }>, path: string, context: Context,
  precedingEffect?: string): boolean {
  if (context.barrier || context.phase !== "resolution" || context.decider || !precedingEffect ||
      !isDeepStrictEqual(effect.who, { player: "controller", of: { ref: "it" } }) || effect.steps.length !== 3)
    return false;
  const [search, move, shuffle] = effect.steps;
  if (search.effect !== "search" || move.effect !== "move-zone" || shuffle.effect !== "shuffle" ||
      Object.keys(search).length !== 4 || Object.keys(move).length !== 5 || Object.keys(shuffle).length !== 3 ||
      !isDeepStrictEqual(search.who, effect.who) || !isDeepStrictEqual(move.who, effect.who) ||
      !isDeepStrictEqual(shuffle.who, effect.who) ||
      !isDeepStrictEqual(search.zones, { selection: "all", of: [{ zone: "library", owner: "their" }] }) ||
      search.for.ref !== "filter" || search.for.filter.cls !== "card" || objectCardinality(search.for) !== "one" ||
      !isDeepStrictEqual(move.what, { ref: "that", filter: { cls: "card" } }) ||
      !isDeepStrictEqual(move.to, { zone: "battlefield" }) || move.tapped !== true ||
      shuffle.library.zone !== "library" || !isDeepStrictEqual(shuffle.library.owner, effect.who)) return false;
  const original = this.result.nodes.find(node => node.id === this.id(precedingEffect, "event"));
  const target = this.at(child(precedingEffect, "what"));
  if (original?.node !== "event" || original.operation !== "exile" || original.scope !== context.ability ||
      original.optional || original.conditional || !target || !this.announced(target) || target.cardinality !== "one" ||
      !target.description?.types?.includes("creature") || target.description.cls === "card" || target.description.cls === "spell")
    return false;

  const rules = ["110.2a", "400.7", "608.2b", "608.2c", "608.2d", "608.2h", "701.23b", "701.24a"];
  const whoPath = child(path, "who");
  const player = this.node({ id: this.id(whoPath, "controller-before-exile"), path: whoPath, node: "derived-player",
    scope: context.ability, role: "controller", of: target.id, information: "current-or-last-known", rules });
  for (const at of [whoPath, `${path}/steps/0/who`, `${path}/steps/1/who`, `${path}/steps/2/who`, `${path}/steps/2/library/owner`]) {
    this.bound({ ref: "it" }, child(at, "of"), target.id, rules);
    this.bound(effect.who, at, player, rules);
  }

  const actor: Candidate = { id: player, domain: "player", cardinality: "one" };
  const searchPath = `${path}/steps/0`;
  const movePath = `${path}/steps/1`;
  const shufflePath = `${path}/steps/2`;
  this.walk(search.zones, child(searchPath, "zones"), { ...context, actor });
  const selection = this.object(search.for, child(searchPath, "for"), context);
  if (!selection) return true;
  const decision = this.event(path, context, "optional-controller-search", [actor], {
    optional: true, decider: player, ...(context.conditional ? { conditional: true as const } : {}),
  });
  const searchEvent = this.event(searchPath, context, search.effect, [actor, selection], { conditional: true, condition: decision });
  const found = this.eventResult(searchPath, searchEvent, "objects", "one", "same-objects", selection.description);
  this.bound(move.what, child(movePath, "what"), found.id, rules);
  const entry = this.event(movePath, context, move.effect, [actor, found], { conditional: true, condition: decision, requiresResult: found.id });
  const shuffled = this.event(shufflePath, context, shuffle.effect, [actor], { conditional: true, condition: decision });
  this.node({ id: this.id(path, "controller-search"), path, node: "controller-search", scope: context.ability,
    exile: original.id, player, decision, search: searchEvent, entry, shuffle: shuffled, rules });
  this.engineFeature("controller-search", path,
    "Use the exiled target's controller or last known controller for the single optional search, entry and shuffle sequence, including searches that find nothing.", rules);
  context.objects = [];
  context.players = [];
  context.quantities = [];
  context.mentionedObject = true;
  return true;
}

export function libraryTopMana(this: Analyzer, effect: Extract<Effect, { effect: "look-top-may-reveal-for-mana" }>, path: string, context: Context): void {
  this.walk(effect, path, context);
  const controller = this.at(child(path, "who"));
  const library = child(path, "library");
  const reveal = child(path, "reveal");
  const revealIf = child(path, "revealIf");
  const to = child(path, "to");
  const libraryOwner = this.at(child(library, "owner"));
  const handOwner = this.at(child(to, "owner"));
  if (!controller || !libraryOwner || !handOwner) return;

  const lookEvent = this.event(library, context, "look-at", combine([controller], [libraryOwner]));
  const looked = this.eventResult(library, lookEvent, "objects", "one", "same-objects", { cls: "card" });
  const revealEvent = this.event(reveal, context, "reveal", [controller, looked],
    { optional: true, decider: controller.id, conditional: true, requiresResult: looked.id });
  this.node({ id: this.id(revealIf, "eligibility"), path: revealIf, node: "colored-mana-symbol-count",
    scope: context.ability, card: looked.id, guardedEvent: revealEvent, comparison: effect.revealIf,
    rules: ["107.4e", "107.4f", "202.1", "608.2d"] });
  const revealed = this.eventResult(reveal, revealEvent, "objects", "one", "same-objects", { cls: "card" }, looked.id);
  this.event(child(path, "mana"), context, "add-mana", [controller, revealed],
    { conditional: true, requiresResult: revealed.id });
  this.event(to, context, "move-zone", [looked, handOwner], { requiresResult: looked.id });
  this.engineFeature(effect.effect, path,
    "Look privately at the top card, offer its public reveal only when eligible, and allocate the printed amount among its colors only after revealing it. Put the looked-at card into hand in either branch.",
    ["105.2", "106.1a", "107.4e", "107.4f", "400.1", "608.2c", "608.2d", "609.3", "701.20"]);
}
/** CR 614.15, 701.23b, and 608.2c–d: choose one search alternative,
 * reveal its actual finds and partition them before either destination move. */
export function replacedSearchPartition(this: Analyzer, sentence: Sentence, path: string, context: Context): boolean {
  if (context.barrier || context.decider || context.phase !== "resolution" ||
      Object.keys(sentence).length !== 1 || sentence.steps.length !== 3) return false;
  const [replace, reveal, split] = sentence.steps;
  if (replace.effect !== "self-replacement" || replace.replacementChoice ||
      replace.targetPolicy !== "retain-original" || replace.optional || replace.decidedBy || replace.forEach || replace.forEachVote ||
      !("effect" in replace.original) || replace.original.effect !== "search" ||
      !Array.isArray(replace.instead) || replace.instead.length !== 1 || replace.instead[0].effect !== "search" ||
      !isDeepStrictEqual(reveal, { effect: "reveal", who: { player: "you" },
        what: { ref: "that", filter: { cls: "card" }, plural: true } }) ||
      split.effect !== "simultaneous" || Object.keys(split).length !== 2 || split.steps.length !== 2 ||
      !isDeepStrictEqual(split.steps, [
        { effect: "move-zone", what: { ref: "from-pool", pool: { ref: "them" }, count: { amount: "fixed", value: 1 } },
          to: { zone: "battlefield" }, tapped: true },
        { effect: "move-zone", what: { ref: "rest" }, to: { zone: "hand", owner: "your" } },
      ])) return false;
  const original = replace.original;
  const alternate = replace.instead[0];
  if (original.for.ref !== "filter" || alternate.for.ref !== "filter" || !original.for.upTo || !alternate.for.upTo ||
      original.for.count.amount !== "fixed" || alternate.for.count.amount !== "fixed" ||
      original.for.count.value < 1 || alternate.for.count.value < 1 ||
      original.for.count.value === alternate.for.count.value || original.for.filter.cls !== "card" ||
      Object.keys(original).length !== 4 || Object.keys(original.for).length !== 4 ||
      !isDeepStrictEqual(original.who, { player: "you" }) ||
      !isDeepStrictEqual(original.zones, { selection: "all", of: [{ zone: "library", owner: "your" }] }) ||
      !isDeepStrictEqual(alternate, { ...original, for: { ...original.for, count: alternate.for.count } })) return false;
  const at = `${path}/steps/0`;
  const originalPath = child(at, "original");
  const alternatePath = `${at}/instead/0`;
  this.walk(replace.condition, child(at, "condition"), context);
  this.effect(original, originalPath, { ...this.branch(context), conditional: true });
  this.effect(alternate, alternatePath, { ...this.branch(context), conditional: true });
  const rules = ["608.2c", "608.2d", "609.3", "614.15", "701.20", "701.23b"];
  const choice = this.id(at, "replacement-search-result");
  const before = this.eventResult(originalPath, this.id(originalPath, "event"), "objects", "many", "same-objects", original.for.filter);
  const after = this.eventResult(alternatePath, this.id(alternatePath, "event"), "objects", "many", "same-objects", alternate.for.filter);
  const pool = this.node({ id: choice, path: at, node: "replacement-search-result", scope: context.ability,
    original: before.id, replacement: after.id, condition: replace.condition, cardinality: "many", rules });
  const revealer = this.player({ player: "you" }, `${path}/steps/1/who`, context);
  const revealed = this.bound({ ref: "that", filter: { cls: "card" }, plural: true }, `${path}/steps/1/what`, pool, rules);
  this.event(`${path}/steps/1`, context, "reveal", [revealed, ...(revealer ? [revealer] : [])]);
  const splitPath = `${path}/steps/2`;
  const move = `${splitPath}/steps/0`;
  const hand = `${splitPath}/steps/1`;
  const selection = { ref: "from-pool", pool: { ref: "them" }, count: { amount: "fixed", value: 1 } } as const;
  this.bound(selection.pool, `${move}/what/pool`, pool, rules);
  const chosen = this.selection(selection, child(move, "what"), context, false);
  if (!chosen) {
    this.diagnostic("unsupported-scope", move, "The found pool needs its distinct pre-move battlefield selection.", rules);
    return true;
  }
  const remaining = this.node({ id: this.id(hand, "found-remainder"), path: child(hand, "what"), node: "remainder",
    scope: context.ability, pool, excluding: chosen.id, rules });
  const rest = this.bound({ ref: "rest" }, child(hand, "what"), remaining, rules);
  const mover = this.player({ player: "you" }, child(move, "who"), context);
  const handOwner = this.player({ player: "you" }, child(hand, "who"), context);
  this.walk({ zone: "hand", owner: "your" }, child(hand, "to"), context);
  const battleMove = this.event(move, context, "move-zone", [chosen, ...(mover ? [mover] : [])]);
  const handMove = this.event(hand, context, "move-zone", [rest, ...(handOwner ? [handOwner] : [])]);
  this.node({ id: this.id(splitPath, "simultaneous-search-partition"), path: splitPath, node: "simultaneous-search-partition",
    scope: context.ability, pool, selected: chosen.id, remaining, battlefield: battleMove, hand: handMove,
    chooser: context.you, count: "one-if-found", order: "choose-before-both-moves", rules });
  this.engineFeature("search-result-partition", path,
    "Search only under the chosen alternative. Reveal what was found, split that result before either move, and put the chosen card onto the battlefield tapped and the rest into hand simultaneously.", rules);
  context.objects = [];
  context.quantities = [];
  context.mentionedObject = true;
  context.uncertainRevealOrigin = true;
  return true;
}

export function searchStage(this: Analyzer, search: Extract<Effect, { effect: "search" }>, path: string, context: Context, actor: Candidate,
  rules: string[], grouped = false): { event: string; found: Candidate } | undefined {
  this.bound(search.who, child(path, "who"), actor.id, rules);
  this.walk(search.zones, child(path, "zones"), { ...context, actor });
  const selection = this.object(search.for, child(path, "for"), { ...context, actor });
  if (!selection) return;
  const event = this.event(path, context, "search", [actor, selection], context.conditional ? { conditional: true } : {});
  return { event, found: this.eventResult(path, event, "objects", grouped ? "many" : "one", "same-objects", { cls: "card" }) };
}

/** CR 608.2h, 701.23h and 730.3. Count actual original creature
 * departures per last controller, even if the exile successors moved again. */
export function searchesForExiledCreatures(this: Analyzer, sentences: Sentence[], path: string, context: Context): boolean {
  if (!context.spellAbility || context.uncertainExileOrigin || context.exileResults?.length ||
      sentences.length !== 3 || sentences.some(sentence => Object.keys(sentence).length !== 1) ||
      sentences[0].steps.length !== 1 || sentences[1].steps.length !== 1 || sentences[2].steps.length !== 2) return false;
  const [exile] = sentences[0].steps;
  const [loop] = sentences[1].steps;
  const [place, shuffle] = sentences[2].steps;
  const each = { ref: "each", filter: { types: ["creature"], relation: { relation: "event-result", event: "exile", scope: "this-way" } } } satisfies ObjectRef;
  const controller = { player: "controller", of: { ref: "it" } } as const;
  const players = { player: "those-players" } as const;
  const results = { ref: "related", number: "plural", filter: { cls: "card" },
    relation: { relation: "grouped-search-results", from: "preceding-search-loop", by: "searching-player" } } as const;
  if (exile.effect !== "exile" || Object.keys(exile).length !== 2 || exile.what.ref !== "target" ||
      objectCardinality(exile.what) !== "one" || !exile.what.selectionId ||
      !isDeepStrictEqual(exile.what.filter, { types: ["creature"], control: "not-you" }) ||
      loop.effect !== "for-each" || loop.effects.length !== 1 || Object.keys(loop.effects[0]).length !== 1 ||
      loop.effects[0].steps.length !== 1 || !isDeepStrictEqual(loop, { effect: "for-each", each, processing: "instruction-wise",
        groupedSearch: "last-controllers-of-exiled-creatures", effects: loop.effects }) ||
      !isDeepStrictEqual(place, { effect: "move-zone", what: results, to: { zone: "battlefield" }, who: players, tapped: true }) ||
      !isDeepStrictEqual(shuffle, { effect: "shuffle", who: players, library: { zone: "library", owner: players },
        coalesceEarlierLibrarySearches: true })) return false;
  const [search] = loop.effects[0].steps;
  if (search.effect !== "search" || Object.keys(search).length !== 4 ||
      !isDeepStrictEqual(search.who, controller) ||
      !isDeepStrictEqual(search.zones, { selection: "all", of: [{ zone: "library", owner: "their" }] }) ||
      search.for.ref !== "filter" || Object.keys(search.for).length !== 3 ||
      !isDeepStrictEqual(search.for.count, { amount: "fixed", value: 1 }) ||
      search.for.filter.cls !== "card" || !isDeepStrictEqual(search.for.filter.types, ["land"]) ||
      Object.keys(search.for.filter).some(key => !["cls", "types", "supertypes", "subtypes"].includes(key))) return false;
  const first = `${path}/0/steps/0`;
  this.effect(exile, first, context);
  const rules = ["101.4", "110.2a", "400.7", "608.2f", "608.2h", "701.23b", "701.23h", "702.96a", "712.21e", "730.3"];
  const at = `${path}/1/steps/0`;
  const actual = this.node({ id: this.id(at, "exiled-creature-controllers"), path: at, node: "exiled-creature-controllers",
    scope: context.ability, event: this.id(first, "event"), information: "before-move", cardinality: "many", rules });
  this.bound(each, child(at, "each"), actual, rules);
  const actor = this.node({ id: this.id(at, "searching-controller"), path: at, node: "instruction-member",
    scope: context.ability, group: actual, domain: "player", rules });
  const who: Candidate = { id: actor, domain: "player", cardinality: "one" };
  const searchPath = `${at}/effects/0/steps/0`;
  const original = this.node({ id: this.id(searchPath, "exiled-creature"), path: searchPath, node: "instruction-member",
    scope: context.ability, group: actual, domain: "object", rules });
  this.bound({ ref: "it" }, `${searchPath}/who/of`, original, rules);
  const stage = this.searchStage(search, searchPath, context, who, rules, true);
  if (!stage) return true;
  const searched = this.eventResult(child(searchPath, "searchers"), stage.event, "player", "many", "same-objects");
  const destination = `${path}/2/steps/0`;
  this.bound(players, child(destination, "who"), searched.id, rules);
  this.bound(results, child(destination, "what"), stage.found.id, rules);
  const move = this.event(destination, context, "move-zone", [searched, stage.found]);
  const shufflePath = `${path}/2/steps/1`;
  this.bound(players, child(shufflePath, "who"), searched.id, rules);
  this.bound(players, `${shufflePath}/library/owner`, searched.id, rules);
  const finish = this.event(shufflePath, context, "shuffle", [searched]);
  this.node({ id: this.id(at, "coalesced-library-search"), path: at, node: "coalesced-library-search", scope: context.ability,
    grouping: "same-player-library-before-shuffle", actualSearchers: searched.id, shuffle: finish,
    stages: [{ search: stage.event, actor, quota: { kind: "per-last-controller", source: actual }, found: stage.found.id,
      place: move, entersTapped: true }], rules });
  this.engineFeature("coalesced-library-search", at,
    "Count each actual exiled creature once by its last controller, combine their search slots, and place that player's finds tapped before each actual searcher shuffles.", rules);
  context.objects = [];
  context.players = [];
  context.quantities = [];
  context.mentionedObject = true;
  context.uncertainExileOrigin = true;
  return true;
}

/** CR 701.23h combines the controller's library searches without
 * changing the visible placement and optional-offer instruction order. */
export function searchesForOpponentOffers(this: Analyzer, sentences: Sentence[], path: string, context: Context): boolean {
  if (!context.spellAbility || sentences.length !== 4 || sentences.some(sentence => Object.keys(sentence).length !== 1) ||
      sentences[0].steps.length !== 2 || sentences.slice(1).some(sentence => sentence.steps.length !== 1)) return false;
  const [first, firstPlace] = sentences[0].steps;
  const [offer] = sentences[1].steps;
  const [bonus] = sentences[2].steps;
  const [shuffle] = sentences[3].steps;
  const you = { player: "you" } as const;
  const opponents = { player: "each-opponent" } as const;
  const searched = { searched: { zone: { zone: "library", owner: "any" }, scope: "this-way" } } as const;
  const actualOpponents = { player: "filtered", base: opponents, filter: searched } as const;
  const allSearchers = { player: "filtered", base: { player: "each-player" }, filter: searched } as const;
  const move = { effect: "move-zone", what: { ref: "it" }, to: { zone: "battlefield" } } as const;
  if (first.effect !== "search" || Object.keys(first).length !== 4 ||
      !isDeepStrictEqual(first.who, you) || !isDeepStrictEqual(first.zones, { selection: "all", of: [{ zone: "library", owner: "your" }] }) ||
      first.for.ref !== "filter" || Object.keys(first.for).length !== 3 ||
      !isDeepStrictEqual(first.for.count, { amount: "fixed", value: 1 }) ||
      first.for.filter.cls !== "card" || !isDeepStrictEqual(first.for.filter.types, ["land"]) ||
      Object.keys(first.for.filter).some(key => !["cls", "types", "supertypes", "subtypes"].includes(key)) ||
      !isDeepStrictEqual(firstPlace, move) || offer.effect !== "optional-group" || offer.steps.length !== 2 ||
      !isDeepStrictEqual(offer, { effect: "optional-group", who: opponents, steps: [
        { ...first, who: opponents, zones: { selection: "all", of: [{ zone: "library", owner: "their" }] } }, { ...move, who: opponents },
      ] }) || bonus.effect !== "for-each" || !isDeepStrictEqual(bonus, { effect: "for-each", each: actualOpponents,
        effects: [{ steps: [first, move] }], processing: "instruction-wise" }) ||
      !isDeepStrictEqual(shuffle, { effect: "shuffle", who: allSearchers, library: { zone: "library", owner: allSearchers },
        coalesceEarlierLibrarySearches: true })) return false;
  const [opponentSearch] = offer.steps;
  const [bonusSearch] = bonus.effects[0].steps;
  if (opponentSearch.effect !== "search" || bonusSearch.effect !== "search") return false;
  const rules = ["101.4", "110.2a", "608.2c", "608.2e", "608.2f", "701.23b", "701.23f", "701.23g", "701.23h"];
  const self: Candidate = { id: context.you, domain: "player", cardinality: "one" };
  const at = `${path}/1/steps/0`;
  const players = this.player(opponents, child(at, "who"), context);
  if (!players) return true;
  const id = this.node({ id: this.id(at, "opponent-searcher"), path: at, node: "instruction-member",
    scope: context.ability, group: players.id, domain: "player", rules });
  const actor: Candidate = { id, domain: "player", cardinality: "one" };
  const run = (search: Extract<Effect, { effect: "search" }>, searchPath: string, movePath: string, who: Candidate, offered = false) => {
    const stage = this.searchStage(search, searchPath, { ...context, conditional: offered }, who, rules);
    if (!stage) return;
    const found = this.bound({ ref: "it" }, child(movePath, "what"), stage.found.id, rules);
    this.bound(offered ? opponents : you, child(movePath, "who"), who.id, rules);
    return { ...stage, place: this.event(movePath, context, "move-zone", [found, who], { requiresResult: found.id }) };
  };
  const open = run(first, `${path}/0/steps/0`, `${path}/0/steps/1`, self);
  const choose = run(opponentSearch, `${at}/steps/0`, `${at}/steps/1`, actor, true);
  const then = `${path}/2/steps/0`;
  const repeat = run(bonusSearch, `${then}/effects/0/steps/0`, `${then}/effects/0/steps/1`, self);
  if (!open || !choose || !repeat) return true;
  const actual = this.eventResult(child(at, "participants"), choose.event, "player", "many", "same-objects");
  this.bound(opponents, `${then}/each/base`, players.id, rules);
  this.bound(actualOpponents, child(then, "each"), actual.id, rules);
  const shufflePath = `${path}/3/steps/0`;
  const active = this.node({ id: this.id(shufflePath, "actual-library-searchers"), path: shufflePath, node: "actual-library-searchers",
    scope: context.ability, searches: [open.event, choose.event, repeat.event], cardinality: "many", rules });
  const everyone = this.player({ player: "each-player" }, `${shufflePath}/who/base`, context);
  if (everyone) this.bound({ player: "each-player" }, `${shufflePath}/library/owner/base`, everyone.id, rules);
  const shuffling = this.bound(allSearchers, child(shufflePath, "who"), active, rules);
  this.bound(allSearchers, `${shufflePath}/library/owner`, active, rules);
  const finish = this.event(shufflePath, context, "shuffle", [shuffling]);
  this.node({ id: this.id(at, "coalesced-library-search"), path: at, node: "coalesced-library-search", scope: context.ability,
    grouping: "same-player-library-before-shuffle", actualSearchers: active, shuffle: finish,
    stages: [
      { search: open.event, actor: self.id, quota: { kind: "fixed", count: 1 }, found: open.found.id, place: open.place },
      { search: choose.event, actor: id, quota: { kind: "optional-per-opponent", group: players.id }, found: choose.found.id, place: choose.place },
      { search: repeat.event, actor: self.id, quota: { kind: "one-per-actual-searcher", source: actual.id }, found: repeat.found.id, place: repeat.place },
    ], rules });
  this.engineFeature("coalesced-library-search", at,
    "Place the controller's first find, collect optional opponent searches in APNAP order and place their finds together, then apply the controller's bonus before the participating players shuffle.", rules);
  context.objects = [];
  context.players = [];
  context.quantities = [];
  context.mentionedObject = true;
  return true;
}

export function graveyardReturnScope(this: Analyzer, effect: Effect, path: string, context: Context): Context {
  const alternatives = context.zoneAlternatives;
  if (!alternatives?.length || context.barrier || context.phase !== "resolution" || context.decider || context.conditional ||
      !context.lexicalAbilityPath || path !== `${context.lexicalAbilityPath}/effects/0/steps/0` ||
      effect.effect !== "return-to-hand" || effect.to !== "your" || effect.optional || effect.decidedBy || effect.forEach ||
      effect.forEachVote || effect.what.ref !== "target" || !effect.what.selectionId || objectCardinality(effect.what) !== "one" ||
      effect.what.filter.cls !== "card" || !effect.what.filter.other || !isDeepStrictEqual(effect.what.filter.types, ["creature"]) ||
      !isDeepStrictEqual(effect.what.filter.zone, { zone: "graveyard", owner: "your" }) ||
      Object.keys(effect.what.filter).some(key => !["cls", "types", "other", "zone", "implicitComparison"].includes(key)))
    return context;
  const comparison = effect.what.filter.implicitComparison;
  if (comparison && (!isDeepStrictEqual(comparison, { attribute: "toughness", op: "lt" }) ||
      alternatives.some(branch => !branch.previous || !branch.successor))) return context;
  const filterPath = `${path}/what/filter`;
  const rules = ["400.7e", "601.2c", "603.6c", "603.10a", "608.2b", "608.2h"];
  const view = this.node({ id: this.id(filterPath, "trigger-graveyard-view"), path: filterPath,
    node: "trigger-graveyard-view", scope: context.ability, alternatives, successorZone: "graveyard",
    ...(comparison ? { comparison, information: "before-move" as const } : {}), rules });
  this.engineFeature("event-history", filterPath,
    "Exclude only the initial graveyard successor of the matched trigger event and use its departed permanent for any last-known comparison.", rules);
  return { ...context, graveyardExclusion: { filterPath, view } };
}

/** "Search your library and/or graveyard for …": one search instruction by
 * the ability's controller over distinct zones they own. The zone choice and
 * the single found-card selection belong to that one search event (CR
 * 701.23a, 701.23b), as for a library-only search. Another player's zones,
 * a repeated or delegated search and duplicated zones stay unsupported. */
export function ownZonesSearch(this: Analyzer, effect: Record<string, unknown>, context: Context): boolean {
  if (context.barrier || context.phase !== "resolution" || effect.decidedBy || effect.forEach || effect.forEachVote ||
      !isDeepStrictEqual(effect.who, { player: "you" }) || !isRecord(effect.zones) || !Array.isArray(effect.zones.of)) return false;
  const { selection, of } = effect.zones;
  if (selection !== "one" && selection !== "one-or-more" && selection !== "all") return false;
  const zones = of.map(zone => isRecord(zone) && Object.keys(zone).length === 2 && zone.owner === "your" ? zone.zone : undefined);
  return zones.length > 1 && new Set(zones).size === zones.length &&
    zones.every(zone => zone === "library" || zone === "graveyard" || zone === "hand");
}

/** "If you search your library this way": the one search instruction of the
 * preceding sentence, the only search of this ability, made by the same
 * player over a set of zones that includes the named one (CR 608.2c,
 * 701.23). Whether that zone was searched is a fact of that event,
 * including a search that found nothing. */
export function searchedZoneLink(this: Analyzer, condition: Record<string, unknown>, path: string, context: Context): boolean {
  const previous = context.precedingSentence;
  if (context.barrier || condition.scope !== "this-way" || !previous || !isRecord(condition.zone)) return false;
  const searches = previous.steps.flatMap((step, index) => step.effect === "search" ? [{ step, at: `${previous.path}/steps/${index}` }]
    : step.effect === "optional-group" ? step.steps.flatMap((inner, nested) =>
      inner.effect === "search" ? [{ step: inner, at: `${previous.path}/steps/${index}/steps/${nested}` }] : []) : []);
  const events = this.result.nodes.filter(node => node.node === "event" && node.operation === "search" && node.scope === context.ability);
  if (searches.length !== 1 || events.length !== 1 || events[0].path !== searches[0].at) return false;
  const [{ step }] = searches;
  if (step.effect !== "search" || !isDeepStrictEqual(condition.who, step.who) || !isDeepStrictEqual(condition.who, { player: "you" }) ||
      !step.zones.of.some(zone => isDeepStrictEqual(zone, condition.zone))) return false;
  const rules = ["608.2c", "701.23a"];
  this.node({ id: this.id(path, "event-link"), path, node: "event-link", scope: context.ability, action: "search", event: events[0].id, rules });
  return true;
}
