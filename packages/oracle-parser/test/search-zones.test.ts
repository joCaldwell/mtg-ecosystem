import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseOracleText } from '../src/index.ts';
import { buildCardNameCatalog } from '../src/card-names.ts';
import { analyzeCardBindings } from '../src/semantic/bindings.ts';
import { Cursor } from '../src/parser/cursor.ts';
import { lex } from '../src/lexer.ts';
import { parseSearchZones } from '../src/parser/search.ts';

// Oracle excerpts checked against the frozen Scryfall source. Searches that
// find no card still count as searches; library/graveyard is one total found
// selection, with different hidden/public search rules (CR 701.23a-d).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: 'fixed', value });
const you = { player: 'you' };
const zones = [{ zone: 'library', owner: 'your' }, { zone: 'graveyard', owner: 'your' }];
const shuffleAfterSearch = {
  condition: { condition: 'searched-zone', who: you, zone: { zone: 'library', owner: 'your' }, scope: 'this-way' },
  conditionKind: 'if', steps: [{ effect: 'shuffle', who: you, library: { zone: 'library', owner: you } }],
};
const cardNames = buildCardNameCatalog([{ name: 'Huatli, Dinosaur Knight', layout: 'normal' }]);
function abilities(text: string, typeLine?: string) {
  const r = parseOracleText(text, undefined, { typeLine, cardNames, includeSourceProvenance: true });
  assert.ok(r.ok, JSON.stringify(r.lines));
  return JSON.parse(JSON.stringify(r.abilities));
}

