import assert from 'node:assert/strict';
import test from 'node:test';
import { moveSelection, revalidateSelection, toggleSelection, type SelectionDraft } from '../src/decisions/selection.ts';
import { parseQuantity } from '../src/decisions/quantity.ts';

const binding = { match: 'fixture', seat: 'own', token: 'token-a' };
const empty: SelectionDraft<string, string> = { binding, selected: [] };

test('draft identity is match, authorized seat and token', () => {
  for (const changed of [{ ...binding, match: 'other' }, { ...binding, seat: 'other' }, { ...binding, token: 'token-b' }]) {
    assert.equal(revalidateSelection(empty, changed, []), null);
  }
});
test('only supplied handles can be selected; selection is ordered, unique and bounded', () => {
  const candidates = ['copy-a', 'copy-b'];
  assert.equal(toggleSelection(empty, 'foreign', candidates, 1), empty);
  const first = toggleSelection(empty, 'copy-a', candidates, 1);
  assert.equal(toggleSelection(first, 'copy-b', candidates, 1), first);
  assert.deepEqual(toggleSelection(first, 'copy-a', candidates, 1).selected, []);
  const second = toggleSelection(first, 'copy-b', candidates, 2);
  assert.deepEqual(moveSelection(second, 1, 0).selected, ['copy-b', 'copy-a']);
  assert.deepEqual(second.selected, ['copy-a', 'copy-b']);
  assert.equal(moveSelection(second, -1, 0), second);
});
test('new incarnation never inherits selection, even when a display name would match', () => {
  const draft = { binding, selected: ['old-incarnation', 'stable'] };
  assert.deepEqual(revalidateSelection(draft, binding, ['new-incarnation', 'stable'])?.selected, ['stable']);
});
test('quantity syntax preserves values beyond Number precision and rejects coercion', () => {
  assert.equal(parseQuantity('9007199254740993', 20), 9007199254740993n);
  assert.equal(parseQuantity('0', 20), 0n);
  for (const text of ['', '-1', '+1', '01', '1.0', '1e4', ' 1', '1 ', '1\n', '1\r', '1\r\n', '١', '1000']) {
    assert.equal(parseQuantity(text, 3), null, text);
  }
});
