import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { GameClient, ProtocolError } from '../src/client.ts';
import { manifest } from '../src/validation.ts';

const catalog = JSON.parse(fs.readFileSync(new URL('../../../game-engine/tests/fixtures/expected-match-ir.json', import.meta.url), 'utf8'));
// Wire fixture uses the actual compiled catalog and the source-backed white
// deck recipe from core/match_admission.rs; no engine admission claim is made.
const oracle = name => {
  const card = catalog.definitions.find(card => card.name === name);
  assert(card, `missing source-backed fixture ${name}`);
  return card.oracle_id;
};
const deck = {
  commander: oracle('Isamaru, Hound of Konda'),
  cards: ['Isamaru, Hound of Konda', 'Yoked Ox', 'Samite Healer', 'Sacred Nectar', 'Healing Hands', 'Healing Salve', 'Plains'].map(name => ({ oracle_id: oracle(name), count: name === 'Plains' ? 94 : 1 })),
};
const fixture = () => ({
  protocol_version: 1, match_id: 'abc', engine_build_id: 'b'.repeat(64), server_build_id: 'c'.repeat(64),
  catalog_sha256: 'fc4bd3390e63ea352e129ac370fa848dbee11cf0700e7e6781d449fd49244a7f',
  profile: 'commander-foundations-v1', decks: Array.from({ length: 4 }, () => structuredClone(deck)),
  starting_player: 0, catalog: structuredClone(catalog),
});
const capability = 'private-manifest-seat-capability';
const client = fetch => new GameClient({ baseURL: 'https://engine.example', matchId: 'abc', capability, fetch });
const json = value => new Response(JSON.stringify(value), { headers: { 'content-type': 'application/json' } });

test('manifest route returns precise public catalog and four decklists through Rust-generated authority', async () => {
  const expected = fixture(); const before = JSON.stringify(expected); let call;
  const sdk = client(async (url, init) => { call = { url, init }; return json(expected); });
  assert.deepEqual(await sdk.manifest(), expected);
  assert.equal(JSON.stringify(expected), before);
  assert.equal(expected.catalog.definitions.length, 34);
  assert.equal(call.url, 'https://engine.example/v1/matches/abc/manifest');
  assert(!call.url.includes(capability));
  assert.equal(call.init.method, 'GET');
  assert.equal(call.init.body, undefined);
  assert.equal(call.init.headers.Authorization, `Bearer ${capability}`);
});

test('public manifest rejects hidden metadata, nested extras and malformed typed catalogs/decks', () => {
  for (const key of ['seed', 'profile_digest', 'rng', 'snapshot', 'history', 'journal']) {
    assert.throws(() => manifest({ ...fixture(), [key]: [] }), ProtocolError);
  }
  const nestedDeck = fixture(); nestedDeck.decks[0].seed = [7];
  const nestedCatalog = fixture(); nestedCatalog.catalog.private_state = {};
  const unsafeCount = fixture(); unsafeCount.decks[0].cards[0].count = Number.MAX_SAFE_INTEGER + 1;
  const missingCardFact = fixture(); delete missingCardFact.catalog.definitions[0].characteristics;
  const shortDecks = fixture(); shortDecks.decks.pop();
  const longDecks = fixture(); longDecks.decks.push(structuredClone(deck));
  for (const invalid of [nestedDeck, nestedCatalog, unsafeCount, missingCardFact, shortDecks, longDecks]) {
    assert.throws(() => manifest(invalid), ProtocolError);
  }
});

test('manifest lookup binds match identity/version and retains strict network JSON decoding', async () => {
  for (const invalid of [{ ...fixture(), match_id: 'def' }, { ...fixture(), protocol_version: 2 }]) {
    await assert.rejects(client(async () => json(invalid)).manifest(), ProtocolError);
  }
  const duplicate = JSON.stringify(fixture()).replace('"protocol_version":1', '"protocol_version":1,"\\u0070rotocol_version":1');
  await assert.rejects(client(async () => new Response(duplicate, { headers: { 'content-type': 'application/json' } })).manifest(), /Duplicate/);
  const fractional = JSON.stringify(fixture()).replace('"starting_player":0', '"starting_player":0.000000000000000000001');
  await assert.rejects(client(async () => new Response(fractional, { headers: { 'content-type': 'application/json' } })).manifest(), /Noncanonical/);
});
