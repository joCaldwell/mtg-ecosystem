// What a creature contributes when it is tapped to pay a crew or saddle cost
// or to station a permanent. CR 702.122a–b (crew), 702.171a/c (saddle),
// 702.184a/c (station), 609.4 ("as though" applies only to that payment).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { ObjectRef, StaticEffect } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseObjectRef } from "./refs.ts";

type Contribution = Extract<StaticEffect, { static: "crew-contribution" }>;

const ACTIONS: Record<string, { action: Contribution["actions"][number]; objects: string[] }> = {
  crews: { action: "crew", objects: ["vehicles"] },
  crew: { action: "crew", objects: ["vehicles"] },
  saddles: { action: "saddle", objects: ["mounts"] },
  saddle: { action: "saddle", objects: ["mounts"] },
  stations: { action: "station", objects: ["permanents"] },
  station: { action: "station", objects: ["permanents"] },
};

/** "[creature] saddles Mounts and crews Vehicles as though its power were 2
 * greater" / "… using its toughness rather than its power". The complete
 * line is this rule; every listed action keeps its printed object. */
export function parseCrewContribution(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    const generic = c.isWord("a", "an");
    let what: ObjectRef | null = parseObjectRef(c);
    if (!what) return null;
    if (generic && what.ref === "filter" && what.count.amount === "fixed" && what.count.value === 1) what = { ref: "each", filter: what.filter };
    if (what.ref === "target" || what.ref === "any-target" || what.ref === "filter") return null;
    const actions: Contribution["actions"] = [];
    for (;;) {
      const verb = c.peek();
      const entry = verb?.kind === "word" && !verb.possessive ? ACTIONS[verb.value] : undefined;
      if (!entry || actions.includes(entry.action)) return null;
      c.next();
      if (c.word(...entry.objects) === null) return null;
      actions.push(entry.action);
      if (c.word("and") === null) break;
    }
    let value: Contribution["value"];
    if (c.word("using") !== null) {
      const whose = c.word("its", "their");
      if (!whose || !c.words("toughness", "rather", "than", whose, "power")) return null;
      value = { value: "toughness" };
    } else {
      if (!c.words("as", "though") || c.word("its", "their") === null || !c.words("power", "were")) return null;
      const amount = c.number();
      if (amount === null || amount <= 0 || c.word("greater") === null) return null;
      value = { value: "power-plus", amount };
    }
    c.punct(".");
    if (!c.done()) return c.fail("complete crew/saddle/station contribution rule");
    return { static: "crew-contribution", what, actions, value };
  });
}
