// Mana production is distinct from payment. CR 106.8-.11 define production
// even for hybrid, Phyrexian, generic, and snow symbols; keep their validated
// symbols so those rules determine the produced color/type and quantity.
// CR 106.6 governs spending restrictions, 106.7 "could produce" choices.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, ManaProduction, ManaSpendingRestriction, ManaSpendingRider, ManaSpendingUse, ObjectFilter, PlayerRef, Sentence } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { isManaSymbol } from "../symbols.ts";
import { isPluralPlayerRef, parseAmount, parseFilter, parseObjectRef, parsePlayerRef } from "./refs.ts";
import { parseChosenColorRef, parseChosenValueRef, parseThatChoiceQualifier } from "./choices.ts";
import { parseSpecialSpendingUse } from "./mana-uses.ts";
import { parseStaticEntryCounters } from "./entry.ts";

function readSymbols(c: Cursor): string[] | null {
  return c.attempt(c => {
    const symbols: string[] = [];
    while (c.peek()?.kind === "symbol") {
      const token = c.peek();
      if (token?.kind !== "symbol" || !isManaSymbol(token.value)) return c.fail("mana-production symbol");
      symbols.push(token.value);
      c.pos++;
    }
    return symbols.length ? symbols : null;
  });
}

function parseAtomicProduction(c: Cursor): ManaProduction | null {
  return c.attempt(c => {
    if (c.peek()?.kind === "symbol") {
      const symbols = readSymbols(c);
      return symbols ? { mana: "fixed", symbols } : null;
    }
    const scaled = c.attempt((c): ManaProduction | null => {
      if (!c.words("an", "amount", "of")) return null;
      // "an amount of mana of that color equal to …": a number of mana of
      // one chosen color (CR 106.1, 608.2c).
      if (c.words("mana", "of")) {
        const choice = parseChosenColorRef(c, parseFilter);
        if (!choice || !c.words("equal", "to")) return null;
        const amount = parseAmount(c);
        return amount && { mana: "chosen-color", amount, choice };
      }
      const symbol = c.symbol();
      if (!symbol || !/^[WUBRGC]$/.test(symbol) || !c.words("equal", "to")) return null;
      const amount = parseAmount(c);
      return amount && { mana: "scaled-symbol", symbol, amount };
    });
    if (scaled) return scaled;
    const amount = parseAmount(c);
    if (!amount) return null;
    // "Add six {G}" / "add that much {R}": a number of one printed symbol.
    // A fixed number is the same production as that many printed symbols.
    const unit = c.peek();
    if (unit?.kind === "symbol") {
      if (!/^[WUBRGC]$/.test(unit.value) || c.peek(1)?.kind === "symbol") return c.fail("one basic mana symbol after its number");
      c.pos++;
      if (amount.amount === "fixed") return amount.value >= 1 && Number.isInteger(amount.value)
        ? { mana: "fixed", symbols: Array<string>(amount.value).fill(unit.value) } : null;
      return { mana: "scaled-symbol", symbol: unit.value, amount };
    }
    if (c.word("mana") === null) return null;
    if (c.words("in", "any", "combination", "of")) {
      if (c.word("colors") !== null) return { mana: "combination", amount, options: "colors" };
      const options: string[] = [];
      for (;;) {
        const symbol = c.symbol();
        // Listed choices are mana types, not multi-unit bundles or costs.
        if (!symbol || !/^[WUBRGC]$/.test(symbol)) return null;
        options.push(symbol);
        const join = c.attempt(c => {
          const comma = c.punct(",");
          const and = c.word("and") !== null;
          const slash = and && c.punct("/");
          if (slash && c.word("or") === null) return null;
          const or = !and && c.word("or") !== null;
          // A comma before "where X is …" ends the list.
          return (comma || and || or) && c.peek()?.kind === "symbol" ? true : null;
        });
        if (!join) break;
      }
      return { mana: "combination", amount, options };
    }
    // "Choose a color. Add one mana of that color": the same linked
    // reference as "of the chosen color" (CR 607.2d, 608.2c).
    const that = parseThatChoiceQualifier(c, parseFilter);
    if (that?.chosenColor) return { mana: "chosen-color", amount, choice: that.chosenColor };
    if (that) return null;
    if (c.word("of") === null) return null;
    if (c.words("different", "colors"))
      return amount.amount === "fixed" && amount.value >= 2 && amount.value <= 5 ? { mana: "different-colors", amount } : null;
    const choice = parseChosenColorRef(c, parseFilter);
    if (choice) return { mana: "chosen-color", amount, choice };
    if (parseChosenValueRef(c)) return null;
    if (c.word("any") === null) return null;
    if (c.words("one", "color")) return { mana: "any-one-color", amount };
    const kind = c.word("color", "type");
    if (!kind) return null;
    const productionSource = c.attempt((c): ManaProduction | null => {
      const what = parseObjectRef(c) ?? c.attempt(c => {
        if (c.word("that") === null) return null;
        return parseObjectRef(c);
      });
      if (!what) return null;
      if (c.words("could", "produce")) return { mana: "could-produce", amount, what, colorOnly: kind === "color" };
      return c.word("produced") !== null ? { mana: "produced", amount, what, colorOnly: kind === "color" } : null;
    });
    if (productionSource) return productionSource;
    if (kind === "color" && c.word("among") !== null) {
      const among = parseObjectRef(c);
      return among ? { mana: "color-among", amount, among } : null;
    }
    if (kind === "type") return { mana: "any-type", amount };
    if (c.attempt(c => c.words("in", "your") && c.possessive("commander") !== null && c.words("color", "identity") ? true : null))
      return { mana: "commander-color", amount, whose: { player: "you" } };
    return { mana: "any-color", amount };
  });
}

