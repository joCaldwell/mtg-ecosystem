// Separate synthetic IR inventory: no printed-card/admission claims. Native and
// schema execution belong to the parent. --prepare-only loads no Ajv/schema.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { resolve, join } from 'node:path';
const frozen = JSON.parse(readFileSync(new URL('../../../tests/fixtures/expected-effects-ir.json', import.meta.url), 'utf8'));
const clone = value => structuredClone(value);
const family = 'temporary_permanent_set_keyword_program_v1';
const creatureFamily = 'temporary_creature_set_keyword_program_v1';
const keywordCaps = { flying: 'flying_v1', deathtouch: 'deathtouch_v1', indestructible: 'indestructible_v1',
  lifelink: 'lifelink_v1', hexproof: 'attachments_v1', haste: 'haste_v1', trample: 'trample_v1', 'first strike': 'first_strike_v1' };
const all = Object.keys(keywordCaps);
function control(keywords, sorcery = false) {
  const d = clone(frozen.definitions.find(d => d.name === 'Giant Growth'));
  d.spell.targets = [];
  d.spell.instructions = [{ op: 'grant_keywords_set', set: 'controller_permanents', keywords: clone(keywords),
    duration: 'until_end_of_turn', source_block: 0 }];
  if (sorcery) { d.characteristics.kind = 'sorcery'; d.spell.timing = 'main_phase'; }
  d.capabilities = [...new Set(['normal_layout_v1', sorcery ? 'cast_sorcery_v1' : 'cast_instant_v1',
    'board_wipes_v1', family, ...keywords.map(k => keywordCaps[k])])];
  return { ...clone(frozen), definitions: [d] };
}
const probes = [], positives = [];
function positive(name, value) {
  const copy = clone(value);
  positives.push({ name, value: copy, exact: JSON.stringify(copy) });
  probes.push({ name, value: copy, expected: true });
}
function negative(name, base, change) {
  const value = clone(base); change(value.definitions[0]);
  probes.push({ name, value, expected: false });
}
for (const keywords of [...all.map(k => [k]), ['hexproof', 'indestructible'], ['indestructible', 'hexproof'], all, [...all].reverse()]) {
  for (const sorcery of [false, true]) {
    const base = control(keywords, sorcery), label = `synthetic:${keywords}:${sorcery}`;
    positive(label, base);
    for (const cap of base.definitions[0].capabilities) negative(`${label}:missing:${cap}`, base, d => {
      d.capabilities = d.capabilities.filter(c => c !== cap);
    });
    for (const [other, cap] of Object.entries(keywordCaps)) if (!keywords.includes(other)) {
      negative(`${label}:spare-primitive:${other}`, base, d => { d.capabilities.push(cap); });
    }
  }
}
const base = control(['hexproof', 'indestructible']);
const explicitNone = clone(base); explicitNone.definitions[0].spell.instructions[0].modifier = null;
positive('permanent:explicit-none', explicitNone);
for (const cap of [creatureFamily, 'temporary_creature_keyword_program_v1', 'temporary_flying_spell_v1',
  'temporary_deathtouch_spell_v1', 'temporary_indestructible_spell_v1', 'compound_indestructible_spell_v1',
  'temporary_power_toughness_v1', 'creature_target_v1', 'flash_v1', 'reach_v1', 'cast_sorcery_v1']) {
  negative(`permanent:spare:${cap}`, base, d => { d.capabilities.push(cap); });
}
for (const modifier of [{ power: '0', toughness: '0' }, { power: '-2', toughness: '3' },
  { power: '1', toughness: '-1' }, { power: '99999999999999999999999999999999', toughness: '-99999999999999999999999999999999' },
  {}, { power: '0' }, { power: 0, toughness: '0' }, { power: '-0', toughness: '0' }]) {
  negative(`permanent:forbidden-modifier:${JSON.stringify(modifier)}`, base, d => { d.spell.instructions[0].modifier = modifier; });
}
for (const [label, change] of [
  ['empty-keywords', p => { p.instructions[0].keywords = []; }],
  ['duplicate-keywords', p => { p.instructions[0].keywords.push('hexproof'); }],
  ['unreviewed-keyword', p => { p.instructions[0].keywords = ['shroud']; }],
  ['first-strike-alias', p => { p.instructions[0].keywords = ['first_strike']; }],
  ['duration', p => { p.instructions[0].duration = 'forever'; }],
  ['missing-duration', p => { delete p.instructions[0].duration; }],
  ['block', p => { p.instructions[0].source_block = 1; }],
  ['max-block', p => { p.instructions[0].source_block = 4294967295; }],
  ['target-slot', p => { p.instructions[0].target_slot = 0; }],
  ['target', p => { p.targets = clone(frozen.definitions.find(d => d.name === 'Giant Growth').spell.targets); }],
  ['zone', p => { p.functioning_zone = 'battlefield'; }],
  ['resolution', p => { p.resolution = 'permanent'; }],
  ['timing', p => { p.timing = 'main_phase'; }],
  ['cost', p => { p.additional_sacrifice = { types: ['artifact'], subtypes: [] }; }],
  ['extra-grant', p => { p.instructions.push(clone(p.instructions[0])); }],
  ['no-instructions', p => { p.instructions = []; }],
  ['mode', p => { const child = clone(p); p.instructions = []; p.modes = [{ program: child }, { program: clone(child) }]; }],
]) negative(`permanent:shape:${label}`, base, d => change(d.spell));
for (const set of ['creatures', 'controller_creatures', 'attacking_creatures', 'nonland_permanents', 'enchantments',
  'opponent_creatures', 'flying_creatures', 'nonflying_creatures', 'permanents', { cls: 'permanent', control: 'you' }]) {
  negative(`permanent:wrong-selector-with-permanent-cap:${JSON.stringify(set)}`, base, d => { d.spell.instructions[0].set = set; });
}
for (const set of ['controller_creatures', 'attacking_creatures']) {
  const creature = clone(base);
  creature.definitions[0].spell.instructions[0].set = set;
  creature.definitions[0].capabilities = creature.definitions[0].capabilities.map(c => c === family ? creatureFamily : c);
  positive(`existing-creature-family:${set}`, creature);
  negative(`existing-creature-family:spare-permanent-cap:${set}`, creature, d => { d.capabilities.push(family); });
}
for (const [label, change] of [
  ['kind', d => { d.characteristics.kind = 'creature'; }],
  ['power', d => { d.characteristics.power = '1'; }],
  ['toughness', d => { d.characteristics.toughness = '1'; }],
  ['basic', d => { d.characteristics.basic = true; }],
  ['legendary', d => { d.characteristics.legendary = true; }],
  ['additional-types', d => { d.characteristics.additional_types = ['artifact']; }],
  ['intrinsic-zone', d => { d.intrinsic_zone = 'battlefield'; }],
  ['intrinsic-flash', d => { d.keywords = ['flash']; }],
  ['x-cost', d => { d.x_mana_cost = true; }],
  ['static', d => { d.static_abilities = [{ functioning_zone: 'battlefield', effect: 'cant_be_countered' }]; }],
]) negative(`permanent:definition:${label}`, base, change);
const instructions = [
  ...['destroy', 'exile', 'hand'].map(action => ({ op: 'move_set', set: 'controller_permanents', action, source_block: 0 })),
  { op: 'pump_set', set: 'controller_permanents', power: '0', toughness: '0', source_block: 0 },
  { op: 'damage_set', sets: ['controller_permanents'], each_player: false, amount: { amount: 'fixed', value: '1' }, source_block: 0 },
];
for (const i of instructions) {
  const label = `${i.op}:${i.action ?? ''}`;
  negative(`permanent:forbidden-op:${label}`, base, d => {
    d.spell.instructions = [clone(i)];
    d.capabilities = ['normal_layout_v1', 'cast_instant_v1', 'board_wipes_v1'];
  });
  negative(`permanent:forbidden-op-modal:${label}`, base, d => {
    const child = clone(d.spell); child.instructions = [clone(i)];
    d.spell.instructions = []; d.spell.modes = [{ program: child }, { program: clone(child) }];
    d.capabilities = ['normal_layout_v1', 'cast_instant_v1', 'board_wipes_v1', 'modal_choice_v1'];
  });
}
for (const i of [...instructions, base.definitions[0].spell.instructions[0]]) {
  for (const host of ['Samite Healer', 'Blood Artist']) {
    negative(`permanent:ability:${host}:${i.op}:${i.action ?? ''}`, base, d => {
      const program = clone(d.spell); program.instructions = [clone(i)];
      const source = clone(frozen.definitions.find(d => d.name === host));
      if (source.activated?.length) source.activated[0].program = program; else source.triggered[0].program = program;
      Object.assign(d, source);
    });
  }
}
for (const reverse of [false, true]) negative(`permanent:split-pump-grant:${reverse}`, base, d => {
  d.spell.instructions.unshift({ op: 'pump_set', set: 'controller_permanents', power: '0', toughness: '0', source_block: 0 });
  if (reverse) d.spell.instructions.reverse();
});
for (const row of positives) assert.equal(JSON.stringify(row.value), row.exact);
const hash = value => createHash('sha256').update(JSON.stringify(value)).digest('hex');
const summary = { format_version: 1, status: 'prepared_only', probes: probes.length,
  positive: probes.filter(p => p.expected).length, negative: probes.filter(p => !p.expected).length,
  frozen_controls_sha256: hash(positives), probe_payloads_sha256: hash(probes) };
