// Explicit resolution instructions for each member of a printed set.
// CR 608.2e/f, 101.4: each action spans the members before the next action;
// this is not a sequential JavaScript-style loop over whole instruction lists.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, ObjectFilter, ObjectRef, PlayerRef, Sentence } from "../ast.ts";
import type { Token } from "../lexer.ts";
import { Cursor } from "./cursor.ts";
import { isPluralPlayerRef, parseObjectRef, parsePlayerRef } from "./refs.ts";

function containsLoop(value: unknown): boolean {
  if (!value || typeof value !== "object") return false;
  if (Array.isArray(value)) return value.some(containsLoop);
  if ("effect" in value && value.effect === "for-each") return true;
  return Object.values(value).some(containsLoop);
}

/** Words through which later text could reach back into the loop: a member
 * ("it", "that creature"), the aggregated results ("those tokens", "the
 * rest", "the chosen permanents"), the events of its body ("this way") or
 * whether its actions happened ("if you do", "otherwise", "instead"). */
const BACK_REFERENCES = new Set(["it", "its", "it's", "itself", "them", "they", "they're", "they've", "their", "theirs",
  "that", "that's", "those", "these", "way", "rest", "other", "others", "another", "chosen", "same", "one", "ones", "x",
  "do", "does", "don't", "doesn't", "did", "didn't", "can't", "otherwise", "instead", "repeat", "process", "again"]);
/** Nouns after "the" that never name something the loop established. */
const INDEPENDENT_DEFINITES = new Set(["battlefield", "top", "bottom", "beginning", "end", "number", "game", "stack", "command",
  "monarch", "initiative", "ring", "next", "total", "amount", "greatest"]);

/** Text after the loop's sentence is a separate instruction only when it
 * cannot refer to the loop at all. Everything else would need a per-member
 * or aggregated scope (see parseForEach) and keeps the line failing. */
function independentOfLoop(tokens: Token[], earlierEvent?: string): boolean {
  const word = (index: number, value: string): boolean => {
    const token = tokens[index];
    return token?.kind === "word" && !token.possessive && token.value === value;
  };
  return tokens.every((token, index) => {
    if (token.kind === "punct" && token.value === '"') return false;
    if (token.kind !== "word") return true;
    // "<participle> this way" naming the event the loop's own set came
    // from, which the loop body never performs (Culmination of Studies,
    // Phabine, Boss's Confidant, Haunting Imitation).
    if (earlierEvent && (token.value === "this" || token.value === "way")) {
      const participle = token.value === "this" ? index - 1 : index - 2;
      if (word(participle, earlierEvent) && word(participle + 1, "this") && word(participle + 2, "way")) return true;
    }
    // "~ to its owner's hand": the self reference's own owner.
    if (token.value === "its" && tokens[index - 2]?.kind === "selfref" && (word(index - 1, "to") || word(index - 1, "into"))) return true;
    if (BACK_REFERENCES.has(token.value)) return false;
    if (token.value !== "the") return true;
    const next = tokens[index + 1];
    return next?.kind === "word" && INDEPENDENT_DEFINITES.has(next.value);
  });
}

type Loop = Extract<Effect, { effect: "for-each" }>;

/** A step that reveals, exiles, looks at, mills, scries or otherwise reads or
 * reorders cards of a library (searches are governed by CR 701.23h instead). */
function readsLibrary(step: Effect): boolean {
  if (step.effect === "scan-library" || step.effect === "mill" || step.effect === "scry" || step.effect === "surveil") return true;
  return hasOwnNode(step, node => node.ref === "library-cards" || ("top" in node && "of" in node));
}

/** CR 608.2f: a multi-step body that reads a library is processed
 * instruction-wise only when no two members can touch the same library: a
 * player loop in which each such step is that member's own action on that
 * member's own library. Shared libraries would need member-wise processing,
 * which the AST does not represent (Chaos Mutation, Indomitable Creativity). */
