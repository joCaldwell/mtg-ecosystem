import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import type { Ability, Effect, Trigger } from '../src/ast.ts';
import { parseOracleText } from '../src/parser/index.ts';
import { parseClassCard } from '../src/parser/level-cards.ts';
import { parseTrigger } from '../src/parser/triggers.ts';
import { Cursor } from '../src/parser/cursor.ts';
import { lex } from '../src/lexer.ts';
import { analyzeCardBindings } from '../src/semantic/bindings.ts';

// Source: pinned Scryfall Oracle text for Stormchaser's Talent and Ranger
// Class. CR716.2/3 defines Class bars' paired activation + static grant and
// explicitly distinguishes their designation from level counters:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed', value }) as const;
const you = { player: 'you' } as const;
const self = { ref: 'this', noun: 'class' } as const;
const mana = (...symbols: string[]) => ({ cost: 'mana', symbols }) as const;
const triggered = (trigger: Trigger, steps: Effect[]): Ability => ({ kind: 'triggered', abilityWord: undefined,
  condition: undefined, trigger, effects: [{ steps }] });
const bar = (level: number, symbols: string[], abilities: Ability[]): Ability => ({ kind: 'class-level-bar', level,
  costs: [mana(...symbols)], abilities });
function parse(text: string, provenance = false) {
  const parsed = parseOracleText(text, 'Class fixture', { typeLine: 'Enchantment — Class', includeSourceProvenance: provenance });
  assert.ok(parsed.ok, JSON.stringify(parsed.lines));
  return parsed;
}

