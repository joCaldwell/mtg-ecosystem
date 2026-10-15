// Requirements on another player's target choices: "While an opponent is
// choosing targets as part of casting a spell they control or activating an
// ability they control, that player must choose at least one Flagbearer on
// the battlefield if able." Targets are chosen as a spell is cast (CR 601.2c)
// or an ability is activated (602.2b); a requirement "if able" only applies
// when a legal choice including such an object exists.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { PlayerRef, StaticEffect } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseCount, parseFilter, parsePlayerRef } from "./refs.ts";

export function parseTargetChoiceRequirement(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    if (c.word("while") === null) return null;
    // "an opponent is choosing ..., that player must": the requirement is on
    // each opponent while they choose. Otherwise the subject follows.
    const stated = c.attempt(c => c.words("an", "opponent", "is") ? true : null) === true;
    if (!c.words("choosing", "targets", "as", "part", "of")) return null;
    // "they control" restates the chooser; it is required in the stated form.
    const theirs = () => stated ? c.words("they", "control") : !c.isWord("they");
    if (!c.words("casting", "a", "spell") || !theirs()) return null;
    if (!c.words("or", "activating", "an", "ability") || !theirs()) return null;
    if (!c.punct(",")) return null;
    let who: PlayerRef | null;
    if (stated) who = c.words("that", "player") ? { player: "each-opponent" } : null;
    else {
      who = parsePlayerRef(c);
      if (who?.player !== "each-opponent") return null;
    }
    if (!who || !c.words("must", "choose", "at", "least")) return null;
    const minimum = parseCount(c);
    if (!minimum) return null;
    const mustChoose = parseFilter(c);
    if (!mustChoose || !c.words("if", "able")) return null;
    c.punct(".");
    if (!c.done()) return null;
    return { static: "target-choice-requirement", who, when: ["casting-spell", "activating-ability"], mustChoose, minimum, ifAble: true };
  });
}