function ownLibrariesOnly(each: ObjectRef | PlayerRef, steps: Effect[]): boolean {
  const reads = steps.filter(readsLibrary);
  if (!reads.length) return true;
  if (!("player" in each)) return false;
  return reads.every(step => {
    const actor = "who" in step ? step.who : undefined;
    if (!actor || !("player" in actor) || actor.player !== "that-player") return false;
    let own = true;
    hasOwnNode(step, node => {
      if (node.zone === "library" && JSON.stringify(node.owner) !== '"that-player"' && JSON.stringify(node.owner) !== '"their"') own = false;
      return false;
    });
    return own;
  });
}

const PARTICIPLES: Record<string, string> = { exile: "exiled", reveal: "revealed", destroy: "destroyed", sacrifice: "sacrificed", discard: "discarded", mill: "milled" };

/** The participle of the earlier event that produced "each <card> <event>
 * this way", when the loop body cannot itself perform that event. */
function earlierEventParticiple(each: ObjectRef | PlayerRef, body: Sentence[]): string | undefined {
  if (!("ref" in each) || each.ref !== "each") return undefined;
  const relation = each.filter.relation;
  if (relation?.relation !== "event-result" || relation.scope !== "this-way" || relation.who) return undefined;
  const participle = PARTICIPLES[relation.event];
  if (!participle) return undefined;
  // Only bodies made of instructions that can never exile, reveal, destroy,
  // sacrifice, discard or mill anything (a keyword action such as discover,
  // CR 701.57, exiles cards) leave the earlier event as the sole antecedent.
  const steps = body.flatMap(sentence => sentence.condition || sentence.otherwise ? [null] : sentence.steps);
  return steps.every(step => step && EVENTLESS_EFFECTS.has(step.effect) && !JSON.stringify(step).includes('"effects"'))
    ? participle : undefined;
}

const EVENTLESS_EFFECTS = new Set<Effect["effect"]>(["create-token", "draw", "damage", "gain-life", "lose-life", "put-counters", "pump", "gain-abilities"]);

/** Loops whose following text needs no scope proof: nothing follows, it
 * cannot refer to the loop, or forEachContinuationsScoped proved its scope.
 * Any other loop makes its sentence list fail there. */
const settled = new WeakSet<Loop>();

/** Independent loops of member searches whose following shuffle, if any,
 * forEachContinuationsScoped must still match (CR 701.23h). */
const shuffleScopeUnchecked = new WeakSet<Loop>();

export function settleScopedContinuation(loop: Loop): void {
  settled.add(loop);
}

/** What the loop's single action handles, when that is at most one object
 * per member: the nouns a later plural reference can use for all of them
 * together, and the event that "... this way" names. */
type MemberResult = { nouns: Set<string>; events: string[]; memberActors?: true };

function memberResult(loop: Loop): MemberResult | null {
  const body = loop.effects[0];
  if (loop.effects.length !== 1 || body.condition || body.otherwise) return null;
  const one = (count: unknown) => JSON.stringify(count) === '{"amount":"fixed","value":1}';
  const searched = searchResult(loop, one);
  if (searched) return searched;
  if (body.steps.length !== 1) return null;
  const step = body.steps[0];
  if (step.effect === "create-token") return one(step.count) && !step.forEach ? { nouns: new Set(["token"]), events: [] } : null;
  const slot: unknown = "what" in step ? step.what : "on" in step ? step.on : undefined;
  if (!slot || typeof slot !== "object" || !("ref" in slot)) return null;
  const what = slot as ObjectRef;
  if ((what.ref !== "target" && what.ref !== "filter") || !one(what.count) || step.forEach) return null;
  const nouns = new Set<string>();
  const collect = (filter: ObjectFilter): void => {
    for (const type of filter.types ?? []) nouns.add(type);
    if (filter.cls) nouns.add(filter.cls);
    for (const part of [...filter.allOf ?? [], ...filter.anyOf ?? []]) collect(part);
  };
  collect(what.filter);
  // A permanent is named by its types or simply as a permanent.
  if (!nouns.has("card") && !nouns.has("spell") && !nouns.has("ability") && !nouns.has("source")) nouns.add("permanent");
  const events: Record<string, string[]> = { destroy: ["destroy"], exile: ["exile"], tap: ["tap"], sacrifice: ["sacrifice"],
    "choose-object": ["choose"], "declare-target": ["choose"] };
  return { nouns, events: events[step.effect] ?? [] };
}

