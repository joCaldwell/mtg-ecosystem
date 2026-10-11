import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseOracleText } from '../src/index.ts';
import type { Ability } from '../src/ast.ts';
// These named examples were checked in the frozen Scryfall corpus. CR614.15
// and616.1a require self-replacements before other replacements; neither the
// original and replacement nor their target announcements are independent.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed' as const, value });
const signed = (value: number) => ({ sign: 1, amount: n(value) });
const end = { duration: 'end-of-turn' };
const kicked = { condition: 'kicked', what: { ref: 'this', noun: 'spell' }, comparison: { op: 'ge', value: n(1) } };
function one(text: string, name?: string): Ability {
  const parsed = parseOracleText(text, name, { includeSourceProvenance: true });
  assert.ok(parsed.ok, JSON.stringify(parsed.lines));
  assert.equal(parsed.abilities.length, 1);
  return JSON.parse(JSON.stringify(parsed.abilities[0]));
}
describe('self-replacements of resolving instructions', () => {
  it('preserves Burst Lightning announced target when the later damage amount replaces it', () => {
    const to = { to: 'any-target', selectionId: 'line:0:6' };
    assert.deepEqual(one('Burst Lightning deals 2 damage to any target. If this spell was kicked, it deals 4 damage instead.', 'Burst Lightning'), {
      kind: 'spell', effects: [{ steps: [{ effect: 'self-replacement', targetPolicy: 'retain-original',
        original: { effect: 'damage', source: { ref: 'self' }, amount: n(2), to }, condition: kicked,
        instead: [{ effect: 'damage', source: { ref: 'it' }, amount: n(4), to }],
      }] }],
    });
  });
  it('preserves a landfall condition separately from Akoum Hellkite replacement condition', () => {
    const to = { to: 'any-target', selectionId: 'line:0:14' };
    assert.deepEqual(one('Whenever a land you control enters, this creature deals 1 damage to any target. If that land is a Mountain, this creature deals 2 damage instead.'), {
      kind: 'triggered', trigger: { trigger: 'enters', what: { ref: 'filter', filter: { types: ['land'], control: 'you' }, count: n(1) } },
      effects: [{ steps: [{ effect: 'self-replacement', targetPolicy: 'retain-original',
        original: { effect: 'damage', source: { ref: 'this', noun: 'creature' }, amount: n(1), to },
        condition: { condition: 'is-filter', what: { ref: 'that', filter: { types: ['land'] } }, filter: { subtypes: ['Mountain'] } },
        instead: [{ effect: 'damage', source: { ref: 'this', noun: 'creature' }, amount: n(2), to }],
      }] }],
    });
  });
  it('replaces a pump with Colossal Growth full pump and ability grant group', () => {
    const what = { ref: 'that', filter: { types: ['creature'] } };
    assert.deepEqual(one('Target creature gets +3/+3 until end of turn. If this spell was kicked, instead that creature gets +4/+4 and gains trample and haste until end of turn.'), {
      kind: 'spell', effects: [{ steps: [{ effect: 'self-replacement', targetPolicy: 'retain-original',
        original: { effect: 'pump', what: { ref: 'target', filter: { types: ['creature'] }, count: n(1), selectionId: 'line:0:0' }, power: signed(3), toughness: signed(3), duration: end },
        condition: kicked,
        instead: [
          { effect: 'pump', what, power: signed(4), toughness: signed(4), duration: end },
          { effect: 'gain-abilities', what, abilities: [{ gained: 'keyword', keyword: { keyword: 'trample' } }, { gained: 'keyword', keyword: { keyword: 'haste' } }], duration: end },
        ],
      }] }],
    });
  });
  it('supports a trailing instead after a full replacement and keeps later instructions independent', () => {
    assert.deepEqual(one('Draw a card. If you control an Island, draw two cards instead. You gain 1 life.'), {
      kind: 'spell', effects: [
        { steps: [{ effect: 'self-replacement', targetPolicy: 'retain-original', original: { effect: 'draw', who: { player: 'you' }, amount: n(1) },
          condition: { condition: 'control', who: { player: 'you' }, what: { ref: 'filter', filter: { subtypes: ['Island'] }, count: n(1) } },
          instead: [{ effect: 'draw', who: { player: 'you' }, amount: n(2) }],
        }] },
        { steps: [{ effect: 'gain-life', who: { player: 'you' }, amount: n(1) }] },
      ],
    });
  });
  it('does not mistake ordinary conditionals for replacement instructions', () => {
    const parsed = one('Draw a card. If you control an Island, draw two cards.');
    assert.deepEqual(parsed, { kind: 'spell', effects: [
      { steps: [{ effect: 'draw', who: { player: 'you' }, amount: n(1) }] },
      { condition: { condition: 'control', who: { player: 'you' }, what: { ref: 'filter', filter: { subtypes: ['Island'] }, count: n(1) } }, conditionKind: 'if', steps: [{ effect: 'draw', who: { player: 'you' }, amount: n(2) }] },
    ] });
  });
  it('announces alternative kicked targets from the selected branch, not both branches', () => {
    assert.deepEqual(one('Fight with Fire deals 5 damage to target creature. If this spell was kicked, it deals 10 damage divided as you choose among any number of targets instead.', 'Fight with Fire'), {
      kind: 'spell', effects: [{ steps: [{ effect: 'self-replacement', targetPolicy: 'alternative-by-kicker',
        original: { effect: 'damage', source: { ref: 'self' }, amount: n(5), to: { to: 'object', ref: {
          ref: 'target', filter: { types: ['creature'] }, count: n(1), selectionId: 'line:0:5',
        } } }, condition: kicked,
        instead: [{ effect: 'damage', source: { ref: 'it' }, amount: n(10), to: { to: 'divided', among: {
          ref: 'any-target', count: { amount: 'chosen', minimum: 0 }, selectionId: 'line:0:26',
        } } }],
      }] }],
    });
    assert.deepEqual(one('Target creature phases out. If this spell was kicked, each creature target player controls phases out instead.'), {
      kind: 'spell', effects: [{ steps: [{ effect: 'self-replacement', targetPolicy: 'alternative-by-kicker',
        original: { effect: 'phase-out', what: { ref: 'target', filter: { types: ['creature'] }, count: n(1), selectionId: 'line:0:0' } }, condition: kicked,
        instead: [{ effect: 'phase-out', what: { ref: 'each', filter: { types: ['creature'], controlledBy: { player: 'target-player', selectionId: 'line:0:13' } } } }],
      }] }],
    });
  });
  it('performs the original action if the printed chooser declines an optional replacement', () => {
    // Sweep Away: declining does not leave the creature on the battlefield.
    assert.deepEqual(one("Return target creature to its owner's hand. If that creature is attacking, you may put it on top of its owner's library instead."), {
      kind: 'spell', effects: [{ steps: [{ effect: 'self-replacement', targetPolicy: 'retain-original',
        original: { effect: 'return-to-hand', what: { ref: 'target', filter: { types: ['creature'] }, count: n(1), selectionId: 'line:0:1' }, to: 'owner' },
        condition: { condition: 'status', what: { ref: 'that', filter: { types: ['creature'] } }, status: 'attacking' },
        replacementChoice: { who: { player: 'you' }, declined: 'original' },
        instead: [{ effect: 'move-zone', what: { ref: 'it' }, to: { zone: 'library', owner: 'its-owner', position: 'top' } }],
      }] }],
    });
    // Synthetic structural scope regression: one choice governs the whole
    // replacement group, while a later sentence remains an ordinary action.
    assert.deepEqual(one('Draw a card. If you control an Island, you may draw two cards and gain 2 life instead. Draw a card.'), {
      kind: 'spell', effects: [
        { steps: [{ effect: 'self-replacement', targetPolicy: 'retain-original',
          original: { effect: 'draw', who: { player: 'you' }, amount: n(1) },
          condition: { condition: 'control', who: { player: 'you' }, what: { ref: 'filter', filter: { subtypes: ['Island'] }, count: n(1) } },
          replacementChoice: { who: { player: 'you' }, declined: 'original' },
          instead: [{ effect: 'draw', who: { player: 'you' }, amount: n(2) }, { effect: 'gain-life', who: { player: 'you' }, amount: n(2) }],
        }] },
        { steps: [{ effect: 'draw', who: { player: 'you' }, amount: n(1) }] },
      ],
    });
  });
  it('does not apply casting target exceptions to unresolved sources or resolution-time decisions', () => {
    for (const text of [
      'Counter target spell. If it was kicked, counter target creature spell instead.',
      'Counter target spell. If this creature was kicked, counter target creature spell instead.',
      'Counter target spell. If you control an Island, counter target creature spell instead.',
      '{T}: Counter target spell. If this spell was kicked, counter target creature spell instead.',
      'When this creature enters, destroy target artifact. If this spell was kicked, destroy target creature instead.',
      'Counter target spell. If this spell was kicked, you may counter target creature spell instead.',
      'Draw a card. If you control an Island, draw two cards and you may gain 2 life instead.',
    ]) assert.equal(parseOracleText(text).ok, false, text);
  });
  it('uses the declared scope of delayed targets instead of skipping the whole delayed instruction', () => {
    const condition = { condition: 'control', who: { player: 'you' }, what: { ref: 'filter', filter: { subtypes: ['Island'] }, count: n(1) } };
    assert.deepEqual(one('Draw a card. If you control an Island, at the beginning of the next end step, destroy target creature instead.'), {
      kind: 'spell', effects: [{ steps: [{ effect: 'self-replacement', targetPolicy: 'retain-original',
        original: { effect: 'draw', who: { player: 'you' }, amount: n(1) }, condition,
        instead: [{ effect: 'create-delayed-trigger', trigger: { trigger: 'phase', phase: 'end-step', whose: 'each-player' },
          occurrence: 'next', nextOnly: true, targets: 'delayed-ability',
          effects: [{ steps: [{ effect: 'destroy', what: { ref: 'target', filter: { types: ['creature'] }, count: n(1), selectionId: 'line:0:20' } }] }],
        }],
      }] }],
    });
    assert.deepEqual(one('Draw a card. If this spell was kicked, destroy target creature at the beginning of the next end step instead.'), {
      kind: 'spell', effects: [{ steps: [{ effect: 'self-replacement', targetPolicy: 'alternative-by-kicker',
        original: { effect: 'draw', who: { player: 'you' }, amount: n(1) }, condition: kicked,
        instead: [{ effect: 'create-delayed-trigger', trigger: { trigger: 'phase', phase: 'end-step', whose: 'each-player' },
          occurrence: 'next', nextOnly: true, targets: 'enclosing-ability',
          effects: [{ steps: [{ effect: 'destroy', what: { ref: 'target', filter: { types: ['creature'] }, count: n(1), selectionId: 'line:0:11' } }] }],
        }],
      }] }],
    });
    for (const text of [
      'Draw a card. If you control an Island, destroy target creature at the beginning of the next end step instead.',
      'Draw a card. If you control an Island, when target creature dies, draw two cards instead.',
    ]) assert.equal(parseOracleText(text).ok, false, text);
  });
  it('rejects unproved associations and malformed or double instead markers', () => {
    for (const text of [
      'If you control a Swamp, draw two cards instead.',
      'Draw a card and gain 1 life. If you control an Island, draw two cards instead.',
      'If you control a Swamp, draw a card. If you control an Island, draw two cards instead.',
      'Draw a card. If you control an Island, instead draw two cards instead.',
      'Draw a card. If you control an Island, it deals 4 damage instead.',
      'Draw a card. If you control an Island, draw two cards instead and gain 1 life.',
    ]) assert.equal(parseOracleText(text).ok, false, text);
  });
});
