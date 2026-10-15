// Keyword abilities, table-driven.
//
// The table below is the single source of truth for which keywords the
// parser understands and what parameter shape each takes. An unknown
// keyword FAILS the line — per the package rule that a wrong parse is
// worse than a failed parse, we never guess.

import type { Color, Cost, KeywordInstance, ObjectFilter, ProtectionScope } from "../ast.ts";
import { CARD_TYPES, COLORS, SUPERTYPES, canonicalSubtypeName } from "../vocab.ts";
import { lex } from "../lexer.ts";
import type { Cursor } from "./cursor.ts";
import { parseCostList, parseSymbolCosts } from "./costs.ts";
import { parseChosenValueRef } from "./choices.ts";
import { parseCraft } from "./craft.ts";
import { parseCompanion } from "./companion.ts";
import { parseActivationCostModifications, parseKeywordCastCostModifications } from "./cost-modifiers.ts";
import { parseActivationRules } from "./restrictions.ts";
import { mentionsX, parseVariableConstraints } from "./variable-constraints.ts";
import { parseAmount, parseComparison, parseFilter, parseObjectRef, parsePlayerRef, parsePossessiveObject, parsePossessivePlayer } from "./refs.ts";

type Shape =
  | "bare"           // Flying
  | "cost"           // Equip {2}, Ward {2}, Ward—Pay 3 life.
  | "number"         // Crew 2, Toxic 1
  | "number-cost"    // Suspend 3—{2}{U}, Reinforce 2—{2}{W}, Awaken 4—{5}{U}
  | "filter"         // Enchant creature, Affinity for artifacts
  | "protection"    // Protection from red, from creatures, from everything
  | "prototype"
  | "gift"
  | "optional-number"
  | "optional-cost";