/** CR 701.23: a search for one card per member, optionally moving "it" and
 * shuffling the searched library. Every found card is a member result, so
 * "those cards" after the loop names all of them together (Doubling Chant,
 * Clarion Ultimatum, Jace, Architect of Thought). */
function searchResult(loop: Loop, one: (count: unknown) => boolean): MemberResult | null {
  const [search, ...rest] = loop.effects[0].steps;
  if (search?.effect !== "search" || search.for.ref !== "filter" || !one(search.for.count) || search.for.upTo || search.forEach) return null;
  const events = ["search"];
  for (const step of rest) {
    if (step.optional || step.forEach) return null;
    if (step.effect === "shuffle") continue;
    if ((step.effect === "exile" || step.effect === "move-zone") && step.what.ref === "it") { events.push(step.effect); continue; }
    return null;
  }
  const nouns = new Set<string>();
  const { filter } = search.for;
  for (const type of filter.types ?? []) nouns.add(type);
  if (filter.cls) nouns.add(filter.cls);
  return nouns.has("card") ? { nouns, events, ...(isMemberActor(search.who) ? { memberActors: true } : {}) } : null;
}

/** "its controller" / "that player": each member supplies its own actor, so
 * "those players" after the loop can name all of them. */
function isMemberActor(who: PlayerRef): boolean {
  return who.player === "that-player" || (who.player === "controller" && "of" in who && who.of.ref === "it");
}

/** "If a spell cast this way would be put into a graveyard, exile it
 * instead." after a loop of casts watches every spell any member's cast
 * made, so it is created once, after the loop (Diluvian Primordial). */
function replacesCastResults(loop: Loop, next: Sentence): boolean {
  if (next.condition || next.otherwise || next.bindings || next.steps.length !== 1 || loop.effects[0].steps.length !== 1) return false;
  const [cast] = loop.effects[0].steps;
  const [replacement] = next.steps;
  if (cast.effect !== "play-card" || cast.instruction.action !== "cast" || replacement.effect !== "create-replacement") return false;
  const { event } = replacement.replacement;
  if (event.event !== "move-zone" || event.what.ref !== "filter") return false;
  const { filter } = event.what;
  if (filter.cls !== "spell" || filter.relation?.relation !== "event-result" || filter.relation.event !== "cast" ||
    filter.relation.scope !== "this-way" || filter.relation.who || Object.keys(filter).length !== 2) return false;
  // Nothing else in the replacement may name a loop member.
  return !hasNode(replacement.replacement.replace, node => node.player === "that-player" || node.ref === "it" || node.ref === "that");
}

/** A loop over objects cannot make "that player" its member; when its body
 * already names "that player", the phrase keeps that enclosing antecedent
 * after the loop as well (Dichotomancy, Haunting Echoes, Reap Intellect). */
function namesEnclosingPlayer(loop: Loop): boolean {
  if (!("ref" in loop.each)) return false;
  return hasNode(loop.effects, node => node.player === "that-player" || node.owner === "that-player" || node.control === "that-player");
}

/** Like hasNode, but never inside an ability the body creates (a delayed or
 * reflexive trigger, a quoted grant): those instructions happen later, in
 * their own resolution. */
function hasOwnNode(value: unknown, test: (node: Record<string, unknown>) => boolean): boolean {
  if (!value || typeof value !== "object") return false;
  if (Array.isArray(value)) return value.some(item => hasOwnNode(item, test));
  const node = value as Record<string, unknown>;
  if (test(node)) return true;
  if (node.effect === "create-delayed-trigger" || node.effect === "create-reflexive-trigger" || node.gained === "quoted") return false;
  return Object.values(node).some(item => hasOwnNode(item, test));
}

const isSearch = (node: Record<string, unknown>) => node.effect === "search";

function hasNode(value: unknown, test: (node: Record<string, unknown>) => boolean): boolean {
  if (!value || typeof value !== "object") return false;
  if (Array.isArray(value)) return value.some(item => hasNode(item, test));
  const node = value as Record<string, unknown>;
  return test(node) || Object.values(node).some(item => hasNode(item, test));
}

