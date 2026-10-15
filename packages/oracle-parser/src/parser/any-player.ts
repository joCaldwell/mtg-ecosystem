// "Any opponent may …" / "any other player may …": one optional instruction
// that at most one of the quantified players performs (see PlayerRef
// "any-one-of"), and the printed follow-ups "If a player does" / "If no
// player does" / "If no one does", which ask whether one of them did
// (CR 118.12, 608.2c).
import type { Condition, PlayerRef, Sentence } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

/** At "any": "any opponent", "any player", "any other player", "any other
 * opponent", directly followed by "may". The other-contrast stays unresolved
 * like PlayerFilter.other. */
export function parseAnyOnePlayerMay(c: Cursor): PlayerRef | null {
  return c.attempt((c): PlayerRef | null => {
    if (c.word("any") === null) return null;
    const other = c.word("other") !== null;
    const noun = c.word("opponent", "player");
    if (!noun || !c.isWord("may")) return null;
    const base: PlayerRef = { player: noun === "player" ? "each-player" : "each-opponent" };
    return { player: "any-one-of", among: other ? { player: "filtered", base, filter: { other: true } } : base };
  });
}

/** "If a player does," / "If no player does," / "If no one does,": whether
 * any player took the directly preceding optional action. Only the bare
 * auxiliary before its comma; a restated action is a different question. */
export function parseAnyPlayerOutcome(c: Cursor): Condition | null {
  return c.attempt((c): Condition | null => {
    const negated = c.words("no", "player") || c.words("no", "one");
    if (!negated && !c.words("a", "player")) return null;
    if (c.word("does") === null || !c.isPunct(",")) return null;
    const performed: Condition = { condition: "preceding-action", who: { player: "a-player" }, outcome: "performed" };
    return negated ? { condition: "not", of: performed } : performed;
  });
}

const offered = (sentence: Sentence | undefined): boolean => {
  const step = sentence?.steps[0];
  return sentence?.steps.length === 1 && !sentence.otherwise && step?.effect !== "damage" && step?.decidedBy?.player === "any-one-of";
};
const asked = (sentence: Sentence | undefined) => {
  const condition = sentence?.conditionKind === "if" ? sentence.condition : undefined;
  const test = condition?.condition === "not" ? condition.of : condition;
  return test?.condition === "preceding-action" && test.outcome === "performed" ? test.who.player : null;
};

/** An "any opponent may" instruction is accepted only as the sole step of
 * its sentence and directly followed by the sentence that asks whether a
 * player did it ("If a player does" / "If no one does" / "If they do"):
 * that follow-up is what states that one player's action settles it. A
 * damage offer is the has-damage-dealt condition (see unless-damage.ts).
 * Conversely "If a player does" is accepted only there: after "each player
 * may …" it means each player who did (Orzhov Advokist), which an
 * existential condition would misstate. */
export function anyOneOfAnswered(sentences: Sentence[]): boolean {
  return sentences.every((sentence, index) => {
    if (asked(sentence) === "a-player" && !offered(sentences[index - 1])) return false;
    if (!JSON.stringify(sentence).includes('"any-one-of"')) return true;
    const who = asked(sentences[index + 1]);
    return offered(sentence) && (who === "a-player" || who === "they");
  });
}