if (process.argv.includes('--prepare-only')) console.log(JSON.stringify(summary));
else {
  const option = name => { const i = process.argv.indexOf(name); return i < 0 ? undefined : process.argv[i + 1]; };
  const path = option('--schema'); assert(path, '--schema PATH required (or --prepare-only)');
  const root = option('--ajv-root');
  const require = createRequire(root ? join(resolve(root), '__schema_probe__.cjs') : import.meta.url);
  const Ajv2020 = require('ajv/dist/2020.js').default;
  const bytes = readFileSync(path), ajv = new Ajv2020({ strict: false, allErrors: true });
  ajv.addFormat('uint32', { type: 'number', validate: n => Number.isInteger(n) && n >= 0 && n <= 4294967295 });
  const validate = ajv.compile(JSON.parse(bytes)), failures = [];
  for (const probe of probes) {
    const actual = validate(clone(probe.value));
    if (actual !== probe.expected) failures.push({ name: probe.name, expected: probe.expected, actual, errors: clone(validate.errors) });
  }
  for (const row of positives) assert.equal(JSON.stringify(row.value), row.exact);
  console.log(JSON.stringify({ ...summary, status: failures.length ? 'failed' : 'passed',
    schema_sha256: createHash('sha256').update(bytes).digest('hex'), failures }, null, 2));
  if (failures.length) process.exitCode = 1;
}