/** "Then that player shuffles." after a loop over objects whose body searched
 * that enclosing player's library: one shuffle of that library afterwards
 * (CR 701.23). */
function shufflesSearchedLibrary(loop: Loop, next: Sentence): boolean {
  if (next.condition || next.otherwise || next.bindings || next.steps.length !== 1 || !namesEnclosingPlayer(loop)) return false;
  const [shuffle] = next.steps;
  if (shuffle.effect !== "shuffle" || shuffle.optional || shuffle.forEach || shuffle.coalesceEarlierLibrarySearches ||
    !("player" in shuffle.who) || shuffle.who.player !== "that-player" || shuffle.library.zone !== "library" ||
    JSON.stringify(shuffle.library.owner) !== JSON.stringify(shuffle.who)) return false;
  return hasNode(loop.effects, node => node.effect === "search" && hasNode((node as { zones?: unknown }).zones,
    zone => zone.zone === "library" && JSON.stringify(zone.owner) === '"that-player"'));
}

/** CR 701.23h: members' searches of one library before a single later
 * shuffle of it are one search for all of those cards; the shuffle records
 * that. Accepted only when every member search is of exactly the library
 * that shuffle shuffles: your library and your shuffle (Doubling Chant), the
 * enclosing player's library and that player's shuffle in an object loop
 * (Haunting Echoes), or each controller's library and "each player who
 * searched their library this way" (From the Ashes). A player loop that
 * shuffles each member's own library inside the body searched it once
 * (Jace, Architect of Thought). With no shuffle after the loop, the wording
 * has none to record. */
function coalesceMemberSearches(loop: Loop, next: Sentence): boolean {
  const shuffles = next.steps.filter(step => step.effect === "shuffle");
  const searches = ownSearches(loop);
  // A search inside an ability the body creates happens later; it is never
  // the search of a shuffle made now.
  if (!searches.length) return !hasNode(loop.effects, isSearch) || !shuffles.length;
  if (hasOwnNode(loop.effects, node => node.effect === "shuffle")) {
    if (shuffles.length || !("player" in loop.each)) return false;
    return searches.every(search => librarySearched(search) === '"that-player"') &&
      hasOwnNode(loop.effects, node => node.effect === "shuffle" && JSON.stringify(node.who) === '{"player":"that-player"}');
  }
  if (!shuffles.length) return true;
  return markSearchShuffle(loop, next);
}

function ownSearches(loop: Loop): Extract<Effect, { effect: "search" }>[] {
  const found: Extract<Effect, { effect: "search" }>[] = [];
  hasOwnNode(loop.effects, node => { if (node.effect === "search") found.push(node as unknown as Extract<Effect, { effect: "search" }>); return false; });
  return found;
}

/** The serialized owner of the one library a search covers, or null. */
function librarySearched(search: Extract<Effect, { effect: "search" }>): string | null {
  const zones = search.zones.of;
  return zones.length === 1 && zones[0].zone === "library" ? JSON.stringify(zones[0].owner) : null;
}

function markSearchShuffle(loop: Loop, next: Sentence): boolean {
  if (next.condition || next.otherwise) return false;
  const shuffles = next.steps.filter(step => step.effect === "shuffle");
  if (shuffles.length !== 1) return false;
  const [shuffle] = shuffles;
  if (shuffle.effect !== "shuffle" || shuffle.optional || shuffle.forEach || shuffle.library.zone !== "library" ||
    JSON.stringify(shuffle.library.owner) !== JSON.stringify(shuffle.who)) return false;
  const shuffler = JSON.stringify(shuffle.who);
  const objectLoop = "ref" in loop.each;
  const searchers = '{"player":"filtered","base":{"player":"each-player"},"filter":{"searched":{"zone":{"zone":"library","owner":"their"},"scope":"this-way"}}}';
  const matches = ownSearches(loop).every(search => {
    const library = librarySearched(search);
    const who = JSON.stringify(search.who);
    if (library === '"your"') return who === '{"player":"you"}' && shuffler === '{"player":"you"}';
    if (library === '"that-player"') return objectLoop && shuffler === '{"player":"that-player"}';
    if (library === '"their"') return objectLoop && who === '{"player":"controller","of":{"ref":"it"}}' && shuffler === searchers;
    return false;
  });
  if (!matches) return false;
  shuffle.coalesceEarlierLibrarySearches = true;
  return true;
}

