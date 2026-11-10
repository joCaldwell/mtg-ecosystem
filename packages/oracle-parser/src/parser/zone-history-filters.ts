// Relative clauses about an object's own earlier zone change: "that entered
// this turn", "that died under your control this turn", "that was put there
// from the battlefield this turn". CR 608.2i (looking back), 700.4 (dies),
// 400.7 (an object that changes zones is a new object).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { ObjectFilter, PlayerRef, ZoneRef } from "../ast.ts";
import { Cursor } from "./cursor.ts";

interface ReferenceGrammar {
  parseZoneRef(c: Cursor): ZoneRef | null;
  parsePossessivePlayer(c: Cursor): PlayerRef | null;
}
type History = NonNullable<ObjectFilter["zoneChangeHistory"]>;

const personal = (zone: ZoneRef): boolean => ["hand", "library", "graveyard"].includes(zone.zone);

/** "under your control": who controlled the object as it moved. */
function parseControllerAtMove(c: Cursor, g: ReferenceGrammar): PlayerRef | null | undefined {
  if (c.word("under") === null) return undefined;
  const who = g.parsePossessivePlayer(c);
  return who && c.word("control") !== null ? who : null;
}

/** `head` is the filter parsed so far; "put there" needs its stated zone. */
export function parseZoneChangeHistoryQualifier(c: Cursor, head: ObjectFilter, g: ReferenceGrammar): ObjectFilter | null {
  return c.attempt((c): ObjectFilter | null => {
    if (c.word("that") === null) return null;
    let history: Omit<History, "during">;
    if (c.word("died") !== null) {
      const controllerAtMove = parseControllerAtMove(c, g);
      if (controllerAtMove === null) return null;
      // Within one resolution the deaths are results of its own instruction.
      if (!controllerAtMove && c.words("this", "way")) return head.relation ? null : { relation: { relation: "event-result", event: "die", scope: "this-way" } };
      history = { event: "dies", from: { zone: "battlefield" }, to: { zone: "graveyard" }, ...(controllerAtMove ? { controllerAtMove } : {}) };
    } else if (c.word("entered") !== null) {
      c.words("the", "battlefield");
      const controllerAtMove = parseControllerAtMove(c, g);
      if (controllerAtMove === null) return null;
      history = { event: "enters", to: { zone: "battlefield" }, ...(controllerAtMove ? { controllerAtMove } : {}) };
    } else {
      if (c.word("was", "were") === null || c.word("put") === null) return null;
      let to: ZoneRef | null;
      if (c.word("there") !== null) {
        // "There" is the zone this same noun phrase places the card in.
        to = head.zone && !head.zone.position ? head.zone : null;
      } else {
        to = c.word("into", "onto") !== null ? g.parseZoneRef(c) : null;
        if (to && (to.position || (personal(to) && to.owner === undefined))) to = null;
      }
      if (!to || to.zone === "battlefield") return null;
      let from: History["from"];
      if (c.word("from") !== null) {
        const origin = c.word("anywhere") !== null ? "anywhere" as const : g.parseZoneRef(c);
        if (!origin || (origin !== "anywhere" && (origin.position || (personal(origin) && origin.owner === undefined)))) return null;
        // A second origin ("from your hand or library") is not one zone.
        if (c.isWord("or", "and")) return null;
        from = origin;
      }
      history = { event: "put-into-zone", to, ...(from ? { from } : {}) };
    }
    return c.words("this", "turn") ? { zoneChangeHistory: { ...history, during: "this-turn" } } : null;
  });
}
