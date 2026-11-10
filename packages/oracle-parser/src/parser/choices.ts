// Value choices are not selections of cards/permanents. CR 205.2a lists card
// types; CR 205.3e requires subtype choices to be existing, singular values.
// Linked references to such choices are governed by CR 607.2d.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { ChosenValueRef, Color, ObjectFilter, ValueChoice } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { COLORS, canonicalSubtypeName, wordNumber } from "../vocab.ts";
import { characteristicSubtypeFamily } from "./characteristics.ts";

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

type FilterParser = (c: Cursor) => ObjectFilter | null;

/** "[article] <card description> name": the description up to "card" is an
 * ordinary card filter (CR 201.4a). A bare "card name" has no filter. */
function cardNameDescription(c: Cursor, parseFilter: FilterParser): { filter?: ObjectFilter } | null {
  return c.attempt(c => {
    if (c.words("card", "name")) return {};
    let offset = 0;
    for (;; offset++) {
      const token = c.peek(offset);
      if (!token || (token.kind === "punct" && token.value !== ",")) return null;
      if (token.kind === "word" && !token.possessive && token.value === "card") break;
    }
    const next = c.peek(offset + 1);
    if (next?.kind !== "word" || next.possessive || next.value !== "name") return null;
    const description = new Cursor(c.tokens.slice(c.pos, c.pos + offset + 1));
    const filter = parseFilter(description);
    if (!filter || !description.done() || filter.cls !== "card") return null;
    c.pos += offset + 2;
    return { filter };
  });
}

/** CR 201.4: "any card name" states the same unrestricted choice as "a card
 * name". The exclusion is itself a described class of card names. */
function cardNameChoice(c: Cursor, parseFilter: FilterParser): ValueChoice | null {
  return c.attempt((c): ValueChoice | null => {
    if (c.word("a", "an", "any") === null) return null;
    const chosen = cardNameDescription(c, parseFilter);
    if (!chosen) return null;
    let excluding: ObjectFilter | undefined;
    if (c.words("other", "than")) {
      const excluded = c.word("a", "an") !== null ? cardNameDescription(c, parseFilter) : null;
      if (!excluded?.filter) return null;
      excluding = excluded.filter;
    }
    return { category: "card-name", ...chosen, ...(excluding ? { excluding } : {}) };
  });
}

function creatureType(c: Cursor): string | null {
  return c.attempt(c => {
    if (c.words("time", "lord")) return "Time Lord";
    const word = c.anyWord();
    const name = word && canonicalSubtypeName(word.raw);
    // A singular, existing creature type (CR 205.3e); "Walls" is not a value.
    return name && name.toLowerCase() === word.value && characteristicSubtypeFamily(name) === "creature" ? name : null;
  });
}

/** Called immediately after the choose/chooses verb, before object selection.
 * An article declares an unrestricted value category; bare exact OR lists
 * declare literal options. "Choose a Forest" remains an object selection. */
