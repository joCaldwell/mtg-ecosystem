// Explicit resolution instructions for each member of a printed set.
// CR 608.2e/f, 101.4: each action spans the members before the next action;
// this is not a sequential JavaScript-style loop over whole instruction lists.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, ObjectFilter, ObjectRef, Sentence } from "../ast.ts";
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
function independentOfLoop(tokens: Token[]): boolean {
  return tokens.every((token, index) => {
    if (token.kind === "punct" && token.value === '"') return false;
    if (token.kind !== "word") return true;
    if (BACK_REFERENCES.has(token.value)) return false;
    if (token.value !== "the") return true;
    const next = tokens[index + 1];
    return next?.kind === "word" && INDEPENDENT_DEFINITES.has(next.value);
  });
}

type Loop = Extract<Effect, { effect: "for-each" }>;

/** Loops whose following text needs no scope proof: nothing follows, it
 * cannot refer to the loop, or forEachContinuationsScoped proved its scope.
 * Any other loop makes its sentence list fail there. */
const settled = new WeakSet<Loop>();

/** What the loop's single action handles, when that is at most one object
 * per member: the nouns a later plural reference can use for all of them
 * together, and the event that "... this way" names. */
function memberResult(loop: Loop): { nouns: Set<string>; events: string[] } | null {
  const body = loop.effects[0];
  if (loop.effects.length !== 1 || body.condition || body.otherwise || body.steps.length !== 1) return null;
  const step = body.steps[0];
  const one = (count: unknown) => JSON.stringify(count) === '{"amount":"fixed","value":1}';
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

/** The sentence names, in the plural, everything the loop's action handled:
 * "those creatures", "the chosen permanents", "them", "each land destroyed
 * this way". With at most one object per member that can only be the
 * objects of all members together, so the sentence is carried out once,
 * after the loop, and is no part of what is done for each member. A
 * reference to the single member ("that player") would contradict that. */
function refersToAllResults(sentence: Sentence, result: { nouns: Set<string>; events: string[] }, chose: boolean): boolean {
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
    if (node.player === "that-player" || node.player === "the-player" || node.control === "that-player"
      || node.owner === "that-player" || (node.ref === "that" && node.plural !== true)) member = true;
    if (node.ref === "them") plural = true;
    if (node.ref === "that" && node.plural === true && describes(node.filter as ObjectFilter)) plural = true;
    if (chose && node.ref === "choice-result" && node.number === "plural" && describes(node.filter as ObjectFilter)) plural = true;
    const relation = node.relation as Record<string, unknown> | undefined;
    if (relation && relation.relation === "event-result" && relation.scope === "this-way" && relation.who === undefined
      && result.events.includes(relation.event as string) && describes(node as ObjectFilter)) plural = true;
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
    for (const loop of unsettledLoops(sentences[i])) {
      const next = sentences[i + 1];
      const result = memberResult(loop);
      if (loop !== sentences[i].steps.at(-1) || sentences[i].otherwise || !next || !result) return false;
      const chose = loop.effects[0].steps[0].effect === "choose-object" || loop.effects[0].steps[0].effect === "declare-target";
      if (!refersToAllResults(next, result, chose)) return false;
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
    const independent = !after || (after.kind === "word" && !after.possessive && after.value === "activate") || independentOfLoop(c.tokens.slice(end + 1));
    const body = new Cursor(c.tokens.slice(start, end));
    const effects = parseBody(body);
    if (!body.done() || effects?.length !== 1 || containsLoop(effects)) return null;
    // CR 608.2f (Soulfire Eruption example): a dependent action sequence for
    // each member of an established group is completed member by member when
    // it cannot be simultaneous. That is not instruction-wise processing, so
    // only a single action is accepted for "For each of <group>, ...".
    if (establishedGroup && effects[0].steps.length !== 1)
      return c.fail("a single action for each member of an established group");
    c.pos = end;
    const loop: Loop = { effect: "for-each", each, effects, processing: "instruction-wise" };
    if (independent) settled.add(loop);
    return loop;
  });
}
