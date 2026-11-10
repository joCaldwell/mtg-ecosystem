// "Whenever you cast an instant or sorcery spell, exile it instead of putting
// it into your graveyard as it resolves." The triggered ability creates a
// replacement effect (CR 614.1a) for the spell's move to the graveyard as
// the final part of its resolution (CR 608.2n). A spell that is countered or
// otherwise leaves the stack is not affected.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, ObjectRef, Sentence } from "../ast.ts";
import type { Token } from "../lexer.ts";
import { Cursor } from "./cursor.ts";
import type { EffectContext } from "./effects.ts";
import { parseZoneRef } from "./refs.ts";

type BodyParser = (c: Cursor, ctx: EffectContext) => Sentence[] | null;

const isWord = (token: Token | undefined, value: string) => token?.kind === "word" && !token.possessive && token.value === value;

/** The printed spell: "it", "that card" or "that spell", without further
 * description. */
function isSpellAnaphor(ref: ObjectRef): boolean {
  if (ref.ref === "it") return true;
  return ref.ref === "that" && !ref.plural && Object.keys(ref).length === 2
    && Object.keys(ref.filter).length === 1 && (ref.filter.cls === "card" || ref.filter.cls === "spell");
}

export function parseResolutionReplacement(c: Cursor, ctx: EffectContext, parseBody: BodyParser): Sentence | null {
  return c.attempt(c => {
    let at = c.pos;
    while (at < c.tokens.length && !isWord(c.tokens[at], "instead") && !(c.tokens[at].kind === "punct" && (c.tokens[at] as { value: string }).value === ".")) at++;
    if (!isWord(c.tokens[at], "instead")) return null;
    const body = new Cursor(c.tokens.slice(c.pos, at));
    const sentences = parseBody(body, { modalOptions: ctx.modalOptions });
    const step = sentences?.length === 1 && body.done() && sentences[0].steps.length === 1 && !sentences[0].condition && !sentences[0].bindings
      ? sentences[0].steps[0] : undefined;
    if (step?.effect !== "exile" || !isSpellAnaphor(step.what) || step.from || step.who || step.faceDown || step.pile
      || step.optional || step.forEach || step.decidedBy) return null;
    c.pos = at + 1;
    if (!c.words("of", "putting", "it", "into")) return null;
    const to = parseZoneRef(c);
    if (!to || to.zone !== "graveyard" || !(to.owner === "your" || to.owner === "any")) return null;
    if (!c.words("as", "it", "resolves")) return null;
    if (!c.punct(".") && !c.done()) return null;
    const exile: Effect = { ...step, what: { ref: "replacement-event-object", role: "affected" } };
    // "If you do, return it to your hand at the beginning of the next end
    // step." (Feather) / "Then if the exiled card doesn't have suspend, it
    // gains suspend." (Gandalf of the Secret Fire): the card is exiled only
    // when the replacement applies, after this ability has resolved, so the
    // rest of the ability continues the replacement. Its pronouns stay symbolic.
    let after: Sentence[] = [];
    if (!c.done()) {
      if (!(c.isWord("then") || (c.isWord("if") && isWord(c.peek(1), "you") && isWord(c.peek(2), "do")))) return null;
      const rest = new Cursor(c.tokens.slice(c.pos));
      const parsed = parseBody(rest, { modalOptions: ctx.modalOptions });
      if (!parsed || !rest.done()) return null;
      after = parsed;
      c.pos = c.tokens.length;
    }
    return { steps: [{ effect: "create-replacement", replacement: {
      event: { event: "move-zone", what: step.what, to, resolving: true }, occurrence: "each",
      replace: { replacement: "effects", effects: [{ steps: [exile] }, ...after] } } }] };
  });
}
