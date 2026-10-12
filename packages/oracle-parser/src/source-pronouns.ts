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
import { CARD_TYPES, OBJECT_CLASSES, canonicalSubtypeName } from "./vocab.ts";

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
