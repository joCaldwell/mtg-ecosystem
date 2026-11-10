import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';

function one(text: string, name?: string, typeLine?: string) {
  const result = parseOracleText(text, name, typeLine ? { typeLine } : {});
  assert.ok(result.ok, JSON.stringify(result.lines));
  assert.equal(result.abilities.length, 1);
  return JSON.parse(JSON.stringify(result.abilities[0]));
}
const fails = (text: string, name?: string, typeLine?: string) =>
  assert.equal(parseOracleText(text, name, typeLine ? { typeLine } : {}).ok, false, text);
const fixed = (value: number) => ({ amount: 'fixed', value });
const tap = [{ cost: 'tap-self' }];
const you = { player: 'you' };
const chosenColor = { choice: 'chosen-value', category: 'color', which: 'chosen' };

describe('mana of a chosen or derived color', () => {
  it('reads "that color" after one color choice as the chosen color (Astral Cornucopia)', () => {
    assert.deepEqual(one('{T}: Choose a color. Add one mana of that color for each charge counter on this artifact.', undefined, 'Artifact'), {
      kind: 'activated', costs: tap, effects: [
        { steps: [{ effect: 'choose-value', who: you, category: 'color' }] },
        { steps: [{ effect: 'add-mana', who: you, mana: { mana: 'chosen-color', choice: chosenColor,
          amount: { amount: 'product', factors: [fixed(1), { amount: 'counter-count', counter: { type: 'charge' }, on: { ref: 'this', noun: 'artifact' } }] } } }] }] });
  });
  it('scales the chosen color by a distinct-value count (Selvala, Eager Trailblazer)', () => {
    assert.deepEqual(one('{T}: Choose a color. Add one mana of that color for each different power among creatures you control.', undefined, 'Legendary Creature — Elf Warrior').effects[1], {
      steps: [{ effect: 'add-mana', who: you, mana: { mana: 'chosen-color', choice: chosenColor,
        amount: { amount: 'product', factors: [fixed(1), { amount: 'distinct-count', characteristic: 'power',
          among: { ref: 'each', filter: { types: ['creature'], control: 'you' } } }] } } }] });
  });
  it('needs exactly one preceding single-color choice for "that color"', () => {
    fails('{T}: Add one mana of that color.', undefined, 'Artifact');
    fails('{T}: Choose two colors. Add one mana of that color.', undefined, 'Artifact');
    fails('{T}: Choose a creature type. Add one mana of that color.', undefined, 'Artifact');
  });
  it('restricts the color choice to colors of matching objects (Meteor Crater, CR 106.5)', () => {
    assert.deepEqual(one('{T}: Choose a color of a permanent you control. Add one mana of that color.', undefined, 'Land'), {
      kind: 'activated', costs: tap, effects: [
        { steps: [{ effect: 'choose-value', who: you, category: 'color', among: { cls: 'permanent', control: 'you' } }] },
        { steps: [{ effect: 'add-mana', who: you, mana: { mana: 'chosen-color', amount: fixed(1), choice: chosenColor } }] }] });
    assert.deepEqual(one('{T}: Choose a color of a card in your graveyard. Add one mana of that color.', undefined, 'Artifact').effects[0].steps[0],
      { effect: 'choose-value', who: you, category: 'color', among: { cls: 'card', zone: { zone: 'graveyard', owner: 'your' } } });
  });
  it('adds mana of different colors (Firemind Vessel)', () => {
    assert.deepEqual(one('{T}: Add two mana of different colors.', undefined, 'Artifact'), {
      kind: 'activated', costs: tap, effects: [{ steps: [{ effect: 'add-mana', who: you, mana: { mana: 'different-colors', amount: fixed(2) } }] }] });
    fails('{T}: Add one mana of different colors.', undefined, 'Artifact');
  });
  it('adds one mana of any color among objects (Plaza of Heroes, Mox Amber)', () => {
    assert.deepEqual(one('{T}: Add one mana of any color among legendary permanents you control.', undefined, 'Land').effects[0].steps[0], {
      effect: 'add-mana', who: you, mana: { mana: 'color-among', amount: fixed(1),
        among: { ref: 'each', filter: { supertypes: ['legendary'], cls: 'permanent', control: 'you' } } } });
  });
  it('adds one mana of each color among objects (Faeburrow Elder)', () => {
    assert.deepEqual(one('{T}: For each color among permanents you control, add one mana of that color.', undefined, 'Creature — Treefolk Druid'), {
      kind: 'activated', costs: tap, effects: [{ steps: [{ effect: 'add-mana', who: you,
        mana: { mana: 'each-color-among', among: { ref: 'each', filter: { cls: 'permanent', control: 'you' } } } }] }] });
    fails('{T}: For each color among permanents you control, add two mana of that color.', undefined, 'Creature — Treefolk Druid');
  });
  it('lets the adding player choose the color (Spectral Searchlight, Stadium Vendors)', () => {
    assert.deepEqual(one('{T}: Choose a player. That player adds one mana of any color they choose.', undefined, 'Artifact').effects[1],
      { steps: [{ effect: 'add-mana', who: { player: 'that-player' }, mana: { mana: 'any-color', amount: fixed(1) } }] });
    assert.deepEqual(one('When this creature enters, choose a player. That player adds two mana of any one color they choose.', undefined, 'Creature — Human Citizen').effects[1],
      { steps: [{ effect: 'add-mana', who: { player: 'that-player' }, mana: { mana: 'any-one-color', amount: fixed(2) } }] });
    // "you" never chooses as "they".
    fails('{T}: Add one mana of any color they choose.', undefined, 'Artifact');
  });
});

