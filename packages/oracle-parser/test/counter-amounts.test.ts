import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

describe('counter removal quantities', () => {
  it('preserves both Confront the Past modes and twice X independently from its target', () => {
    // Actual Oracle text in the frozen source. One mode returns a card;
    // the other removes counters, without selecting twice X planeswalkers.
    const result = parseOracleText('Choose one —\n• Return target planeswalker card with mana value X or less from your graveyard to the battlefield.\n• Remove twice X loyalty counters from target planeswalker an opponent controls.', 'Confront the Past', { includeSourceProvenance: true });
    assert.ok(result.ok, JSON.stringify(result.lines));
    const x = { amount: 'x' }, one = { amount: 'fixed', value: 1 };
    assert.deepEqual(JSON.parse(JSON.stringify(result.abilities)), [{ kind: 'spell', effects: [{ steps: [{
      effect: 'modal', count: { exactly: 1 }, options: [
        { effects: [{ steps: [{ effect: 'move-zone', what: { ref: 'target', filter: { types: ['planeswalker'], cls: 'card', properties: [{ property: 'mana-value', comparison: { op: 'le', value: x } }], zone: { zone: 'graveyard', owner: 'your' } }, count: one, selectionId: 'line:1:2' }, to: { zone: 'battlefield' } }] }] },
        { effects: [{ steps: [{ effect: 'remove-counters', counter: { type: 'loyalty' }, count: { amount: 'twice', of: x }, from: { ref: 'target', filter: { types: ['planeswalker'], control: 'opponent' }, count: one, selectionId: 'line:2:7' } }] }] },
      ],
    }] }] }]);
  });
  it('does not accept a missing arithmetic operand, counter type or source', () => {
    for (const text of [
      'Remove twice loyalty counters from target planeswalker.',
      'Remove twice X counters from target planeswalker.',
      'Remove twice X loyalty counters from.',
    ]) assert.equal(parseOracleText(text).ok, false, text);
  });
});