/** "Then each player who searched their library this way shuffles." names
 * every member's searcher at once (From the Ashes). */
function shufflesAllSearchers(loop: Loop, next: Sentence): boolean {
  if (next.condition || next.otherwise || next.bindings || next.steps.length !== 1) return false;
  const [shuffle] = next.steps;
  if (shuffle.effect !== "shuffle" || shuffle.optional || shuffle.forEach || !("player" in shuffle.who) ||
    shuffle.who.player !== "filtered" || shuffle.who.base.player !== "each-player" ||
    Object.keys(shuffle.who.filter).length !== 1 || shuffle.who.filter.searched?.scope !== "this-way" ||
    shuffle.library.zone !== "library" || JSON.stringify(shuffle.library.owner) !== JSON.stringify(shuffle.who)) return false;
  return hasOwnNode(loop.effects, isSearch) && !hasNode(loop.effects, node => node.effect === "create-delayed-trigger" ||
    node.effect === "create-reflexive-trigger");
}

/** The sentence names, in the plural, everything the loop's action handled:
 * "those creatures", "the chosen permanents", "them", "each land destroyed
 * this way". With at most one object per member that can only be the
 * objects of all members together, so the sentence is carried out once,
 * after the loop, and is no part of what is done for each member. A
 * reference to the single member ("that player") would contradict that. */
function refersToAllResults(sentence: Sentence, result: MemberResult, chose: boolean, enclosingPlayer: boolean, playerLoop: boolean): boolean {
  const describes = (filter: ObjectFilter): boolean => {
    const { types = [], cls } = filter;
    return (types.length > 0 || cls !== undefined) && types.every(type => result.nouns.has(type)) && (cls === undefined || result.nouns.has(cls));
  };
  let plural = false, member = false;
  const visit = (value: unknown): void => {
    if (!value || typeof value !== "object") return;
    if (Array.isArray(value)) { value.forEach(visit); return; }
    const node = value as Record<string, unknown>;
    // A nested loop binds its own member references.
    if (node.effect === "for-each") { visit(node.each); return; }
    if ((!enclosingPlayer && (node.player === "that-player" || node.control === "that-player" || node.owner === "that-player"))
      || node.player === "the-player" || (node.ref === "that" && node.plural !== true)) member = true;
    // "Those players" names a player loop's members; after a loop over objects
    // it has an antecedent only when each member supplied its own actor.
    if (node.player === "those-players" && !playerLoop && !result.memberActors) member = true;
    if (node.ref === "them") plural = true;
    if (node.ref === "that" && node.plural === true && describes(node.filter as ObjectFilter)) plural = true;
    if (chose && node.ref === "choice-result" && node.number === "plural" && describes(node.filter as ObjectFilter)) plural = true;
    const relation = node.relation as Record<string, unknown> | undefined;
    if (relation && relation.relation === "event-result" && relation.scope === "this-way" && relation.who === undefined
      && result.events.includes(relation.event as string) && describes(node as ObjectFilter)) plural = true;
    // "the exiled cards": every card the members' exile actions moved.
    if (relation && node.ref === "related" && node.number === "plural" && relation.relation === "event-result"
      && relation.scope === "contextual" && relation.who === undefined && result.events.includes(relation.event as string)
      && describes(node.filter as ObjectFilter)) plural = true;
    Object.values(node).forEach(visit);
  };
  visit(sentence);
  return plural && !member;
}

function unsettledLoops(value: unknown, found: Loop[] = []): Loop[] {
  if (!value || typeof value !== "object") return found;
  if (Array.isArray(value)) { for (const item of value) unsettledLoops(item, found); return found; }
  if ("effect" in value && value.effect === "for-each" && !settled.has(value as Loop)) found.push(value as Loop);
  for (const item of Object.values(value)) unsettledLoops(item, found);
  return found;
}

