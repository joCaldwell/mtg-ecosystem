import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import type { SeatResume, PlayerRequest } from '@multiverse/game-server-sdk';
import type { PresentationArtifact } from '../generated/presentation.d.ts';
import opening from '../fixtures/foundations-opening/opening-seat0-http.json' with { type: 'json' };
import presentation from '../fixtures/foundations-opening/presentation.json' with { type: 'json' };
import pins from '../fixtures/foundations-opening/provenance.json' with { type: 'json' };
import { inspectDecision, openingFixtureTable } from '../src/presentation/opening-fixture.ts';

// Digests/schema validation run in prepare:fixtures before the suite.
const resume = opening.resume as SeatResume;
const metadata = presentation as PresentationArtifact;

test('tracked fixtures preserve exact producer and HTTP capture bytes', async () => {
  for (const file of pins.files) {
    const bytes = await readFile(new URL(`../fixtures/foundations-opening/${file.path}`, import.meta.url));
    assert.equal(createHash('sha256').update(bytes).digest('hex'), file.sha256, file.path);
  }
});

test('source-qualified own hand and public commanders retain distinct object handles', () => {
  const table = openingFixtureTable(resume, metadata);
  assert.equal(table.hand.length, 7);
  assert.equal(new Set(table.hand.map(card => card.inspectionId)).size, 7);
  assert.equal(new Set(table.hand.map(card => card.name)).size, 1, 'same source name, distinct handles');
  assert.equal(table.seats.filter(seat => seat.isViewer).length, 1);
  assert.deepEqual(table.seats.map(seat => seat.handCount), ['7', '7', '7', '7']);
  assert.equal(table.commandZone.length, 1);
  assert.equal(table.seats.flatMap(seat => seat.commandZone).length, 4);
  assert.ok(table.hand.every(card => card.sourceIdentity.includes('Source SHA-256')));
  assert.ok(!JSON.stringify(table).includes('image_url'));
});

test('missing metadata keeps exact current characteristics with no invented printed text', () => {
  const table = openingFixtureTable(resume, { ...metadata, cards: [] });
  const commander = table.commandZone[0]!;
  assert.equal(commander.name, null);
  assert.equal(commander.printedType, null);
  assert.equal(commander.oracleText, null);
  assert.equal(commander.currentFacts.find(fact => fact.label === 'Power')?.value, '2');
  assert.ok(commander.sourceIdentity.includes('Presentation metadata unavailable'));
});

test('ambiguous face mapping does not assume first face; known empty text stays empty', () => {
  const commanderOracle = opening.resume.state.observation.view.objects.find(object => object.zone.zone === 'command' && object.owner === 0)!.oracle_id;
  const printed = metadata.cards.find(card => card.oracle_id === commanderOracle)!;
  assert.equal(openingFixtureTable(resume, metadata).commandZone[0]?.oracleText, '');
  const ambiguous = { ...metadata, cards: [{ ...printed, faces: [printed.faces[0]!, { ...printed.faces[0]!, face_index: 1 }] }] } as PresentationArtifact;
  assert.equal(openingFixtureTable(resume, ambiguous).commandZone[0]?.name, null);
});

test('fixture adapter rejects mixed identities, pending state and unreviewed layouts', () => {
  assert.throws(() => openingFixtureTable(resume, { ...metadata, catalog_sha256: 'other' }), /catalog/);
  assert.throws(() => openingFixtureTable({ ...resume, viewer: 1 }, metadata), /viewer/);
  assert.throws(() => openingFixtureTable({ ...resume, state: { status: 'pending' } }, metadata), /unavailable/);
  const changed = structuredClone(resume);
  if (changed.state.status !== 'available') assert.fail('expected fixture');
  changed.state.observation.view.objects[0]!.zone = { zone: 'graveyard', owner: 0 };
  assert.throws(() => openingFixtureTable(changed, metadata), /Unsupported opening fixture shape/);
});

test('only mulligan has a reviewed inspection adapter; unsupported requests stop explicitly', () => {
  if (resume.state.status !== 'available') assert.fail('expected fixture');
  assert.equal(inspectDecision(resume.state.observation.decision)?.title, 'Opening hand');
  assert.equal(inspectDecision(null), null);
  const unsupported: PlayerRequest = { request: 'priority', player: 0, token: 'synthetic-test-token' };
  assert.equal(inspectDecision(unsupported)?.title, 'Unsupported decision');
});

test('live adapter uses permitted data and retains all zones, exact quantities and lifecycle-independent view', async () => {
  const { liveTable } = await import('../src/presentation/live-table.ts');
  const changed = structuredClone(resume);
  if (changed.state.status !== 'available') assert.fail();
  const card = changed.state.observation.view.objects.find(object => object.zone.zone === 'hand')!;
  card.zone = { zone: 'graveyard', owner: 0 };
  changed.state.observation.view.players[0]!.life = '9007199254740993';
  changed.lifecycle = { status: 'faulted' };
  const table = liveTable(changed, metadata)!;
  assert.equal(table.live, true); assert.equal(table.seats[0]!.life, '9007199254740993');
  assert.equal(table.otherObjects?.length, 1); assert.equal(table.otherObjects?.[0]?.inspectionId, card.handle);
  assert.equal(table.hand.length, 6); assert.equal(table.stack.length, 0);
  assert.equal(liveTable({ ...changed, state: { status: 'pending' } }, metadata), null);
  assert.ok(liveTable(changed, null)!.hand.every(value => value.name === null));
});