describe('"protection from X or from the color of your choice"', () => {
  const fresh = { scope: 'fresh-choice', category: 'color', who: you, when: 'granting-effect-applies' };
  it('grants one protection ability whose quality you choose (Giver of Runes)', () => {
    assert.deepEqual(one('{T}: Another target creature you control gains protection from colorless or from the color of your choice until end of turn.', undefined, 'Creature — Kor Cleric'), {
      kind: 'activated', costs: tap, effects: [{ steps: [{ effect: 'gain-abilities',
        what: { ref: 'target', filter: { types: ['creature'], control: 'you', other: true }, count: fixed(1) },
        abilities: [{ gained: 'keyword-choice', who: you, count: fixed(1), when: 'granting-effect-applies', options: [
          { keyword: 'protection', from: [{ scope: 'colorless' }] }, { keyword: 'protection', from: [fresh] }] }],
        duration: { duration: 'end-of-turn' } }] }] });
  });
  it('keeps a type quality as its own option (Razor Barrier)', () => {
    assert.deepEqual(one('Target permanent you control gains protection from artifacts or from the color of your choice until end of turn.', undefined, 'Instant').effects[0].steps[0].abilities, [
      { gained: 'keyword-choice', who: you, count: fixed(1), when: 'granting-effect-applies', options: [
        { keyword: 'protection', from: [{ scope: 'filter', filter: { types: ['artifact'] } }] }, { keyword: 'protection', from: [fresh] }] }]);
  });
  it('is not a continuous static grant and needs the second "from"', () => {
    fails('Creatures you control have protection from colorless or from the color of your choice.', undefined, 'Enchantment');
    fails('Target creature gains protection from colorless or the color of your choice until end of turn.', undefined, 'Instant');
  });
});

describe('"Its power is equal to … and its toughness is equal to …" after a token', () => {
  it('completes the created token\'s power and toughness (Ritual of the Returned)', () => {
    const card = { ref: 'that', filter: { cls: 'card' } };
    assert.deepEqual(one('Exile target creature card from your graveyard. Create a black Zombie creature token. Its power is equal to that card\'s power and its toughness is equal to that card\'s toughness.', undefined, 'Instant').effects, [
      { steps: [{ effect: 'exile', what: { ref: 'target', filter: { types: ['creature'], cls: 'card', zone: { zone: 'graveyard', owner: 'your' } }, count: fixed(1) } }] },
      { steps: [{ effect: 'create-token', count: fixed(1), token: { colors: ['black'], supertypes: [], types: ['creature'], subtypes: ['Zombie'], keywords: [],
        power: { amount: 'attribute', of: card, attribute: 'power' }, toughness: { amount: 'attribute', of: card, attribute: 'toughness' } } }] }]);
  });
  it('rejects tokens that already have stats, several tokens, and noncreature tokens', () => {
    fails('Destroy target creature. Create a 2/2 black Zombie creature token. Its power is equal to that creature\'s power and its toughness is equal to that creature\'s toughness.', undefined, 'Sorcery');
    fails('Destroy target creature. Create two black Zombie creature tokens. Its power is equal to that creature\'s power and its toughness is equal to that creature\'s toughness.', undefined, 'Sorcery');
    fails('Destroy target creature. Create a Treasure token. Its power is equal to that creature\'s power and its toughness is equal to that creature\'s toughness.', undefined, 'Sorcery');
    fails('Destroy target creature. Create a black Zombie creature token. Its power is equal to that creature\'s power.', undefined, 'Sorcery');
  });
});

