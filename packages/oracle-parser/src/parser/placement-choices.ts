// A library placement whose position is chosen from two printed positions:
// "puts it on their choice of the top or bottom of their library", "into its
// owner's library second from the top or on the bottom". Exactly one placement
// happens, chosen by the stated player as the instruction is carried out
// (CR 608.2d); a target shared by both positions is one declaration. CR 401.7
// places a card N from the top of a too-short library on the bottom.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, ObjectRef, PlayerRef, ZoneRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseLibraryDepth } from "./library-placement.ts";
import { parseObjectRef, parseZoneRef } from "./refs.ts";

type ZoneMove = Extract<Effect, { effect: "move-zone" }>;

/** A whole personal library ("their library", "its owner's library"). */
function parseLibrary(c: Cursor): ZoneRef | null {
  return c.attempt(c => {
    const library = parseZoneRef(c);
    return library?.zone === "library" && !library.position && library.owner ? library : null;
  });
}

/** Called right after the put verb of `who`. The chooser is the putting
 * player: "their choice" names that player when it is not you, "your
 * choice" only when it is you; an unstated choice is the putting player's. */
export function parsePlacementChoice(c: Cursor, who: PlayerRef): Effect | null {
  return c.attempt((c): Effect | null => {
    const what: ObjectRef | null = parseObjectRef(c);
    if (!what) return null;
    let positions: ZoneRef[];
    if (c.word("on") !== null) {
      const whose = c.attempt(c => {
        const word = c.word("your", "their");
        return word && c.words("choice", "of") ? word : null;
      });
      if (whose && (whose === "your") !== (who.player === "you")) return c.fail("the putting player's choice");
      if (!c.words("the", "top", "or", "bottom", "of")) return null;
      const library = parseLibrary(c);
      if (!library) return null;
      positions = [{ ...library, position: "top" }, { ...library, position: "bottom" }];
    } else if (c.word("into") !== null) {
      const library = parseLibrary(c);
      const deep = library && parseLibraryDepth(c, library);
      if (!library || !deep || !c.words("or", "on", "the", "bottom")) return null;
      positions = [deep, { ...library, position: "bottom" }];
    } else return null;
    const actor = who.player === "you" ? {} : { who };
    const options = positions.map((to): ZoneMove => ({ effect: "move-zone", what, to, ...actor }));
    return { effect: "choice", who, options };
  });
}
