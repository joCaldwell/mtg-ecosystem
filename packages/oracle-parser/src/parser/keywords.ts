// Keyword abilities, table-driven.
//
// The table below is the single source of truth for which keywords the
// parser understands and what parameter shape each takes. An unknown
// keyword FAILS the line — per the package rule that a wrong parse is
// worse than a failed parse, we never guess.

import type { Color, KeywordInstance, ObjectFilter, ProtectionScope } from "../ast.ts";
import { COLORS } from "../vocab.ts";
import type { Cursor } from "./cursor.ts";
import { parseCostList, parseSymbolCosts } from "./costs.ts";
import { parseFilter, parsePlayerRef } from "./refs.ts";

type Shape =
  | "bare"           // Flying
  | "cost"           // Equip {2}, Ward {2}, Ward—Pay 3 life.
  | "number"         // Crew 2, Toxic 1
  | "number-cost"    // Suspend 3—{2}{U}, Reinforce 2—{2}{W}, Awaken 4—{5}{U}
  | "filter"         // Enchant creature, Affinity for artifacts
  | "protection";    // Protection from red, from creatures, from everything

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
  "haunt": "bare", "conspire": "bare", "provoke": "bare", "amass": "bare",
  "compleated": "bare", "friends forever": "bare", "vanishing": "number",
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
  "level up": "cost", "boast": "cost", "channel": "cost", "blitz": "cost",
  "escape": "cost", "disguise": "cost", "plot": "cost", "offspring": "cost",
  "bestow": "cost", "dredge": "number", "recover": "cost", "ripple": "number",
  "splice onto arcane": "cost", "commander ninjutsu": "cost",
  "squad": "cost", "prototype": "cost", "more than meets the eye": "cost",
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
  // special
  "protection": "protection", "hexproof from": "protection",
};

const KEYWORD_NAMES = Object.keys(KEYWORDS).sort(
  (a, b) => b.split(" ").length - a.split(" ").length,
);

const LAND_WALKS: Record<string, string> = {
  plainswalk: "Plains", islandwalk: "Island", swampwalk: "Swamp",
  mountainwalk: "Mountain", forestwalk: "Forest", landwalk: "",
};

function matchKeywordName(c: Cursor): string | null {
  for (const name of KEYWORD_NAMES) {
    const found = c.attempt((c) => {
      for (const part of name.split(" ")) {
        if (c.word(part) === null) return null;
      }
      return name;
    });
    if (found) return found;
  }
  return c.fail("keyword");
}

function parseProtectionScopes(c: Cursor): ProtectionScope[] | null {
  const scopes: ProtectionScope[] = [];
  for (;;) {
    if (c.word("from") === null && scopes.length === 0) return null;
    const scope = c.attempt((c): ProtectionScope | null => {
      if (c.words("everything")) return { scope: "everything" };
      if (c.words("all", "colors")) return { scope: "all-colors" };
      if (c.words("colorless")) return { scope: "colorless" };
      if (c.words("multicolored")) return { scope: "multicolored" };
      if (c.words("monocolored")) return { scope: "monocolored" };
      const color = c.attempt((c) => {
        c.word("each"); // "protection from each color"? rare; tolerate
        const w = c.anyWord();
        return w && COLORS.has(w.value) ? w.value : null;
      });
      if (color) return { scope: "color", color: color as Color };
      const player = parsePlayerRef(c);
      if (player) return { scope: "player", player };
      const filter = parseFilter(c);
      if (filter) return { scope: "filter", filter };
      return null;
    });
    if (!scope) return scopes.length ? scopes : null;
    scopes.push(scope);
    // "protection from red and from blue"
    if (c.attempt((c) => (c.word("and") !== null && c.isWord("from") ? true : null))) {
      c.word("from");
      continue;
    }
    break;
  }
  return scopes;
}

/** Parse one keyword instance (with parameters). */
export function parseKeyword(c: Cursor): KeywordInstance | null {
  return c.attempt((c): KeywordInstance | null => {
    // …walk variants are single words
    const walk = c.attempt((c) => {
      const w = c.anyWord();
      if (!w) return null;
      const land = LAND_WALKS[w.value];
      if (land === undefined) return null;
      return land;
    });
    if (walk !== null) {
      const filter: ObjectFilter = walk ? { subtypes: [walk], types: ["land"] } : { types: ["land"] };
      return { keyword: "landwalk", filter };
    }

    // Typecycling: "plainscycling {2}", "basic landcycling {1}"
    const typeCycling = c.attempt((c): KeywordInstance | null => {
      const isBasic = c.words("basic");
      const w = c.anyWord();
      if (!w || !w.value.endsWith("cycling") || w.value === "cycling") return null;
      const sub = w.raw.slice(0, w.raw.toLowerCase().indexOf("cycling"));
      const cost = parseSymbolCosts(c);
      if (!cost) return null;
      const filter: ObjectFilter =
        sub.toLowerCase() === "land"
          ? { types: ["land"], supertypes: isBasic ? ["basic"] : undefined }
          : { subtypes: [sub[0].toUpperCase() + sub.slice(1)] };
      return { keyword: "typecycling", cost, filter };
    });
    if (typeCycling) return typeCycling;

    const name = matchKeywordName(c);
    if (!name) return null;
    const shape = KEYWORDS[name];

    switch (shape) {
      case "bare":
        return { keyword: name };
      case "number": {
        const n = c.number();
        if (n !== null) return { keyword: name, amount: n };
        if (c.word("x") !== null) return { keyword: name, amount: "x" };
        return null;
      }
      case "cost": {
        const symbols = c.attempt(parseSymbolCosts);
        if (symbols) return { keyword: name, cost: symbols };
        // "Ward—Pay 3 life." / "Equip—Sacrifice a creature."
        if (c.punct("—")) {
          const cost = parseCostList(c);
          if (!cost) return null;
          c.punct(".");
          return { keyword: name, cost };
        }
        return null;
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
        c.word("for"); // "affinity for artifacts"
        const filter = parseFilter(c);
        if (!filter) return null;
        return { keyword: name, filter };
      }
      case "protection": {
        const from = parseProtectionScopes(c);
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
    c.punct(".");
    if (!c.done()) return null;
    return keywords;
  });
}
