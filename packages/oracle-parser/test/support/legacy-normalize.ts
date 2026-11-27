// Frozen R5 differential oracle from the A1 base. Retire at R7.
import { CARD_TYPES, OBJECT_CLASSES, canonicalSubtypeName } from "../../src/vocab.ts";

// Normalization: raw Scryfall oracle text → clean ability lines.
//
// Contract:
//  - Newlines are ability boundaries and are PRESERVED (the old pipeline
//    flattened them, forcing the grammar to guess where abilities split).
//  - Reminder text (parentheticals) is removed.
//  - Self-references (the card's own name, and its pre-comma short name)
//    become `~`. Matching is exact-case and word-bounded — a card named
//    "Fear" must not rewrite the word "fear" in other contexts.
//  - Typographic quotes/minus are canonicalized so the lexer stays simple.

import { canonicalizeNamePunctuation, type CardNameCatalog } from "../../src/card-names.ts";

// CR 207.2a: parenthetical reminder text summarizes a rule rather than adding
// an ability. Remove a whole balanced parenthetical, including nested pairs,
// but never remove its line boundaries or join the words on either side.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function stripReminderText(text: string): string {
  let result = "";
  let copiedThrough = 0;
  for (let i = 0; i < text.length; i++) {
    if (text[i] !== "(") continue;
    let depth = 1;
    let end = i + 1;
    while (end < text.length && depth > 0) {
      if (text[end] === "(") depth++;
      else if (text[end] === ")") depth--;
      end++;
    }
    // Malformed parentheticals remain visible to the lexer, which rejects
    // them. Silently deleting an unterminated tail would create false success.
    if (depth !== 0) break;
    const newlines = text.slice(i, end).match(/\n/g)?.join("") ?? "";
    result += text.slice(copiedThrough, i) + (newlines || " ");
    copiedThrough = end;
    i = end - 1;
  }
  return result + text.slice(copiedThrough);
}

function canonicalizePunctuation(text: string): string {
  return canonicalizeNamePunctuation(text);
}

function escapeRegExp(s: string): string {
  return s.replace(/[\\^$*+?.()|[\]{}]/g, "\\$&");
}

