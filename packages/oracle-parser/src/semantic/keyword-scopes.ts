// Intrinsic scopes of keyword abilities (CR 702), for reference binding.
//
// A keyword's own rules text is fixed by the Comprehensive Rules, so its
// intrinsic targets and choices ("target creature you control" for equip,
// "that player" for ward) are not printed references and need no binding.
// What the printed parameters need is the scope they belong to: which kind of
// ability the keyword represents, and which player "you" and the payer of a
// printed cost are. Each entry below takes the ability kind from the keyword's
// CR 702.N definition. A keyword without an entry, or whose scope is not
// modeled, stays an unsupported scope.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt

/** CR 113.3: the kinds of ability a keyword represents. "special-action" is a
 * special action the keyword allows (CR 116.2); "deck-construction" functions
 * before the game begins (CR 702.124a). */
export type KeywordAbilityKind = "static" | "triggered" | "activated" | "spell" | "characteristic-defining" | "special-action" | "deck-construction";

/** The player that "you" and a printed cost's payer refer to in the keyword's
 * parameters (CR 109.5). */
export type KeywordRole = "static-source-controller" | "activator" | "trigger-controller" | "prospective-caster" | "spell-controller"
  | "special-action-player" | "targeting-player" | "card-owner";

export type KeywordScope = { rules: string[]; abilities: KeywordAbilityKind[]; role: KeywordRole } | { unsupported: string };

const S = "static", T = "triggered", A = "activated", P = "spell", C = "characteristic-defining", X = "special-action", D = "deck-construction";
const source = "static-source-controller", activator = "activator", trigger = "trigger-controller", caster = "prospective-caster";
const scope = (rule: string, abilities: KeywordAbilityKind[], role: KeywordRole): KeywordScope => ({ rules: [rule], abilities, role });

