// Value choices are not selections of cards/permanents. CR 205.2a lists card
// types; CR 205.3e requires subtype choices to be existing, singular values.
// Linked references to such choices are governed by CR 607.2d.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { ChosenValueRef, Color, ValueChoice } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { COLORS, canonicalSubtypeName, wordNumber } from "../vocab.ts";

type TypeOption = Extract<ValueChoice, { category: "type" }>["options"][number];
// The full CR card-type universe includes types outside ordinary deck cards.
// "Tribal" is an obsolete name, not an additional current choice.
const CARD_TYPE_VALUES = new Set([
  "artifact", "battle", "conspiracy", "creature", "dungeon", "enchantment", "instant", "kindred",
  "land", "phenomenon", "plane", "planeswalker", "scheme", "sorcery", "vanguard",
]);

/** The reference is intentionally symbolic. CR 607.2d determines which
 * earlier choice it names; lexical proximity does not establish that link. */
export function parseChosenValueRef(c: Cursor): ChosenValueRef | null {
  return c.attempt(c => {
    if (c.word("the") === null) return null;
    const last = c.word("last") !== null;
    if (c.word("chosen") === null) return null;
    const category = c.words("card", "type") ? "card-type" : c.words("card", "name") ? "card-name" : c.word("color", "player");
    return category === "card-type" || category === "card-name" || category === "color" || category === "player"
      ? { choice: "chosen-value", category, which: last ? "last-chosen" : "chosen" } : null;
  });
}

/** One closed value, or a comma-separated list ending in explicit "or".
 * A comma that starts a later instruction is not part of the value list. */
function closedValues<T extends string>(c: Cursor, read: (c: Cursor) => T | null): T[] | null {
  return c.attempt(c => {
    const first = read(c);
    if (first === null) return null;
    const values = [first];
    let finalOr = false;
    for (;;) {
      const start = c.pos;
      const comma = c.punct(",");
      const or = c.word("or") !== null;
      if (!comma && !or) break;
      const next = read(c);
      if (next === null) {
        if (comma && !or && c.isWord("then")) { c.pos = start; break; }
        return null;
      }
      if (values.includes(next)) return null;
      values.push(next); finalOr = or;
    }
    return values.length === 1 || finalOr ? values : null;
  });
}

function color(c: Cursor): Color | null {
  return c.attempt(c => {
    const word = c.anyWord();
    return word && COLORS.has(word.value) ? word.value as Color : null;
  });
}

function typeOption(c: Cursor): TypeOption | null {
  return c.attempt(c => {
    if (c.words("time", "lord")) return { type: "subtype", value: "Time Lord" };
    if (c.possessive("urza") !== null) return { type: "subtype", value: "Urza's" };
    const word = c.anyWord();
    if (!word) return null;
    if (CARD_TYPE_VALUES.has(word.value)) return { type: "card-type", value: word.value };
    const subtype = canonicalSubtypeName(word.raw);
    // In particular, the anchor word "Dragons" is not a literal Dragon type.
    return subtype && subtype.toLowerCase() === word.value ? { type: "subtype", value: subtype } : null;
  });
}

/** Called immediately after the choose/chooses verb, before object selection.
 * An article declares an unrestricted value category; bare exact OR lists
 * declare literal options. "Choose a Forest" remains an object selection. */
export function parseValueChoice(c: Cursor): ValueChoice | null {
  return c.attempt(c => {
    // A color is one of the five values in CR 105.1. Choosing two colors
    // chooses distinct values, not a multicolored game object.
    const colors = c.attempt((c): ValueChoice | null => {
      let count: number | null;
      if (c.word("a") !== null) count = 1;
      else {
        count = c.number();
        if (count === null) {
          const word = c.anyWord();
          count = word ? wordNumber(word.value) ?? null : null;
        }
      }
      if (count === null || count < 1 || count > 5 || c.word(count === 1 ? "color" : "colors") === null) return null;
      const excluding = c.words("other", "than") ? closedValues(c, color) : undefined;
      if (excluding === null) return null;
      return { category: "color", ...(count !== 1 ? { count } : {}), ...(excluding ? { excluding } : {}) };
    });
    if (colors) return colors;
    const category = c.attempt(c => {
      if (c.word("a") === null) return null;
      if (c.words("basic", "land", "type")) return "basic-land-type" as const;
      if (c.words("creature", "type")) return "creature-type" as const;
      if (c.words("card", "type")) return "card-type" as const;
      if (c.words("land", "type")) return "land-type" as const;
      if (c.words("card", "name")) return "card-name" as const;
      return c.word("number") as "number" | null;
    });
    if (category === "card-type" && c.words("other", "than")) {
      const excluding = closedValues(c, c => c.attempt(c => {
        const word = c.anyWord();
        return word && CARD_TYPE_VALUES.has(word.value) ? word.value : null;
      }));
      return excluding ? { category, excluding } : null;
    }
    if (category) return { category };
    const colorOptions = closedValues(c, color);
    if (colorOptions) return colorOptions.length >= 2 ? { category: "color", options: colorOptions } : null;
    const first = typeOption(c);
    if (!first) return null;
    const options = [first];
    let finalOr = false;
    for (;;) {
      const start = c.pos;
      const comma = c.punct(",");
      const or = c.word("or") !== null;
      if (!comma && !or) break;
      const next = typeOption(c);
      if (!next) {
        // A following instruction can start after the comma of a complete
        // choice ("Island or Swamp, then draw"); an incomplete OR cannot.
        if (comma && !or && c.isWord("then")) { c.pos = start; break; }
        return null;
      }
      if (next.type !== first.type || options.some(option => option.value === next.value)) return null;
      options.push(next);
      finalOr = or;
    }
    return options.length >= 2 && finalOr ? { category: "type", options } : null;
  });
}