/** CR 106: a production alternative can be a fixed mana bundle or a
 * symbolic color/amount. All options are choices by the stated recipient;
 * they are not separate add instructions and are not simultaneously added. */
function parseProduction(c: Cursor): ManaProduction | null {
  return c.attempt(c => {
    const first = parseAtomicProduction(c);
    if (!first) return null;
    const options = [first];
    let finalOr = false;
    for (;;) {
      const start = c.pos;
      const comma = c.punct(",");
      const or = c.word("or") !== null;
      if (!comma && !or) break;
      const option = parseAtomicProduction(c);
      if (!option) {
        // An ordinary coordinated action may follow a comma. An explicit
        // "or" after a production always needs another complete production.
        if (comma && !or) { c.pos = start; break; }
        return null;
      }
      options.push(option); finalOr = or;
    }
    if (options.length > 1 && !finalOr) return c.fail("final mana-choice conjunction");
    return options.length === 1 ? first : { mana: "choice", options };
  });
}

export function parseSpendingUse(c: Cursor): ManaSpendingUse | null {
  return c.attempt((c): ManaSpendingUse | null => {
    const special = parseSpecialSpendingUse(c, "to");
    if (special) return special;
    if (c.word("cast") !== null) {
      if (c.words("your", "commander")) return { use: "cast-commander", whose: { player: "you" } };
      c.word("a", "an");
      const filter = parseFilter(c);
      if (!filter) return null;
      // The restricted purpose is casting from a zone, not casting a spell
      // currently in that zone. Karolina Dean, Runaway forbids hand-origin
      // spells; by payment time those spells are already on the stack.
      const nestedZone = (filter: ObjectFilter): boolean => Boolean(filter.zone) ||
        (filter.allOf ?? []).some(nestedZone) || (filter.anyOf ?? []).some(nestedZone);
      if ([...(filter.allOf ?? []), ...(filter.anyOf ?? [])].some(nestedZone)) return null;
      const { zone: from, ...qualities } = filter;
      return { use: "cast", filter: qualities, ...(from ? { from } : {}) };
    }
    if (c.word("activate") === null) return null;
    c.word("an");
    if (c.word("ability", "abilities") === null) return null;
    // CR 602.1: with no stated source, any activated ability qualifies.
    if (c.word("of") === null) return { use: "activate" };
    c.word("a", "an");
    const source = parseFilter(c);
    return source && { use: "activate", source };
  });
}

