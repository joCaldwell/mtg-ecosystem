// CR 207.2c–d: ability words and flavor words are italic labels in front of
// an ability, separated from it by a long dash. They "have no special rules
// meaning"; flavor words in particular are free text tailored to one ability
// ("Avalanche!", "Nitro-9", "In You, All Things Are Possible"), so the label
// is kept verbatim and never interpreted.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Token } from "../lexer.ts";
import { isKeywordName } from "./keywords.ts";

/** Labels that define an ability's rules (see tryAbilityWord), never cosmetic. */
const MECHANICAL_LABELS = new Set(["visit", "max speed", "boast", "exhaust", "power-up", "forecast"]);
/** A label is short; a longer run before a dash is rules text. */
const MAX_LABEL_TOKENS = 8;

export interface FlavorLabel { label: string; rest: Token[] }

/** Splits "<label> — <ability>". The label is words, numbers and the
 * punctuation printed inside flavor words (an exclamation mark, commas, a
 * hyphen); it holds at least one word, so a dice-table range ("1—9 | …") is
 * never one, and it does not end in a word's numeric parameter. Sentence
 * punctuation, symbols, quotes and self-references end the search: those are
 * rules text, not a label. */
export function splitFlavorLabel(tokens: Token[]): FlavorLabel | null {
  let label = "";
  let words = 0;
  // "<words> N —" is the shape of a parameterized rule ("Descend 5", "Stage
  // 2"), not of a label; a number inside a label is followed by more words.
  let trailingParameter = false;
  for (let i = 0; i <= MAX_LABEL_TOKENS && i < tokens.length; i++) {
    const token = tokens[i];
    const previous = tokens[i - 1];
    if (token.kind === "punct" && token.value === "—") {
      const rest = tokens.slice(i + 1);
      const name = label.toLowerCase();
      if (!words || trailingParameter || rest.length === 0 || MECHANICAL_LABELS.has(name) || isKeywordName(name)) return null;
      // CR 701.55a: "[A player] faces a villainous choice — …" is an instruction.
      if (name.endsWith("faces a villainous choice") || name.endsWith("face a villainous choice")) return null;
      return { label, rest };
    }
    if (token.kind === "word") {
      const text = token.possessive ? `${token.raw}${token.possessiveSuffix ?? "'s"}` : token.raw;
      label += previous && !(previous.kind === "punct" && previous.value === "-") ? ` ${text}` : text;
      words++;
      trailingParameter = false;
    } else if (token.kind === "number") {
      // "10,000 Needles", "Nitro-9": digits printed against the punctuation.
      const joined = previous?.kind === "punct" && (previous.value === "-" || (previous.value === "," && tokens[i - 2]?.kind === "number"));
      label += previous && !joined ? ` ${token.raw}` : token.raw;
      if (previous?.kind === "word") trailingParameter = true;
    } else if (token.kind === "punct" && (token.value === "!" || token.value === "," || token.value === "-")) {
      if (!previous || previous.kind === "punct") return null;
      label += token.value;
    } else return null;
  }
  return null;
}
