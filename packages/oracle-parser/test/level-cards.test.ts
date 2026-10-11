import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import type { Ability, Effect, StaticEffect } from '../src/ast.ts';
import { parseOracleText } from '../src/parser/index.ts';
import { parseLevelCard } from '../src/parser/level-cards.ts';
import { analyzeCardBindings } from '../src/semantic/bindings.ts';

// Oracle examples checked against the pinned Scryfall snapshot. Level
// symbols and their striations are static abilities, CR 711.2 and 711.4-6:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
// The noncreature form is confirmed by Under-Construction Skyscraper:
// https://magic.wizards.com/en/news/feature/mystery-booster-2-release-notes
const n = (value: number) => ({ amount: 'fixed', value }) as const;
const keywords = (...names: string[]): Ability => ({ kind: 'keywords', keywords: names.map(keyword => ({ keyword })) });
const levelUp = (...symbols: string[]): Ability => ({ kind: 'keywords', keywords: [{ keyword: 'level up', cost: [{ cost: 'mana', symbols }] }] });
function band(min: number, max: number | undefined, power: number | undefined, toughness: number | undefined, abilities: Ability[]): Ability {
  return { kind: 'static', effect: { static: 'level-band', range: { min, ...(max === undefined ? {} : { max }) },
    ...(power === undefined || toughness === undefined ? {} : { basePT: { power, toughness } }), abilities } };
}
const activated = (effects: Effect[], costs: Extract<Ability, { kind: 'activated' }>['costs'] = [{ cost: 'tap-self' }]): Ability => ({
  kind: 'activated', abilityWord: undefined, restriction: undefined, costs, effects: [{ steps: effects }],
});
function parse(text: string, typeLine?: string, provenance = false) {
  const result = parseOracleText(text, 'Level fixture', { typeLine, includeSourceProvenance: provenance });
  assert.ok(result.ok, JSON.stringify(result.lines));
  return result;
}
function effect(ability: Ability): Extract<StaticEffect, { static: 'level-band' }> {
  assert.equal(ability.kind, 'static');
  assert.ok(ability.kind === 'static' && ability.effect.static === 'level-band');
  return ability.effect;
}

