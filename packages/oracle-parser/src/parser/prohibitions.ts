// Rule prohibitions retain action, actor, eligible object/source, recurring
// window, and stated lifetime independently. CR 101.2, 601.3, 602.1c, 611.2c,
// 611.3, 615.12: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type {
  CastLimit, Condition, Duration, Effect, GameProhibition, ObjectFilter, ObjectRef,
  PlayerRef, ProhibitionSource, ProhibitionWindow, StaticEffect, Zone,
} from "../ast.ts";
import type { Token } from "../lexer.ts";
import { Cursor } from "./cursor.ts";
import { parseEligibleCardRef } from "./permissions.ts";
import {
  isTargetedPlayerRef, parseAmount, parseCondition, parseDuration, parseFilter, parseObjectRef,
  parsePlayerRef, parsePossessiveObject, parseZoneRef,
} from "./refs.ts";

type Rule = { prohibition: GameProhibition } | { limit: CastLimit };
interface Parsed { rules: Rule[]; duration?: Duration }

function paymentCondition(condition: Condition): boolean {
  if (["pays", "you-do", "you-dont"].includes(condition.condition)) return true;
  if (condition.condition === "not") return paymentCondition(condition.of);
  if (condition.condition === "all-of" || condition.condition === "any-of") return condition.conditions.some(paymentCondition);
  return false;
}

function parseActor(c: Cursor): PlayerRef | null {
  return c.attempt(c => {
    if (c.word("players") !== null) return { player: "each-player" };
    if (c.words("your", "opponents") || c.word("opponents") !== null) return { player: "each-opponent" };
    return parsePlayerRef(c);
  });
}

function parseWindow(c: Cursor): ProhibitionWindow | null {
  return c.attempt(c => {
    if (c.words("during", "combat")) return { during: "combat" };
    if (c.words("during", "your", "turn")) return { during: "turn", whose: { player: "you" } };
    return null;
  });
}

/** Plural source kinds connected by "and" enumerate alternative sources.
 * Damping Matrix affects artifacts OR creatures; "artifact creatures" still
 * requires both types. This local grammar never rewrites object modifiers. */
function parseSourceQuery(c: Cursor): ObjectRef | null {
  return c.attempt(c => {
    const start = c.pos;
    const tokens = c.tokens.slice(start);
    const types = new Set(["artifacts", "creatures", "enchantments", "lands", "planeswalkers", "battles"]);
    let beforeModifier = true;
    const local = new Cursor(tokens.map((token, index): Token => {
      if (token.kind === "word" && ["with", "without", "that", "which", "you", "your", "an", "a", "from", "in", "under", "attached"].includes(token.value)) beforeModifier = false;
      if (!beforeModifier || token.kind !== "word" || token.value !== "and") return token;
      const before = tokens[index - 1]?.kind === "punct" ? tokens[index - 2] : tokens[index - 1];
      const after = tokens[index + 1];
      return before?.kind === "word" && after?.kind === "word" && types.has(before.value) && types.has(after.value)
        ? { ...token, value: "or", raw: "or" } : token;
    }));
    const source = parseObjectRef(local);
    if (!source) return null;
    c.pos = start + local.pos;
    return source;
  });
}

function parseAbilityQuery(c: Cursor): ObjectRef | null {
  return c.attempt(c => {
    const possessed = c.attempt(c => {
      let source: ObjectRef | null = c.word("their") !== null ? { ref: "them" } : parsePossessiveObject(c);
      // Explicit plural possessors are source queries, not one chosen object.
      if (!source) {
        const noun = c.peek();
        if (noun?.kind !== "word" || !noun.possessive) return null;
        const local = new Cursor([{ ...noun, possessive: false }]);
        const ref = parseObjectRef(local);
        if (!ref || ref.ref !== "each" || local.pos !== 1) return null;
        c.pos++;
        source = ref;
      }
      const kind = c.word("activated", "loyalty");
      if (!kind || c.word("abilities") === null) return null;
      const filter: ObjectFilter = { cls: "ability", abilityKind: "activated", abilitySource: source,
        ...(kind === "loyalty" ? { loyaltyAbility: true } : {}) };
      return { ref: "each" as const, filter };
    });
    if (possessed) return possessed;
    const kind = c.word("activated", "loyalty");
    if (kind) {
      if (c.word("abilities") === null) return null;
      const filter: ObjectFilter = { cls: "ability", abilityKind: "activated",
        ...(kind === "loyalty" ? { loyaltyAbility: true } : {}) };
      if (c.word("of") !== null) {
        const source = parseSourceQuery(c);
        if (!source) return null;
        filter.abilitySource = source;
      }
      return { ref: "each", filter };
    }
    // CR 602.1c: "activate abilities" always means activated abilities.
    if (c.word("abilities") === null) return null;
    const filter: ObjectFilter = { cls: "ability", abilityKind: "activated" };
    if (c.word("of") !== null) {
      const source = parseSourceQuery(c);
      if (!source) return null;
      filter.abilitySource = source;
    } else if (c.words("that", "aren't")) {
      if (!c.words("mana", "abilities")) return null;
      filter.manaAbility = false;
      if (c.word("or") !== null) {
        if (!c.words("loyalty", "abilities")) return null;
        filter.loyaltyAbility = false;
      }
    }
    return { ref: "each", filter };
  });
}

