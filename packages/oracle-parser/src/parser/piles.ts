// Piles (CR 700.3): a player temporarily groups objects into piles; every
// object goes into exactly one pile (700.3a), a pile can be empty (700.3d),
// and nothing changes zones while grouped (700.3c). A pile is not an object
// (700.3b), so a pile reference denotes the objects in it.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, ObjectRef, PileSpec, PlayerRef, Sentence } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

type PileRef = Extract<ObjectRef, { ref: "pile" }>;

interface StepGrammar {
  actor(c: Cursor, imperative: string, conjugated: string): PlayerRef | null;
  parseObjectRef(c: Cursor): ObjectRef | null;
}

/** Exactly two piles: "the other" pile is then everything not picked. */
function parsePileSpecs(c: Cursor): [PileSpec, PileSpec] | null {
  return c.attempt((c): [PileSpec, PileSpec] | null => {
    if (c.words("two", "piles")) return [{}, {}];
    if (c.words("two", "face-down", "piles")) return [{ facing: "down" }, { facing: "down" }];
    const one = (c: Cursor): PileSpec | null => c.attempt(c => {
      if (c.word("a") === null) return null;
      const facing = c.word("face-down", "face-up");
      return facing && c.word("pile") !== null ? { facing: facing === "face-down" ? "down" : "up" } : null;
    });
    const first = one(c);
    const second = first && c.word("and") !== null ? one(c) : null;
    return first && second && first.facing !== second.facing ? [first, second] : null;
  });
}

/** "<player> separates <objects> into two piles" and "<player> chooses one
 * of those piles". The chooser never targets; the pile is picked on
 * resolution (CR 608.2d). */
export function parsePileStep(c: Cursor, g: StepGrammar): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    const who = g.actor(c, "separate", "separates");
    if (!who) return null;
    const what = g.parseObjectRef(c);
    if (!what || c.word("into") === null) return null;
    const piles = parsePileSpecs(c);
    return piles && [{ effect: "separate-piles", who, what, piles }];
  }) ?? c.attempt((c): Effect[] | null => {
    const who = g.actor(c, "choose", "chooses");
    if (!who || !c.words("one", "of") || c.word("those", "the") === null || c.word("piles") === null) return null;
    return [{ effect: "choose-pile", who }];
  });
}

/** "that pile" / "the chosen pile": the pile picked by the preceding
 * choose-pile. "the pile of <player>'s choice": that player picks a pile as
 * the instruction is carried out. */
export function parsePileRef(c: Cursor, parsePossessivePlayer: (c: Cursor) => PlayerRef | null): PileRef | null {
  return c.attempt((c): PileRef | null => {
    if (c.words("that", "pile") || c.words("the", "chosen", "pile")) return { ref: "pile", pile: "chosen" };
    if (!c.words("the", "pile", "of")) return null;
    const who = parsePossessivePlayer(c);
    return who && c.word("choice") !== null ? { ref: "pile", pile: "choice", who } : null;
  });
}

/** "Put one pile into your hand …": the player carrying out the put picks
 * the pile. Only directly before a destination. */
export function parseOnePile(c: Cursor, who: PlayerRef): PileRef | null {
  return c.attempt((c): PileRef | null =>
    c.words("one", "pile") && c.isWord("onto", "into", "on") ? { ref: "pile", pile: "choice", who } : null);
}

/** A pile exists only after a separation, and "that pile" only after a
 * player chose one. Checked over the sentences read so far plus `sentence`. */
export function pileReferencesResolved(previous: Sentence[], sentence: Sentence): boolean {
  const scan = (value: unknown, found: { separated: boolean; chosen: boolean; usesPile: boolean; usesChosen: boolean }): void => {
    if (Array.isArray(value)) { value.forEach(item => scan(item, found)); return; }
    if (!value || typeof value !== "object") return;
    const node = value as Record<string, unknown>;
    if (node.effect === "separate-piles") found.separated = true;
    if (node.effect === "choose-pile") { found.chosen = true; found.usesPile = true; }
    if (node.ref === "pile") { found.usesPile = true; if (node.pile === "chosen") found.usesChosen = true; }
    Object.values(node).forEach(item => scan(item, found));
  };
  const found = { separated: false, chosen: false, usesPile: false, usesChosen: false };
  scan(sentence, found);
  if (!found.usesPile) return true;
  const earlier = { separated: false, chosen: false, usesPile: false, usesChosen: false };
  scan(previous, earlier);
  return (found.separated || earlier.separated) && (!found.usesChosen || found.chosen || earlier.chosen);
}
