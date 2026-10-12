import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import type { Effect, ObjectRef, ResolvingCharacteristicModifier, StaticEffect, StaticModifier } from '../src/ast.ts';
import { parseOracleText } from '../src/parser/index.ts';

// CR 105.3, 205.1a/b, 305.7 (set vs. add), 608.2d (choices on resolution),
// 611.2c/e (locked-in set; "is/has" vs. "becomes"), 613.4b (base P/T).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed', value }) as const;
const you = { player: 'you' } as const;
const end = { duration: 'end-of-turn' } as const;
const thisCreature = { ref: 'this', noun: 'creature' } as const;
const target = (filter: object): ObjectRef => ({ ref: 'target', filter, count: n(1) });
function steps(text: string, typeLine = 'Instant'): unknown {
  const result = parseOracleText(text, 'Fixture', { typeLine });
  assert.ok(result.ok, JSON.stringify(result.lines));
  const ability = result.abilities[0];
  assert.ok(ability.kind === 'spell' || ability.kind === 'activated' || ability.kind === 'triggered');
  return ability.effects.map(sentence => sentence.steps);
}
function statics(text: string, typeLine = 'Enchantment'): StaticEffect {
  const result = parseOracleText(text, 'Fixture', { typeLine });
  assert.ok(result.ok, JSON.stringify(result.lines));
  const ability = result.abilities[0];
  assert.ok(ability.kind === 'static');
  return ability.effect;
}
const fails = (text: string, typeLine = 'Instant') => assert.equal(parseOracleText(text, 'Fixture', { typeLine }).ok, false, text);
function change(what: ObjectRef, modifiers: ResolvingCharacteristicModifier[], rest: Partial<Extract<Effect, { effect: 'modify-characteristics' }>> = {}): Effect {
  return { effect: 'modify-characteristics', what, modifiers, snapshot: 'effect-created', ...rest };
}
const freshColor = (quantity: 'one' | 'one-or-more') =>
  ({ choice: 'fresh-choice', category: 'color', who: you, when: 'effect-created', quantity }) as const;
const freshType = (category: 'creature-type' | 'basic-land-type') =>
  ({ choice: 'fresh-choice', category, who: you, when: 'effect-created' }) as const;

describe('characteristic values supplied by a choice', () => {
  it('sets one freshly chosen color, or a nonempty set of colors, with or without a lifetime', () => {
    assert.deepEqual(steps('Target permanent becomes the color of your choice.'), [[change(target({ cls: 'permanent' }),
      [{ modifier: 'colors', operation: 'set', colors: freshColor('one') }])]]);
    assert.deepEqual(steps('Target creature becomes the color or colors of your choice until end of turn.'), [[change(target({ types: ['creature'] }),
      [{ modifier: 'colors', operation: 'set', colors: freshColor('one-or-more') }], { duration: end })]]);
    assert.deepEqual(steps('{1}: This creature becomes all colors until end of turn.', 'Creature — Scarecrow'), [[change(thisCreature,
      [{ modifier: 'colors', operation: 'set', colors: ['white', 'blue', 'black', 'red', 'green'] }], { duration: end })]]);
    fails('Target permanent becomes the colors of your choice.');
    fails('Target permanent becomes the color of their choice.');
  });
  it('distinguishes a set basic land type (305.7) from an added one, and creature types', () => {
    const land = target({ types: ['land'] });
    assert.deepEqual(steps('Target land becomes the basic land type of your choice until end of turn.'), [[change(land,
      [{ modifier: 'types', change: { subtypes: [{ family: 'land', operation: 'set', types: freshType('basic-land-type') }] } }], { duration: end })]]);
    assert.deepEqual(steps('Until end of turn, target land becomes the basic land type of your choice in addition to its other types.'), [[change(land,
      [{ modifier: 'types', change: { subtypes: [{ family: 'land', operation: 'add', types: freshType('basic-land-type') }] } }], { duration: end })]]);
    assert.deepEqual(steps('{1}: This creature becomes the creature type of your choice until end of turn.', 'Creature — Illusion'), [[change(thisCreature,
      [{ modifier: 'types', change: { subtypes: [{ family: 'creature', operation: 'set', types: freshType('creature-type') }] } }], { duration: end })]]);
    fails('Target land becomes the land type of your choice until end of turn.');
  });
  it('links "that type" only to a subtype choice in the preceding sentence', () => {
    assert.deepEqual(steps('Choose a basic land type. Each land you control becomes that type until end of turn.'), [
      [{ effect: 'choose-value', who: you, category: 'basic-land-type' }],
      [change({ ref: 'each', filter: { types: ['land'], control: 'you' } },
        [{ modifier: 'types', change: { chosenSubtype: { operation: 'set', choice: { choice: 'chosen-type', kind: 'land' } } } }], { duration: end })]]);
    fails('Target creature becomes that type until end of turn.');
    fails('Choose a color. Target creature becomes that type until end of turn.');
  });
  it('sets an earlier chosen color symbolically (Puca\'s Eye)', () => {
    assert.deepEqual(steps('When this artifact enters, draw a card, then choose a color. This artifact becomes the chosen color.', 'Artifact'), [
      [{ effect: 'draw', who: you, amount: n(1) }, { effect: 'choose-value', who: you, category: 'color' }],
      [change({ ref: 'this', noun: 'artifact' }, [{ modifier: 'colors', operation: 'set', colors: { choice: 'chosen-value', category: 'color', which: 'chosen' } }])]]);
  });
});

