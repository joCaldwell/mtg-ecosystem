import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import type { Effect, GainedAbility, ObjectRef, ResolvingCharacteristicModifier } from '../src/ast.ts';
import { parseOracleText } from '../src/parser/index.ts';
import { parseResolvingCharacteristics } from '../src/parser/resolving-characteristics.ts';
import { parseGainedAbilities } from '../src/parser/effects.ts';
import { Cursor } from '../src/parser/cursor.ts';
import { lex } from '../src/lexer.ts';
import { analyzeCardBindings } from '../src/semantic/bindings.ts';

// Pinned Oracle clauses: the animation changes are one continuous effect,
// not independent selections or copiable definitions. CR205.1a/b,611.2c/d,
// 604.3a and613.6: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed', value }) as const;
const end = { duration: 'end-of-turn' } as const;
const land = { ref: 'this', noun: 'land' } as const;
const artifact = { ref: 'this', noun: 'artifact' } as const;
const pt = (p: number, t: number): ResolvingCharacteristicModifier => ({ modifier: 'base-pt', power: n(p), toughness: n(t) });
const grant = (...keywords: string[]): ResolvingCharacteristicModifier => ({ modifier: 'abilities', abilities: keywords.map(keyword => ({ gained: 'keyword', keyword: { keyword } })) });
function parse(text: string) {
  const result = parseOracleText(text, 'Fixture', { includeSourceProvenance: true });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return result;
}
function activation(cost: string[], effect: Effect) {
  return { kind: 'activated', abilityWord: undefined, restriction: undefined,
    costs: [{ cost: 'mana', symbols: cost }], effects: [{ steps: [effect] }] };
}
function change(what: ObjectRef, modifiers: ResolvingCharacteristicModifier[], duration?: Effect['duration']): Effect {
  return { effect: 'modify-characteristics', what, modifiers, snapshot: 'effect-created', ...(duration ? { duration } : {}) };
}

