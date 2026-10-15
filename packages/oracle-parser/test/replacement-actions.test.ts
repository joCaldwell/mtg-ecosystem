import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseOracleText } from '../src/index.ts';
// Replacement of a player's keyword action or randomizer: mill (CR 701.17a),
// scry (701.22a), proliferate (701.34a), energy (107.14, 122.1), dice (706.1,
// 706.6, 706.7) and coins (705.1). Checked against the frozen Scryfall corpus:
// Bruvac the Grandiloquent, The Water Crystal, Kenessos, Eligeth, Tekuthal,
// Aether Refinery, Izzet Generatorium, Wyll, Ichor Elixir, Krark's Thumb,
// Bamboozling Beeble.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed' as const, value });
const you = { player: 'you' };
const oneOrMore = { op: 'ge', value: n(1) };
function abilities(text: string, typeLine = 'Artifact') {
  const parsed = parseOracleText(text, 'Test Card', { typeLine });
  assert.ok(parsed.ok, JSON.stringify(parsed.lines));
  return JSON.parse(JSON.stringify(parsed.abilities)) as unknown[];
}
const standing = (event: object, replace: object) =>
  [{ kind: 'static', effect: { static: 'replacement', replacement: { event, occurrence: 'each', replace } } }];
const fails = (text: string, typeLine = 'Artifact') => assert.equal(parseOracleText(text, 'Test Card', { typeLine }).ok, false, text);

describe('mill and scry replacements', () => {
  it('changes the number of cards milled (Bruvac, The Water Crystal)', () => {
    const event = { event: 'mill', who: { player: 'an-opponent' }, quantity: oneOrMore };
    assert.deepEqual(abilities('If an opponent would mill one or more cards, they mill twice that many cards instead.'),
      standing(event, { replacement: 'modify-quantity', change: { operation: 'multiply', factor: 2 } }));
    assert.deepEqual(abilities('If an opponent would mill one or more cards, they mill that many cards plus four instead.'),
      standing(event, { replacement: 'modify-quantity', change: { operation: 'add', amount: n(4) } }));
  });
  it('scries more cards, or draws instead (Kenessos, Eligeth)', () => {
    const event = { event: 'scry', who: you };
    assert.deepEqual(abilities('If you would scry a number of cards, scry that many cards plus one instead.'),
      standing(event, { replacement: 'modify-quantity', change: { operation: 'add', amount: n(1) } }));
    assert.deepEqual(abilities('If you would scry a number of cards, draw that many cards instead.'),
      standing(event, { replacement: 'effects', effects: [{ steps: [{ effect: 'draw', who: you, amount: { amount: 'replacement-event-quantity' } }] }] }));
  });
  it('proliferates twice (Tekuthal)', () => {
    assert.deepEqual(abilities('If you would proliferate, proliferate twice instead.'),
      standing({ event: 'proliferate', who: you }, { replacement: 'effects',
        effects: [{ steps: [{ effect: 'keyword-action', action: 'proliferate', who: you, times: n(2) }] }] }));
  });
  it('rejects an unstated count, a different action, or a mismatched actor', () => {
    fails('If you would scry, scry that many cards plus one instead.');
    fails('If you would scry a number of cards, mill that many cards plus one instead.');
    fails('If an opponent would mill one or more cards, they mill twice that many cards plus one instead.');
    fails('If an opponent would mill one or more cards, they scry twice that many cards instead.');
  });
});

describe('energy replacements', () => {
  it('gives more energy counters (Aether Refinery, Izzet Generatorium)', () => {
    const event = { event: 'put-counters', on: [you], counter: { type: 'energy' }, cause: 'any', quantity: oneOrMore };
    assert.deepEqual(abilities('If you would get one or more {E}, you get twice that many {E} instead.'),
      standing(event, { replacement: 'modify-quantity', change: { operation: 'multiply', factor: 2 } }));
    assert.deepEqual(abilities('If you would get one or more {E}, you get that many plus one {E} instead.'),
      standing(event, { replacement: 'modify-quantity', change: { operation: 'add', amount: n(1) } }));
  });
  it('rejects a different counter in the replacement', () => {
    fails('If you would get one or more {E}, you get twice that many poison counters instead.');
  });
});

describe('dice and coin replacements', () => {
  it('rolls an extra die and ignores the lowest roll (Wyll, Barbarian Class)', () => {
    assert.deepEqual(abilities('If you would roll one or more dice, instead roll that many dice plus one and ignore the lowest roll.', 'Creature — Elf'),
      standing({ event: 'roll-dice', who: you, quantity: oneOrMore },
        { replacement: 'roll-and-ignore', change: { operation: 'add', amount: n(1) }, ignore: { ignore: 'lowest', count: 1 } }));
  });
  it('keeps the planar die kind and the chooser (Ichor Elixir)', () => {
    assert.deepEqual(abilities('If you would roll one or more planar dice, instead roll that many planar dice plus one and ignore one.'),
      standing({ event: 'roll-dice', who: you, quantity: oneOrMore, die: { die: 'planar' } },
        { replacement: 'roll-and-ignore', change: { operation: 'add', amount: n(1) }, ignore: { ignore: 'chosen', who: you, count: 1 } }));
  });
  it('flips two coins for each coin and ignores one (Krark\'s Thumb)', () => {
    assert.deepEqual(abilities('If you would flip a coin, instead flip two coins and ignore one.'),
      standing({ event: 'flip-coin', who: you },
        { replacement: 'roll-and-ignore', change: { operation: 'set', amount: n(2) }, ignore: { ignore: 'chosen', who: you, count: 1 } }));
  });
  it('lets a different player choose the ignored roll of a targeted player (Bamboozling Beeble)', () => {
    assert.deepEqual(abilities('{1}, {T}: The next time target player would roll one or more dice this turn, instead they roll that many dice plus one and you choose one of those rolls to ignore.', 'Artifact Creature — Beeble'),
      [{ kind: 'activated', costs: [{ cost: 'mana', symbols: ['1'] }, { cost: 'tap-self' }], effects: [{ steps: [{ effect: 'create-replacement',
        replacement: { event: { event: 'roll-dice', who: { player: 'target-player' }, quantity: oneOrMore }, occurrence: 'next',
          replace: { replacement: 'roll-and-ignore', change: { operation: 'add', amount: n(1) }, ignore: { ignore: 'chosen', who: you, count: 1 } } },
        duration: { duration: 'this-turn' } }] }] }]);
  });
  it('rejects a lowest planar roll, a missing ignore, and a mismatched die kind', () => {
    fails('If you would roll one or more planar dice, instead roll that many planar dice plus one and ignore the lowest roll.');
    fails('If you would roll one or more dice, instead roll that many dice plus one.');
    fails('If you would roll one or more planar dice, instead roll that many dice plus one and ignore one.');
    fails('If you would flip a coin, instead flip one coin and ignore one.');
  });
});
