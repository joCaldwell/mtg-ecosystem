// CR 114.1–114.4: an emblem is an object in the command zone with only the
// abilities its creating effect gives it; those abilities function there and
// refer to the emblem itself as "this emblem" (the source of their damage,
// CR 609.7). The phrase is only meaningful inside an emblem's own abilities,
// so it is recognized only while those are being parsed.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
let depth = 0;

/** Parse the abilities of an emblem being created. */
export function withinEmblem<T>(body: () => T): T {
  depth++;
  try { return body(); } finally { depth--; }
}

/** Whether "this emblem" currently names an emblem. */
export function insideEmblem(): boolean {
  return depth > 0;
}