function searchesAnyLibrary(who: ObjectRef | PlayerRef, base: "each-player" | "each-opponent"): boolean {
  return "player" in who && who.player === "filtered" && who.base.player === base &&
    Object.keys(who.filter).length === 1 && who.filter.searched?.zone.zone === "library" &&
    who.filter.searched.zone.owner === "any";
}

/** CR 701.23h: a final shuffle can refer to all players who participated
 * in the preceding searches, including the controller's additional searches.
 * Each player shuffles their own library once. The rule combines any
 * repeated searches of the same library before this common shuffle. */
function endsWithSearchersShuffling(sentences: Sentence[], index: number, loop: Loop): boolean {
  if (index !== sentences.length - 2 || !searchesAnyLibrary(loop.each, "each-opponent") || loop.effects.length !== 1) return false;
  const [body] = loop.effects;
  const [search, placement] = body.steps;
  if (body.condition || body.otherwise || body.bindings || body.steps.length !== 2 ||
    search.effect !== "search" || search.who.player !== "you" || search.zones.selection !== "all"
    || search.zones.of.length !== 1 || search.zones.of[0].zone !== "library" || search.zones.of[0].owner !== "your"
    || placement.effect !== "move-zone" || placement.what.ref !== "it" || placement.to.zone !== "battlefield") return false;

  const optionalSearchGroups = sentences.slice(0, index).flatMap(sentence =>
    sentence.condition || sentence.otherwise || sentence.bindings ? [] : sentence.steps.filter(step =>
      step.effect === "optional-group" && step.who.player === "each-opponent" && step.steps.some(action =>
        action.effect === "search" && action.who.player === "each-opponent" && action.zones.selection === "all"
        && action.zones.of.length === 1 && action.zones.of[0].zone === "library" && action.zones.of[0].owner === "their")));
  if (optionalSearchGroups.length !== 1) return false;
  const next = sentences[index + 1];
  const [shuffle] = next.steps;
  if (next.condition || next.otherwise || next.bindings || next.steps.length !== 1 ||
    shuffle.effect !== "shuffle" || shuffle.optional || shuffle.forEach ||
    !searchesAnyLibrary(shuffle.who, "each-player") || shuffle.library.zone !== "library" ||
    JSON.stringify(shuffle.library.owner) !== JSON.stringify(shuffle.who)) return false;
  shuffle.coalesceEarlierLibrarySearches = true;
  return true;
}

/** The conditional singular "it" denotes the same exiled card that was
 * just given its own number of time counters (CR 400.7j, 608.2f, 702.62).
 * It cannot denote the entire pool; cards that already have suspend
 * still receive the counters from the first instruction. */
function foldEachCardSuspend(loop: Loop, previous: Sentence, next: Sentence): boolean {
  const [body] = loop.effects;
  const [count] = body.steps;
  const [grant] = next.steps;
  const { each } = loop;
  if (!("ref" in each) || each.ref !== "each" || each.filter.cls !== "card" ||
    each.filter.relation?.relation !== "event-result" || each.filter.relation.event !== "exile" ||
    each.filter.relation.scope !== "this-way" || each.filter.relation.who ||
    loop.effects.length !== 1 || body.condition || body.otherwise || body.bindings || body.steps.length !== 1 ||
    previous.condition || previous.otherwise || previous.steps.length !== 1 || previous.steps[0].effect !== "exile" ||
    previous.steps[0].optional || previous.steps[0].forEach ||
    count.effect !== "put-counters" || count.optional || count.forEach || count.upTo ||
    count.counter.type !== "time" || count.on.ref !== "that" || count.on.filter.cls !== "card" ||
    count.on.plural || Object.keys(count.on.filter).length !== 1 ||
    count.count.amount !== "attribute" || count.count.attribute !== "mana-value" || count.count.of.ref !== "it" ||
    next.conditionKind !== "if" || next.condition?.condition !== "not" ||
    next.condition.of.condition !== "has-keyword" || next.condition.of.keyword !== "suspend" ||
    next.condition.of.what.ref !== "it" || next.otherwise || next.bindings || next.steps.length !== 1 ||
    grant.effect !== "gain-abilities" || !("ref" in grant.what) || grant.what.ref !== "it" ||
    grant.optional || grant.forEach || grant.duration || grant.abilities.length !== 1 ||
    grant.abilities[0].gained !== "keyword" || grant.abilities[0].keyword.keyword !== "suspend") return false;
  loop.effects.push(next);
  settled.add(loop);
  return true;
}