describe('static chosen subtypes and their extension to other zones', () => {
  const chosen = (operation: 'set' | 'add', kind?: 'creature'): StaticModifier =>
    ({ modifier: 'types', change: { chosenSubtype: { operation, choice: { choice: 'chosen-type', ...(kind ? { kind } : {}) } } } });
  it('keeps set and add apart and never guesses the linked family', () => {
    assert.deepEqual(statics('Enchanted land is the chosen type.', 'Enchantment — Aura'),
      { static: 'modify', what: { ref: 'enchanted', noun: 'land' }, modifiers: [chosen('set')], condition: undefined, duration: undefined });
    assert.deepEqual(statics('Vehicle creatures you control are the chosen creature type in addition to their other types.', 'Artifact — Vehicle'),
      { static: 'modify', what: { ref: 'each', filter: { subtypes: ['Vehicle'], types: ['creature'], control: 'you' } },
        modifiers: [chosen('add', 'creature')], condition: undefined, duration: undefined });
    assert.deepEqual(statics('This creature is the chosen type in addition to its other types.', 'Creature — Construct'),
      { static: 'cda-types', what: thisCreature, change: { chosenSubtype: { operation: 'add', choice: { choice: 'chosen-type' } } } });
    fails("Creatures you control aren't the chosen type.", 'Enchantment');
    fails('Creatures you control are the chosen type in addition to their other colors.', 'Enchantment');
  });
  it('applies the same change to stated spells and to cards outside the battlefield (Conspiracy)', () => {
    const creatures = (extra: object): ObjectRef => ({ ref: 'each', filter: { types: ['creature'], ...extra } });
    assert.deepEqual(statics("Creatures you control are the chosen type. The same is true for creature spells you control and creature cards you own that aren't on the battlefield."), {
      static: 'group', effects: [
        { static: 'modify', what: creatures({ control: 'you' }), modifiers: [chosen('set')] },
        { static: 'modify', what: creatures({ cls: 'spell', control: 'you' }), modifiers: [chosen('set')] },
        { static: 'modify', what: creatures({ cls: 'card', own: 'you', outsideZone: { zone: 'battlefield' } }), modifiers: [chosen('set')] },
      ] });
    fails('Creatures you control are the chosen type. The same is true for creatures your opponents control.', 'Enchantment');
    fails('Creatures you control get +1/+1. The same is true for creature spells you control.', 'Enchantment');
  });
  it('lets "it\'s" carry the verb of a conditional type change (Aerial Modification)', () => {
    assert.deepEqual(statics("As long as enchanted permanent is a Vehicle, it's a creature in addition to its other types.", 'Enchantment — Aura'), {
      static: 'conditional', condition: { condition: 'is-filter', what: { ref: 'enchanted', noun: 'permanent' }, filter: { subtypes: ['Vehicle'] } },
      effect: { static: 'modify', what: { ref: 'it' }, modifiers: [{ modifier: 'types', change: { cardTypes: { operation: 'add', types: ['creature'] } } }],
        condition: undefined, duration: undefined } });
  });
});

