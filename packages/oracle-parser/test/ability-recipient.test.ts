import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText, analyzeCardBindings } from '../src/index.ts';

const x = { amount: 'x' };
const eot = { duration: 'end-of-turn' };
const haste = [{ gained: 'keyword', keyword: { keyword: 'haste' } }];
function parse(text: string, name = 'Recipient fixture') {
  const result = parseOracleText(text, name);
  assert.ok(result.ok, JSON.stringify(result.lines));
  return result;
}

describe('ability recipient domains', () => {
  // Oracle text checked against the frozen Scryfall source; player and object
  // domains are distinct under CR109.1 and both can have abilities (113.1).
  it('retains all of Orcus, including group mana value and the plural grant', () => {
    const result = parse('Flying, trample\nWhen Orcus enters, choose one —\n• Each other creature gets -X/-X until end of turn. You lose X life.\n• Return up to X target creature cards with total mana value X or less from your graveyard to the battlefield. They gain haste until end of turn.', 'Orcus, Prince of Undeath');
    assert.deepEqual(JSON.parse(JSON.stringify(result.abilities)), [
      { kind: 'keywords', keywords: [{ keyword: 'flying' }, { keyword: 'trample' }] },
      { kind: 'triggered', trigger: { trigger: 'enters', what: { ref: 'self' } }, effects: [{ steps: [{
        effect: 'modal', count: { exactly: 1 }, options: [
          { effects: [
            { steps: [{ effect: 'pump', what: { ref: 'each', filter: { other: true, types: ['creature'] } }, power: { sign: -1, amount: x }, toughness: { sign: -1, amount: x }, duration: eot }] },
            { steps: [{ effect: 'lose-life', who: { player: 'you' }, amount: x }] },
          ] },
          { effects: [
            { steps: [{ effect: 'move-zone', what: { ref: 'target', filter: { types: ['creature'], cls: 'card', zone: { zone: 'graveyard', owner: 'your' } }, groupConstraints: [{ constraint: 'attribute-sum', attribute: 'mana-value', comparison: { op: 'le', value: x } }], count: x, upTo: true }, to: { zone: 'battlefield' } }] },
            { steps: [{ effect: 'gain-abilities', what: { recipient: 'they' }, abilities: haste, duration: eot }] },
          ] },
        ],
      }] }] },
    ]);
  });
  it('does not invent either an object or player antecedent for the same pronoun', () => {
    for (const prefix of ['Return all creature cards from your graveyard to the battlefield.', 'Target player draws a card.']) {
      const result = parse(`${prefix} They gain hexproof until end of turn.`);
      const grant = result.abilities[0];
      assert.ok(grant.kind === 'spell');
      assert.deepEqual(grant.effects[1], { steps: [{ effect: 'gain-abilities', what: { recipient: 'they' }, abilities: [{ gained: 'keyword', keyword: { keyword: 'hexproof' } }], duration: eot }] });
      const binding = analyzeCardBindings({ identity: 'recipient', source: { name: 'Recipient fixture', layout: 'normal' }, faces: [{ sourceFaceIndex: 0, result }] });
      assert.equal(binding.fullyBound, false);
      assert.ok(binding.references.some(ref => 'recipient' in ref.expression && ref.expression.recipient === 'they' && ref.binding === 'unresolved'));
    }
  });
  it('keeps an explicitly named player and an explicitly named creature distinct', () => {
    for (const [subject, effect, key, reference] of [
      ['Target player', 'player-gain-abilities', 'who', { player: 'target-player' }],
      ['Target creature', 'gain-abilities', 'what', { ref: 'target', filter: { types: ['creature'] }, count: { amount: 'fixed', value: 1 } }],
    ] as const) {
      const result = parse(`${subject} gains hexproof until end of turn.`);
      assert.deepEqual(JSON.parse(JSON.stringify(result.abilities)), [{ kind: 'spell', effects: [{ steps: [{ effect, [key]: reference, abilities: [{ gained: 'keyword', keyword: { keyword: 'hexproof' } }], duration: eot }] }] }]);
    }
  });
});