/** The delayed trigger captures the targeted creature pool already tapped
 * and goaded, including any creature that was already tapped. A qualifying
 * damage event from any one member creates an occurrence (CR 603.2c, 603.7). */
export function parseGoadedCombatTrigger(
  c: Cursor, targeting: Sentence | undefined, goading: Sentence | undefined,
): Sentence | null {
  return c.attempt(c => {
    if (!c.isWord("until") || !targeting || targeting.condition || targeting.otherwise || targeting.bindings ||
      targeting.steps.length !== 1 || !goading || goading.condition || goading.otherwise || goading.bindings ||
      goading.steps.length !== 1) return null;
    const [loop] = targeting.steps;
    const [goad] = goading.steps;
    if (loop.effect !== "for-each" || !("player" in loop.each) || loop.each.player !== "each-opponent" ||
      loop.optional || loop.effects.length !== 1 || goad.effect !== "goad" || goad.optional || goad.duration || goad.forEach ||
      goad.what.ref !== "that" || !goad.what.plural || goad.what.filter.types?.length !== 1 ||
      goad.what.filter.types[0] !== "creature" || Object.keys(goad.what.filter).length !== 1) return null;
    const [body] = loop.effects;
    if (body.condition || body.otherwise || body.bindings || body.steps.length !== 1) return null;
    const [tap] = body.steps;
    if (tap.effect !== "tap" || tap.optional || tap.forEach || tap.what.ref !== "target" || !tap.what.upTo ||
      tap.what.count.amount !== "fixed" || tap.what.count.value !== 1 ||
      tap.what.filter.types?.length !== 1 || tap.what.filter.types[0] !== "creature" ||
      tap.what.filter.control !== "that-player" || Object.keys(tap.what.filter).length !== 2) return null;
    if (!c.words("until", "your", "next", "turn") || !c.punct(",") ||
      !c.words("whenever", "any", "of", "those", "creatures", "deals", "combat", "damage", "to", "a", "player") ||
      !c.punct(",") || !c.words("draw", "a", "card") || (!c.punct(".") && !c.done())) return null;
    return { steps: [
      { effect: "create-delayed-trigger",
        trigger: { trigger: "deals-damage", what: { ref: "from-pool", pool: goad.what, count: { amount: "fixed", value: 1 } },
          combat: true, to: "any" },
        effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: { amount: "fixed", value: 1 } }] }],
        occurrence: "each", targets: "delayed-ability", duration: { duration: "your-next-turn" } },
    ] };
  });
}

/** Decides the scope of the text after each loop in a complete sentence
 * list (CR 608.2c). A loop's `effects` are done for each member; a sentence
 * after the loop's own sentence is done once, afterwards. That reading is
 * accepted only when the first such sentence provably speaks of all members'
 * results together (refersToAllResults); the sentences after it then follow
 * an instruction outside the loop. A continuation that could be meant for
 * each member (Fighting Chance: "If you win the flip, ... that creature")
 * has no representation and fails. */
