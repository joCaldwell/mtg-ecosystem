import assert from 'node:assert/strict';
import test from 'node:test';
import type { Decision } from '@multiverse/game-server-sdk';
import { completeDecision, decisionSurface, emptyDraft } from '../web/src/decisions/controls.ts';

const base = {
  token: 'original',
  player: 0,
  candidates: [
    { candidate: 'object', object: 'a' },
    { candidate: 'object', object: 'b' },
    { candidate: 'player', player: 1 },
  ],
} satisfies Partial<Decision>;
const complete = (d: Decision, values: [string, string][]) => completeDecision(d, [], { ...emptyDraft, values });

test('multiple picks enforce per-slot membership, shared minimums and distinct groups', () => {
  const slots = [0, 1].map((subject) => ({
    role: { role: 'block_with' as const },
    subject,
    allowed: [2],
    min: 0,
    max: 1,
    repeat: false,
  }));
  const d: Decision = {
    ...base,
    reason: { reason: 'blockers' },
    shape: { shape: 'pick', slots, rules: [{ rule: 'picked_at_least', candidates: [2], count: 2 }] },
  };
  assert.equal(
    complete(d, [
      ['0', 'unused'],
      ['1', '2'],
    ]),
    null,
  );
  assert.equal(
    complete(d, [
      ['0', '1'],
      ['1', '2'],
    ]),
    null,
  );
  const both: [string, string][] = [
    ['0', '2'],
    ['1', '2'],
  ];
  assert.deepEqual(complete(d, both), { answer: 'pick', picks: [[2], [2]] });
  assert.equal(
    complete({ ...d, shape: { shape: 'pick', slots, rules: [{ rule: 'distinct', slots: [0, 1] }] } }, both),
    null,
  );
  assert.equal(
    complete({ ...d, shape: { shape: 'pick', slots, rules: [{ rule: 'total', min: 0, max: 1 }] } }, both),
    null,
  );
});
test('picked_at_most caps repeated candidates across slots (CR 509.1b)', () => {
  const slots = [0, 1].map((subject) => ({
    role: { role: 'block_with' as const },
    subject,
    allowed: [2],
    min: 0,
    max: 1,
    repeat: false,
  }));
  const d: Decision = {
    ...base,
    reason: { reason: 'blockers' },
    shape: { shape: 'pick', slots, rules: [{ rule: 'picked_at_most', candidates: [2], count: 1 }] },
  };
  assert.deepEqual(complete(d, [['0', '2'], ['1', 'unused']]), { answer: 'pick', picks: [[2], []] });
  assert.equal(complete(d, [['0', '2'], ['1', '2']]), null);
});
test('mode and stack candidates label from core output without guessing', () => {
  const d: Decision = {
    token: 'original',
    player: 0,
    reason: { reason: 'choose_modes' },
    candidates: [
      { candidate: 'modes', mask: 3, labels: ['Draw a card.', 'Gain 3 life.'] },
      { candidate: 'stack', stack: 's1' },
    ],
    shape: { shape: 'pick', slots: [{ role: { role: 'choice' }, subject: null, allowed: [0, 1], min: 1, max: 1, repeat: false }], rules: [] },
  };
  const surface = decisionSurface(d, []);
  assert.equal(surface.status, 'supported');
  if (surface.status !== 'supported' || surface.control.kind !== 'choice') assert.fail('expected a choice');
  assert.deepEqual(
    surface.control.options.map(({ label, detail }) => [label, detail]),
    [['Draw a card. + Gain 3 life.', ''], ['Stack item', 's1']],
  );
});
test('back-face priority actions use core action text without duplicating the front name', () => {
  const d: Decision = {
    token: 'original',
    player: 0,
    reason: { reason: 'priority' },
    candidates: [
      { candidate: 'action', action: { action: 'cast_back_face', card: 'h1' } },
      { candidate: 'action', action: { action: 'play_land_back_face', object: 'h2' } },
    ],
    shape: {
      shape: 'pick',
      slots: [{ role: { role: 'choice' }, subject: null, allowed: [0, 1], min: 1, max: 1, repeat: false }],
      rules: [],
    },
  };
  const labels = [
    { handle: 'h1', name: 'Front Spell' },
    { handle: 'h2', name: 'Front Land' },
  ];
  const context = {
    candidates: [{ text: 'Cast Back Spell' }, { text: 'Play Back Land' }],
  };
  const surface = decisionSurface(d, labels, context);
  assert.equal(surface.status, 'supported');
  if (surface.status !== 'supported' || surface.control.kind !== 'choice') assert.fail('expected a choice');
  assert.deepEqual(
    surface.control.options.map(({ label, detail }) => [label, detail]),
    [
      ['Cast Back Spell', 'h1'],
      ['Play Back Land', 'h2'],
    ],
  );
  assert.deepEqual(completeDecision(d, labels, { ...emptyDraft, selected: ['0'] }, context), { answer: 'pick', picks: [[0]] });
  assert.deepEqual(completeDecision(d, labels, { ...emptyDraft, selected: ['1'] }, context), { answer: 'pick', picks: [[1]] });
});
test('distribute checks exact totals, bucket bounds and conditional allocations', () => {
  const d: Decision = {
    ...base,
    reason: { reason: 'combat_damage', source: 'a', power: '5' },
    shape: {
      shape: 'distribute',
      total: '5',
      buckets: [
        { candidate: 1, min: '1', max: '5' },
        { candidate: 2, min: '0', max: '4' },
      ],
      gates: [{ bucket: 1, requires: [[0, '3']] }],
    },
  };
  for (const [a, b] of [
    ['1', '4'],
    ['3', '1'],
    ['6', '0'],
    ['0', '5'],
  ])
    assert.equal(
      complete(d, [
        ['0', a!],
        ['1', b!],
      ]),
      null,
    );
  assert.deepEqual(
    complete(d, [
      ['0', '3'],
      ['1', '2'],
    ]),
    { answer: 'distribute', amounts: ['3', '2'] },
  );
});
test('payment preserves source indices, both finish policies and exact unrestricted spending', () => {
  const cost = { generic: '9007199254740993', colored: ['U' as const] };
  const pool = { unrestricted: { U: '2', C: '9007199254740994' } };
  const d: Decision = {
    ...base,
    reason: { reason: 'mana', source: null },
    shape: { shape: 'pay', cost, pool, step: { step: 'produce', sources: [1] } },
  };
  for (const [id, pay] of [
    ['1', { step: 'activate', source: 1 }],
    ['finish', { step: 'finish', undo_on_failure: false }],
    ['finish-undo', { step: 'finish', undo_on_failure: true }],
  ] as const)
    assert.deepEqual(completeDecision(d, [], { ...emptyDraft, selected: [id] }), { answer: 'pay', pay });
  assert.equal(completeDecision(d, [], { ...emptyDraft, selected: ['2'] }), null);
  const spend: Decision = { ...d, shape: { shape: 'pay', cost, pool, step: { step: 'spend' } } };
  assert.deepEqual(
    complete(spend, [
      ['U', '1'],
      ['C', '9007199254740993'],
    ]),
    { answer: 'pay', pay: { step: 'spend', unrestricted: { U: '1', C: '9007199254740993' } } },
  );
  assert.equal(
    complete(spend, [
      ['U', '0'],
      ['C', '9007199254740994'],
    ]),
    null,
  );
});

test('pick honors multi-item slots, multiplicities and per-slot bounds', () => {
  const d: Decision = {
    ...base,
    reason: { reason: 'discard' },
    shape: {
      shape: 'pick',
      rules: [],
      slots: [
        { role: { role: 'choice' }, subject: null, allowed: [2, 0], min: 2, max: 3, repeat: true },
        { role: { role: 'choice' }, subject: null, allowed: [1, 2], min: 1, max: 2, repeat: false },
      ],
    },
  };
  assert.deepEqual(
    complete(d, [
      ['0/2', '2'],
      ['1/2', '1'],
    ]),
    { answer: 'pick', picks: [[2, 2], [2]] },
  );
  assert.equal(
    complete(d, [
      ['0/2', '1'],
      ['1/2', '1'],
    ]),
    null,
  );
  assert.equal(
    complete(d, [
      ['0/2', '2'],
      ['1/2', '2'],
    ]),
    null,
  );
});
