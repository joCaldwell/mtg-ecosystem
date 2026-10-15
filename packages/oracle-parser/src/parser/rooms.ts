// Unlocking a door of a Room (CR 709.5c, 709.5f, 709.5j): the instruction and
// the count of unlocked doors.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, Effect, ObjectFilter } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseFilter, parseObjectRef } from "./refs.ts";

/** "unlock a locked door of up to one target Room you control": the player
 * chooses a locked half of the named permanent, which gets that half's
 * unlocked designation (CR 709.5f). A permanent without a locked door is
 * unaffected. */
export function parseUnlockDoor(c: Cursor): Effect[] | null {
  return c.attempt(c => {
    // "Lock or unlock a door of target Room": either action is chosen as it
    // resolves (CR 709.5f–g); the Room is named the same way.
    const either = c.words("lock", "or", "unlock") && c.words("a", "door", "of");
    if (!either && (c.word("unlock") === null || !c.words("a", "locked", "door", "of"))) return null;
    const what = parseObjectRef(c);
    if (!what || !(what.ref === "target" || what.ref === "filter")) return null;
    if (!("filter" in what) || !what.filter.subtypes?.includes("Room")) return null;
    return [{ effect: either ? "lock-or-unlock-door" : "unlock-door", what }];
  });
}

/** "unlocked doors among Rooms you control", after "the number of" (CR 709.5c). */
export function parseUnlockedDoorCount(c: Cursor): Amount | null {
  return c.attempt((c): Amount | null => {
    if (!c.words("unlocked", "doors", "among")) return null;
    const rooms: ObjectFilter | null = parseFilter(c);
    if (!rooms?.subtypes?.includes("Room")) return null;
    return { amount: "unlocked-door-count", among: rooms };
  });
}