describe('resolving characteristic changes preserve layers, choices, and snapshots', () => {
  it('retains all prior types for a still-land animation, but sets its printed color (Faerie Conclave)', () => {
    assert.deepEqual(parse("{1}{U}: This land becomes a 2/1 blue Faerie creature with flying until end of turn. It's still a land.").abilities, [activation(['1', 'U'], change(land, [
      { modifier: 'types', change: { cardTypes: { operation: 'add', types: ['creature'] }, subtypes: [{ family: 'creature', operation: 'add', types: ['Faerie'] }] } },
      { modifier: 'colors', operation: 'set', colors: ['blue'] }, pt(2, 1), grant('flying'),
    ], end))]);
  });
  it('artifact-creature wording retains other type families but replaces creature types (Silumgar Monument)', () => {
    assert.deepEqual(parse('{4}{U}{B}: This artifact becomes a 4/4 blue and black Dragon artifact creature with flying until end of turn.').abilities, [activation(['4', 'U', 'B'], change(artifact, [
      { modifier: 'types', change: { cardTypes: { operation: 'add', types: ['artifact', 'creature'] }, subtypes: [{ family: 'creature', operation: 'set', types: ['Dragon'] }] } },
      { modifier: 'colors', operation: 'set', colors: ['blue', 'black'] }, pt(4, 4), grant('flying'),
    ], end))]);
    assert.deepEqual(parse("{1}: This land becomes a 1/1 Blinkmoth artifact creature with flying until end of turn. It's still a land.").abilities, [activation(['1'], change(land, [
      { modifier: 'types', change: { cardTypes: { operation: 'add', types: ['artifact', 'creature'] }, subtypes: [{ family: 'creature', operation: 'add', types: ['Blinkmoth'] }] } }, pt(1, 1), grant('flying'),
    ], end))]);
  });
  it('adds colors only when printed, preserving a target and leading duration (Halsin)', () => {
    const target: ObjectRef = { ref: 'target', filter: { cls: 'token', control: 'you' }, count: n(1), selectionId: 'line:0:7' };
    assert.deepEqual(parse('{1}: Until end of turn, target token you control becomes a green Bear creature with base power and toughness 4/4 in addition to its other colors and types.').abilities, [activation(['1'], change(target, [
      { modifier: 'types', change: { cardTypes: { operation: 'add', types: ['creature'] }, subtypes: [{ family: 'creature', operation: 'add', types: ['Bear'] }] } },
      { modifier: 'colors', operation: 'add', colors: ['green'] }, pt(4, 4),
    ], end))]);
    assert.deepEqual(parse('{2}: This creature becomes colorless until end of turn.').abilities, [activation(['2'], change({ ref: 'this', noun: 'creature' }, [{ modifier: 'colors', operation: 'set', colors: [] }], end))]);
  });
  it('snapshots direct per-object P/T values, not a live CDA (Karn)', () => {
    const value = { amount: 'attribute', of: { ref: 'it' }, attribute: 'mana-value' } as const;
    assert.deepEqual(parse('{1}: Target noncreature artifact becomes an artifact creature with power and toughness each equal to its mana value until end of turn.').abilities, [activation(['1'], change(
      { ref: 'target', filter: { nonTypes: ['creature'], types: ['artifact'] }, count: n(1), selectionId: 'line:0:2' }, [
        { modifier: 'types', change: { cardTypes: { operation: 'add', types: ['artifact', 'creature'] } } },
        { modifier: 'base-pt', power: value, toughness: value, valueScope: 'each-affected-object' },
      ], end))]);
  });
  it('granted self-calculation remains a dynamic ordinary layer7b ability (Chimeric Mass)', () => {
    const value = { amount: 'counter-count', counter: { type: 'charge' }, on: { ref: 'it' } } as const;
    const gained: GainedAbility = { gained: 'quoted', ability: { kind: 'static', abilityWord: undefined,
      effect: { static: 'modify', what: { ref: 'this', noun: 'creature' }, modifiers: [{ modifier: 'base-pt', power: value, toughness: value }] } } };
    assert.deepEqual(parse('{1}: Until end of turn, this artifact becomes a Construct artifact creature with "This creature\'s power and toughness are each equal to the number of charge counters on it."').abilities, [activation(['1'], change(artifact, [
      { modifier: 'types', change: { cardTypes: { operation: 'add', types: ['artifact', 'creature'] }, subtypes: [{ family: 'creature', operation: 'set', types: ['Construct'] }] } },
      { modifier: 'abilities', abilities: [gained] },
    ], end))]);
  });
  it('keeps quoted ability duration distinct from the granting effect and retains original nested target IDs', () => {
    const text = '{2}: Until end of turn, this land becomes a 2/2 creature with "{T}: Tap target creature." It\'s still a land.';
    assert.deepEqual(parse(text).abilities, [activation(['2'], change(land, [
      { modifier: 'types', change: { cardTypes: { operation: 'add', types: ['creature'] } } }, pt(2, 2),
      { modifier: 'abilities', abilities: [{ gained: 'quoted', ability: { kind: 'activated', abilityWord: undefined, restriction: undefined,
        costs: [{ cost: 'tap-self' }], effects: [{ steps: [{ effect: 'tap', what: { ref: 'target', filter: { types: ['creature'] }, count: n(1), selectionId: 'line:0:20' } }] }] } }] },
    ], end))]);
  });
  it('retains all creature types and a shared plural affected set (Mutavault/Sylvan Awakening)', () => {
    assert.deepEqual(parse("{1}: This land becomes a 2/2 creature with all creature types until end of turn. It's still a land.").abilities, [activation(['1'], change(land, [
      { modifier: 'types', change: { cardTypes: { operation: 'add', types: ['creature'] }, subtypes: [{ family: 'creature', operation: 'add', types: { all: true } }] } }, pt(2, 2),
    ], end))]);
    assert.deepEqual(parse("Until your next turn, all lands you control become 2/2 Elemental creatures with reach, indestructible, and haste. They're still lands.").abilities, [{ kind: 'spell', abilityWord: undefined, effects: [{ steps: [change(
      { ref: 'all', filter: { types: ['land'], control: 'you' } }, [
        { modifier: 'types', change: { cardTypes: { operation: 'add', types: ['creature'] }, subtypes: [{ family: 'creature', operation: 'add', types: ['Elemental'] }] } }, pt(2, 2), grant('reach', 'indestructible', 'haste'),
      ], { duration: 'your-next-turn' })] }] }]);
  });
  it('does not reset the preceding printed actor after an intervening object change', () => {
    const player = { player: 'target-opponent', selectionId: 'line:0:0' } as const;
    assert.deepEqual(parse('Target opponent sacrifices a creature, then target land becomes a 3/3 creature with haste until end of turn, then draws a card.').abilities, [{ kind: 'spell', abilityWord: undefined, effects: [{ steps: [
      { effect: 'sacrifice', who: player, what: { ref: 'filter', filter: { types: ['creature'] }, count: n(1) } },
      change({ ref: 'target', filter: { types: ['land'] }, count: n(1), selectionId: 'line:0:7' }, [
        { modifier: 'types', change: { cardTypes: { operation: 'set', types: ['creature'] } } }, pt(3, 3), grant('haste'),
      ], end),
      { effect: 'draw', who: player, amount: n(1) },
    ] }] }]);
  });
  it('preserves old simple descriptions and rejects unsupported/malformed tails', () => {
    assert.deepEqual(parse('Target creature becomes a 4/4 green Bear creature until end of turn.').abilities, [{ kind: 'spell', abilityWord: undefined, effects: [{ steps: [{ effect: 'become',
      what: { ref: 'target', filter: { types: ['creature'] }, count: n(1), selectionId: 'line:0:0' }, spec: { types: ['creature'], subtypes: ['Bear'], colors: ['green'], power: n(4), toughness: n(4) }, duration: end,
    }] }] }]);
    for (const text of [
      'Target land becomes a 3/3 creature with flying until end of turn. It is still a blorple.',
      'Target land becomes a green creature with haste in addition to its other colors and properties.',
      'Target land becomes a colorless green creature with haste.',
      'Target land becomes a 2/2 creature with flying and nonsense until end of turn.',
      'Each artifact becomes a creature with flying. This effect continues after its source leaves.',
    ]) assert.equal(parseOracleText(text).ok, false, text);
    const c = new Cursor(lex('target land becomes a 2/2 creature with nonsense'));
    assert.equal(parseResolvingCharacteristics(c, undefined, parseGainedAbilities), null);
    assert.equal(c.pos, 0);
  });
  it('binds a characteristic change and leaves its layers to the engine', () => {
    const result = parse('Target land becomes a 3/3 creature with haste until end of turn.');
    const binding = analyzeCardBindings({ identity: 'resolving-characteristics-fixture', source: { name: 'Fixture', layout: 'normal', type_line: 'Instant' }, faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(binding.fullyBound, false);
    assert.equal(binding.referencesBound, true, JSON.stringify(binding.diagnostics));
    assert.ok(binding.engineFeatures.some(need => need.feature === 'characteristic-layers'));
    assert.ok(binding.references.some(r => 'ref' in r.expression && r.expression.ref === 'target' && r.binding === 'bound'));
  });
  it('keeps existing object effects outside the new snapshot barrier', () => {
    for (const text of [
      'Destroy target creature.',
      'Counter target spell.',
      'Target creature gets +1/+1 until end of turn. Tap it.',
      'Target creature gains flying until end of turn. Tap it.',
    ]) {
      const result = parse(text);
      const binding = analyzeCardBindings({ identity: 'existing-effect-fixture', source: { name: 'Fixture', layout: 'normal', type_line: 'Instant' }, faces: [{ sourceFaceIndex: 0, result }] });
      // A granted keyword binds in the recipient's scope; the target remains the later tap's antecedent.
      assert.equal(binding.fullyBound, true, text + ': ' + JSON.stringify(binding.diagnostics));
      assert.ok(!binding.diagnostics.some(d => /affected-object snapshot/.test(d.message)), text);
      assert.ok(binding.references.every(r => r.binding === 'bound'), text);
    }
  });
});
