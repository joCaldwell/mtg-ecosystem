import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';
import { lex } from '../src/lexer.ts';
import { Cursor } from '../src/parser/cursor.ts';
import { parseSentences } from '../src/parser/effects.ts';

// Oracle text checked in the frozen corpus; CR500.7-8,505.1b,724.1-2.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed', value });
const you = { player: 'you' };
function abilities(text: string, name?: string, typeLine?: string) {
  const result = parseOracleText(text, name, { typeLine, includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return JSON.parse(JSON.stringify(result.abilities));
}

describe('turn and phase creation and termination', () => {
  it('keeps Time Warp and Time Stretch counts and their single original target', () => {
    for (const [text, count] of [['an extra turn', 1], ['two extra turns', 2]] as const)
      assert.deepEqual(abilities(`Target player takes ${text} after this one.`), [{ kind: 'spell', effects: [{ steps: [{
        effect: 'add-turns', who: { player: 'target-player', selectionId: 'line:0:0' }, count: n(count), after: { turn: 'current' },
      }] }] }]);
    assert.deepEqual(abilities('Take an extra turn after this one.'), [{ kind: 'spell', effects: [{ steps: [{
      effect: 'add-turns', who: you, count: n(1), after: { turn: 'current' },
    }] }] }]);
  });
  it('retains Aggravated Assault ordered combat/main insertion, immediate untap, and activation timing', () => {
    assert.deepEqual(abilities('{3}{R}{R}: Untap all creatures you control. After this main phase, there is an additional combat phase followed by an additional main phase. Activate only as a sorcery.', 'Aggravated Assault', 'Enchantment'), [{
      kind: 'activated', costs: [{ cost: 'mana', symbols: ['3', 'R', 'R'] }], effects: [
        { steps: [{ effect: 'untap', what: { ref: 'all', filter: { types: ['creature'], control: 'you' } } }] },
        { steps: [{ effect: 'add-phases', after: { phase: 'current', kind: 'main' }, sequence: [{ phase: 'combat', count: n(1) }, { phase: 'main', count: n(1) }] }] },
      ], restriction: { restriction: 'sorcery' },
    }]);
  });
  it('preserves Aurelia entire trigger and its once-per-turn attacking occurrence', () => {
    assert.deepEqual(abilities('Flying, vigilance, haste\nWhenever Aurelia attacks for the first time each turn, untap all creatures you control. After this phase, there is an additional combat phase.', 'Aurelia, the Warleader', 'Legendary Creature — Angel'), [
      { kind: 'keywords', keywords: [{ keyword: 'flying' }, { keyword: 'vigilance' }, { keyword: 'haste' }] },
      { kind: 'triggered', trigger: { trigger: 'attacks', what: { ref: 'self' }, occurrence: { ordinal: 1, period: 'turn' } }, effects: [
        { steps: [{ effect: 'untap', what: { ref: 'all', filter: { types: ['creature'], control: 'you' } } }] },
        { steps: [{ effect: 'add-phases', after: { phase: 'current' }, sequence: [{ phase: 'combat', count: n(1) }] }] },
      ] },
    ]);
  });
  it('keeps ending combat distinct from ending the turn, including Sundial timing', () => {
    assert.deepEqual(abilities('End the turn.'), [{ kind: 'spell', effects: [{ steps: [{ effect: 'end-turn' }] }] }]);
    assert.deepEqual(abilities('End the combat phase.'), [{ kind: 'spell', effects: [{ steps: [{ effect: 'end-combat' }] }] }]);
    assert.deepEqual(abilities('{1}, {T}: End the turn. Activate only during your turn.', 'Sundial of the Infinite', 'Artifact'), [{
      kind: 'activated', costs: [{ cost: 'mana', symbols: ['1'] }, { cost: 'tap-self' }],
      effects: [{ steps: [{ effect: 'end-turn' }] }], restriction: { restriction: 'timing', turn: 'your' },
    }]);
  });
  it('retains insertion counts, anchor ordinals, and current phase conditions', () => {
    assert.deepEqual(abilities("After the second main phase this turn, there's an additional combat phase followed by an additional main phase."), [{ kind: 'spell', effects: [{ steps: [{
      effect: 'add-phases', after: { phase: 'ordinal', kind: 'main', ordinal: 2, turn: 'current' }, sequence: [{ phase: 'combat', count: n(1) }, { phase: 'main', count: n(1) }],
    }] }] }]);
    assert.deepEqual(abilities("If it's the first combat phase of your turn, there are two additional combat phases after this phase."), [{ kind: 'spell', effects: [{
      condition: { condition: 'current-phase', phase: 'combat', whose: you, ordinal: 1 }, conditionKind: 'if',
      steps: [{ effect: 'add-phases', after: { phase: 'current' }, sequence: [{ phase: 'combat', count: n(2) }] }],
    }] }]);
  });
  it('rolls back incomplete anchors and rejects unrepresented turn-linked riders', () => {
    for (const text of [
      'Target player takes an extra turn.',
      'Take an extra turn after that one.',
      'After this main phase, there is an additional.',
      'There is an additional combat phase after.',
      'After this phase, there is an additional combat phase followed by.',
      'End the main phase.',
    ]) {
      const c = new Cursor(lex(text));
      assert.equal(parseSentences(c, { modalOptions: [] }), null, text);
      assert.equal(c.pos, 0, text);
    }
    assert.equal(parseOracleText("Take an extra turn after this one. During that turn, damage can't be prevented.").ok, false);
  });
  it('retains Full Throttle recurring delayed untaps and attack history distinct from current attacking status', () => {
    const historical = { ref: 'all', filter: { types: ['creature'], combatHistory: { action: 'attacked', during: 'this-turn' } } };
    assert.deepEqual(abilities('After this main phase, there are two additional combat phases.\nAt the beginning of each combat this turn, untap all creatures that attacked this turn.', 'Full Throttle', 'Sorcery'), [
      { kind: 'spell', effects: [{ steps: [{ effect: 'add-phases', after: { phase: 'current', kind: 'main' }, sequence: [{ phase: 'combat', count: n(2) }] }] }] },
      { kind: 'spell', effects: [{ steps: [{ effect: 'create-delayed-trigger', trigger: { trigger: 'phase', phase: 'combat', whose: 'each-player' },
        occurrence: 'each', duration: { duration: 'this-turn' }, effects: [{ steps: [{ effect: 'untap', what: historical }] }], targets: 'delayed-ability',
      }] }] },
    ]);
    assert.deepEqual(abilities('Untap all creatures that attacked this turn. After this main phase, there is an additional combat phase followed by an additional main phase.', 'Relentless Assault', 'Sorcery'), [{
      kind: 'spell', effects: [{ steps: [{ effect: 'untap', what: historical }] }, { steps: [{ effect: 'add-phases', after: { phase: 'current', kind: 'main' },
        sequence: [{ phase: 'combat', count: n(1) }, { phase: 'main', count: n(1) }],
      }] }],
    }]);
    assert.deepEqual(abilities('Untap all attacking creatures.'), [{ kind: 'spell', effects: [{ steps: [{ effect: 'untap', what: { ref: 'all', filter: { status: ['attacking'], types: ['creature'] } } }] }] }]);
    for (const text of ['Untap all creatures that attacked.', 'Untap all creatures that attacked last turn.', 'Untap all creatures that attacked this turn that attacked this combat.'])
      assert.equal(parseOracleText(text).ok, false, text);
  });
});
