// A separate sentence "X is <amount>." defines the X of its whole ability,
// including an {X} in the activation cost: the controller doesn't choose it
// (CR 107.3a), and that is its value while the ability is on the stack
// (107.3c). https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Sentence, VariableDefinition } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseAmount } from "./refs.ts";

export function parseVariableDefinitions(c: Cursor): VariableDefinition[] | null {
  return c.attempt(c => {
    if (!c.words("x", "is")) return null;
    const value = parseAmount(c);
    // "X is 0" style constants and self-references define nothing usable.
    if (!value || value.amount === "x" || value.amount === "fixed" || !c.punct(".")) return null;
    return [{ variable: "x", value }];
  });
}

/** An ability's text defines X once; a "where X is" inside one of its
 * instructions would be a second, competing definition. */
export function hasInstructionBinding(sentences: Sentence[]): boolean {
  return sentences.some(sentence => !!sentence.bindings?.length) || JSON.stringify(sentences).includes('"bindings"');
}