describe('Class bars preserve upgrades and cumulative ability grants', () => {
  it('distinguishes entering, becoming level 2, and a level 3 cast trigger (Stormchaser\'s Talent)', () => {
    const text = '(Gain the next level as a sorcery to add its ability.)\nWhen this Class enters, create a 1/1 blue and red Otter creature token with prowess.\n{3}{U}: Level 2\nWhen this Class becomes level 2, return target instant or sorcery card from your graveyard to your hand.\n{5}{U}: Level 3\nWhenever you cast an instant or sorcery spell, create a 1/1 blue and red Otter creature token with prowess.';
    const otter: Effect = { effect: 'create-token', count: n(1), token: { colors: ['blue', 'red'], supertypes: [], types: ['creature'], subtypes: ['Otter'], keywords: [{ keyword: 'prowess' }], power: n(1), toughness: n(1) } };
    const union = { allOf: [{ anyOf: [{ types: ['instant'] }, { types: ['sorcery'] }] }] };
    assert.deepEqual(parse(text, true).abilities, [
      triggered({ trigger: 'enters', what: self }, [otter]),
      bar(2, ['3', 'U'], [triggered({ trigger: 'becomes-class-level', what: self, level: 2 }, [{ effect: 'return-to-hand', to: 'your', what: {
        ref: 'target', filter: { ...union, cls: 'card', zone: { zone: 'graveyard', owner: 'your' } }, count: n(1), selectionId: 'line:2:8',
      } }])]),
      bar(3, ['5', 'U'], [triggered({ trigger: 'cast', who: you, what: { ...union, cls: 'spell' } }, [otter])]),
    ]);
    assert.equal(parse(text).lines.length, 3);
    assert.equal(parseOracleText(text, undefined, { layout: 'normal', typeLine: 'Enchantment' }).ok, true);
  });
  it('retains two distinct static permissions in the level 3 section (Ranger Class)', () => {
    const text = 'When this Class enters, create a 2/2 green Wolf creature token.\n{1}{G}: Level 2\nWhenever you attack, put a +1/+1 counter on target attacking creature.\n{3}{G}: Level 3\nYou may look at the top card of your library any time.\nYou may cast creature spells from the top of your library.';
    assert.deepEqual(parse(text, true).abilities, [
      triggered({ trigger: 'enters', what: self }, [{ effect: 'create-token', count: n(1), token: { colors: ['green'], supertypes: [], types: ['creature'], subtypes: ['Wolf'], keywords: [], power: n(2), toughness: n(2) } }]),
      bar(2, ['1', 'G'], [triggered({ trigger: 'you-attack' }, [{ effect: 'put-counters', on: { ref: 'target', filter: { status: ['attacking'], types: ['creature'] }, count: n(1), selectionId: 'line:2:13' }, counter: { type: '+1/+1' }, count: n(1) }])]),
      bar(3, ['3', 'G'], [
        { kind: 'static', abilityWord: undefined, effect: { static: 'look-permission', permission: { who: you, what: [{ ref: 'library-cards', position: 'top', count: n(1), of: { zone: 'library', owner: 'your' } }], timing: 'any-time', topLibraryChanges: 'after-casting-activation-or-special-action' } } },
        { kind: 'static', abilityWord: undefined, effect: { static: 'play-permission', permission: { who: you, action: 'cast', what: { ref: 'each', filter: { types: ['creature'], cls: 'spell' } }, from: { zone: 'library', owner: 'your', position: 'top' } } } },
      ]),
    ]);
  });
  it('keeps a granted activation separate from the level-bar activation and its own restrictions', () => {
    assert.deepEqual(parse('{1}{U}: Level 2\n{T}: Draw a card. Activate only once each turn.\n{2}{U}: Level 3\nCreatures you control have flying.').abilities, [
      bar(2, ['1', 'U'], [{ kind: 'activated', abilityWord: undefined, costs: [{ cost: 'tap-self' }], effects: [{ steps: [{ effect: 'draw', who: you, amount: n(1) }] }], restriction: { restriction: 'once-each-turn' } }]),
      bar(3, ['2', 'U'], [{ kind: 'static', abilityWord: undefined, effect: { static: 'modify', what: { ref: 'each', filter: { types: ['creature'], control: 'you' } }, condition: undefined, duration: undefined, modifiers: [{ modifier: 'abilities', abilities: [{ gained: 'keyword', keyword: { keyword: 'flying' } }] }] } }]),
    ]);
  });
  it('does not invent mana-only costs or drop a second printed payment', () => {
    assert.deepEqual(parse('{G}, Sacrifice a creature: Level 2\nCreatures you control have trample.\n{2}{G}: Level 3\nWhen this Class becomes level 3, draw a card.').abilities[0], {
      kind: 'class-level-bar', level: 2, costs: [mana('G'), { cost: 'sacrifice', what: { ref: 'filter', filter: { types: ['creature'] }, count: n(1) } }], abilities: [
        { kind: 'static', abilityWord: undefined, effect: { static: 'modify', what: { ref: 'each', filter: { types: ['creature'], control: 'you' } }, condition: undefined, duration: undefined, modifiers: [{ modifier: 'abilities', abilities: [{ gained: 'keyword', keyword: { keyword: 'trample' } }] }] } },
      ],
    });
  });
  it('preserves global line provenance through nested modal blocks', () => {
    const result = parse('{U}: Level 2\n{T}: Choose one —\n• Tap target creature.\n• Untap target creature.\n{1}{U}: Level 3\nWhen this Class becomes level 3, tap target creature.', true);
    const lower = result.abilities[0], upper = result.abilities[1];
    assert.ok(lower.kind === 'class-level-bar' && upper.kind === 'class-level-bar');
    const ids: string[] = [];
    function collect(value: unknown): void {
      if (!value || typeof value !== 'object') return;
      if ('selectionId' in value && typeof value.selectionId === 'string') ids.push(value.selectionId);
      Object.values(value).forEach(collect);
    }
    collect(result.abilities);
    assert.deepEqual(ids, ['line:2:2', 'line:3:2', 'line:5:8']);
  });
  it('parses class-level transitions independently and backtracks unsupported transition bounds', () => {
    const c = new Cursor(lex('When this Class becomes level 3'));
    assert.deepEqual(parseTrigger(c), { trigger: 'becomes-class-level', what: self, level: 3 });
    assert.ok(c.done());
    for (const text of ['When this Class becomes level X', 'When this Class becomes level 0', 'When this Class becomes level']) {
      const c = new Cursor(lex(text));
      assert.equal(parseTrigger(c), null, text);
      assert.equal(c.pos, 0);
    }
    assert.equal(parseOracleText('When this Class becomes level 3 or greater, draw a card.').ok, false);
    const entered = parse('When this Class enters, draw a card.').abilities[0];
    assert.ok(entered.kind === 'triggered');
    assert.equal(entered.trigger.trigger, 'enters');
  });
  it('rejects malformed bars and incomplete scope instead of treating them as activations', () => {
    for (const text of [
      '{U}: Level 2\nFlying', '{U}: Level 3\nFlying\n{U}: Level 2\nFlying',
      '{U}: Level 2\nFlying\n{U}: Level 2\nFlying', '{U}: Level 1\nFlying\n{U}: Level 3\nFlying',
      '{U}: Level X\nFlying\n{U}: Level 3\nFlying', '{U}: Level 2 more\nFlying\n{U}: Level 3\nFlying',
      '{UNKNOWN}: Level 2\nFlying\n{U}: Level 3\nFlying', '{U}: Level 2\n{U}: Level 3\nFlying',
      '{U}: Level 2\nFlying\n{U}: Level 3', '{U}: Level 2\nFlying\n{U}: Level 3\nUnknown rules.',
      '{U}: Level 2\nDraw a card.\n{U}: Level 3\nFlying', 'Draw a card.\n{U}: Level 2\nFlying\n{U}: Level 3\nFlying',
      '{U}: Level 2\nWhen this Class becomes level 2, invent a mechanic.\n{U}: Level 3\nFlying',
      '{U}: Level 2\nFlying\n{U}: Level 3\nFlying\n{U}: Level 4\nFlying',
      '{U}: Level 2\nFlying\n{U}: Level 3. Activate only as an instant.\nFlying',
    ]) assert.equal(parseOracleText(text).ok, false, text);
    assert.equal(parseClassCard(['Creatures you control have "{U}: Level 2."'], () => [], text => lex(text)), null);
  });
  it('does not certify Class activation history, designation changes or nested targets as bound', () => {
    const result = parse('When this Class becomes level 2, draw a card.\n{U}: Level 2\n{T}: Tap target creature.\n{U}: Level 3\nWhenever you attack, untap target creature.', true);
    const analysis = analyzeCardBindings({ identity: 'class-fixture', source: { name: 'Class fixture', layout: 'class', type_line: 'Enchantment — Class' }, faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(analysis.fullyBound, false);
    assert.ok(analysis.diagnostics.some(diagnostic => diagnostic.message.startsWith('A Class-level trigger')));
    assert.equal(analysis.diagnostics.filter(diagnostic => diagnostic.message.startsWith('A Class level bar') && /\/abilities\/[12]$/.test(diagnostic.path)).length, 2);
    assert.ok(analysis.references.filter(reference => 'ref' in reference.expression && reference.expression.ref === 'target').every(reference => reference.binding === 'unresolved'));
  });
});
