import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseOracleText } from '../src/index.ts';
// Replacement bodies that refer to nothing of the watched event, the
// "would lose the game" event (CR 104.3, 614.1a) and the empty-library draw
// condition (614.11), checked against the frozen Scryfall corpus: Words of
// War, Words of Waste, Laboratory Maniac, Ormos, Archive Keeper, Exquisite
// Archangel, Stunning Reversal, Lich's Mirror.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed' as const, value });
const you = { player: 'you' };
const drawOne = { event: 'draw', who: you, quantity: { op: 'eq', value: n(1) } };
const emptyLibrary = { condition: 'count', of: { cls: 'card', zone: { zone: 'library', owner: 'your' } }, comparison: { op: 'eq', value: n(0) } };
const effects = (...steps: object[]) => ({ replacement: 'effects', effects: [{ steps }] });
function abilities(text: string, typeLine = 'Instant') {
  const parsed = parseOracleText(text, 'Test Card', { typeLine });
  assert.ok(parsed.ok, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.abilities)) as unknown[];
}
const fails = (text: string, typeLine = 'Instant') => assert.equal(parseOracleText(text, 'Test Card', { typeLine }).ok, false, text);

describe('replacement bodies independent of the watched event', () => {
  it('accepts any complete instruction without event references (Words of War, Words of Waste)', () => {
    const next = (replace: object) => [{ kind: 'spell', effects: [{ steps: [{ effect: 'create-replacement',
      replacement: { event: drawOne, occurrence: 'next', replace }, duration: { duration: 'this-turn' } }] }] }];
    assert.deepEqual(abilities('The next time you would draw a card this turn, Test Card deals 2 damage to any target instead.'),
      next(effects({ effect: 'damage', source: { ref: 'self' }, amount: n(2), to: { to: 'any-target' } })));
    assert.deepEqual(abilities('The next time you would draw a card this turn, each opponent discards a card instead.'),
      next(effects({ effect: 'discard', who: { player: 'each-opponent' }, what: { ref: 'filter', filter: { cls: 'card' }, count: n(1) } })));
  });
  it('keeps the empty-library condition on the watched draw (Laboratory Maniac, Ormos)', () => {
    assert.deepEqual(abilities('If you would draw a card while your library has no cards in it, you win the game instead.', 'Creature — Human Wizard'), [{ kind: 'static', effect: {
      static: 'replacement', replacement: { event: drawOne, occurrence: 'each', replace: effects({ effect: 'win-game', who: you }), condition: emptyLibrary } } }]);
    assert.deepEqual(abilities('If you would draw a card while your library has no cards in it, instead put five +1/+1 counters on Test Card.', 'Creature — Sphinx'), [{ kind: 'static', effect: {
      static: 'replacement', replacement: { event: drawOne, occurrence: 'each',
        replace: effects({ effect: 'put-counters', on: { ref: 'self' }, counter: { type: '+1/+1' }, count: n(5) }), condition: emptyLibrary } } }]);
  });
  it('watches losing the game as its own event (Exquisite Archangel, Stunning Reversal)', () => {
    const loseGame = { event: 'lose-game', who: you };
    assert.deepEqual(abilities('If you would lose the game, instead exile this creature and your life total becomes equal to your starting life total.', 'Creature — Angel'), [{ kind: 'static', effect: {
      static: 'replacement', replacement: { event: loseGame, occurrence: 'each', replace: effects(
        { effect: 'exile', what: { ref: 'this', noun: 'creature' } },
        { effect: 'set-life-total', who: you, amount: { amount: 'starting-life-total', of: you } }) } } }]);
    assert.deepEqual(abilities('The next time you would lose the game this turn, instead draw seven cards and your life total becomes 1.'), [{ kind: 'spell', effects: [{ steps: [{
      effect: 'create-replacement', replacement: { event: loseGame, occurrence: 'next', replace: effects(
        { effect: 'draw', who: you, amount: n(7) }, { effect: 'set-life-total', who: you, amount: n(1) }) }, duration: { duration: 'this-turn' } }] }] }]);
    // Losing life is a different event.
    const [lifeLoss] = abilities('If you would lose life, you lose twice that much life instead.', 'Enchantment') as { effect: { replacement: { event: { event: string } } } }[];
    assert.equal(lifeLoss.effect.replacement.event.event, 'lose-life');
  });
  it('still rejects bodies whose references to the event are not bound', () => {
    // Pronouns, "that many", "other" and relative zone owners need the event.
    fails('The next time you would draw a card this turn, each player returns a permanent they control to its owner\'s hand instead.');
    fails('If you would lose the game, instead exile it.', 'Enchantment');
    fails('If you would lose the game, instead each other player draws a card.', 'Enchantment');
    fails('If you would lose the game, instead destroy that many creatures.', 'Enchantment');
    // Tokens made in place of tokens keep their dedicated production.
    fails('If you would create a Food token, instead create a Food token and a Treasure token.', 'Enchantment');
    // An unsupported library wording is not silently dropped from the header.
    fails('If you would draw a card while your library has no cards, you win the game instead.', 'Enchantment');
  });
});
