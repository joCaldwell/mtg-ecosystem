// Objects identified by a zone change they actually made: "creature put into
// your graveyard from the battlefield this turn", "card returned to your hand
// this way". CR 400.7: an object that changes zones becomes a new object, so
// the enclosing filter describes it as that move happened (608.2h–i).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { ObjectRelation, ZoneRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

type ZoneChange = Extract<ObjectRelation, { relation: "zone-change" }>;

/** The participle only names the move; the stated destination and origin are
 * the event's own. A window ("this way" and/or "this turn") is required:
 * without one the words describe a zone, not a history. */
export function parseZoneChangeRelation(c: Cursor, parseZoneRef: (c: Cursor) => ZoneRef | null): ZoneChange | null {
  return c.attempt((c): ZoneChange | null => {
    // "exiled from their hand this way": the move into exile with its printed
    // origin. Without an origin, "exiled this way" is the plain event result.
    const exiled = c.word("exiled") !== null;
    if (exiled && !c.isWord("from")) return null;
    if (!exiled && !c.words("put", "into") && !c.words("put", "onto") && !c.words("returned", "to") && !c.words("shuffled", "into")) return null;
    const to: ZoneRef | null = exiled ? { zone: "exile" } : parseZoneRef(c);
    // A positioned or unowned personal zone needs its own grammar.
    if (!to || to.position || (["graveyard", "hand", "library"].includes(to.zone) && to.owner === undefined)) return null;
    let from: ZoneChange["from"];
    if (c.word("from") !== null) {
      const origin = c.word("anywhere") !== null ? "anywhere" as const : parseZoneRef(c);
      if (!origin || (origin !== "anywhere" && origin.position)) return null;
      from = origin;
    }
    const thisWay = c.words("this", "way");
    const thisTurn = c.words("this", "turn");
    if (!thisWay && !thisTurn) return null;
    return { relation: "zone-change", to, ...(from ? { from } : {}), ...(thisWay ? { scope: "this-way" as const } : {}),
      ...(thisTurn ? { period: "this-turn" as const } : {}) };
  });
}