export const KEYWORD_SCOPES: Readonly<Record<string, KeywordScope>> = {
  // Static abilities of the object itself.
  "deathtouch": scope("702.2", [S], source), "defender": scope("702.3", [S], source),
  "double strike": scope("702.4", [S], source), "enchant": scope("702.5", [S], source),
  "first strike": scope("702.7", [S], source), "flash": scope("702.8", [S], caster),
  "flying": scope("702.9", [S], source), "haste": scope("702.10", [S], source),
  "hexproof": scope("702.11", [S], source), "indestructible": scope("702.12", [S], source),
  "intimidate": scope("702.13", [S], source), "landwalk": scope("702.14", [S], source),
  "lifelink": scope("702.15", [S], source), "protection": scope("702.16", [S], source),
  "reach": scope("702.17", [S], source), "shroud": scope("702.18", [S], source),
  "trample": scope("702.19", [S], source), "vigilance": scope("702.20", [S], source),
  "banding": scope("702.22", [S], source), "bands with other": scope("702.22", [S], source),
  "phasing": scope("702.26", [S], source), "shadow": scope("702.28", [S], source),
  "horsemanship": scope("702.31", [S], source), "fear": scope("702.36", [S], source),
  "amplify": scope("702.38", [S], source), "sunburst": scope("702.44", [S], source),
  "dredge": scope("702.52", [S], source), "bloodthirst": scope("702.54", [S], source),
  "split second": scope("702.61", [S], caster), "absorb": scope("702.64", [S], source),
  "changeling": scope("702.73", [C], source), "wither": scope("702.80", [S], source),
  "devour": scope("702.82", [S], source), "umbra armor": scope("702.89", [S], source),
  "totem armor": scope("702.89", [S], source), "infect": scope("702.90", [S], source),
  "unleash": scope("702.98", [S], source), "tribute": scope("702.104", [S], source),
  "menace": scope("702.111", [S], source), "devoid": scope("702.114", [C], source),
  "skulk": scope("702.118", [S], source), "riot": scope("702.136", [S], source),
  "compleated": scope("702.150", [S], source), "read ahead": scope("702.155", [S], source),
  "living metal": scope("702.161", [S], source), "toxic": scope("702.164", [S], source),
  "start your engines!": scope("702.179", [S], source), "storied": scope("702.195", [S], source),
  "ascend": scope("702.131", [S, P], source),
  "fading": scope("702.32", [S, T], source), "vanishing": scope("702.63", [S, T], source),
  "graft": scope("702.58", [S, T], source), "modular": scope("702.43", [S, T], source),
  "decayed": scope("702.147", [S, T], source),
  "daybound": scope("702.145", [S, T], source), "nightbound": scope("702.145", [S, T], source),
  "hidden agenda": scope("702.106", [S], "card-owner"), "double agenda": scope("702.106", [S], "card-owner"),
  // Triggered abilities.
  "ward": scope("702.21", [T], "targeting-player"), "rampage": scope("702.23", [T], trigger),
  "cumulative upkeep": scope("702.24", [T], trigger), "flanking": scope("702.25", [T], trigger),
  "echo": scope("702.30", [T], trigger), "provoke": scope("702.39", [T], trigger),
  "storm": scope("702.40", [T], trigger), "bushido": scope("702.45", [T], trigger),
  "soulshift": scope("702.46", [T], trigger), "epic": scope("702.50", [T, P], "spell-controller"),
  "haunt": scope("702.55", [T], trigger), "recover": scope("702.59", [T], trigger),
  "ripple": scope("702.60", [T], trigger), "frenzy": scope("702.68", [T], trigger),
  "gravestorm": scope("702.69", [T], trigger), "poisonous": scope("702.70", [T], trigger),
  "champion": scope("702.72", [T], trigger), "hideaway": scope("702.75", [T], trigger),
  "persist": scope("702.79", [T], trigger), "exalted": scope("702.83", [T], trigger),
  "cascade": scope("702.85", [T], trigger), "annihilator": scope("702.86", [T], trigger),
  "rebound": scope("702.88", [S, T], caster), "battle cry": scope("702.91", [T], trigger),
  "living weapon": scope("702.92", [T], trigger), "undying": scope("702.93", [T], trigger),
  "soulbond": scope("702.95", [T], trigger), "evolve": scope("702.100", [T], trigger),
  "extort": scope("702.101", [T], trigger), "dethrone": scope("702.105", [T], trigger),
  "prowess": scope("702.108", [T], trigger), "exploit": scope("702.110", [T], trigger),
  "renown": scope("702.112", [T], trigger), "ingest": scope("702.115", [T], trigger),
  "myriad": scope("702.116", [T], trigger), "melee": scope("702.121", [T], trigger),
  "fabricate": scope("702.123", [T], trigger), "afflict": scope("702.130", [T], trigger),
  "mentor": scope("702.134", [T], trigger), "afterlife": scope("702.135", [T], trigger),
  "demonstrate": scope("702.144", [T], trigger), "training": scope("702.149", [T], trigger),
  "enlist": scope("702.154", [S, T], trigger), "ravenous": scope("702.156", [T], trigger),
  "for mirrodin!": scope("702.163", [T], trigger), "backup": scope("702.165", [T], trigger),
  "mobilize": scope("702.181", [T], trigger), "job select": scope("702.182", [T], trigger),
  "firebending": scope("702.189", [T], trigger), "increment": scope("702.191", [T], trigger),
  "paradigm": scope("702.192", [T, P], trigger),
  // Activated abilities; the printed cost is paid by the activator.
  "equip": scope("702.6", [A], activator), "cycling": scope("702.29", [A], activator),
  "typecycling": scope("702.29", [A], activator), "ninjutsu": scope("702.49", [A], activator),
  "commander ninjutsu": scope("702.49", [A], activator), "transmute": scope("702.53", [A], activator),
  "aura swap": scope("702.65", [A], activator), "fortify": scope("702.67", [A], activator),
  "transfigure": scope("702.71", [A], activator), "reinforce": scope("702.77", [A], activator),
  "unearth": scope("702.84", [A], activator), "level up": scope("702.87", [A], activator),
  "scavenge": scope("702.97", [A], activator), "outlast": scope("702.107", [A], activator),
  "crew": scope("702.122", [A], activator), "embalm": scope("702.128", [A], activator),
  "eternalize": scope("702.129", [A], activator), "encore": scope("702.141", [A], activator),
  "reconfigure": scope("702.151", [A], activator), "craft": scope("702.167", [A], activator),
  "saddle": scope("702.171", [A], activator), "station": scope("702.184", [A], activator),
  // Abilities that apply while the spell is cast; the printed cost is paid by the caster.
  "buyback": scope("702.27", [S], caster), "kicker": scope("702.33", [S], caster),
  "multikicker": scope("702.33", [S], caster), "flashback": scope("702.34", [S], caster),
  "madness": scope("702.35", [S, T], caster), "affinity": scope("702.41", [S], caster),
  "entwine": scope("702.42", [S], caster), "splice": scope("702.47", [S], caster),
  "offering": scope("702.48", [S], caster), "convoke": scope("702.51", [S], caster),
  "replicate": scope("702.56", [S, T], caster), "delve": scope("702.66", [S], caster),
  "evoke": scope("702.74", [S, T], caster), "prowl": scope("702.76", [S], caster),
  "conspire": scope("702.78", [S, T], caster), "retrace": scope("702.81", [S], caster),
  "miracle": scope("702.94", [S, T], caster), "overload": scope("702.96", [S], caster),
  "cipher": scope("702.99", [S, P], "spell-controller"), "fuse": scope("702.102", [S], caster),
  "bestow": scope("702.103", [S], caster), "dash": scope("702.109", [S, T], caster),
  "awaken": scope("702.113", [S, P], caster), "surge": scope("702.117", [S], caster),
  "emerge": scope("702.119", [S], caster), "escalate": scope("702.120", [S], caster),
  "undaunted": scope("702.125", [S], caster), "improvise": scope("702.126", [S], caster),
  "aftermath": scope("702.127", [S], caster), "assist": scope("702.132", [S], caster),
  "jump-start": scope("702.133", [S], caster), "spectacle": scope("702.137", [S], caster),
  "escape": scope("702.138", [S], caster), "mutate": scope("702.140", [S], caster),
  "foretell": scope("702.143", [S, X], caster), "disturb": scope("702.146", [S], caster),
  "cleave": scope("702.148", [S], caster), "blitz": scope("702.152", [S, T], caster),
  "casualty": scope("702.153", [S, T], caster), "squad": scope("702.157", [S, T], caster),
  "prototype": scope("702.160", [S], caster), "more than meets the eye": scope("702.162", [S], caster),
  "bargain": scope("702.166", [S], caster), "freerunning": scope("702.173", [S], caster),
  "gift": scope("702.174", [S, T], caster), "offspring": scope("702.175", [S, T], caster),
  "impending": scope("702.176", [S, T], caster), "harmonize": scope("702.180", [S], caster),
  "warp": scope("702.185", [S, T], caster), "mayhem": scope("702.187", [S], caster),
  "web-slinging": scope("702.188", [S], caster), "sneak": scope("702.190", [S], caster),
  "teamwork": scope("702.194", [S], caster),
  // Costs paid as a special action (CR 116.2): turning face up, suspending, plotting.
  "morph": scope("702.37", [S, X], "special-action-player"), "megamorph": scope("702.37", [S, X], "special-action-player"),
  "suspend": scope("702.62", [S, T, X], "special-action-player"), "disguise": scope("702.168", [S, X], "special-action-player"),
  "plot": scope("702.170", [S, X], "special-action-player"),
  // Deck construction (Commander pairing), before the game begins.
  "partner": scope("702.124", [D], "card-owner"), "partner with": scope("702.124", [D, T], "card-owner"),
  "partner—friends forever": scope("702.124", [D], "card-owner"), "partner—character select": scope("702.124", [D], "card-owner"),
  "partner—survivors": scope("702.124", [D], "card-owner"), "friends forever": scope("702.124", [D], "card-owner"),
  "choose a background": scope("702.124", [D], "card-owner"), "doctor's companion": scope("702.124", [D], "card-owner"),
  // Not modeled.
  "companion": { unsupported: "A companion condition is checked against the owner's starting deck before the game (CR 702.139); that pregame scope is not bound." },
  "double team": { unsupported: "Double team is not defined in the 2026-09-25 Comprehensive Rules." },
};