function replaceName(text: string, name: string, protect: (name: string) => string, shortened = false): string {
  // Word-bounded, case-sensitive. Handles possessive: "Urza's" → "~'s".
  const re = new RegExp(`(?<![\\p{L}\\p{M}\\p{N}_])${escapeRegExp(name)}(?![\\p{L}\\p{M}\\p{N}_])`, "gu");
  return text.replace(re, (matched, offset: number) => {
    const before = text.slice(0, offset);
    if (shortened) {
      // A planeswalker type is not a reference to this particular object:
      // Gideon's emblem says "a Gideon planeswalker" (CR 201.5 / 205.3j).
      // A prefix of a literal name such as Nissa's Chosen is not that name.
      if (/\b(?:named|partner\s+with)\s+"?$/i.test(before)) return matched;
      // A flavor/ability-word label before a dash is printed text, not a
      // reference: "Blade of Magnus — Whenever Magnus the Red deals …".
      const head = before.slice(before.lastIndexOf("\n") + 1);
      const rest = text.slice(offset + matched.length);
      const dash = rest.indexOf(" — "), lineEnd = rest.indexOf("\n");
      if (!head.includes("—") && dash >= 0 && (lineEnd < 0 || dash < lineEnd)) {
        const label = head + matched + rest.slice(0, dash);
        if (!/[.,:;{]/.test(label) && label.trim().split(/\s+/u).length <= 4) return matched;
      }
      if (/\b(?:a|an|another|each|every|all|other)\s+$/i.test(before) ||
          /\btarget\s+$/i.test(before) && !/\bthat\s+target\s+$/i.test(before) ||
          /^\s+(?:planeswalkers?|spells?|cards?|creatures?|tokens?)\b/i.test(text.slice(offset + matched.length))) return matched;
    }
    // "named Squadron Hawk" specifies a literal name, not a reference to
    // this object (CR 201.5). Copying the ability cannot change that name.
    if (/\bnamed\s+"$/i.test(before)) return matched;
    if (/\bnamed\s+$/i.test(before)) return protect(matched);
    return "~";
  });
}

export function normalizeOracleText(text: string, cardName?: string, catalog?: CardNameCatalog, typeLine?: string): string[] {
  if (!text) return [];

  let t = canonicalizePunctuation(text);
  let literalMarker = "@@CATALOG_LITERAL@@";
  while (t.includes(literalMarker)) literalMarker += "@";
  const literals: string[] = [];
  // Only explicit grammar positions establish literal names. In particular,
  // do not extend comma/and lists by name lookup: Sacrifice and X are card
  // names, but can instead begin the following instruction or amount.
  if (catalog) {
    const spans: { start: number; end: number; name: string }[] = [];
    for (const match of t.matchAll(/\b(named|partner\s+with|creates?)\s+/gi)) {
      const context = match[1].toLowerCase();
      if (context === "named" && /\bdifferently\s+$/i.test(t.slice(0, match.index))) continue;
      const start = match.index + match[0].length;
      const kinds = context.startsWith("partner") ? ["card" as const] : ["card" as const, "token" as const];
      const candidates = catalog.matches(t, start, kinds);
      let literal = candidates[0]?.end !== candidates[1]?.end ? candidates[0] : undefined;
      if (!literal && t[start] === '"') {
        let end = start + 1;
        while (end < t.length) {
          if (t[end] === "\\") { end += 2; continue; }
          if (t[end] === '"') break;
          end++;
        }
        if (end < t.length) {
          try {
            const value: unknown = JSON.parse(t.slice(start, end + 1));
            const known = typeof value === "string" ? catalog.exact(value, kinds) : undefined;
            if (known) literal = { ...known, end: end + 1 };
          } catch { /* Malformed literal remains visible to the lexer. */ }
        }
      }
      if (!literal || (context.startsWith("create") && !/^,\s+(?:a|an)\s+/.test(t.slice(literal.end)))) continue;
      spans.push({ start, end: literal.end, name: literal.name });
    }
    for (const span of spans.sort((a, b) => b.start - a.start)) {
      const marker = `${literalMarker}${literals.push(span.name) - 1}${literalMarker}`;
      t = t.slice(0, span.start) + marker + t.slice(span.end);
    }
  }
  t = stripReminderText(t);

  if (cardName) {
    const name = canonicalizePunctuation(cardName);
    let marker = "@@NAMED_CARD@@";
    while (t.includes(marker)) marker += "@";
    const names: string[] = [];
    const protect = (value: string) => `${marker}${names.push(value) - 1}${marker}`;
    t = replaceName(t, name, protect);
    // Mid-sentence the article of "The Wizardly Barge" is printed lowercase.
    if (name.startsWith("The ")) t = replaceName(t, `the ${name.slice(4)}`, protect);
    const comma = name.indexOf(",");
    if (comma > 0) {
      // Legendary short name: "Halsin, Emerald Archdruid" → also replace "Halsin".
      t = replaceName(t, name.slice(0, comma).trim(), protect, true);
    } else if (typeLine && /\bPlaneswalker\b/.test(typeLine.split(/—|\s-\s/u, 1)[0])) {
      // Some planeswalker names lack a comma (Vraska the Unseen, Ajani
      // Vengeant). Source metadata establishes the named character: require
      // an exact initial name word matching a printed planeswalker subtype.
      // Never shorten an arbitrary creature/card name to its first word.
      // CR 201.5c: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
      const prefix = name.split(" ")[0];
      const subtypes = typeLine.split(/—|\s-\s/u)[1]?.trim().split(/\s+/u) ?? [];
      if (name !== prefix && subtypes.includes(prefix)) t = replaceName(t, prefix, protect, true);
    } else if (namesLegendaryCharacter(typeLine)) {
      // CR 201.5c: a legendary character named with an epithet and no comma
      // ("Bontu the Glorified", "Sorin of House Markov") is called by the
      // part of its own name before that epithet. Source metadata supplies
      // both facts; no other leading words of a name are ever a short name.
      const epithet = /^(.+?) (?:the|of) ./u.exec(name);
      if (epithet) t = replaceName(t, epithet[1], protect, true);
      else if (name.includes(" ")) {
        // CR 201.5c: "Edgar Markov" is called "Edgar" in "Whenever Edgar
        // attacks". The first word is taken as the short name only when the
        // card itself uses it as the subject of a trigger, and never when it
        // is one of the card's own creature types or a title before a
        // capitalized surname ("Captain Janeway").
        const first = name.split(" ")[0];
        const subtypes = typeLine?.split(/—|\s-\s/u)[1]?.trim().split(/\s+/u) ?? [];
        const subject = new RegExp(`\\b(?:When|Whenever) ${escapeRegExp(first)}(?= (?:enters|attacks|dies|leaves|blocks|deals|becomes|is|transforms|or|and)\\b)`, "u");
        if (/^\p{Lu}/u.test(first) && !subtypes.includes(first) && subject.test(t)) t = replaceName(t, first, protect, true);
      }
    }
    // Literal names are still opaque markers here, so a pronoun-shaped word
    // inside one ("A Girl and Her Dogs") cannot be rewritten.
    if (namesLegendaryCharacter(typeLine)) t = t.split("\n").map(rewriteSourcePronouns).join("\n");
    names.forEach((value, index) => { t = t.replace(`${marker}${index}${marker}`, JSON.stringify(value)); });
  }

  literals.forEach((value, index) => { t = t.replace(`${literalMarker}${index}${literalMarker}`, JSON.stringify(value)); });

  return t
    .split("\n")
    .map((line) => line.replace(/\s+/g, " ").trim())
    .filter((line) => line.length > 0);
}

// Gendered pronouns for a legendary source ("he", "him", "his", "she", "her").
//
// Oracle prints these only for the named character of a legendary creature or
// planeswalker, i.e. for the object the text otherwise calls by its name
// (CR 201.5 treats a card's own name in its text as "this object"). They are
// the animate spelling of the two ordinary templates for that object:
//   "Whenever ~ attacks, he gets +1/+0"   ↔  "…, it gets +1/+0"
//   "discard a card. If you do, put a +1/+1 counter on him"  ↔  "… on ~"
// Nongendered Oracle text uses "it" where the source is the only candidate and
// repeats the name where another object could be meant. This pass makes the
// same choice, so the grammar sees exactly one of two existing references.
//
// Why normalization and not the reference grammar: whether a pronoun denotes
// the source is a fact about the card (legendary type line, a self-reference
// established in the same ability), which is known here and not to a token
// cursor; and "it"/"its" are consumed at dozens of sites across the grammar,
// each of which would otherwise need a gated alternative. Rewriting to the
// canonical spelling keeps same-meaning text on the same productions and nodes.
//
// A pronoun is rewritten only when it provably denotes the source:
//  - the face is a legendary creature or planeswalker (checked by the caller);
//  - a self-reference ("~", "this card", or an already resolved pronoun)
//    precedes it in the same line. A pronoun without one is left alone and
//    the line fails, as does any pronoun for another character ("Her Majesty",
//    "that permanent … her controller" on a nonlegendary card).
// It becomes "it"/"its" only if that is as unambiguous as the printed word:
// no singular object is mentioned between the self-reference and the pronoun,
// and the line prints no neuter pronoun of its own (which would then denote
// something else: "…equal to its power to him"). Otherwise it becomes "~".
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt

const WORD = "[\\p{L}\\p{M}\\p{N}_'-]";
const PRONOUN = new RegExp(`(?<!${WORD})(?:he's|she's|himself|herself|he|she|him|her|his)(?!${WORD})`, "giu");
const NEUTER = new RegExp(`(?<!${WORD})(?:it|its|it's|itself)(?!${WORD})`, "iu");
const ANCHOR = /~|\bthis card\b/giu;

// "her" is both the object and the possessive form. Only attested, closed
// continuations decide; anything else leaves the line unrewritten.
const HER_POSSESSED = new Set(["owner's", "owners'", "controller's", "power", "toughness", "name", "other", "sneak", "loyalty", "additional", "mana", "base"]);
const HER_OBJECT_FOLLOWERS = new Set(["to", "into", "onto", "and", "for", "from", "deal", "unless", "this", "is", "equal"]);

type Role = "subject" | "object" | "possessive" | "contraction" | "reflexive";

function role(word: string, after: string): Role | null {
  switch (word) {
    case "he": case "she": return "subject";
    case "him": return "object";
    case "his": return "possessive";
    case "he's": case "she's": return "contraction";
    case "himself": case "herself": return "reflexive";
  }
  const next = /^\s+([\p{L}'-]+)/u.exec(after)?.[1].toLowerCase();
  if (next === undefined) return /^\s*(?:[.,;:"]|$)/u.test(after) ? "object" : null;
  if (HER_POSSESSED.has(next)) return "possessive";
  return HER_OBJECT_FOLLOWERS.has(next) ? "object" : null;
}

/** A singular object noun: something a following "it" could denote instead. */
function mentionsOtherObject(window: string): boolean {
  for (const [raw] of window.matchAll(/[\p{L}][\p{L}'-]*/gu)) {
    const word = raw.toLowerCase();
    if (word === "target" || CARD_TYPES.has(word) || OBJECT_CLASSES.has(word)) return true;
    // Subtypes are capitalized in Oracle text; "will" and "mine" are not types.
    if (canonicalSubtypeName(raw) === raw) return true;
  }
  return false;
}

const NEUTER_FORM: Record<Role, string> = { subject: "it", object: "it", possessive: "its", contraction: "it's", reflexive: "itself" };
// A gendered reflexive still names the character: where "itself" could be read
// as another object ("put it into your hand. ~ deals damage to himself"), it
// is the source by name as well.
const NAME_FORM: Record<Role, string> = { subject: "~", object: "~", possessive: "~'s", contraction: "~ is", reflexive: "~" };

/** One normalized line whose self-names are already `~`. Returns the line
 * unchanged unless every gendered pronoun in it provably denotes the source. */
export function rewriteSourcePronouns(line: string): string {
  const printsNeuter = NEUTER.test(line);
  let result = "";
  let copiedThrough = 0;
  // End of the latest established reference to the source, in `line` offsets.
  let anchoredAt = -1;
  const anchors = [...line.matchAll(ANCHOR)].map(match => match.index + match[0].length);
  for (const match of line.matchAll(PRONOUN)) {
    const printed = match[0];
    const start = match.index;
    const before = line.slice(0, start);
    const sentenceStart = /(?:^|[.:—•]\s+|")$/u.test(before);
    // Mid-sentence capitals are titles or names, not pronouns ("Her Majesty").
    if (printed !== printed.toLowerCase() && !sentenceStart) return line;
    for (const end of anchors) if (end <= start && end > anchoredAt) anchoredAt = end;
    if (anchoredAt < 0) return line;
    const as = role(printed.toLowerCase(), line.slice(start + printed.length));
    if (!as) return line;
    const contested = printsNeuter || mentionsOtherObject(line.slice(anchoredAt, start));
    let replacement = (contested ? NAME_FORM : NEUTER_FORM)[as];
    if (sentenceStart && printed !== printed.toLowerCase() && !replacement.startsWith("~"))
      replacement = replacement[0].toUpperCase() + replacement.slice(1);
    result += line.slice(copiedThrough, start) + replacement;
    copiedThrough = anchoredAt = start + printed.length;
  }
  return result + line.slice(copiedThrough);
}

/** Printed card types of a face that names one character (CR 205.4d). */
export function namesLegendaryCharacter(typeLine: string | undefined): boolean {
  const types = typeLine?.split(/—|\s-\s/u, 1)[0] ?? "";
  return /\bLegendary\b/.test(types) && /\b(?:Creature|Planeswalker)\b/.test(types);
}