/** keyword → shape. Multi-word names use spaces; matching is longest-first. */
const KEYWORDS: Record<string, Shape> = {
  // evergreen & common bare
  "deathtouch": "bare", "defender": "bare", "double strike": "bare",
  "first strike": "bare", "flash": "bare", "flying": "bare", "haste": "bare",
  "hexproof": "bare", "indestructible": "bare", "lifelink": "bare",
  "menace": "bare", "reach": "bare", "trample": "bare", "vigilance": "bare",
  "shroud": "bare", "fear": "bare", "intimidate": "bare", "exalted": "bare",
  "changeling": "bare", "wither": "bare", "infect": "bare", "undying": "bare",
  "persist": "bare", "prowess": "bare", "skulk": "bare", "ingest": "bare",
  "devoid": "bare", "improvise": "bare", "convoke": "bare", "delve": "bare",
  "cascade": "bare", "storm": "bare", "banding": "bare", "flanking": "bare",
  "shadow": "bare", "horsemanship": "bare", "phasing": "bare",
  "sunburst": "bare", "epic": "bare", "split second": "bare",
  "living weapon": "bare", "totem armor": "bare", "battle cry": "bare",
  "rebound": "bare", "gravestorm": "bare", "retrace": "bare",
  "soulbond": "bare", "unleash": "bare", "cipher": "bare", "evolve": "bare",
  "extort": "bare", "fuse": "bare", "dethrone": "bare", "melee": "bare",
  "partner": "bare", "ascend": "bare", "assist": "bare", "jump-start": "bare",
  "riot": "bare", "mentor": "bare", "undaunted": "bare", "aftermath": "bare",
  "exploit": "bare", "myriad": "bare", "decayed": "bare", "daybound": "bare",
  "nightbound": "bare", "training": "bare", "enlist": "bare",
  "read ahead": "bare", "for mirrodin!": "bare", "bargain": "bare",
  "haunt": "bare", "conspire": "bare", "provoke": "bare",
  "compleated": "bare", "friends forever": "bare", "vanishing": "optional-number",
  "doctor's companion": "bare", "choose a background": "bare",
  "double team": "bare", "start your engines!": "bare", "job select": "bare",
  // cost-parameterized
  "equip": "cost", "kicker": "cost", "multikicker": "cost", "buyback": "cost",
  "flashback": "cost", "madness": "cost", "evoke": "cost", "dash": "cost",
  "emerge": "cost", "prowl": "cost", "surge": "cost", "spectacle": "cost",
  "embalm": "cost", "eternalize": "cost", "unearth": "cost", "encore": "cost",
  "disturb": "cost", "overload": "cost", "foretell": "cost", "ward": "cost",
  "cycling": "cost", "transmute": "cost", "ninjutsu": "cost", "outlast": "cost",
  "scavenge": "cost", "cumulative upkeep": "cost", "echo": "cost",
  "morph": "cost", "megamorph": "cost", "miracle": "cost", "replicate": "cost",
  "entwine": "cost", "fortify": "cost", "reconfigure": "cost",
  "level up": "cost", "blitz": "cost",
  "escape": "cost", "disguise": "cost", "plot": "cost", "offspring": "cost",
  "bestow": "cost", "dredge": "number", "recover": "cost", "ripple": "number",
  "commander ninjutsu": "cost",
  "squad": "cost", "prototype": "prototype", "more than meets the eye": "cost",
  // CR 702: explicit keyword abilities, as opposed to keyword actions (701)
  // or cosmetic ability words (207.2c). Each parameter remains structured.
  // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
  "umbra armor": "bare", "living metal": "bare", "ravenous": "bare",
  "demonstrate": "bare", "mutate": "cost", "cleave": "cost",
  "escalate": "cost", "freerunning": "cost", "harmonize": "cost",
  "warp": "cost", "mayhem": "optional-cost", "web-slinging": "cost",
  "sneak": "cost", "aura swap": "cost", "transfigure": "cost",
  "mobilize": "number", "firebending": "number",
  "teamwork": "number", "station": "bare", "increment": "bare",
  "paradigm": "bare", "storied": "bare",
  // CR 702.106a/f: the secret card-name choice(s) a conspiracy makes as it is
  // put into the command zone; "the chosen name" abilities are linked (702.106d).
  "hidden agenda": "bare", "double agenda": "bare",
  "gift": "gift",
  // number-parameterized
  "crew": "number", "fading": "number", "bloodthirst": "number",
  "graft": "number", "modular": "number", "annihilator": "number",
  "absorb": "number", "afflict": "number", "afterlife": "number",
  "amplify": "number", "bushido": "number", "devour": "number",
  "fabricate": "number", "frenzy": "number", "poisonous": "number",
  "rampage": "number", "renown": "number", "soulshift": "number",
  "toxic": "number", "tribute": "number", "casualty": "number",
  "backup": "number", "saddle": "number", "hideaway": "number",
  "impending": "number-cost", "suspend": "number-cost",
  "reinforce": "number-cost", "awaken": "number-cost",
  // filter-parameterized
  "enchant": "filter", "affinity": "filter", "landwalk": "filter",
  "champion": "filter", "offering": "filter",
  // special
  "protection": "protection", "hexproof from": "protection",
};

const KEYWORD_NAMES = Object.keys(KEYWORDS).sort(
  (a, b) => b.split(" ").length - a.split(" ").length,
).map(name => ({ name, tokens: lex(name) }));
const KEYWORDS_BY_START = new Map<string, typeof KEYWORD_NAMES>();
for (const entry of KEYWORD_NAMES) {
  const first = entry.tokens[0];
  if (first.kind !== "word") throw new Error(`Keyword must begin with a word: ${entry.name}`);
  const bucket = KEYWORDS_BY_START.get(first.value) ?? [];
  bucket.push(entry);
  KEYWORDS_BY_START.set(first.value, bucket);
}

/** Used by the ability classifier to avoid discarding rules-bearing prefixes. */
export function isKeywordName(name: string): boolean {
  const canonical = name.toLowerCase();
  return Object.hasOwn(KEYWORDS, canonical) ||
    ["splice", "boast", "exhaust", "forecast", "companion", "craft", "gift", "partner with", "power-up", "max speed"].includes(canonical);
}

/** Keywords whose only parameter is a cost ("flashback [cost]"). */
export function isCostKeyword(name: string): boolean {
  return KEYWORDS[name] === "cost";
}

const LAND_WALKS: Record<string, string> = {
  plainswalk: "Plains", islandwalk: "Island", swampwalk: "Swamp",
  mountainwalk: "Mountain", forestwalk: "Forest", landwalk: "",
  desertwalk: "Desert",
};

/** CR 702.14a: a specific landwalk named by its one printed word
 * ("islandwalk"), with the keyword instance that word stands for. */
