import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

// CR 702.95b-e (soulbond pairs).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const source = { ref: 'this', noun: 'creature' };
const paired = (what: object) => ({ condition: 'paired', what, with: { types: ['creature'], otherThan: { ref: 'self' } } });
const pair = (of: object) => ({ ref: 'soulbond-pair', of });
function one(text: string, name?: string) {
  const result = parseOracleText(text, name);
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}
const fails = (text: string, name?: string) => assert.equal(parseOracleText(text, name).ok, false, text);

describe('soulbond pair statics', () => {
  it('grants a keyword to both members of the pair (Wingcrafter)', () => {
    assert.deepEqual(one('As long as this creature is paired with another creature, both creatures have flying.'), {
      kind: 'static', effect: { static: 'conditional', condition: paired(source), effect: { static: 'modify', what: pair(source),
        modifiers: [{ modifier: 'abilities', abilities: [{ gained: 'keyword', keyword: { keyword: 'flying' } }] }] } },
    });
  });
  it('gives "each of those creatures" the same pair subject (Wolfir Silverheart)', () => {
    const n = (value: number) => ({ sign: 1, amount: { amount: 'fixed', value } });
    assert.deepEqual(one('As long as this creature is paired with another creature, each of those creatures gets +4/+4.'), {
      kind: 'static', effect: { static: 'conditional', condition: paired(source), effect: { static: 'modify', what: pair(source),
        modifiers: [{ modifier: 'pt', power: n(4), toughness: n(4) }] } },
    });
  });
  it('keeps a quoted ability granted to the pair in its own source scope (Galvanic Alchemist)', () => {
    const self = { ref: 'self' };
    assert.deepEqual(one('As long as ~ is paired with another creature, each of those creatures has "{2}{U}: Untap this creature."'), {
      kind: 'static', effect: { static: 'conditional', condition: paired(self), effect: { static: 'modify', what: pair(self),
        modifiers: [{ modifier: 'abilities', abilities: [{ gained: 'quoted', ability: { kind: 'activated',
          costs: [{ cost: 'mana', symbols: ['2', 'U'] }], effects: [{ steps: [{ effect: 'untap', what: source }] }] } }] }] } },
    });
  });
  it('rejects pair subjects without their paired condition and unsupported pair riders', () => {
    fails('Both creatures have flying.');
    fails('As long as you control an Elf, both creatures have flying.');
    fails('As long as this creature is paired with a battle, each of those permanents has flying.');
    // The partner reference "that creature" is not represented (Tarelily).
    fails('As long as this creature is paired with another creature, both creatures have reach. If that creature is a Dwarf, both creatures have lifelink.');
    fails('As long as this creature is paired with another creature, both creatures have flying as long as you control an Elf.');
  });
});