function addManaException(c: Cursor, abilities: ObjectRef): ObjectRef | null {
  if (!c.isWord("unless")) return abilities;
  return c.attempt(c => {
    if (!c.words("unless", "they're", "mana", "abilities") || !('filter' in abilities)) return null;
    if (abilities.filter.manaAbility !== undefined) return null;
    return { ...abilities, filter: { ...abilities.filter, manaAbility: false } };
  });
}

function sourceZone(c: Cursor): ProhibitionSource["zones"][number] | null {
  return c.attempt(c => {
    const own = c.word("their") !== null;
    const plural: Record<string, Zone> = { hands: "hand", graveyards: "graveyard", libraries: "library" };
    const word = c.peek();
    if (word?.kind === "word" && !word.possessive && plural[word.value]) {
      c.pos++;
      return { zone: plural[word.value], ...(own ? { owner: "acting-player" as const } : {}) };
    }
    if (own) {
      const zone = c.word("hand", "graveyard", "library");
      return zone ? { zone: zone as Zone, owner: "acting-player" } : null;
    }
    const zone = parseZoneRef(c);
    if (!zone || zone.position) return null;
    return zone.owner === "their" ? { ...zone, owner: "acting-player" } : zone;
  });
}

function parseSource(c: Cursor): ProhibitionSource | null {
  return c.attempt(c => {
    if (c.word("from") === null) return null;
    const outside = c.words("anywhere", "other", "than");
    const first = sourceZone(c);
    if (!first) return null;
    const zones = [first];
    while (c.attempt(c => {
      if (c.word("or") === null) return null;
      const zone = sourceZone(c);
      if (!zone) return null;
      zones.push(zone); return true;
    })) { /* explicit source-zone alternatives */ }
    return { relation: outside ? "outside" : "in", zones };
  });
}

function parsePlayerAction(c: Cursor, who: PlayerRef): Rule | null {
  return c.attempt((c): Rule | null => {
    const action = c.word("cast", "play", "activate");
    if (!action) return null;
    if (action === "activate") {
      const query = parseAbilityQuery(c);
      const abilities = query && addManaException(c, query);
      return abilities ? { prohibition: { prohibition: "activate", who, abilities } } : null;
    }
    if (action === "cast" && c.words("more", "than")) {
      const maximum = parseAmount(c);
      const what = maximum && parseFilter(c);
      if (!maximum || !what || what.cls !== "spell" || !c.words("each", "turn")) return null;
      return { limit: { who, what, maximum, period: "turn", count: "each-player-all-casts" } };
    }
    const parsed = parseEligibleCardRef(c);
    if (!parsed) return null;
    let what = parsed;
    let from: ProhibitionSource | null = null;
    if ('filter' in parsed && parsed.filter.zone) {
      const { zone, ...filter } = parsed.filter;
      what = { ...parsed, filter };
      from = { relation: "in", zones: [{ ...zone, ...(zone.owner === "their" ? { owner: "acting-player" as const } : {}) }] };
    }
    if (c.isWord("from")) {
      if (from) return null;
      from = parseSource(c);
      if (!from) return null;
    }
    if (from) while (c.attempt(c => {
      if (c.word("or") === null) return null;
      const zone = sourceZone(c);
      if (!zone) return null;
      from.zones.push(zone); return true;
    })) { /* the ordinary object filter may have consumed the first zone */ }
    return { prohibition: { prohibition: action as "cast" | "play", who, what, ...(from ? { from } : {}) } };
  });
}

function parseDamageRule(c: Cursor): GameProhibition | null {
  return c.attempt(c => {
    const kind = c.word("combat", "noncombat") as "combat" | "noncombat" | null;
    if (c.word("damage") === null) return null;
    const rule: GameProhibition = { prohibition: "prevent-damage", kind: kind ?? "all" };
    if (c.words("that", "would", "be", "dealt")) {
      for (;;) {
        if (!rule.by && c.word("by") !== null) {
          const by = parseObjectRef(c);
          if (!by) return null;
          rule.by = by;
        } else if (!rule.to && c.word("to") !== null) {
          const to = parseActor(c) ?? parseObjectRef(c);
          if (!to) return null;
          rule.to = [to];
        } else break;
      }
      if (!rule.by && !rule.to) return null;
    }
    if (!c.words("can't", "be", "prevented")) return null;
    return rule;
  });
}