export function parseLandwalkName(c: Cursor): { name: string; keyword: KeywordInstance } | null {
  return c.attempt((c) => {
    const word = c.anyWord();
    const land = word && Object.hasOwn(LAND_WALKS, word.value) ? LAND_WALKS[word.value] : undefined;
    if (!word || !land) return null;
    return { name: word.value, keyword: { keyword: "landwalk", filter: { types: ["land"], subtypes: [land] } } };
  });
}

export function parseKeywordName(c: Cursor): string | null {
  const first = c.peek();
  const candidates = first?.kind === "word" ? KEYWORDS_BY_START.get(first.value) ?? [] : [];
  for (const { name, tokens } of candidates) {
    const found = c.attempt((c) => {
      for (const token of tokens) {
        if (token.kind === "word") {
          if ((token.possessive ? c.possessive(token.value) : c.word(token.value)) === null) return null;
        } else if (token.kind === "punct") {
          if (!c.punct(token.value)) return null;
        } else return null;
      }
      return name;
    });
    if (found) return found;
  }
  return c.fail("keyword");
}

export function parseProtectionScopes(c: Cursor, fromConsumed = false, allowFreshChoices = false): ProtectionScope[] | null {
  return c.attempt((c): ProtectionScope[] | null => {
    const scopes: ProtectionScope[] = [];
    if (!fromConsumed && c.word("from") === null) return null;
    for (;;) {
      const scope = c.attempt((c): ProtectionScope | null => {
        const chosen = parseChosenValueRef(c);
        if (chosen) return { scope: "chosen-value", value: chosen };
        const fresh = allowFreshChoices && c.attempt((c): ProtectionScope | null => {
          if (c.word("the") === null) return null;
          const category = c.word("color") !== null ? "color" : c.words("card", "type") ? "card-type" : null;
          if (!category || c.word("of") === null) return null;
          const who = parsePossessivePlayer(c);
          // One printed chooser makes this choice. Distributing choices among
          // several players would need a separate per-player association.
          if (!who || who.player === "each-player" || who.player === "each-opponent" || c.word("choice") === null) return null;
          return { scope: "fresh-choice", category, who, when: "granting-effect-applies" };
        });
        if (fresh) return fresh;
        const colorsOf = c.attempt((c): ProtectionScope | null => {
          if (!c.words("the", "colors", "of")) return null;
          const what = parseObjectRef(c);
          return what ? { scope: "colors-of", what } : null;
        });
        if (colorsOf) return colorsOf;
        // CR 702.11g/702.16i: "each of its colors", "each color among permanents you control":
        // the same color set, named by its possessor or by the objects.
        const eachColorOf = c.attempt((c): ProtectionScope | null => {
          if (c.word("each") === null) return null;
          if (c.words("color", "among")) {
            const what = parseObjectRef(c);
            return what && (what.ref === "each" || what.ref === "all") ? { scope: "colors-of", what } : null;
          }
          if (c.word("of") === null) return null;
          const what = parsePossessiveObject(c);
          return what && c.word("colors") !== null ? { scope: "colors-of", what } : null;
        });
        if (eachColorOf) return eachColorOf;
        if (c.words("everything")) return { scope: "everything" };
        if (c.words("all", "colors") || c.words("each", "color")) return { scope: "all-colors" };
        if (c.words("colorless")) return { scope: "colorless" };
        if (c.words("multicolored")) return { scope: "multicolored" };
        if (c.words("monocolored")) return { scope: "monocolored" };
        const color = c.attempt((c) => {
          const w = c.anyWord();
          return w && COLORS.has(w.value) ? w.value : null;
        });
        if (color) return { scope: "color", color: color as Color };
        const player = parsePlayerRef(c);
        if (player) return { scope: "player", player };
        // CR 702.16a: the quality can be any characteristic value, e.g.
        // "protection from mana value 3 or greater" (sources whose mana value is).
        const manaValue = c.attempt((c): ProtectionScope | null => {
          if (!c.words("mana", "value")) return null;
          const comparison = parseComparison(c);
          return comparison?.value.amount === "fixed" ? { scope: "filter", filter: { properties: [{ property: "mana-value", comparison }] } } : null;
        });
        if (manaValue) return manaValue;
        const filter = parseFilter(c);
        if (filter) return { scope: "filter", filter };
        return null;
      });
      if (!scope) return null;
      scopes.push(scope);
      // CR 702.11f/702.16g: repeated "from" denotes separate scopes.
      // Commas also occur in Oracle's lists (Elite Inquisitor).
      if (c.attempt((c) => {
        if (!c.punct(",") && c.word("and") === null) return null;
        c.word("and");
        return c.word("from") !== null ? true : null;
      })) {
        continue;
      }
      // A type list can omit the repeated "from" (Nevinyrral). A color
      // conjunction without "from" is deliberately not guessed here.
      const moreTypes = scope.scope === "filter" && c.attempt((c) => {
        const comma = c.punct(",");
        const and = c.word("and");
        if (!comma && and === null) return null;
        const next = parseFilter(c);
        if (!next) return null;
        scopes.push({ scope: "filter", filter: next });
        return true;
      });
      if (moreTypes) {
        // The final list member has already been parsed; a following
        // unrelated keyword must remain available to parseKeywordLine.
        break;
      }
      break;
    }
    return scopes;
  });
}