/** Listed purposes are alternatives whether joined by "or" or by "and":
 * mana usable "to cast Elf spells and activate abilities of Elf sources" may
 * pay for either. A repeated "to" is grammatical only. */
function parseSpendingUses(c: Cursor): ManaSpendingUse[] | null {
  return c.attempt(c => {
    const first = parseSpendingUse(c);
    if (!first) return null;
    const uses = [first];
    let closed = true;
    for (;;) {
      // "to cast A, activate B, or pay C": commas separate listed purposes.
      let conjunction = false;
      const next = c.attempt(c => {
        const comma = c.punct(",");
        conjunction = c.word("or", "and") !== null;
        if (!conjunction && !comma) return null;
        c.word("to");
        return parseSpendingUse(c);
      });
      if (!next) break;
      uses.push(next);
      closed = conjunction;
    }
    // A comma-separated list ends with its conjunction before the last purpose.
    if (!closed) return c.fail("final conjunction of listed purposes");
    return uses;
  });
}

/** "Spend this mana only on costs that contain {X}." */
function parseOnCostsRestriction(c: Cursor): ManaSpendingRestriction | null {
  return c.attempt((c): ManaSpendingRestriction | null => {
    if (!c.words("spend", "this", "mana", "only", "on")) return null;
    const use = parseSpecialSpendingUse(c, "on");
    return use && (c.done() || c.isPunct(".")) ? { restriction: "only", uses: [use] } : null;
  });
}

function parseRestriction(c: Cursor): ManaSpendingRestriction | null {
  return c.attempt(c => {
    let restriction: ManaSpendingRestriction["restriction"];
    const onCosts = parseOnCostsRestriction(c);
    if (onCosts) return onCosts;
    if (c.words("spend", "this", "mana", "only", "to")) restriction = "only";
    else if (c.words("this", "mana", "can't", "be", "spent", "to")) restriction = "not";
    else return null;
    const uses = parseSpendingUses(c);
    if (!uses || c.isWord("or", "and")) return null;
    // A modifier or extra effect after the restriction belongs to this mana
    // rider, not to the preceding add instruction's resolution condition.
    if (!c.done() && !c.isPunct(".")) return c.fail("end of mana-spending restriction");
    return { restriction, uses };
  });
}

type BodyParser = (c: Cursor) => Sentence[] | null;

/** Tokens of one sentence, without its final period; quoted text is opaque. */
function sentenceEnd(c: Cursor): number {
  let quoted = false;
  for (let index = c.pos; index < c.tokens.length; index++) {
    const token = c.tokens[index];
    if (token.kind !== "punct") continue;
    if (token.value === '"') quoted = !quoted;
    else if (token.value === "." && !quoted) return index;
  }
  return c.tokens.length;
}

/** CR 106.6: a rider is either an additional effect on the spell or ability
 * the mana is spent on ("If that mana is spent on ...") or a delayed
 * triggered ability ("When that mana is spent to ..." / "When you spend this
 * mana to ...", 603.7a). The body is the rest of that one sentence; only a
 * copy's own retargeting sentence continues it. */
