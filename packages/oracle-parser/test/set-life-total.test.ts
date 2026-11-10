import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

// CR 119.5 (setting a life total).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function one(text: string, name?: string) {
  const result = parseOracleText(text, name);
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}
const fails = (text: string, name?: string) => assert.equal(parseOracleText(text, name).ok, false, text);
const n = (value: number) => ({ amount: 'fixed', value });
const spell = (...steps: object[]) => ({ kind: 'spell', effects: [{ steps }] });

describe('setting life totals', () => {
  it('sets one player to a fixed total (Blessed Wind, Sorin Markov)', () => {
    assert.deepEqual(one("Target player's life total becomes 20."),
      spell({ effect: 'set-life-total', who: { player: 'target-player' }, amount: n(20) }));
    assert.deepEqual(one("-3: Target opponent's life total becomes 10."), { kind: 'loyalty', cost: { sign: -1, amount: 3 },
      effects: [{ steps: [{ effect: 'set-life-total', who: { player: 'target-opponent' }, amount: n(10) }] }] });
  });
  it('accepts "becomes equal to" a starting life total (Oketra\'s Last Mercy)', () => {
    assert.deepEqual(one('Your life total becomes equal to your starting life total.'),
      spell({ effect: 'set-life-total', who: { player: 'you' }, amount: { amount: 'starting-life-total', of: { player: 'you' } } }));
  });
  it('evaluates a per-player amount and a shared extremum for each player (Biorhythm, Repay in Kind)', () => {
    assert.deepEqual(one("Each player's life total becomes the number of creatures they control."),
      spell({ effect: 'set-life-total', who: { player: 'each-player' },
        amount: { amount: 'count', of: { types: ['creature'], controlledBy: { player: 'they' } } } }));
    assert.deepEqual(one("Each player's life total becomes the lowest life total among all players."),
      spell({ effect: 'set-life-total', who: { player: 'each-player' },
        amount: { amount: 'life-total-extremum', extremum: 'minimum', among: { player: 'each-player' } } }));
    assert.deepEqual(one("Each player's life total becomes the highest life total among all players."),
      spell({ effect: 'set-life-total', who: { player: 'each-player' },
        amount: { amount: 'life-total-extremum', extremum: 'maximum', among: { player: 'each-player' } } }));
  });
  it('keeps the causative "may have … become" optional (Eternity Vessel)', () => {
    assert.deepEqual(one('Landfall — Whenever a land you control enters, you may have your life total become the number of charge counters on this artifact.'), {
      kind: 'triggered', abilityWord: 'Landfall',
      trigger: { trigger: 'enters', what: { ref: 'filter', filter: { types: ['land'], control: 'you' }, count: n(1) } },
      effects: [{ steps: [{ effect: 'set-life-total', who: { player: 'you' }, optional: true,
        amount: { amount: 'counter-count', counter: { type: 'charge' }, on: { ref: 'this', noun: 'artifact' } } }] }],
    });
  });
  it('rejects durations, the wrong verb form, and an unbound "it becomes"', () => {
    fails('Your life total becomes 10 until end of turn.');
    fails('Your life total become 10.');
    fails('You may have your life total becomes 10.');
    fails('When this creature enters, if your life total is less than your starting life total, it becomes equal to your starting life total.');
    // The highest life total among a set of players is now the `life-total-extremum` amount (amounts-player-counts.test.ts).
  });
});
