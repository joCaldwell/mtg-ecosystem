// "The next [quality] spell you cast this turn has convoke / can't be
// countered." CR 611.2f: the effect waits for that player's next matching
// spell, begins as it is put onto the stack, and applies to it alone.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, GainedAbility, ObjectFilter, PlayerRef } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseFilter } from "./refs.ts";

type ModifySpells = Extract<Effect, { effect: "modify-spells-cast" }>;

/** One or more "the next … spell" heads share the trailing "[player] cast(s)
 * this turn"; several heads are separate pending effects, marked by "each".
 * Only printed keyword grants and "can't be countered" are represented. */
export function parseNextSpellModification(c: Cursor, parseAbilities: (c: Cursor) => GainedAbility[] | null): ModifySpells[] | null {
  return c.attempt((c): ModifySpells[] | null => {
    const heads: ObjectFilter[] = [];
    let caster: PlayerRef | undefined;
    for (;;) {
      if (!c.words("the", "next")) return null;
      const filter = parseFilter(c);
      if (!filter || filter.cls !== "spell") return null;
      const { cast, ...spells } = filter;
      heads.push(spells);
      if (cast) {
        if (cast.from) return c.fail("next spell cast without an origin zone");
        caster = cast.by;
        break;
      }
      if (c.word("and") === null) return null;
    }
    if (!caster || heads.some(head => head.cast) || !c.words("this", "turn")) return null;
    const each = c.word("each") !== null;
    if (each !== heads.length > 1) return c.fail("'each' exactly when several next spells are named");
    let modifier: ModifySpells["modifiers"][number];
    if (c.words("can't", "be", "countered")) modifier = { modifier: "cant", action: "be-countered" };
    else {
      if (c.word(each ? "have" : "has") === null) return null;
      const abilities = parseAbilities(c);
      if (!abilities || abilities.some(ability => ability.gained !== "keyword")) return c.fail("keywords the next spell has");
      modifier = { modifier: "abilities", abilities };
    }
    return heads.map((filter): ModifySpells => ({ effect: "modify-spells-cast",
      scope: { action: "cast", spells: { ref: "each", filter }, caster, occurrence: { next: true } },
      modifiers: [structuredClone(modifier)], duration: { duration: "this-turn" } }));
  });
}