function parseKeywordCost(c: Cursor): Cost[] | null {
  return c.attempt((c): Cost[] | null => {
    if (c.punct("—")) {
      const cost = parseCostList(c);
      if (!cost) return null;
      // A period that ends the keyword line is part of it; one that is followed by
      // another sentence ("Equip—Pay 3 life. Activate only once each turn.") ends
      // the keyword's sentence for the caller.
      if (c.isPunct(".") && c.peek(1) === undefined) c.punct(".");
      return cost;
    }
    const start = c.pos;
    const symbols = parseSymbolCosts(c);
    if (!symbols) return null;
    if (c.isWord("or")) {
      c.pos = start;
      return parseCostList(c);
    }
    return symbols;
  });
}

/** Parse one keyword instance (with parameters). */
export function parseKeyword(c: Cursor, options: { allowFreshProtectionChoices?: boolean } = {}): KeywordInstance | null {
  return c.attempt((c): KeywordInstance | null => {
    // CR 702.14: landwalk's supertype/type qualifiers restrict the land.
    const walk = c.attempt((c) => {
      const filter: ObjectFilter = { types: ["land"] };
      for (;;) {
        const qualifier = c.peek();
        if (qualifier?.kind !== "word" || qualifier.possessive) break;
        if (SUPERTYPES.has(qualifier.value)) {
          c.next();
          (filter.supertypes ??= []).push(qualifier.value);
        } else if (qualifier.value.startsWith("non") && SUPERTYPES.has(qualifier.value.slice(3))) {
          c.next();
          (filter.nonSupertypes ??= []).push(qualifier.value.slice(3));
        } else if (CARD_TYPES.has(qualifier.value) && qualifier.value !== "land") {
          c.next();
          filter.types!.push(qualifier.value);
        } else break;
      }
      const w = c.anyWord();
      if (!w) return null;
      const land = LAND_WALKS[w.value];
      if (land === undefined) return null;
      if (land) filter.subtypes = [land];
      // "landwalk of the chosen type": the linked land type choice is the
      // walked quality, as in "lands of the chosen type" (CR 702.14a, 607.2d).
      else if (c.words("of", "the", "chosen", "type")) filter.chosenType = { choice: "chosen-type" };
      return filter;
    });
    if (walk !== null) {
      return { keyword: "landwalk", filter: walk };
    }

    // Typecycling: "plainscycling {2}", "basic landcycling {1}"
    const typeCycling = c.attempt((c): KeywordInstance | null => {
      const supertypes: string[] = [];
      while (c.peek()?.kind === "word") {
        const token = c.peek();
        if (token?.kind !== "word" || !SUPERTYPES.has(token.value)) break;
        supertypes.push(token.value);
        c.next();
      }
      const w = c.anyWord();
      if (!w || !w.value.endsWith("cycling") || w.value === "cycling") return null;
      const sub = w.value.slice(0, -"cycling".length);
      // CR 702.29e allows card types, subtypes and supertypes. Validate the
      // suffix against the rules vocabulary; arbitrary words ending in
      // "cycling" do not become invented typecycling abilities.
      const subtype = canonicalSubtypeName(sub);
      if (!CARD_TYPES.has(sub) && !SUPERTYPES.has(sub) && !subtype) return null;
      const cost = parseKeywordCost(c);
      if (!cost) return null;
      const filter: ObjectFilter = CARD_TYPES.has(sub) ? { types: [sub] } :
        SUPERTYPES.has(sub) ? { supertypes: [sub] } : { subtypes: [subtype!] };
      if (supertypes.length) filter.supertypes = [...supertypes, ...(filter.supertypes ?? [])];
      return { keyword: "typecycling", cost, filter };
    });
    if (typeCycling) return typeCycling;

    // CR 702.48: [subtype] offering retains the sacrificed object's quality.
    const offering = c.attempt((c): KeywordInstance | null => {
      const filter = parseFilter(c);
      return filter && c.word("offering") !== null ? { keyword: "offering", filter } : null;
    });
    if (offering) return offering;

    // CR 702.47: splice carries a quality filter, not a fixed Arcane name.
    const splice = c.attempt((c): KeywordInstance | null => {
      if (!c.words("splice", "onto")) return null;
      const filter = parseFilter(c);
      const cost = parseKeywordCost(c);
      return filter && cost ? { keyword: "splice", filter, cost } : null;
    });
    if (splice) return splice;

    // CR 702.22b-c: "bands with other [quality]" names the quality that the
    // creatures of its bands must have; it is not banding.
    const bandsWith = c.attempt((c): KeywordInstance | null => {
      if (!c.words("bands", "with", "other")) return null;
      const filter = parseFilter(c);
      return filter && { keyword: "bands with other", filter };
    });
    if (bandsWith) return bandsWith;

    const craft = parseCraft(c, parseKeywordCost);
    if (craft) return craft;
    const companion = parseCompanion(c);
    if (companion) return companion;

    const name = parseKeywordName(c);
    if (!name) return null;
    const shape = KEYWORDS[name];

    if (name === "partner" && c.word("with") !== null) {
      // CR 702.124j: the exact name controls both pairing and library search.
      const paired = c.peek();
      if (paired?.kind !== "card-name") return c.fail("catalogued partner card name");
      c.pos++;
      return { keyword: "partner with", pairedWith: paired.value };
    }

    if (name === "partner" && c.punct("—")) {
      // CR 702.124i explicitly enumerates these variants. The father & son
      // variant awaits ampersand lexing; its text must not be discarded.
      const variant = c.words("friends", "forever") ? "friends forever" :
        c.words("character", "select") ? "character select" :
        c.word("survivors") !== null ? "survivors" : null;
      return variant ? { keyword: `partner—${variant}` } : null;
    }

    if (name === "enchant") {
      const player = c.word("player", "opponent");
      if (player) return { keyword: name, enchantPlayer: player as "player" | "opponent" };
    }

    if (name === "equip") {
      const restricted = c.attempt((c): KeywordInstance | null => {
        const filter = parseFilter(c);
        const cost = parseKeywordCost(c);
        return filter && cost ? { keyword: name, filter, cost } : null;
      });
      if (restricted) return restricted;
    }

    if (name === "devour") {
      const restricted = c.attempt((c): KeywordInstance | null => {
        const filter = parseFilter(c);
        const amount = c.number();
        return filter && amount !== null ? { keyword: name, filter, amount } : null;
      });
      if (restricted) return restricted;
    }

    switch (shape) {
      case "bare":
        return { keyword: name };
      case "gift": {
        // CR 702.174d-i gives exact predefined gifts. The gift's semantics
        // depend on card type; retain the identity for semantic lowering.
        const gift = c.words("a", "card") ? "card" :
          c.words("a", "tapped", "fish") ? "tapped-fish" :
          c.words("a", "treasure") ? "treasure" :
          c.words("a", "food") ? "food" :
          c.words("an", "octopus") ? "octopus" :
          c.words("an", "extra", "turn") ? "extra-turn" : null;
        return gift ? { keyword: name, gift } : null;
      }
      case "optional-number":
      case "number": {
        const n = c.number();
        if (n !== null) return { keyword: name, amount: n };
        if (c.word("x") !== null) return { keyword: name, amount: "x" };
        return shape === "optional-number" ? { keyword: name } : null;
      }
      case "optional-cost":
      case "cost": {
        const cost = parseKeywordCost(c);
        if (cost) {
          // CR 702.33b: the two kicker costs can be paid independently.
          if (name === "kicker" && c.attempt(c =>
            c.word("and") !== null && c.punct("/") && c.word("or") !== null ? true : null)) {
            const second = parseKeywordCost(c);
            return second ? { keyword: name, additionalCosts: [cost, second] } : null;
          }
          return { keyword: name, cost };
        }
        return shape === "optional-cost" ? { keyword: name } : null;
      }
      case "prototype": {
        // CR 702.160/718: the alternate mana cost AND both stats are required.
        const cost = parseSymbolCosts(c);
        if (!cost || cost.some(item => item.cost !== "mana") || !c.punct("—")) return null;
        const power = c.number();
        if (power === null || !c.punct("/")) return null;
        const toughness = c.number();
        if (toughness === null) return null;
        return { keyword: name, cost, prototype: {
          power: { amount: "fixed", value: power }, toughness: { amount: "fixed", value: toughness },
        } };
      }
      case "number-cost": {
        const n = c.number();
        const amount = n !== null ? n : c.word("x") !== null ? ("x" as const) : null;
        if (amount === null) return null;
        if (!c.punct("—")) return null;
        const cost = parseCostList(c);
        if (!cost) return null;
        return { keyword: name, amount, cost };
      }
      case "filter": {
        if (name === "affinity" && c.word("for") === null) return null;
        if (name === "champion" && c.word("a", "an") === null) return null;
        // Offering is written with its subtype before the keyword, handled
        // above. Never reinterpret "offering Goblin" as valid keyword syntax.
        if (name === "offering") return null;
        const filter = parseFilter(c);
        if (!filter) return null;
        return { keyword: name, filter };
      }
      case "protection": {
        const from = parseProtectionScopes(c, name === "hexproof from", name === "protection" && options.allowFreshProtectionChoices);
        if (!from) return null;
        return { keyword: name === "hexproof from" ? "hexproof" : name, from };
      }
    }
  });
}

