import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';
import { analyzeCardBindings } from '../src/semantic/bindings.ts';
import { lex } from '../src/lexer.ts';
import { Cursor } from '../src/parser/cursor.ts';
import { parseSentences } from '../src/parser/effects.ts';

// Oracle text checked in the frozen source. CR609.3 (exhaustion),701.13
// (exile),701.20a/b (revealed cards stay in their zone) define these scans.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed', value });
const you = { player: 'you' };
const lib = { zone: 'library', owner: 'your' };
const scan = (matching: object, count: object = n(1), operation = 'reveal') => ({
  effect: 'scan-library', who: you, operation, library: lib, until: { count, matching },
});
function abilities(text: string, name?: string, typeLine?: string) {
  const r = parseOracleText(text, name, { typeLine, includeSourceProvenance: true });
  assert.ok(r.ok, JSON.stringify(r.lines));
  return JSON.parse(JSON.stringify(r.abilities));
}

describe('sequential library scans', () => {
  it('binds frozen Creative Technique to the first nonland and only its exile successor', () => {
    const fixture = new URL('../../game-engine/tests/fixtures/ramos/c336b735-3e6b-471e-be7a-84d7402ae2f5.json', import.meta.url);
    const row = readFileSync(fixture, 'utf8').trim();
    assert.equal(createHash('sha256').update(row).digest('hex'), 'a7440993de664d72cc9c3a85a5628363e48272a4fc5b1a7208b4c394e3dba55d');
    const { source, oracleIdentity: identity, status } = JSON.parse(row);
    assert.equal(status, 'unsupported');
    const text: string = source.oracle_text;
    const parse = (oracle: string) => parseOracleText(oracle, source.name, { typeLine: source.type_line, includeSourceProvenance: true });
    const result = parse(text);
    assert.ok(result.ok, JSON.stringify(result.lines));
    const nonland = { nonTypes: ['land'], cls: 'card' };
    const exiledCard = { ref: 'related', number: 'singular', filter: { cls: 'card' },
      relation: { relation: 'event-result', event: 'exile', scope: 'contextual' } };
    assert.deepEqual(result.abilities, [
      { kind: 'keywords', keywords: [{ keyword: 'demonstrate' }] },
      { kind: 'spell', abilityWord: undefined, effects: [
        { steps: [{ effect: 'shuffle', who: you, library: lib }, scan(nonland)] },
        { steps: [
          { effect: 'exile', what: { ref: 'that', filter: { cls: 'card' } } },
          { effect: 'move-zone', what: { ref: 'rest' }, to: { ...lib, position: 'bottom' }, order: 'random' },
        ] },
        { steps: [{ effect: 'play-card', optional: true, instruction: {
          who: you, action: 'cast', what: exiledCard, alternativeCost: { cost: 'without-mana-cost' },
        } }] },
      ] },
    ]);
    const bind = (result: ReturnType<typeof parse>) => analyzeCardBindings({ identity, source, faces: [{ sourceFaceIndex: 0, result }] });
    const graph = bind(result);
    assert.deepEqual(graph.diagnostics, []);
    assert.equal(graph.referencesBound, true);
    assert.deepEqual(graph.engineFeatures.map(need => need.feature), ['demonstrate', 'reveal-and-cast']);
    assert.equal(graph.fullyBound, false);
    assert.equal(graph.executable, false);
    const keyword = graph.nodes.find(node => node.node === 'keyword-ability');
    assert.ok(keyword?.node === 'keyword-ability' && keyword.keyword === 'demonstrate');
    assert.deepEqual(keyword.abilities, ['triggered']);
    assert.ok(graph.nodes.some(node => node.id === keyword.you && node.node === 'runtime-player' && node.role === 'trigger-controller'));
    const events = graph.nodes.filter(node => node.node === 'event');
    assert.deepEqual(events.map(event => event.operation), ['shuffle', 'scan-library', 'exile', 'move-zone', 'play-card']);
    const [shuffle, reveal, exile, bottom, cast] = events;
    const [controller] = shuffle.inputs;
    assert.deepEqual(reveal.inputs, [controller]);
    const results = graph.nodes.filter(node => node.node === 'event-result');
    const pool = results.find(node => node.event === reveal.id && node.cardinality === 'many');
    const hit = results.find(node => node.event === reveal.id && node.cardinality === 'one');
    assert.ok(pool?.identity === 'same-objects');
    assert.ok(hit?.identity === 'same-objects' && hit.subsetOf === pool.id && hit.availability === 'if-produced');
    assert.deepEqual(hit.description, nonland);
    assert.deepEqual(exile.inputs, [hit.id]);
    const remainder = graph.nodes.find(node => node.node === 'remainder');
    assert.ok(remainder?.node === 'remainder' && remainder.pool === pool.id && remainder.excluding === hit.id);
    assert.deepEqual(bottom.inputs, [remainder.id]);
    const successor = results.find(node => node.event === exile.id && node.identity === 'zone-successors');
    assert.ok(successor?.cardinality === 'one' && successor.availability === 'if-produced');
    assert.deepEqual(cast.inputs, [successor.id, controller]);
    assert.equal(cast.requiresResult, successor.id);
    assert.equal(cast.optional, true);
    assert.equal(cast.decider, controller);
    assert.ok(graph.references.some(ref => ref.path === cast.path + '/instruction/what' && ref.binding === 'bound' && ref.target === successor.id));

    for (const changed of [
      text.replace('Shuffle your library, then reveal', 'Reveal'),
      text.replace('Shuffle your library', "Shuffle target player's library"),
      text.replace('a nonland card.', 'two nonland cards.'),
      text.replace('a nonland card.', 'a nonland cards.'),
      text.replace('a nonland card.', 'a nonland card from your graveyard.'),
    ]) assert.equal(parse(changed).ok, false, changed);
    // A valid change in the source still parses, but no longer establishes
    // the complete, supported scan/exile/cast binding.
    const plural = parse(text.replace('cast the exiled card', 'cast the exiled cards'));
    assert.ok(plural.ok);
    const unsupported = bind(plural);
    assert.equal(unsupported.referencesBound, false);
    assert.ok(unsupported.diagnostics.some(d => d.code === 'cardinality-mismatch' || d.code === 'ambiguous-antecedent'));
    assert.ok(!unsupported.engineFeatures.some(need => need.feature === 'reveal-and-cast'));
  });
  it('keeps Treasure Hunt full revealed pool separate from its nonland stopping match', () => {
    assert.deepEqual(abilities('Reveal cards from the top of your library until you reveal a nonland card, then put all cards revealed this way into your hand.'), [{
      kind: 'spell', effects: [{ steps: [scan({ nonTypes: ['land'], cls: 'card' }), {
        effect: 'move-zone', what: { ref: 'all', filter: { cls: 'card', relation: { relation: 'event-result', event: 'reveal', scope: 'this-way' } } }, to: { zone: 'hand', owner: 'your' },
      }] }],
    }]);
  });
  it('preserves Old Stickfingers X matches, subset destinations, random remainder and intrinsic P/T', () => {
    assert.deepEqual(abilities("When you cast this spell, reveal cards from the top of your library until you reveal X creature cards. Put all creature cards revealed this way into your graveyard, then put the rest on the bottom of your library in a random order.\nOld Stickfingers's power and toughness are each equal to the number of creature cards in your graveyard.", 'Old Stickfingers', 'Legendary Creature — Horror'), [{
      kind: 'triggered', trigger: { trigger: 'cast', who: you, what: { ref: 'this', noun: 'spell' } }, effects: [
        { steps: [scan({ types: ['creature'], cls: 'card' }, { amount: 'x' })] },
        { steps: [
          { effect: 'move-zone', what: { ref: 'all', filter: { types: ['creature'], cls: 'card', relation: { relation: 'event-result', event: 'reveal', scope: 'this-way' } } }, to: { zone: 'graveyard', owner: 'your' } },
          { effect: 'move-zone', what: { ref: 'rest' }, to: { ...lib, position: 'bottom' }, order: 'random' },
        ] },
      ],
    }, {
      kind: 'static', effect: { static: 'cda-pt', what: { ref: 'self' }, stat: 'both', value: { amount: 'count', of: { types: ['creature'], cls: 'card', zone: { zone: 'graveyard', owner: 'your' } } } },
    }]);
  });
  it('keeps Territorial Bruntar exile distinct from revealing and a later timed casting permission', () => {
    assert.deepEqual(abilities('Reach\nLandfall — Whenever a land you control enters, exile cards from the top of your library until you exile a nonland card. You may cast that card this turn.', 'Territorial Bruntar', 'Creature — Beast'), [
      { kind: 'keywords', keywords: [{ keyword: 'reach' }] },
      { kind: 'triggered', abilityWord: 'Landfall', trigger: { trigger: 'enters', what: { ref: 'filter', filter: { types: ['land'], control: 'you' }, count: n(1) } }, effects: [
        { steps: [scan({ nonTypes: ['land'], cls: 'card' }, n(1), 'exile')] },
        { steps: [{ effect: 'grant-play-permission', permission: { who: you, action: 'cast', what: { ref: 'that', filter: { cls: 'card' } } }, duration: { duration: 'this-turn' } }] },
      ] },
    ]);
  });
  it('retains Balustrade Spy target identity and actor across the following move', () => {
    const target = { player: 'target-player', selectionId: 'line:1:5' };
    assert.deepEqual(abilities('Flying\nWhen this creature enters, target player reveals cards from the top of their library until they reveal a land card, then puts those cards into their graveyard.', 'Balustrade Spy', 'Creature — Vampire Rogue'), [
      { kind: 'keywords', keywords: [{ keyword: 'flying' }] },
      { kind: 'triggered', trigger: { trigger: 'enters', what: { ref: 'this', noun: 'creature' } }, effects: [{ steps: [
        { ...scan({ types: ['land'], cls: 'card' }), who: target, library: { zone: 'library', owner: 'their' } },
        { effect: 'move-zone', who: target, what: { ref: 'that', filter: { cls: 'card' }, plural: true }, to: { zone: 'graveyard', owner: 'their' } },
      ] }] },
    ]);
  });
  it('keeps zero, multiple and per-player stopping counts distinct', () => {
    for (const count of [0, 1, 4]) assert.deepEqual(abilities(`Reveal cards from the top of your library until you reveal ${count} land cards.`), [{
      kind: 'spell', effects: [{ steps: [scan({ types: ['land'], cls: 'card' }, n(count))] }],
    }]);
    assert.deepEqual(abilities('Each opponent exiles cards from the top of their library until they exile a nonland card.'), [{
      kind: 'spell', effects: [{ steps: [{ ...scan({ nonTypes: ['land'], cls: 'card' }, n(1), 'exile'), who: { player: 'each-opponent' }, library: { zone: 'library', owner: 'their' } }] }],
    }]);
  });
  it('rejects changed actor/action, implicit count, foreign pool and unsupported scan order; restores context', () => {
    for (const text of [
      'Reveal cards from the top of your library until they reveal a creature card.',
      'Each opponent reveals cards from the top of their library until you reveal a land card.',
      'Reveal cards from the top of your library until you exile a land card.',
      'Exile cards from the top of your library until you reveal a land card.',
      'Reveal cards from the top of your library until you reveal creature cards.',
      'Reveal cards from the top of your library until you reveal a creature card from your graveyard.',
      'Reveal cards from the bottom of your library until you reveal a land card.',
      'Reveal cards from the top of library until you reveal a land card.',
      'Reveal cards from the top of your library until you reveal a creature card or a land card.',
      'Reveal cards from your hand at random until you reveal a land card.',
    ]) assert.equal(parseOracleText(text).ok, false, text);
    const c = new Cursor(lex('Target opponent exiles cards from the top of their library until you reveal a land card.'));
    const ctx = { modalOptions: [], lastPlayer: you as { player: 'you' } };
    const original = JSON.stringify(ctx);
    assert.equal(parseSentences(c, ctx), null);
    assert.equal(c.pos, 0);
    assert.equal(JSON.stringify(ctx), original);
  });
  it('binds the stopping card to the matching subset of the scan', () => {
    const result = parseOracleText('Reveal cards from the top of your library until you reveal a land card. Put that card into your hand.', 'Scan test');
    assert.ok(result.ok);
    const binding = analyzeCardBindings({ identity: 'scan-test', source: { name: 'Scan test', layout: 'normal', type_line: 'Sorcery', mana_cost: '{G}' }, faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(binding.fullyBound, false);
    assert.equal(binding.referencesBound, true, JSON.stringify(binding.diagnostics));
    const scan = binding.nodes.find(node => node.node === 'library-scan');
    const moved = binding.references.find(ref => ref.path.endsWith('/effects/1/steps/0/what'));
    assert.ok(scan && moved?.binding === 'bound');
    assert.equal(moved.target, scan.matches);
    assert.notEqual(moved.target, scan.pool);
  });
});

describe('hand reveal ownership', () => {
  it('stores the revealed hand owner independently of the player performing the action', () => {
    for (const owner of ['your', 'their']) assert.deepEqual(abilities(`Target opponent reveals ${owner} hand.`), [{ kind: 'spell', effects: [{ steps: [{
      effect: 'reveal', who: { player: 'target-opponent', selectionId: 'line:0:0' }, what: { ref: 'all', filter: { cls: 'card', zone: { zone: 'hand', owner } } },
    }] }] }]);
    assert.equal(parseOracleText('Reveal hand.').ok, false);
  });
});
