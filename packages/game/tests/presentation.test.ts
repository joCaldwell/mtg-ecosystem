import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import type { SeatResume, Decision } from '@multiverse/game-server-sdk';
import type { PresentationArtifact } from '../generated/presentation.d.ts';
import opening from '../fixtures/foundations-opening/opening-seat0.json' with { type: 'json' };
import presentation from '../fixtures/foundations-opening/presentation.json' with { type: 'json' };
import pins from '../fixtures/foundations-opening/provenance.json' with { type: 'json' };
import { inspectDecision, openingFixtureTable } from '../web/src/presentation/opening-fixture.ts';

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
  const commander = openingFixtureTable(resume, metadata).commandZone[0];
  assert.equal(commander?.oracleText, '');
  assert.equal(commander?.printImage, printed.faces[0]?.image_url);
  const ambiguous = { ...metadata, cards: [{ ...printed, faces: [printed.faces[0]!, { ...printed.faces[0]!, face_index: 1 }] }] } as PresentationArtifact;
  const unknown = openingFixtureTable(resume, ambiguous).commandZone[0];
  assert.equal(unknown?.name, null);
  assert.equal(unknown?.printImage, undefined);
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

test('opening and priority headings describe the current supplied request', () => {
  if (resume.state.status !== 'available') assert.fail('expected fixture');
  assert.equal(inspectDecision(resume.state.observation.decision)?.title, 'Opening hand');
  assert.equal(inspectDecision(null), null);
  const priority: Decision = { reason: {reason:'priority'},candidates:[{candidate:'action',action:{action:'pass'}}],shape:{shape:'pick',slots:[{role:{role:'choice'},subject:null,allowed:[0],min:1,max:1,repeat:false}],rules:[]}, player: 0, token: 'synthetic-test-token' };
  assert.equal(inspectDecision(priority)?.title, 'Priority');
});

test('live adapter uses permitted data and retains all zones, exact quantities and lifecycle-independent view', async () => {
  const { liveTable } = await import('../web/src/presentation/live-table.ts');
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
  assert.ok(table.hand.every(card => !card.printImage));
  assert.equal(table.otherObjects?.[0]?.printImage, undefined);
  assert.ok(table.commandZone[0]?.printImage);
  assert.equal(liveTable({ ...changed, state: { status: 'pending' } }, metadata), null);
  assert.ok(liveTable(changed, null)!.hand.every(value => value.name === null));
});

test('live adapter states core face, creature-type and stack facts', async () => {
  const { liveTable } = await import('../web/src/presentation/live-table.ts');
  const changed = structuredClone(resume);
  if (changed.state.status !== 'available') assert.fail();
  const { view } = changed.state.observation;
  const card = view.objects.find(object => object.zone.zone === 'command')!;
  card.faces = { layout: 'modal_double_faced', front_name: 'Front Name', back_name: 'Back Name', active_face: 'back' };
  card.characteristics = { ...card.characteristics!, all_creature_types: true, subtypes: ['Shapeshifter'] };
  const spell = structuredClone(card);
  spell.handle = 'spell-1';
  spell.zone = { zone: 'stack' };
  view.objects.push(spell);
  view.stack = [{ spell: spell.handle, controller: 0, cast_complete: true, targets: [], mode_mask: 5, flashback: true, handle: 's1' }];
  const table = liveTable(changed, metadata)!;
  const shown = table.commandZone.find(value => value.inspectionId === card.handle)!;
  assert.equal(shown.name, 'Back Name');
  assert.equal(shown.printImage, undefined);
  assert.ok(shown.currentFacts.some(fact => fact.label === 'Active face' && fact.value === 'Back · Back Name'));
  assert.ok(shown.currentFacts.some(fact => fact.label === 'Creature types' && fact.value === 'All creature types'));
  const stackSpell = table.stack[0]!;
  assert.equal(stackSpell.name, 'Back Name');
  const facts = stackSpell.currentFacts;
  assert.deepEqual(facts.filter(fact => fact.label === 'Controller'), [{ label: 'Controller', value: 'Seat 1' }]);
  assert.ok(facts.some(fact => fact.label === 'Chosen modes' && fact.value === 'Mode 1 · Mode 3'));
  assert.ok(facts.some(fact => fact.label === 'Flashback'));
  assert.ok(facts.some(fact => fact.label === 'Active face' && fact.value === 'Back · Back Name'));
  assert.ok(facts.some(fact => fact.label === 'Creature types' && fact.value === 'All creature types'));
  assert.ok(facts.some(fact => fact.label === 'Subtypes' && fact.value === 'Shapeshifter'));
});

test('live adapter shows repeated printed modes to another seat', async () => {
  const { liveTable } = await import('../web/src/presentation/live-table.ts');
  const changed = structuredClone(resume);
  if (changed.state.status !== 'available') assert.fail();
  const entry = {
    spell: null, controller: 1, cast_complete: true, targets: [],
    mode_mask: 5, chosen_modes: [0, 0, 2],
  };
  changed.state.observation.view.stack = [entry];
  const shown = liveTable(changed, null)!.stack[0]!;
  assert.deepEqual(shown.currentFacts.find(fact => fact.label === 'Chosen modes'),
    { label: 'Chosen modes', value: 'Mode 1 · Mode 1 · Mode 3' });
});