describe('resolving statements that an object is or has a characteristic (611.2e)', () => {
  const returned: Effect = { effect: 'move-zone', what: target({ types: ['creature'], cls: 'card', zone: { zone: 'graveyard', owner: 'your' } }), to: { zone: 'battlefield' } };
  it('marks "is" statements, with added or replaced values as printed', () => {
    assert.deepEqual(steps('Return target creature card from your graveyard to the battlefield. That creature is a black Zombie in addition to its other colors and types.'), [[returned],
      [change({ ref: 'that', filter: { types: ['creature'] } }, [
        { modifier: 'types', change: { subtypes: [{ family: 'creature', operation: 'add', types: ['Zombie'] }] } },
        { modifier: 'colors', operation: 'add', colors: ['black'] }], { statedAs: 'is' })]]);
    assert.deepEqual(steps("Return target creature card from your graveyard to the battlefield. It's an enchantment."), [[returned],
      [change({ ref: 'it' }, [{ modifier: 'types', change: { cardTypes: { operation: 'set', types: ['enchantment'] } } }], { statedAs: 'is' })]]);
    assert.deepEqual(steps('Return target creature card from your graveyard to the battlefield. That creature is black and is a Nightmare in addition to its other creature types.'), [[returned],
      [change({ ref: 'that', filter: { types: ['creature'] } }, [
        { modifier: 'colors', operation: 'set', colors: ['black'] },
        { modifier: 'types', change: { subtypes: [{ family: 'creature', operation: 'add', types: ['Nightmare'] }] } }], { statedAs: 'is' })]]);
    // "becomes" applies after entry and keeps its existing representation.
    assert.deepEqual(steps('Return target creature card from your graveyard to the battlefield. It becomes a Zombie in addition to its other types.'), [[returned],
      [{ effect: 'become', what: { ref: 'it' }, spec: { subtypes: ['Zombie'], inAddition: 'types' } }]]);
  });
  it('accepts only an indefinite or "for as long as" lifetime, and no delayed antecedent', () => {
    assert.deepEqual(steps('Put a flood counter on target land. That land is an Island in addition to its other types for as long as it has a flood counter on it.'), [
      [{ effect: 'put-counters', on: target({ types: ['land'] }), counter: { type: 'flood' }, count: n(1) }],
      [change({ ref: 'that', filter: { types: ['land'] } }, [{ modifier: 'types', change: { subtypes: [{ family: 'land', operation: 'add', types: ['Island'] }] } }], {
        statedAs: 'is', duration: { duration: 'as-long-as', condition: { condition: 'is-filter', what: { ref: 'it' },
          filter: { counterConstraints: [{ counter: { type: 'flood' }, comparison: { op: 'ge', value: n(1) } }] } } } })]]);
    fails('Target creature is a green Frog until end of turn.');
    fails('Destroy target creature. It has flying and is an Angel in addition to its other types.');
    fails('Whenever a creature dies, return it to the battlefield under your control at the beginning of the next end step. That creature is a black Zombie in addition to its other colors and types.', 'Enchantment');
  });
});

describe('setting base power and/or toughness (613.4b)', () => {
  it('states a single base value without touching the other', () => {
    assert.deepEqual(steps('{T}: Target attacking creature has base power 0 until end of turn.', 'Creature — Plant'), [[change(
      target({ status: ['attacking'], types: ['creature'] }), [{ modifier: 'base-pt', power: n(0) }], { statedAs: 'has', duration: end })]]);
    fails('{T}: Until end of turn, target creature has base power 1 or base toughness 1.', 'Creature — Human');
    // The complete fixed pair keeps its existing node.
    assert.deepEqual(steps('Target creature has base power and toughness 5/5 until end of turn.'), [[
      { effect: 'set-pt', what: target({ types: ['creature'] }), power: n(5), toughness: n(5), duration: end }]]);
  });
  it('changes a possessor\'s base values to snapshotted amounts', () => {
    const that: ObjectRef = { ref: 'that', filter: { types: ['creature'] } };
    assert.deepEqual(steps("{1}{U}: This creature's base power becomes equal to target creature's power.", 'Creature — Beast'), [[change(thisCreature,
      [{ modifier: 'base-pt', power: { amount: 'attribute', of: target({ types: ['creature'] }), attribute: 'power' }, valueScope: 'each-affected-object' }])]]);
    assert.deepEqual(steps("Whenever this creature blocks a creature, this creature's base power and toughness become equal to that creature's power and toughness until end of turn.", 'Creature — Spirit'), [[change(thisCreature,
      [{ modifier: 'base-pt', power: { amount: 'attribute', of: that, attribute: 'power' }, toughness: { amount: 'attribute', of: that, attribute: 'toughness' },
        valueScope: 'each-affected-object' }], { duration: end })]]);
    assert.deepEqual(steps("Whenever a land you control enters, you may have this creature's base power and toughness become 4/2 until end of turn.", 'Creature — Elf'), [[change(thisCreature,
      [{ modifier: 'base-pt', power: n(4), toughness: n(2) }], { duration: end, optional: true })]]);
    fails("{1}: This creature's base power and toughness become 4/1 or 1/4 until end of turn.", 'Creature — Sphinx');
    fails("{1}: This creature's power becomes 4 until end of turn.", 'Creature — Sphinx');
  });
  it('states both base values from one dynamic amount for the affected object (Zur)', () => {
    const enchantment = target({ nonSubtypes: ['Aura'], types: ['enchantment'], control: 'you' });
    const value = { amount: 'attribute', of: { ref: 'it' }, attribute: 'mana-value' } as const;
    assert.deepEqual(steps('{1}{W}: Target non-Aura enchantment you control becomes a creature in addition to its other types and has base power and base toughness each equal to its mana value.', 'Creature — Human'), [[
      { effect: 'become', what: enchantment, spec: { types: ['creature'], inAddition: 'types' } },
      change(enchantment, [{ modifier: 'base-pt', power: value, toughness: value, valueScope: 'each-affected-object' }], { statedAs: 'has' })]]);
  });
});