describe('search scope and conditional search history', () => {
  it('keeps Runeforge Champion optional search, shared selection and conditional shuffle', () => {
    assert.deepEqual(abilities('When this creature enters, you may search your library and/or graveyard for a Rune card, reveal it, and put it into your hand. If you search your library this way, shuffle.\nYou may pay {1} rather than pay the mana cost for Rune spells you cast.', 'Creature — Dwarf Warrior'), [
      { kind: 'triggered', trigger: { trigger: 'enters', what: { ref: 'this', noun: 'creature' } }, effects: [
        { steps: [{ effect: 'optional-group', who: you, steps: [
          { effect: 'search', who: you, zones: { selection: 'one-or-more', of: zones }, for: { ref: 'filter', filter: { subtypes: ['Rune'], cls: 'card' }, count: n(1) } },
          { effect: 'reveal', who: you, what: { ref: 'it' } },
          { effect: 'move-zone', what: { ref: 'it' }, to: { zone: 'hand', owner: 'your' } },
        ] }] }, shuffleAfterSearch,
      ] },
      { kind: 'static', effect: { static: 'alternative-cost', who: you,
        spells: { ref: 'each', filter: { subtypes: ['Rune'], cls: 'spell', cast: { by: you } } }, costs: [{ cost: 'mana', symbols: ['1'] }],
      } },
    ]);
  });
  it('retains a complete named search and the past-tense shuffle condition on Sun-Blessed Mount', () => {
    assert.deepEqual(abilities('When this creature enters, you may search your library and/or graveyard for a card named Huatli, Dinosaur Knight, reveal it, then put it into your hand. If you searched your library this way, shuffle.', 'Creature — Dinosaur'), [{
      kind: 'triggered', trigger: { trigger: 'enters', what: { ref: 'this', noun: 'creature' } }, effects: [
        { steps: [{ effect: 'optional-group', who: you, steps: [
          { effect: 'search', who: you, zones: { selection: 'one-or-more', of: zones }, for: { ref: 'filter', filter: { cls: 'card', name: { name: 'literal', value: 'Huatli, Dinosaur Knight' } }, count: n(1) } },
          { effect: 'reveal', who: you, what: { ref: 'it' } },
          { effect: 'move-zone', what: { ref: 'it' }, to: { zone: 'hand', owner: 'your' } },
        ] }] }, shuffleAfterSearch,
      ],
    }]);
  });
  it('keeps Invasion of Ikoria non-Human eligibility and X bound across both possible source zones', () => {
    assert.deepEqual(abilities('When this Siege enters, search your library and/or graveyard for a non-Human creature card with mana value X or less and put it onto the battlefield. If you search your library this way, shuffle.', 'Battle — Siege'), [{
      kind: 'triggered', trigger: { trigger: 'enters', what: { ref: 'this', noun: 'siege' } }, effects: [
        { steps: [
          { effect: 'search', who: you, zones: { selection: 'one-or-more', of: zones }, for: { ref: 'filter', filter: {
            nonSubtypes: ['Human'], types: ['creature'], cls: 'card', properties: [{ property: 'mana-value', comparison: { op: 'le', value: { amount: 'x' } } }],
          }, count: n(1) } },
          { effect: 'move-zone', what: { ref: 'it' }, to: { zone: 'battlefield' } },
        ] }, shuffleAfterSearch,
      ],
    }]);
    for (const noun of ['Sieges', 'Cases', 'bananas']) assert.equal(parseOracleText(`When this ${noun} enters, draw a card.`).ok, false);
  });
  it('separates all/one/one-or-more zones from the total number of found cards', () => {
    for (const [join, selection] of [['and', 'all'], ['or', 'one'], ['and/or', 'one-or-more']]) {
      assert.deepEqual(abilities(`Search your library ${join} graveyard for up to two creature cards.`), [{ kind: 'spell', effects: [{ steps: [{
        effect: 'search', who: you, zones: { selection, of: zones },
        for: { ref: 'filter', filter: { types: ['creature'], cls: 'card' }, count: n(2), upTo: true },
      }] }] }]);
    }
    const target = { player: 'target-opponent', selectionId: 'line:0:1' };
    assert.deepEqual(abilities("Search target opponent's graveyard, hand, and library for up to two cards."), [{ kind: 'spell', effects: [{ steps: [{
      effect: 'search', who: you, zones: { selection: 'all', of: ['graveyard', 'hand', 'library'].map(zone => ({ zone, owner: target })) },
      for: { ref: 'filter', filter: { cls: 'card' }, count: n(2), upTo: true },
    }] }] }]);
  });
  it('retains distinct owners and rejects incomplete, mixed, duplicated and positioned zone lists', () => {
    assert.deepEqual(parseSearchZones(new Cursor(lex("your graveyard and target opponent's library"))), {
      selection: 'all', of: [{ zone: 'graveyard', owner: 'your' }, { zone: 'library', owner: { player: 'target-opponent' } }],
    });
    assert.deepEqual(parseSearchZones(new Cursor(lex("your graveyard, target opponent's library, and hand"))), {
      selection: 'all', of: [{ zone: 'graveyard', owner: 'your' },
        { zone: 'library', owner: { player: 'target-opponent' } }, { zone: 'hand', owner: { player: 'target-opponent' } }],
    });
    for (const text of ['your library and/', 'your library, graveyard', 'your library and graveyard or hand', 'your library and your library', 'the top of your library', 'library']) {
      const cursor = new Cursor(lex(text));
      const result = parseSearchZones(cursor);
      assert.ok(result === null || !cursor.done(), text);
      if (result === null) assert.equal(cursor.pos, 0, text);
      assert.equal(parseOracleText(`Search ${text} for a card.`).ok, false, text);
    }
    for (const clause of ['you found a card this way', 'you searched your library last turn', 'you search the top of your library this way'])
      assert.equal(parseOracleText(`If ${clause}, shuffle.`).ok, false, clause);
  });
  it('leaves the precise search-event and multi-zone binding obligations visible', () => {
    const result = parseOracleText('Search your library and/or graveyard for a card. If you search your library this way, shuffle.', 'Search test');
    assert.ok(result.ok);
    const binding = analyzeCardBindings({ identity: 'search-test', source: { name: 'Search test', layout: 'normal', type_line: 'Sorcery', mana_cost: '{G}' }, faces: [{ sourceFaceIndex: 0, result }] });
    assert.equal(binding.fullyBound, false);
    assert.ok(binding.diagnostics.some(d => d.code === 'unsupported-scope'));
    assert.ok(binding.diagnostics.some(d => d.code === 'unsupported-reference'));
  });
});
