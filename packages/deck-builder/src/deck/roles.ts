/** Panel choices and checks share the ingested commander classification:
 * front-face legendary creature, or explicit “can be your commander” text.
 * Companion here recognizes the printed keyword, not its deck condition.
 * These facts never gate membership writes (§5.1, §9). */
export function roleEligibility(card: { is_commander: number; oracle_text: string }) {
  return {
    can_be_commander: Boolean(card.is_commander),
    can_be_companion: /^Companion —/m.test(card.oracle_text),
  };
}