function parseRider(c: Cursor, parseBody: BodyParser): ManaSpendingRider | null {
  return c.attempt((c): ManaSpendingRider | null => {
    let head: { rider: "effect"; mana: "that" | "any-of-that" } | { rider: "delayed-trigger" };
    let connector: string | null;
    if (c.word("if") !== null) {
      const any = c.words("any", "of");
      // "If you spend this mana to ..." states the same condition as "if this mana is spent to ...".
      if (!c.words("that", "mana", "is", "spent") && !c.words("this", "mana", "is", "spent") && !c.words("you", "spend", "this", "mana")) return null;
      connector = c.word("on", "to");
      head = { rider: "effect", mana: any ? "any-of-that" : "that" };
    } else if (c.word("when") !== null) {
      if (!c.words("that", "mana", "is", "spent") && !c.words("you", "spend", "this", "mana")) return null;
      connector = c.word("to");
      head = { rider: "delayed-trigger" };
    } else return null;
    const spentOn = connector === "on" ? parseSpentOnSpell(c) : connector === "to" ? parseSpendingUses(c) : null;
    if (!spentOn || !c.punct(",")) return null;
    if (head.rider === "effect" && head.mana === "that") {
      const entry = c.attempt(c => {
        const body = new Cursor(c.tokens.slice(c.pos, Math.min(sentenceEnd(c) + 1, c.tokens.length)));
        const effect = parseStaticEntryCounters(body);
        if (effect?.static !== "enters-with-counters" || !body.done()) return null;
        c.pos += body.pos;
        return effect;
      });
      if (entry) return { rider: "entry", mana: "that", spentOn, effect: entry };
    }
    let end = sentenceEnd(c);
    const retarget = new Cursor(c.tokens.slice(end + 1));
    if (retarget.words("you", "may", "choose", "new", "targets", "for", "the", "copy") && retarget.isPunct("."))
      end += 1 + retarget.pos;
    const body = new Cursor(c.tokens.slice(c.pos, Math.min(end + 1, c.tokens.length)));
    const effects = parseBody(body);
    if (!effects || !body.done()) return null;
    c.pos = Math.min(end + 1, c.tokens.length);
    return { ...head, spentOn, effects };
  });
}

/** "Spent on a creature spell" and "spent to cast a creature spell" name the
 * same purpose: paying that spell's costs as it is cast (CR 601.2h). */
function parseSpentOnSpell(c: Cursor): ManaSpendingUse[] | null {
  return c.attempt(c => {
    c.word("a", "an");
    const filter = parseFilter(c);
    return filter?.cls === "spell" && !filter.zone ? [{ use: "cast", filter }] : null;
  });
}

/** CR 106.4, 500.5: "Until end of turn, you don't lose this mana as steps
 * and phases end." The player is the one who added the mana; only steps end
 * within a combat phase, so "as steps end" belongs to "until end of combat". */
export function parseManaRetention(c: Cursor, adder: PlayerRef): NonNullable<Extract<Effect, { effect: "add-mana" }>["keptUntil"]> | null {
  return c.attempt(c => {
    const duration = c.words("until", "end", "of", "turn") ? { duration: "end-of-turn" as const }
      : c.words("until", "end", "of", "combat") ? { duration: "end-of-combat" as const } : null;
    if (!duration || !c.punct(",")) return null;
    const pronoun = c.word("you", "they");
    if (!pronoun || (pronoun === "you") !== (adder.player === "you")) return null;
    if (!c.words("don't", "lose", "this", "mana", "as", "steps")) return null;
    // Each period has exactly one printed form, so the period alone records
    // which was printed: "steps and phases" for a turn, "steps" for a combat.
    const phases = c.words("and", "phases");
    if (phases !== (duration.duration === "end-of-turn")) return null;
    if (c.word("end") === null || (!c.punct(".") && !c.done())) return null;
    return duration;
  });
}

export interface ManaSpendingRules { restriction?: ManaSpendingRestriction; riders?: ManaSpendingRider[] }

/** Complete sentences after a mana-producing instruction that govern "this
 * mana" / "that mana". They are properties of the produced mana (CR 106.6),
 * never further instructions of the resolving ability. */