describe('level symbols retain complete gated striations', () => {
  it('keeps level up unconditional and both exact P/T ranges copiable (Student of Warfare)', () => {
    const text = 'Level up {W} ({W}: Put a level counter on this. Level up only as a sorcery.)\nLEVEL 2-6\n3/3\nFirst strike\nLEVEL 7+\n4/4\nDouble strike';
    const result = parse(text, 'Creature — Human Knight');
    assert.deepEqual(result.abilities, [levelUp('W'), band(2, 6, 3, 3, [keywords('first strike')]), band(7, undefined, 4, 4, [keywords('double strike')])]);
    assert.deepEqual(result.lines.map(line => line.text), ['Level up {W}', 'LEVEL 2-6\n3/3\nFirst strike', 'LEVEL 7+\n4/4\nDouble strike']);
    assert.deepEqual(parse(text).abilities, result.abilities);
    assert.deepEqual(parseOracleText(text, 'Different name', { layout: 'normal', typeLine: 'Creature' }).ok, true);
  });
  it('retains empty ability sections and multiple final abilities (Coralhelm Commander)', () => {
    assert.deepEqual(parse('Level up {1}\nLEVEL 2-3\n3/3\nFlying\nLEVEL 4+\n4/4\nFlying\nOther Merfolk creatures you control get +1/+1.', 'Creature — Merfolk Soldier').abilities, [
      levelUp('1'), band(2, 3, 3, 3, [keywords('flying')]), band(4, undefined, 4, 4, [keywords('flying'),
        { kind: 'static', abilityWord: undefined, effect: { static: 'modify', condition: undefined, duration: undefined, what: { ref: 'each', filter: { other: true, subtypes: ['Merfolk'], types: ['creature'], control: 'you' } }, modifiers: [{ modifier: 'pt', power: { sign: 1, amount: n(1) }, toughness: { sign: 1, amount: n(1) } }] } },
      ]),
    ]);
    assert.deepEqual(parse('Level up {2}{G}\nLEVEL 1-3\n4/4\nLEVEL 4+\n6/6\nTrample', 'Creature').abilities,
      [levelUp('2', 'G'), band(1, 3, 4, 4, []), band(4, undefined, 6, 6, [keywords('trample')])]);
  });
  it('preserves distinct targets in their original activation lines (Brimstone Mage)', () => {
    const damage = (amount: number, id: string): Effect => ({ effect: 'damage', source: { ref: 'this', noun: 'creature' }, amount: n(amount), to: { to: 'any-target', selectionId: id } });
    assert.deepEqual(parse('Level up {3}{R}\nLEVEL 1-2\n2/3\n{T}: This creature deals 1 damage to any target.\nLEVEL 3+\n2/4\n{T}: This creature deals 3 damage to any target.', 'Creature', true).abilities, [
      levelUp('3', 'R'), band(1, 2, 2, 3, [activated([damage(1, 'line:3:9')])]), band(3, undefined, 2, 4, [activated([damage(3, 'line:6:9')])]),
    ]);
  });
  it('retains quoted grants rather than promoting them to source activations (Joraga Treespeaker)', () => {
    const mana: Effect = { effect: 'add-mana', who: { player: 'you' }, mana: { mana: 'fixed', symbols: ['G', 'G'] } };
    assert.deepEqual(parse('Level up {1}{G}\nLEVEL 1-4\n1/2\n{T}: Add {G}{G}.\nLEVEL 5+\n1/4\nElves you control have "{T}: Add {G}{G}."', 'Creature').abilities, [
      levelUp('1', 'G'), band(1, 4, 1, 2, [activated([mana])]), band(5, undefined, 1, 4, [{ kind: 'static', abilityWord: undefined,
        effect: { static: 'modify', condition: undefined, duration: undefined, what: { ref: 'each', filter: { subtypes: ['Elf'], control: 'you' } }, modifiers: [{ modifier: 'abilities', abilities: [{ gained: 'quoted', ability: activated([mana]) }] }] } }]),
    ]);
  });
  it('retains separately gated copying quantities and retargeting (Echo Mage)', () => {
    const copy = (count: number, id: string): Effect => ({ effect: 'copy-stack-object', what: { ref: 'target', filter: { allOf: [{ anyOf: [{ types: ['instant'] }, { types: ['sorcery'] }] }], cls: 'spell' }, count: n(1), selectionId: id },
      who: { player: 'you' }, count: n(count), retarget: { who: { player: 'you' }, optional: true } });
    assert.deepEqual(parse('Level up {1}{U}\nLEVEL 2-3\n2/4\n{U}{U}, {T}: Copy target instant or sorcery spell. You may choose new targets for the copy.\nLEVEL 4+\n2/5\n{U}{U}, {T}: Copy target instant or sorcery spell twice. You may choose new targets for the copies.', 'Creature', true).abilities, [
      levelUp('1', 'U'), band(2, 3, 2, 4, [activated([copy(1, 'line:3:6')], [{ cost: 'mana', symbols: ['U', 'U'] }, { cost: 'tap-self' }])]),
      band(4, undefined, 2, 5, [activated([copy(2, 'line:6:6')], [{ cost: 'mana', symbols: ['U', 'U'] }, { cost: 'tap-self' }])]),
    ]);
  });
  it('allows a noncreature leveler without inventing P/T, retaining its prefix mana ability', () => {
    const fixed = (symbols: string[]): Effect => ({ effect: 'add-mana', who: { player: 'you' }, mana: { mana: 'fixed', symbols } });
    const choice: Effect = { effect: 'add-mana', who: { player: 'you' }, mana: { mana: 'choice', options: ['W', 'B', 'G', 'C'].map(symbol => ({ mana: 'fixed', symbols: [symbol] })) } };
    const top = activated([choice]);
    assert.ok(top.kind === 'activated');
    assert.deepEqual(parse('Level up {1}\n{T}: Add {C}.\nLEVEL 1-7\n{T}: Add {W}, {B}, {G}, or {C}.\nLEVEL 8+\n{T}: Add {W}, {B}, {G}, or {C}. Scry 1.', 'Land').abilities, [
      levelUp('1'), activated([fixed(['C'])]), band(1, 7, undefined, undefined, [activated([choice])]),
      band(8, undefined, undefined, undefined, [{ ...top, effects: [...top.effects, { steps: [{ effect: 'scry', amount: n(1) }] }] }]),
    ]);
  });
  it('keeps original line offsets when a striation contains modal or dice blocks', () => {
    const result = parse('Level up {1}\nLEVEL 1-2\n2/2\n{T}: Choose one —\n• Tap target creature.\n• Untap target creature.\nLEVEL 3+\n3/3\n{T}: Roll a d20.\n1-9 | Tap target creature.\n10-20 | Untap target creature.', 'Creature', true);
    const lower = effect(result.abilities[1]).abilities[0], upper = effect(result.abilities[2]).abilities[0];
    assert.ok(lower.kind === 'activated' && upper.kind === 'activated');
    const serialized = JSON.stringify(result.abilities);
    for (const id of ['line:4:2', 'line:5:2', 'line:9:5', 'line:10:5']) assert.ok(serialized.includes(id), `${id}: ${serialized}`);
    assert.equal(lower.effects[0].steps[0].effect, 'modal');
    assert.equal(upper.effects[0].steps[0].effect, 'roll-dice');
  });
  it('treats a characteristic definition gained through a level symbol as a layer setter', () => {
    const result = parse('Level up {1}\nLEVEL 1-2\n1/1\nThis creature\'s power and toughness are each equal to the number of cards in your hand.\nLEVEL 3+\n3/3', 'Creature');
    const gained = effect(result.abilities[1]).abilities[0];
    assert.ok(gained.kind === 'static');
    const value = { amount: 'count', of: { cls: 'card', zone: { zone: 'hand', owner: 'your' } } };
    assert.deepEqual(gained.effect, { static: 'modify', what: { ref: 'this', noun: 'creature' }, modifiers: [{ modifier: 'base-pt', power: value, toughness: value }] });
  });
  it('rejects malformed structures, incomplete children, and unsupported P/T without flattening', () => {
    for (const text of [
      'LEVEL 1-2\n2/2', 'LEVEL 1+\n2/2\nLEVEL 3+\n3/3', 'LEVEL 2-1\n2/2\nLEVEL 3+\n3/3',
      'LEVEL 1-3\n2/2\nLEVEL 3+\n3/3', 'LEVEL 1-2\n2/2\nLEVEL 3-4\n3/3',
      'LEVEL 1-2\n2/2\nLEVEL 3+\n3/3\nLEVEL 4+\n4/4', 'LEVEL two-three\n2/2\nLEVEL 4+\n3/3',
      'LEVEL 1-2\n*/2\nLEVEL 3+\n3/3', 'LEVEL 1-2\n2/X\nLEVEL 3+\n3/3',
      'LEVEL 1-2\n2/2\n4/4\nLEVEL 3+\n3/3', 'LEVEL 1-2\nFlying\nLEVEL 3+\n3/3',
      'LEVEL 1-2\n2/2\nFlying\nLEVEL 3+\n3/3\nUnknown rules.',
      'LEVEL 1-2\n2/2\nDraw a card.\nLEVEL 3+\n3/3',
      'Draw a card.\nLEVEL 1-2\n2/2\nLEVEL 3+\n3/3',
      'LEVEL 1-2\n2/2\nLEVEL 3+\n3/3\n{T}: Choose one —\n• Invent a mechanic.',
    ]) assert.equal(parseOracleText(text).ok, false, text);
    const noStats = 'LEVEL 1-2\n{T}: Add {C}.\nLEVEL 3+\n{T}: Add {W}.';
    for (const typeLine of [undefined, 'Creature', 'Artifact Creature', 'Sorcery'])
      assert.equal(parseOracleText(noStats, undefined, { typeLine }).ok, false, String(typeLine));
    assert.equal(parseOracleText('LEVEL 1-2\nLEVEL 3+', undefined, { typeLine: 'Land' }).ok, false);
    assert.equal(parseLevelCard(['Level up {1}'], () => []), null);
  });
  it('reports nested parsing errors without making partial striation abilities unconditional', () => {
    const result = parseOracleText('Level up {1}\nLEVEL 1-2\n2/2\n{T}: Invent a mechanic.\nLEVEL 3+\n3/3\nFlying', undefined, { typeLine: 'Creature' });
    assert.equal(result.ok, false);
    assert.equal(result.lines.length, 3);
    const failed = result.lines[1];
    assert.ok(!failed.ok && failed.error.includes('level striation:') && failed.text.includes('Invent a mechanic.'));
  });
  it('does not certify continuous range evaluation or nested ability references as bound', () => {
    const result = parse('Level up {1}\nLEVEL 1-2\n2/2\n{T}: Tap target creature.\nLEVEL 3+\n3/3\n{T}: Untap target creature.', 'Creature', true);
    const graph = analyzeCardBindings({ identity: 'level-fixture', source: { name: 'Level fixture', layout: 'normal', type_line: 'Creature' }, faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(graph.fullyBound, false);
    assert.equal(graph.diagnostics.filter(diagnostic => diagnostic.message.startsWith('A level symbol') && diagnostic.path.endsWith('/effect')).length, 2);
    assert.equal(graph.references.filter(reference => reference.expression && 'ref' in reference.expression && reference.expression.ref === 'target' && reference.binding === 'unresolved').length, 2);
  });
});
