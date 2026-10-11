import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import type { Ability, Condition, Effect, ObjectFilter, Trigger } from '../src/ast.ts';
import { parseOracleText } from '../src/parser/index.ts';
import { parseCaseAbility } from '../src/parser/case-cards.ts';
import { Cursor } from '../src/parser/cursor.ts';
import { lex } from '../src/lexer.ts';
import { analyzeCardBindings } from '../src/semantic/bindings.ts';

// Actual fixtures are pinned Scryfall Oracle text. The three distinct Solved
// gates and the intervening-if solve trigger are defined by CR719.3/702.169:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed', value }) as const;
const you = { player: 'you' } as const;
const source = { ref: 'this', noun: 'case' } as const;
const instantSorcery: ObjectFilter = { allOf: [{ anyOf: [{ types: ['instant'] }, { types: ['sorcery'] }] }], cls: 'spell' };
const triggered = (trigger: Trigger, steps: Effect[], condition?: Condition): Ability => ({ kind: 'triggered', abilityWord: undefined,
  condition, trigger, effects: [{ steps }] });
function parse(text: string, provenance = false) {
  const result = parseOracleText(text, 'Case fixture', { typeLine: 'Enchantment — Case', includeSourceProvenance: provenance });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return result;
}

describe('Case keywords preserve designation and gate timing', () => {
  it('keeps solve history and the Solved cast trigger separate (Ransacked Lab)', () => {
    const text = "Instant and sorcery spells you cast cost {1} less to cast.\nTo solve — You've cast four or more instant and sorcery spells this turn. (If unsolved, solve at the beginning of your end step.)\nSolved — Whenever you cast an instant or sorcery spell, draw a card.";
    assert.deepEqual(parse(text).abilities, [
      { kind: 'static', abilityWord: undefined, effect: { static: 'cost-modification', scope: { action: 'cast', spells: { ref: 'each', filter: instantSorcery }, caster: you }, change: { direction: 'reduce', symbols: ['1'] } } },
      { kind: 'case-solve', condition: { condition: 'cast-count', who: you, what: instantSorcery, comparison: { op: 'ge', value: n(4) }, during: 'this-turn' } },
      { kind: 'case-solved', ability: triggered({ trigger: 'cast', who: you, what: instantSorcery }, [{ effect: 'draw', who: you, amount: n(1) }]) },
    ]);
  });
  it('retains a sacrificed Case cost and granted graveyard abilities (Uneaten Feast)', () => {
    const text = 'Whenever a creature you control enters, you gain 1 life.\nTo solve — You\'ve gained 5 or more life this turn.\nSolved — Sacrifice this Case: Creature cards in your graveyard gain "You may cast this card from your graveyard" until end of turn.';
    assert.deepEqual(parse(text).abilities, [
      triggered({ trigger: 'enters', what: { ref: 'filter', filter: { types: ['creature'], control: 'you' }, count: n(1) } }, [{ effect: 'gain-life', who: you, amount: n(1) }]),
      { kind: 'case-solve', condition: { condition: 'life-change', who: you, change: 'gained', comparison: { op: 'ge', value: n(5) }, during: 'this-turn' } },
      { kind: 'case-solved', ability: { kind: 'activated', abilityWord: undefined, restriction: undefined, costs: [{ cost: 'sacrifice', what: source }], effects: [{ steps: [{
        effect: 'gain-abilities', what: { ref: 'each', filter: { types: ['creature'], cls: 'card', zone: { zone: 'graveyard', owner: 'your' } } }, duration: { duration: 'end-of-turn' },
        abilities: [{ gained: 'quoted', ability: { kind: 'static', abilityWord: undefined, effect: { static: 'play-permission', permission: { who: you, action: 'cast', what: { ref: 'this', noun: 'card' }, from: { zone: 'graveyard', owner: 'your' } } } } }],
      }] }] } },
    ]);
  });
  it('keeps current state, historical events, and a conditional layer setter distinct (Gorgon\'s Kiss)', () => {
    const text = "When this Case enters, destroy up to one target creature that was dealt damage this turn.\nTo solve — Three or more creature cards were put into graveyards from anywhere this turn.\nSolved — This Case is a 4/4 Gorgon creature with deathtouch and lifelink in addition to its other types.";
    assert.deepEqual(parse(text, true).abilities, [
      triggered({ trigger: 'enters', what: source }, [{ effect: 'destroy', what: { ref: 'target', filter: { types: ['creature'], damageHistory: { role: 'recipient', window: { during: 'this-turn' } } }, count: n(1), upTo: true, selectionId: 'line:0:9' } }]),
      { kind: 'case-solve', condition: { condition: 'zone-change-history', event: 'put-into-zone', subject: { match: 'filter', filter: { types: ['creature'], cls: 'card' }, comparison: { op: 'ge', value: n(3) } }, from: 'anywhere', to: { zone: 'graveyard', owner: undefined }, during: 'this-turn' } },
      { kind: 'case-solved', ability: { kind: 'static', abilityWord: undefined, effect: { static: 'modify', what: source, condition: undefined, duration: undefined, modifiers: [
        { modifier: 'types', change: { cardTypes: { operation: 'add', types: ['creature'] }, subtypes: [{ family: 'creature', operation: 'add', types: ['Gorgon'] }] } },
        { modifier: 'base-pt', power: n(4), toughness: n(4) },
        { modifier: 'abilities', abilities: [{ gained: 'keyword', keyword: { keyword: 'deathtouch' } }, { gained: 'keyword', keyword: { keyword: 'lifelink' } }] },
      ] } } },
    ]);
  });
  it('does not turn the Solved trigger-time gate into an intervening condition', () => {
    const condition: Condition = { condition: 'control', who: you, what: { ref: 'filter', filter: { types: ['creature'] }, count: n(1) } };
    assert.deepEqual(parse('Solved — At the beginning of your upkeep, if you control a creature, draw a card.').abilities, [
      { kind: 'case-solved', ability: triggered({ trigger: 'phase', phase: 'upkeep', whose: 'your' }, [{ effect: 'draw', who: you, amount: n(1) }], condition) },
    ]);
    assert.deepEqual(parse('Solved — Flying, lifelink.').abilities, [{ kind: 'case-solved', ability: { kind: 'keywords', keywords: [{ keyword: 'flying', solvedGate: 'continuous' }, { keyword: 'lifelink', solvedGate: 'continuous' }] } }]);
    assert.equal(parseOracleText('Solved — Flying.', undefined, { typeLine: 'Artifact', layout: 'normal' }).ok, true);
  });
  it('gates each solved keyword at its intrinsic timing, retaining exact costs', () => {
    assert.deepEqual(parse('Solved — Equip {2}').abilities, [{ kind: 'case-solved', ability: { kind: 'keywords', keywords: [
      { keyword: 'equip', cost: [{ cost: 'mana', symbols: ['2'] }], solvedGate: 'activation' },
    ] } }]);
    assert.deepEqual(parse('Solved — Ward {2}').abilities, [{ kind: 'case-solved', ability: { kind: 'keywords', keywords: [
      { keyword: 'ward', cost: [{ cost: 'mana', symbols: ['2'] }], solvedGate: 'trigger-time' },
    ] } }]);
    assert.deepEqual(parse('Solved — Flying, ward—Pay 3 life.').abilities, [{ kind: 'case-solved', ability: { kind: 'keywords', keywords: [
      { keyword: 'flying', solvedGate: 'continuous' },
      { keyword: 'ward', cost: [{ cost: 'pay-life', amount: n(3) }], solvedGate: 'trigger-time' },
    ] } }]);
    // Neither cost syntax nor being a recognized keyword establishes one
    // intrinsic class. Suspend/Modular have mixed static/triggered parts.
    for (const text of ['Solved — Suspend 3—{2}{U}', 'Solved — Modular 2', 'Solved — Awaken 4—{5}{U}'])
      assert.equal(parseOracleText(text).ok, false, text);
    assert.deepEqual(parseOracleText('Equip {2}').lines[0], { text: 'Equip {2}', ok: true,
      ability: { kind: 'keywords', keywords: [{ keyword: 'equip', cost: [{ cost: 'mana', symbols: ['2'] }] }] } });
  });
  it('retains separate activation restrictions and all original target IDs through modal options', () => {
    const result = parse('To solve — You control a creature.\nSolved — {1}, Sacrifice this Case: Choose one —\n• Tap target creature.\n• Untap target creature.\nSolved — {T}: Tap target creature. Activate only as a sorcery.', true);
    const ids: string[] = [];
    function collect(value: unknown): void {
      if (!value || typeof value !== 'object') return;
      if ('selectionId' in value && typeof value.selectionId === 'string') ids.push(value.selectionId);
      Object.values(value).forEach(collect);
    }
    collect(result.abilities);
    assert.deepEqual(ids, ['line:2:2', 'line:3:2', 'line:4:5']);
    const last = result.abilities[2];
    assert.ok(last.kind === 'case-solved' && last.ability.kind === 'activated');
    assert.deepEqual(last.ability.restriction, { restriction: 'sorcery' });
  });
  it('rejects malformed, resolving, nested, or unrepresented Case keyword clauses', () => {
    for (const text of [
      'To solve — Draw a card.', 'To solve — You control a creature. Draw a card.',
      'To solve — You control target creature.', 'To solve — You control a creature and chant.',
      'To solve You control a creature.', 'Solved — Draw a card.', 'Solved — Solved — Flying.',
      'Solved — To solve — You control a creature.', 'Solved — Max speed — Flying.',
      'Solved — Whenever this Case attacks, chant.', 'Solved — {T}: Draw a card. Activate only after solving twice.',
    ]) assert.equal(parseOracleText(text).ok, false, text);
    const c = new Cursor(lex('To solve — You control a creature and chant.'));
    assert.equal(parseCaseAbility(c, () => null), null);
    assert.equal(c.pos, 0);
  });
  it('does not certify new designation scopes or targets in the gated child', () => {
    const text = 'To solve — You control a creature.\nSolved — {T}: Tap target creature.';
    const parsed = parse(text, true);
    const analysis = analyzeCardBindings({ identity: 'case-fixture', source: { name: 'Case fixture', layout: 'normal', type_line: 'Enchantment — Case' }, faces: [{ sourceFaceIndex: 0, result: parsed }] });
    assert.equal(analysis.fullyBound, false);
    assert.ok(analysis.diagnostics.some(d => d.code === 'unsupported-scope' && /Case designation/.test(d.message)));
    assert.ok(analysis.references.some(reference => reference.binding === 'unresolved'));
  });
});
