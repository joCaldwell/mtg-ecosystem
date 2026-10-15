import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseOracleText } from '../src/index.ts';
// Replacement of where a discarded card goes (CR 701.9a, 614.6), checked
// against the frozen Scryfall corpus: Loxodon Smiter, Dodecapod, Nephalia
// Academy, Obstinate Baloth, Wilt-Leaf Liege.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed' as const, value });
const you = { player: 'you' };
const affected = { ref: 'replacement-event-object', role: 'affected' };
const opposingCause = { ref: 'filter', filter: { allOf: [{ anyOf: [{ cls: 'spell' }, { cls: 'ability' }] }], control: 'opponent' }, count: n(1) };
function abilities(text: string, typeLine = 'Creature — Elephant') {
  const parsed = parseOracleText(text, 'Test Card', { typeLine });
  assert.ok(parsed.ok, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.abilities)) as unknown[];
}
const standing = (replacement: object) => [{ kind: 'static', effect: { static: 'replacement', replacement } }];
const fails = (text: string, typeLine = 'Creature — Elephant') => assert.equal(parseOracleText(text, 'Test Card', { typeLine }).ok, false, text);

describe('replacing the destination of a discarded card', () => {
  it('keeps the cause and moves the card instead (Loxodon Smiter, Dodecapod)', () => {
    assert.deepEqual(abilities('If a spell or ability an opponent controls causes you to discard this card, put it onto the battlefield instead of putting it into your graveyard.'),
      standing({ event: { event: 'discard', who: you, what: { ref: 'this', noun: 'card' }, cause: opposingCause }, occurrence: 'each',
        replace: { replacement: 'effects', effects: [{ steps: [{ effect: 'move-zone', what: affected, to: { zone: 'battlefield' } }] }] } }));
    assert.deepEqual(abilities('If a spell or ability an opponent controls causes you to discard this card, put it onto the battlefield with two +1/+1 counters on it instead of putting it into your graveyard.', 'Artifact Creature — Golem'),
      standing({ event: { event: 'discard', who: you, what: { ref: 'this', noun: 'card' }, cause: opposingCause }, occurrence: 'each',
        replace: { replacement: 'effects', effects: [{ steps: [{ effect: 'move-zone', what: affected, to: { zone: 'battlefield' }, entry: { counters: [{ count: n(2), counter: { type: '+1/+1' } }] } }] }] } }));
  });
  it('makes the replacement optional and binds "that card" (Nephalia Academy)', () => {
    assert.deepEqual(abilities('If a spell or ability an opponent controls causes you to discard a card, you may reveal that card and put it on top of your library instead of putting it anywhere else.', 'Land'),
      standing({ event: { event: 'discard', who: you, what: { ref: 'filter', filter: { cls: 'card' }, count: n(1) }, cause: opposingCause }, occurrence: 'each',
        replace: { replacement: 'effects', effects: [{ steps: [{ effect: 'reveal', who: you, what: affected },
          { effect: 'move-zone', what: affected, to: { zone: 'library', owner: 'your', position: 'top' } }] }] }, optional: { who: you } }));
  });
  it('rejects another destination, another player\'s graveyard and a missing cause', () => {
    fails('If a spell or ability an opponent controls causes you to discard this card, put it onto the battlefield instead of putting it into your hand.');
    fails('If a spell or ability an opponent controls causes you to discard this card, put it onto the battlefield instead of putting it into their graveyard.');
    fails('If a spell or ability an opponent controls causes an opponent to discard a card, put it onto the battlefield instead of putting it into your graveyard.');
    fails('If you would discard this card, put it onto the battlefield instead of putting it into your graveyard.');
  });
});