export function forEachContinuationsScoped(sentences: Sentence[]): boolean {
  for (let i = 0; i < sentences.length; i++) {
    // An otherwise independent loop of member searches: a following shuffle
    // must be exactly their CR 701.23h shuffle.
    for (const step of sentences[i].steps) {
      if (step.effect !== "for-each" || !shuffleScopeUnchecked.has(step)) continue;
      shuffleScopeUnchecked.delete(step);
      const next = sentences[i + 1];
      if (next && next.steps.some(later => later.effect === "shuffle") && !coalesceMemberSearches(step, next)) return false;
    }
    for (const loop of unsettledLoops(sentences[i])) {
      const next = sentences[i + 1];
      if (loop !== sentences[i].steps.at(-1) || sentences[i].otherwise || !next) return false;
      if (endsWithSearchersShuffling(sentences, i, loop)) { settled.add(loop); continue; }
      if (i > 0 && i === sentences.length - 2 && foldEachCardSuspend(loop, sentences[i - 1], next)) {
        sentences.splice(i + 1, 1);
        continue;
      }
      if (shufflesSearchedLibrary(loop, next) || shufflesAllSearchers(loop, next) || replacesCastResults(loop, next)) {
        if (!coalesceMemberSearches(loop, next)) return false;
        settled.add(loop);
        continue;
      }
      const result = memberResult(loop);
      if (!result) return false;
      const chose = loop.effects[0].steps[0].effect === "choose-object" || loop.effects[0].steps[0].effect === "declare-target";
      if (!refersToAllResults(next, result, chose, namesEnclosingPlayer(loop), "player" in loop.each)) return false;
      if (!coalesceMemberSearches(loop, next)) return false;
      settled.add(loop);
    }
  }
  return true;
}

/** The callback must use a fresh resolution context: enclosing actor elision,
 * stack-copy input proof and dice-table scope do not enter this body.
 * Require a single body sentence. Singular continuations can remain
 * per-member while plural ones can aggregate outputs; accepting either
 * without its scope would lose meaning (Fighting Chance / Saheeli), so later
 * sentences are allowed only when they provably do not refer to the loop or
 * forEachContinuationsScoped proves that they stand outside it.
 * Activation restrictions are handled separately by the outer ability. */
export function parseForEach(c: Cursor, parseBody: (body: Cursor) => Sentence[] | null): Effect | null {
  return c.attempt(c => {
    if (c.word("for") === null || !c.isWord("each")) return null;
    const following = c.peek(1);
    const establishedGroup = following?.kind === "word" && !following.possessive && following.value === "of";
    const each = c.attempt(c => {
      const player = parsePlayerRef(c);
      return player && isPluralPlayerRef(player) ? player : null;
    }) ?? c.attempt(c => {
      const object = parseObjectRef(c);
      return object?.ref === "each" ? object : null;
    });
    if (!each || !c.punct(",")) return null;

    const start = c.pos;
    let end = start;
    let quoted = false;
    for (; end < c.tokens.length; end++) {
      const token = c.tokens[end];
      if (token.kind === "punct" && token.value === '"') quoted = !quoted;
      if (!quoted && token.kind === "punct" && token.value === ".") break;
    }
    if (quoted || end === start) return null;
    const after = c.tokens[end + 1];
    const body = new Cursor(c.tokens.slice(start, end));
    const effects = parseBody(body);
    if (!body.done() || effects?.length !== 1 || containsLoop(effects)) return null;
    // CR 608.2f: reading cards from a library and then acting on them cannot
    // be simultaneous when members may share that library; instruction-wise
    // processing would run every read before any later action.
    if (effects[0].steps.length > 1 && !ownLibrariesOnly(each, effects[0].steps))
      return c.fail("member-wise processing of a library action");
    const independent = !after || (after.kind === "word" && !after.possessive && after.value === "activate") ||
      independentOfLoop(c.tokens.slice(end + 1), earlierEventParticiple(each, effects));
    // CR 608.2f (Soulfire Eruption example): a dependent action sequence for
    // each member of an established group is completed member by member when
    // it cannot be simultaneous. That is not instruction-wise processing, so
    // only a single action is accepted for "For each of <group>, ...".
    if (establishedGroup && effects[0].steps.length !== 1)
      return c.fail("a single action for each member of an established group");
    c.pos = end;
    const loop: Loop = { effect: "for-each", each, effects, processing: "instruction-wise" };
    if (independent) settled.add(loop);
    // A following shuffle may be the members' CR 701.23h shuffle.
    // A search of no library at all is unrelated to any library shuffle.
    if (independent && after && hasOwnNode(effects, node => node.effect === "search" &&
      hasNode((node as { zones?: unknown }).zones, zone => zone.zone === "library"))) shuffleScopeUnchecked.add(loop);
    return loop;
  });
}