/**
 * A whole line of keywords: "Flying, first strike, ward {2}".
 * Fails unless the entire remaining input is consumed.
 */
export function parseKeywordLine(c: Cursor): KeywordInstance[] | null {
  return c.attempt((c): KeywordInstance[] | null => {
    const keywords: KeywordInstance[] = [];
    for (;;) {
      const kw = parseKeyword(c);
      if (!kw) return null;
      // "Firebending X, where X is ~'s power": the definition of the keyword's X.
      if (kw.amount === "x" && keywords.length === 0 && !kw.cost) {
        const value = c.attempt(c => (c.punct(",") && c.words("where", "x", "is") ? parseAmount(c) : null));
        if (value) kw.bindings = [{ variable: "x", value }];
      }
      keywords.push(kw);
      if (c.isPunct(",") || c.isPunct(";")) {
        c.next();
        c.word("and");
        continue;
      }
      if (c.isWord("and")) {
        c.word("and");
        continue;
      }
      break;
    }
    const sentenceEnd = c.punct(".");
    if (sentenceEnd && !c.done()) {
      // CR 602.1b and 702.6: Plate Armor's second sentence modifies this
      // equip ability's payment. A list has no single unambiguous antecedent.
      // CR 107.3a/d: "Suspend X—{X}{R}. X can't be 0." limits that keyword's X.
      const constraints = keywords.length === 1 && mentionsX([keywords[0].amount, keywords[0].cost]) ? parseVariableConstraints(c) : null;
      if (constraints) {
        if (!c.done()) return null;
        keywords[0].variableConstraints = constraints;
        return keywords;
      }
      if (keywords.length === 1 && keywords[0].keyword === "flashback") {
        const modifications = parseKeywordCastCostModifications(c);
        if (!modifications || !c.done()) return null;
        keywords[0].costModifications = modifications;
        return keywords;
      }
      if (keywords.length !== 1 || !["equip", "crew"].includes(keywords[0].keyword)) return null;
      const costModifications = keywords[0].keyword === "equip" ? parseActivationCostModifications(c) : null;
      if (costModifications) keywords[0].costModifications = costModifications;
      // CR 602.5b: equip (702.6a) and crew (702.122a) are activated
      // abilities, so a printed "Activate only ..." restricts that ability.
      const rules = c.done() ? null : parseActivationRules(c);
      if (rules?.restriction && !rules.permission) keywords[0].restriction = rules.restriction;
      else if (rules || !costModifications || !c.done()) return null;
    }
    if (!c.done()) return null;
    return keywords;
  });
}
