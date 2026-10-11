import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';
import { lex } from '../src/lexer.ts';
import { Cursor } from '../src/parser/cursor.ts';
import { parseSentences } from '../src/parser/effects.ts';

// Frozen Oracle corpus; replacement semantics checked against CR614.10.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed', value });
const you = { player: 'you' };
const source = { ref: 'this', noun: 'creature' };
function abilities(text: string, name?: string, typeLine?: string) {
  const result = parseOracleText(text, name, { typeLine, includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}

describe('skipping complete temporal occurrences', () => {
  it('keeps Eater of Days two skipped turns and Wormfang Manta separate entry/leave instructions', () => {
    assert.deepEqual(abilities('Flying, trample\nWhen this creature enters, you skip your next two turns.', 'Eater of Days', 'Artifact Creature — Leviathan'), [
      { kind: 'keywords', keywords: [{ keyword: 'flying' }, { keyword: 'trample' }] },
      { kind: 'triggered', trigger: { trigger: 'enters', what: source }, effects: [{ steps: [{ effect: 'skip-time', who: you, what: { kind: 'turn' }, occurrences: { kind: 'next', count: n(2) } }] }] },
    ]);
    assert.deepEqual(abilities('Flying\nWhen this creature enters, you skip your next turn.\nWhen this creature leaves the battlefield, you take an extra turn after this one.', 'Wormfang Manta', 'Creature — Nightmare Fish Beast'), [
      { kind: 'keywords', keywords: [{ keyword: 'flying' }] },
      { kind: 'triggered', trigger: { trigger: 'enters', what: source }, effects: [{ steps: [{ effect: 'skip-time', who: you, what: { kind: 'turn' }, occurrences: { kind: 'next', count: n(1) } }] }] },
      { kind: 'triggered', trigger: { trigger: 'leaves', what: source }, effects: [{ steps: [{ effect: 'add-turns', who: you, count: n(1), after: { turn: 'current' } }] }] },
    ]);
  });
  it('retains Fatigue entire draw step and Moment of Silence expiry instead of replacing a draw or ending a phase', () => {
    const who = { player: 'target-player', selectionId: 'line:0:0' };
    assert.deepEqual(abilities('Target player skips their next draw step.', 'Fatigue', 'Sorcery'), [{ kind: 'spell', effects: [{ steps: [{
      effect: 'skip-time', who, what: { kind: 'step', step: 'draw' }, occurrences: { kind: 'next', count: n(1) },
    }] }] }]);
    assert.deepEqual(abilities('Target player skips their next combat phase this turn.', 'Moment of Silence', 'Instant'), [{ kind: 'spell', effects: [{ steps: [{
      effect: 'skip-time', who, what: { kind: 'phase', phase: 'combat' }, occurrences: { kind: 'next', count: n(1) }, duration: { duration: 'this-turn' },
    }] }] }]);
  });
  it('keeps all phases of a particular future turn distinct from only the next matching phase', () => {
    const who = { player: 'target-player', selectionId: 'line:0:0' };
    assert.deepEqual(abilities('Target player skips all combat phases of their next turn.', 'False Peace', 'Sorcery'), [{ kind: 'spell', effects: [{ steps: [{
      effect: 'skip-time', who, what: { kind: 'phase', phase: 'combat' }, occurrences: { kind: 'all' }, during: { turn: 'next', whose: who },
    }] }] }]);
    assert.deepEqual(abilities('Target player skips their next combat phase.'), [{ kind: 'spell', effects: [{ steps: [{
      effect: 'skip-time', who, what: { kind: 'phase', phase: 'combat' }, occurrences: { kind: 'next', count: n(1) },
    }] }] }]);
  });
  it('models Stasis as a recurring static replacement with its complete upkeep cost', () => {
    assert.deepEqual(abilities('Players skip their untap steps.\nAt the beginning of your upkeep, sacrifice this enchantment unless you pay {U}.', 'Stasis', 'Enchantment'), [
      { kind: 'static', effect: { static: 'skip-time', who: { player: 'each-player' }, what: { kind: 'step', step: 'untap' } } },
      { kind: 'triggered', trigger: { trigger: 'phase', phase: 'upkeep', whose: 'your' }, effects: [{
        steps: [{ effect: 'sacrifice', who: you, what: { ref: 'this', noun: 'enchantment' } }],
        condition: { condition: 'pays', who: you, cost: [{ cost: 'mana', symbols: ['U'] }] }, conditionKind: 'unless',
      }] },
    ]);
    assert.deepEqual(abilities('Hellbent — Skip your upkeep step if you have no cards in hand.', undefined, 'Enchantment'), [{
      kind: 'static', abilityWord: 'Hellbent', effect: { static: 'conditional', condition: { condition: 'cards-in-hand', who: you, comparison: { op: 'eq', value: n(0) } },
        effect: { static: 'skip-time', who: you, what: { kind: 'step', step: 'upkeep' } } },
    }]);
  });
  it('backtracks incomplete skips and rejects unsupported future-turn anaphors or mismatched owners', () => {
    for (const text of ['Skip your next.', 'Skip your next two.', 'Target player skips your next turn.', 'Skip their next turn.',
      'Skip your draw step.', 'Skip that draw.', 'Skip the untap step of that turn.', 'Skip your next turn unexpectedly.']) {
      const c = new Cursor(lex(text));
      assert.equal(parseSentences(c, { modalOptions: [] }), null, text);
      assert.equal(c.pos, 0, text);
    }
    for (const text of ['Take an extra turn after this one. Skip the untap step of that turn.', 'Players skip their next untap step.',
      'Skip your draw step and lose 1 life.', 'Skip your upkeep step if.']) {
      const result = parseOracleText(text, undefined, { typeLine: 'Enchantment' });
      assert.equal(result.ok, false, text);
    }
  });
});