export function parseManaSpendingRules(c: Cursor, parseBody: BodyParser, allowRestriction: boolean): ManaSpendingRules | null {
  return c.attempt((c): ManaSpendingRules | null => {
    const riders: ManaSpendingRider[] = [];
    let restriction: ManaSpendingRestriction | undefined;
    if (allowRestriction) {
      const restricted = c.attempt(c => {
        const onCosts = parseOnCostsRestriction(c);
        if (onCosts) return c.punct(".") || c.done() ? { restriction: onCosts, rider: undefined } : null;
        let kind: ManaSpendingRestriction["restriction"];
        if (c.words("spend", "this", "mana", "only", "to")) kind = "only";
        else if (c.words("this", "mana", "can't", "be", "spent", "to")) kind = "not";
        else return null;
        const article = c.peek(1);
        const uses = parseSpendingUses(c);
        if (!uses) return null;
        // Delighted Halfling: the spell this mana is spent on gains a rule.
        // "That spell" needs the singular "a ... spell" as its antecedent.
        let rider: ManaSpendingRider | undefined;
        if (c.isPunct(",")) {
          if (kind !== "only" || uses.length !== 1 || uses[0].use !== "cast" ||
              article?.kind !== "word" || !["a", "an"].includes(article.value)) return null;
          c.punct(",");
          if (c.word("and") === null || !c.words("that", "spell")) return null;
          c.pos -= 2;
          const body = new Cursor(c.tokens.slice(c.pos, sentenceEnd(c) + 1));
          const effects = parseBody(body);
          if (!effects || !body.done()) return null;
          c.pos += body.pos;
          rider = { rider: "effect", mana: "that", spentOn: uses, effects };
        } else if (!c.punct(".") && !c.done()) return null;
        return { restriction: { restriction: kind, uses }, rider };
      });
      if (restricted) {
        restriction = restricted.restriction;
        if (restricted.rider) riders.push(restricted.rider);
      }
    }
    for (;;) {
      const rider = parseRider(c, parseBody);
      if (!rider) break;
      riders.push(rider);
    }
    if (!restriction && !riders.length) return null;
    return { ...(restriction ? { restriction } : {}), ...(riders.length ? { riders } : {}) };
  });
}

/** Actor state is committed by the caller only on success; failure rolls back
 * all tokens, including optional attached spending-restriction sentences. */
export function parseAddManaEffect(c: Cursor, previousPlayer?: PlayerRef): { effects: Effect[]; actor: PlayerRef } | null {
  const eachColor = c.attempt(c => {
    if (!c.words("for", "each", "color", "among")) return null;
    const among = parseObjectRef(c);
    if (!among || !c.punct(",") || c.word("add") === null || previousPlayer?.player === "they"
      || !c.words("one", "mana", "of", "that", "color")) return null;
    const actor: PlayerRef = { player: "you" };
    return { actor, effects: [{ effect: "add-mana" as const, who: actor, mana: { mana: "each-color-among" as const, among } }] };
  });
  if (eachColor) return eachColor;
  return c.attempt(c => {
    const explicit = parsePlayerRef(c);
    const verb = c.word("add", "adds");
    if (!verb) return null;
    const actor = explicit ?? (verb === "add" && previousPlayer?.player !== "they" ? { player: "you" as const } : previousPlayer);
    if (!actor) return c.fail("actor of mana production");
    c.words("an", "additional");
    const mana = parseProduction(c);
    if (!mana) return null;
    // "That player adds one mana of any color they choose": the adder makes
    // the color choice, as for any mana of any color (CR 106.3).
    if (c.isWord("they") && actor.player !== "you" && !isPluralPlayerRef(actor) &&
        (mana.mana === "any-color" || mana.mana === "any-one-color") && !c.words("they", "choose")) return null;
    const restriction = c.attempt(c => {
      if (!c.punct(".")) return null;
      return parseRestriction(c);
    });
    return { actor, effects: [{ effect: "add-mana", who: actor, mana, ...(restriction ? { restriction } : {}) }] };
  });
}