describe('"any target that isn\'t a <description>"', () => {
  it('keeps the excluded objects on the any-target selection (Wrathful Red Dragon)', () => {
    assert.deepEqual(one('Whenever a Dragon you control is dealt damage, it deals that much damage to any target that isn\'t a Dragon.', undefined, 'Creature — Dragon').effects, [
      { steps: [{ effect: 'damage', source: { ref: 'it' }, amount: { amount: 'that-much' }, to: { to: 'object', ref: { ref: 'any-target', excluding: { subtypes: ['Dragon'] } } } }] }]);
  });
  it('excludes commanders (Lozhan, Dragons\' Legacy)', () => {
    assert.deepEqual(one('~ deals 2 damage to any target that isn\'t a commander.', 'Test Card', 'Instant').effects[0].steps[0].to,
      { to: 'object', ref: { ref: 'any-target', excluding: { commander: true } } });
  });
  it('does not read an inclusive relative clause', () => {
    fails('~ deals 2 damage to any target that is a Dragon.', 'Test Card', 'Instant');
  });
});

describe('"can\'t become untapped"', () => {
  it('is the untap prohibition, not untap-step suppression (Frozen in Ice)', () => {
    assert.deepEqual(one('Enchanted creature loses all abilities and can\'t become untapped.', undefined, 'Enchantment — Aura'), {
      kind: 'static', effect: { static: 'modify', what: { ref: 'enchanted', noun: 'creature' },
        modifiers: [{ modifier: 'lose-abilities', removal: { removal: 'all' } }, { modifier: 'cant', action: 'untap' }] } });
    assert.deepEqual(one('Enchanted creature can\'t become untapped.', undefined, 'Enchantment — Aura'),
      one('Enchanted creature can\'t untap.', undefined, 'Enchantment — Aura'));
  });
  it('keeps a stated duration on a resolving prohibition (Spider-Woman, Secret Agent)', () => {
    assert.deepEqual(one('When ~ enters, tap target creature an opponent controls. That creature can\'t become untapped for as long as you control ~.', 'Test Card', 'Creature — Human').effects[1], {
      steps: [{ effect: 'cant', what: { ref: 'that', filter: { types: ['creature'] } }, action: 'untap',
        duration: { duration: 'as-long-as', condition: { condition: 'control', who: you, what: { ref: 'self' } } } }] });
  });
  it('does not read "become tapped"', () => {
    fails('Enchanted creature can\'t become tapped.', undefined, 'Enchantment — Aura');
  });
});

describe('"their total power"', () => {
  it('sums the attribute of the objects the pronoun stands for (Angrath, Minotaur Pirate)', () => {
    assert.deepEqual(one('Destroy all creatures target opponent controls. ~ deals damage to that player equal to their total power.', 'Test Card', 'Sorcery').effects[1], {
      steps: [{ effect: 'damage', source: { ref: 'self' }, amount: { amount: 'attribute-sum', attribute: 'power', among: { ref: 'them' } },
        to: { to: 'player', ref: { player: 'that-player' } } }] });
  });
  it('reads toughness the same way and needs "total" (Sutured Ghoul)', () => {
    assert.deepEqual(one('Destroy all creatures target opponent controls. You gain life equal to their total toughness.', undefined, 'Sorcery').effects[1].steps[0].amount,
      { amount: 'attribute-sum', attribute: 'toughness', among: { ref: 'them' } });
    fails('Destroy all creatures target opponent controls. You gain life equal to their total loyalty.', undefined, 'Sorcery');
  });
});