function parseRules(c: Cursor): Parsed | null {
  return c.attempt(c => {
    let window = parseWindow(c);
    if (window && !c.punct(",")) return null;
    let rules: Rule[];
    const damage = parseDamageRule(c);
    if (damage) rules = [{ prohibition: damage }];
    else {
      const passive = c.attempt(c => {
        const query = parseAbilityQuery(c);
        if (!query || !c.words("can't", "be", "activated")) return null;
        const abilities = addManaException(c, query);
        return abilities ? { prohibition: { prohibition: "activate" as const, abilities } } : null;
      });
      if (passive) rules = [passive];
      else {
        const who = parseActor(c);
        if (!who || c.word("can't") === null) return null;
        const first = parsePlayerAction(c, who);
        if (!first) return null;
        rules = [first];
        while (c.attempt(c => {
          if (c.word("or") === null) return null;
          const next = parsePlayerAction(c, who);
          if (!next) return null;
          rules.push(next); return true;
        })) { /* one can't governs every coordinated action */ }
        // Experimental Frenzy: a single final source qualifies both play
        // and cast. An explicit earlier source remains local to its action.
        const last = rules[rules.length - 1];
        if ('prohibition' in last && 'from' in last.prohibition && last.prohibition.from &&
            rules.every(rule => 'prohibition' in rule && ['cast', 'play'].includes(rule.prohibition.prohibition))) {
          for (const rule of rules) if ('prohibition' in rule &&
              (rule.prohibition.prohibition === "cast" || rule.prohibition.prohibition === "play") && !rule.prohibition.from)
            rule.prohibition.from = structuredClone(last.prohibition.from);
        }
      }
    }
    const trailingWindow = parseWindow(c);
    if (trailingWindow) {
      if (window) return null;
      window = trailingWindow;
    }
    if (window) {
      if (rules.some(rule => 'limit' in rule)) return null;
      for (const rule of rules) if ('prohibition' in rule) rule.prohibition.when = window;
    }
    const duration = parseDuration(c);
    return { rules, ...(duration ? { duration } : {}) };
  });
}

export function parseProhibitionEffect(c: Cursor, _previousSubject?: ObjectRef): Effect[] | null {
  return c.attempt(c => {
    const result = parseRules(c);
    // An activation/casting exception is not an invitation to make an
    // unrelated payment while this prohibition is created (CR 601.3, 602.1b).
    // The closed mana-ability exception was already consumed above.
    if (c.isWord("unless")) return null;
    if (c.isWord("if")) {
      const suffix = new Cursor(c.tokens.slice(c.pos + 1));
      const condition = parseCondition(suffix);
      if (condition && paymentCondition(condition)) return null;
    }
    return result && result.rules.map(rule => 'limit' in rule
      ? { effect: "cast-limit", limit: rule.limit, ...(result.duration ? { duration: result.duration } : {}) }
      : { effect: "prohibit", prohibition: rule.prohibition, ...(result.duration ? { duration: result.duration } : {}) });
  });
}

function hasTarget(value: unknown): boolean {
  if (!value || typeof value !== "object") return false;
  if (Array.isArray(value)) return value.some(hasTarget);
  if ('ref' in value && (value.ref === "target" || value.ref === "any-target")) return true;
  if ('player' in value && isTargetedPlayerRef(value as PlayerRef)) return true;
  return Object.values(value).some(hasTarget);
}

export function parseStaticProhibition(c: Cursor): StaticEffect | null {
  return c.attempt(c => {
    const result = parseRules(c);
    if (!result || hasTarget(result.rules) || result.duration && result.duration.duration !== "as-long-as") return null;
    const qualifier = c.word("if", "unless");
    const condition = qualifier ? parseCondition(c, qualifier === "unless" ? { purpose: "unless" } : undefined) : null;
    if (qualifier && (!condition || paymentCondition(condition) || result.duration)) return null;
    c.punct(".");
    if (!c.done() && !c.isWord("otherwise")) return null;
    const effects: StaticEffect[] = result.rules.map(rule => 'limit' in rule
      ? { static: "cast-limit", limit: rule.limit } : { static: "prohibit", prohibition: rule.prohibition });
    const effect: StaticEffect = effects.length === 1 ? effects[0] : { static: "group", effects };
    if (condition) return { static: "conditional", condition: qualifier === "unless" ? { condition: "not", of: condition } : condition, effect };
    return result.duration?.duration === "as-long-as"
      ? { static: "conditional", condition: result.duration.condition, effect } : effect;
  });
}
