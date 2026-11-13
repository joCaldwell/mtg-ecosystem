import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { presentation, sha256 } from './client-artifacts.ts';
const catalog = () => JSON.parse(readFileSync(new URL('../../game-engine/tests/fixtures/expected-match-ir.json', import.meta.url), 'utf8'));
test('presentation allows only exact source text and stored artwork, excludes IR/private input', () => {
  const input = catalog();
  const output = presentation(input);
  assert.equal(output.cards.length, input.definitions.length);
  for (const [i, card] of output.cards.entries()) {
    const source = JSON.parse(input.definitions[i].provenance.source_json);
    assert.equal(card.faces[0].name, source.name);
    assert.equal(card.faces[0].oracle_text, source.oracle_text ?? null);
    assert.equal(card.ability_mapping, 'unavailable');
  }
  for (const key of ['instructions', 'source_json', 'spell', 'capabilities', 'seed', 'legalities', 'prices']) assert.ok(!Object.keys(output.cards[0]).includes(key));
  input.definitions[0].provenance.source_json += ' ';
  assert.throws(() => presentation(input), /digest/);
});
test('presentation rejects identity mismatch and preserves every explicit face without artwork synthesis', () => {
  const input = catalog();
  input.definitions[0].provenance.printing_id = 'wrong';
  assert.throws(() => presentation(input), /identity/);
  const multi = catalog();
  const p = multi.definitions[0].provenance;
  const source = JSON.parse(p.source_json);
  source.card_faces = [{ name: 'structural test face', oracle_text: 'exact fixture text' }, { name: 'structural test reverse', mana_cost: '' }];
  p.source_json = JSON.stringify(source); p.source_sha256 = sha256(p.source_json);
  const faces = presentation(multi).cards[0].faces;
  assert.equal(faces.length, 2);
  assert.equal(faces[0].image_url, null);
  assert.equal(faces[1].image_url, null);
  assert.equal(faces[1].oracle_text, null);
  assert.deepEqual(faces.map((f: any) => f.face_index), [0,1]);
});
