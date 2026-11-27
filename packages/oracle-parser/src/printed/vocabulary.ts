// Closed surface forms used by the lexical pass and the generated verifier.
export const INTRODUCERS = ["a", "another", "each", "all", "any", "the", "x"] as const;
export const MENTIONS = ["this", "that", "those", "it", "they", "you", "its", "their", "your", "x"] as const;
export const POLARITIES = ["can't", "don't", "doesn't", "isn't", "not", "no", "only", "without"] as const;
export const EXCEPTIONS = ["another", "other", "except", "unless", "instead"] as const;
