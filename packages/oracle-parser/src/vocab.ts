// Closed word classes the parser needs to classify tokens.
// Open classes (creature subtypes, counter names, …) are NOT enumerated here;
// the parser accepts them positionally and records the raw word.

export const CARD_TYPES = new Set([
  "artifact", "battle", "creature", "enchantment", "instant", "kindred",
  "tribal", "land", "planeswalker", "sorcery",
]);

/** Object classes that head a filter like a type does. */
export const OBJECT_CLASSES = new Set(["permanent", "spell", "card", "token", "ability"]);

export const SUPERTYPES = new Set(["legendary", "basic", "snow", "world"]);

export const COLORS = new Set(["white", "blue", "black", "red", "green"]);

export const ZONES = new Set([
  "battlefield", "graveyard", "library", "hand", "exile", "stack",
]);

export const STATUS_WORDS = new Set([
  "tapped", "untapped", "attacking", "blocking", "blocked", "enchanted",
  "equipped", "monstrous", "face-up", "face-down", "unblocked",
]);

const WORD_NUMBERS: Record<string, number> = {
  one: 1, two: 2, three: 3, four: 4, five: 5, six: 6, seven: 7, eight: 8,
  nine: 9, ten: 10, eleven: 11, twelve: 12, thirteen: 13, fourteen: 14,
  fifteen: 15, sixteen: 16, seventeen: 17, eighteen: 18, nineteen: 19,
  twenty: 20,
};

export function wordNumber(word: string): number | undefined {
  return WORD_NUMBERS[word];
}

/**
 * Singular form of a pluralizable rules word. Only used for closed-class and
 * head-noun positions, so a simple suffix rule plus exceptions is enough.
 */
export function singularize(word: string): string {
  const exceptions: Record<string, string> = {
    sorceries: "sorcery",
    libraries: "library",
    abilities: "ability",
    copies: "copy",
    elves: "elf",
    dwarves: "dwarf",
    wolves: "wolf",
    thopters: "thopter",
    mercenaries: "mercenary",
    allies: "ally",
    armies: "army",
    harpies: "harpy",
    zombies: "zombie",
    faeries: "faerie",
  };
  if (exceptions[word]) return exceptions[word];
  if (word.endsWith("ies") && word.length > 4) return word.slice(0, -3) + "y";
  if (word.endsWith("sses")) return word.slice(0, -2);
  if (word.endsWith("s") && !word.endsWith("ss")) return word.slice(0, -1);
  return word;
}
