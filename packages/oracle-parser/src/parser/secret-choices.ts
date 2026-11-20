import type { Effect } from "../ast.ts";
import { Cursor } from "./cursor.ts";

type SecretChoiceDamage = Extract<Effect, { effect: "secret-choice-damage" }>;

/** CR 101.1, 101.4, 608.2c–f, 701.38c. Capture the complete choice and
 * reveal with both unanimous outcomes and one choice-filtered fallback.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
 * https://mtg-jp.com/products/card-gallery/0000275/649972/
 */
export function parseSecretChoiceDamage(c: Cursor): SecretChoiceDamage | null {
  return c.attempt(c => {
    if (!c.words("each", "opponent", "secretly", "chooses")) return null;
    const first = c.anyWord()?.value;
    if (!first || first === "or" || !c.word("or")) return null;
    const second = c.anyWord()?.value;
    if (!second || second === "or" || second === first || !c.punct(",") ||
        !c.words("then", "the", "choices", "are", "revealed") || !c.punct(".")) return null;

    const unanimous = (): SecretChoiceDamage["outcomes"][0] | null => {
      if (!c.words("if", "each", "opponent", "chose")) return null;
      const choice = c.word(first, second);
      const damage = choice && c.punct(",") && parseDamage(c);
      if (!damage || !c.words("to", "each", "of", "them") || !c.punct(".")) return null;
      return { when: "unanimous", choice, recipients: "all-choosers", ...damage };
    };
    const initial = unanimous();
    const following = initial && unanimous();
    if (!initial || !following || initial.choice === following.choice || !c.word("otherwise") || !c.punct(",")) return null;
    const damage = parseDamage(c);
    if (!damage || !c.words("to", "each", "opponent", "who", "chose")) return null;
    const chose = c.word(first, second);
    if (!chose || !c.punct(".") || !c.done()) return null;
    return { effect: "secret-choice-damage", who: { player: "each-opponent" }, choices: [first, second],
      outcomes: [initial, following, { when: "otherwise", recipients: { chose }, ...damage }] };
  });
}

function parseDamage(c: Cursor): Pick<SecretChoiceDamage["outcomes"][0], "source" | "amount"> | null {
  if (!c.selfref() || !c.word("deals")) return null;
  const value = c.number();
  return value !== null && Number.isSafeInteger(value) && value >= 0 && c.word("damage")
    ? { source: { ref: "self" }, amount: { amount: "fixed", value } } : null;
}