export function parseValueChoice(c: Cursor, parseFilter?: FilterParser): ValueChoice | null {
  return c.attempt(c => {
    const cardName = parseFilter ? cardNameChoice(c, parseFilter) : null;
    if (cardName) return cardName;
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
      // CR 106.5 example (Meteor Crater): "a color of a permanent you control".
      const among = count === 1 && !excluding && parseFilter ? c.attempt(c => {
        if (c.word("of") === null || c.word("a", "an") === null) return null;
        return parseFilter(c);
      }) : null;
      return { category: "color", ...(count !== 1 ? { count } : {}), ...(excluding ? { excluding } : {}), ...(among ? { among } : {}) };
    });
    if (colors) return colors;
    if (c.words("odd", "or", "even")) return { category: "parity" };
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
    if (category === "creature-type" && c.words("other", "than")) {
      const excluding = closedValues(c, creatureType);
      return excluding ? { category, excluding } : null;
    }
    if (category === "number") {
      // "a number between 0 and 13", "a number greater than 0".
      const range = c.attempt((c): ValueChoice | null => {
        if (c.word("between") !== null) {
          const minimum = c.number();
          const maximum = minimum !== null && c.word("and") !== null ? c.number() : null;
          return minimum !== null && maximum !== null && minimum < maximum ? { category, minimum, maximum } : null;
        }
        if (!c.words("greater", "than")) return null;
        const bound = c.number();
        return bound !== null ? { category, minimum: bound + 1 } : null;
      });
      if (range) return range;
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

/** Value choices instructed earlier in this same body, outside any quoted
 * ability. A lexical proof only: each choice sentence must itself parse. */
function precedingValueChoices(c: Cursor, parseFilter: FilterParser): ValueChoice[] {
  const found: ValueChoice[] = [];
  for (let index = 0; index < c.pos; index++) {
    const token = c.tokens[index];
    if (token.kind !== "word" || token.possessive || (token.value !== "choose" && token.value !== "chooses")) continue;
    if (c.tokens.slice(index, c.pos).some(between => between.kind === "punct" && between.value === '"')) continue;
    const choice = parseValueChoice(new Cursor(c.tokens.slice(index + 1, c.pos)), parseFilter);
    if (choice) found.push(choice);
  }
  return found;
}

/** "with the [last] chosen name" / "has the chosen name" refer to the linked
 * card-name choice (CR 607.2d). "That name" is the same reference only when
 * exactly one earlier instruction of this body chooses a card name; the name
 * of some other mentioned object is a different relation and is not guessed. */
export function parseChosenNameQualifier(c: Cursor, parseFilter: FilterParser, connector: "with" | "have"): ChosenValueRef<"card-name"> | null {
  return c.attempt((c): ChosenValueRef<"card-name"> | null => {
    if (connector === "with" ? c.word("with") === null : c.word("has", "have") === null) return null;
    if (c.words("that", "name")) {
      return precedingValueChoices(c, parseFilter).filter(choice => choice.category === "card-name").length === 1
        ? { choice: "chosen-value", category: "card-name", which: "chosen" } : c.fail('one preceding card-name choice for "that name"');
    }
    if (c.word("the") === null) return null;
    const last = c.word("last") !== null;
    return c.words("chosen", "name") ? { choice: "chosen-value", category: "card-name", which: last ? "last-chosen" : "chosen" } : null;
  });
}

/** "of that color" / "of that type" after exactly one earlier choice of one
 * color or one type in this body: the same symbolic references as "of the
 * chosen color/type" (CR 607.2d, 608.2c). */
export function parseThatChoiceQualifier(c: Cursor, parseFilter: FilterParser): Pick<ObjectFilter, "chosenColor" | "chosenType"> | null {
  return c.attempt((c): Pick<ObjectFilter, "chosenColor" | "chosenType"> | null => {
    if (!c.words("of", "that")) return null;
    const noun = c.word("color", "type");
    if (!noun) return null;
    const choices = precedingValueChoices(c, parseFilter);
    if (noun === "color") {
      const colors = choices.filter(choice => choice.category === "color");
      return colors.length === 1 && colors[0].category === "color" && colors[0].count === undefined
        ? { chosenColor: { choice: "chosen-value", category: "color", which: "chosen" } } : c.fail('one preceding color choice for "that color"');
    }
    const kinds = choices.flatMap((choice): ("creature" | "land" | "card" | "other")[] => {
      if (choice.category === "creature-type") return ["creature"];
      if (choice.category === "basic-land-type" || choice.category === "land-type") return ["land"];
      if (choice.category === "card-type") return ["card"];
      if (choice.category === "type") return [choice.options.every(option => option.type === "card-type") ? "card" : "other"];
      return [];
    });
    // The same node as "of the chosen type": the kind is recorded only when
    // printed, and the linked choice itself supplies it.
    return kinds.length === 1 && kinds[0] !== "other"
      ? { chosenType: { choice: "chosen-type" } } : c.fail('one preceding type choice for "that type"');
  });
}

/** A color named as a value: "the [last] chosen color", or "that color" after
 * exactly one earlier choice of one color in this body (CR 607.2d, 608.2c). */
export function parseChosenColorRef(c: Cursor, parseFilter: FilterParser): ChosenValueRef<"color"> | null {
  return c.attempt((c): ChosenValueRef<"color"> | null => {
    if (c.words("that", "color")) {
      const colors = precedingValueChoices(c, parseFilter).filter(choice => choice.category === "color");
      return colors.length === 1 && colors[0].category === "color" && colors[0].count === undefined
        ? { choice: "chosen-value", category: "color", which: "chosen" } : c.fail('one preceding color choice for "that color"');
    }
    const chosen = parseChosenValueRef(c);
    return chosen?.category === "color" ? { ...chosen, category: "color" } : null;
  });
}

/** "of the creature type of your choice" / "of the color of your choice" as
 * a filter qualifier. A target description in the same clause would need
 * the value when targets are announced; that timing is not represented. */
export function parseValueOfChoiceQualifier(c: Cursor): NonNullable<ObjectFilter["valueOfChoice"]> | null {
  return c.attempt((c): NonNullable<ObjectFilter["valueOfChoice"]> | null => {
    if (!c.words("of", "the")) return null;
    const category = c.words("creature", "type") ? "creature-type" : c.word("color") !== null ? "color" : null;
    if (!category || !c.words("of", "your", "choice")) return null;
    for (let index = c.pos - 1; index >= 0; index--) {
      const token = c.tokens[index];
      if (token.kind === "punct" && token.value !== ",") break;
      if (token.kind === "word" && !token.possessive && (token.value === "target" || token.value === "targets"))
        return c.fail("non-target objects for a value chosen on resolution");
    }
    return { choice: "fresh-choice", category, who: { player: "you" }, when: "instruction-applies" };
  });
}

/** A value chosen while an instruction is applied has no meaning in a static
 * ability, which is never applied as an instruction (CR 604.1, 608.2d).
 * Nested abilities are their own scope. */
export function hasValueOfChoice(value: unknown): boolean {
  if (!value || typeof value !== "object") return false;
  if (Array.isArray(value)) return value.some(hasValueOfChoice);
  if ("kind" in value && typeof value.kind === "string") return false;
  if ("valueOfChoice" in value && value.valueOfChoice) return true;
  return Object.values(value).some(hasValueOfChoice);
}
