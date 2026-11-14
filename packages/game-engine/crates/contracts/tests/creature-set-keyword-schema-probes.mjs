// Bounded, separately prepared typed-IR controls. New schema execution belongs
// to the parent. --prepare-only constructs controls without loading Ajv/schema.
// Execute: --schema ABSOLUTE_SCHEMA [--ajv-root EXISTING_NODE_MODULES_DIRECTORY].
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { resolve, join } from 'node:path';
const frozen = JSON.parse(readFileSync(new URL('../../../tests/fixtures/expected-effects-ir.json', import.meta.url), 'utf8'));
const clone = value => structuredClone(value);
const family = 'temporary_creature_set_keyword_program_v1';
const keywordCaps = { flying: 'flying_v1', deathtouch: 'deathtouch_v1', indestructible: 'indestructible_v1',
  lifelink: 'lifelink_v1', hexproof: 'attachments_v1', haste: 'haste_v1', trample: 'trample_v1', 'first strike': 'first_strike_v1' };
const sets = ['controller_creatures', 'attacking_creatures'];
const vectors = [...Object.keys(keywordCaps).map(k => [k]), ['first strike', 'trample', 'lifelink'],
  ['hexproof', 'indestructible'], Object.keys(keywordCaps), Object.keys(keywordCaps).reverse()];
function control(set, keywords, modifier, sorcery = false) {
  const d = clone(frozen.definitions.find(d => d.name === 'Giant Growth'));
  d.spell.targets = [];
  const grant = { op: 'grant_keywords_set', set, keywords: clone(keywords), duration: 'until_end_of_turn', source_block: 0 };
  const pump = { op: 'pump_set', set, power: '-2', toughness: '99999999999999999999999999999999999', source_block: 0 };
  d.spell.instructions = modifier ? [pump, grant] : [grant];
  if (sorcery) { d.characteristics.kind = 'sorcery'; d.spell.timing = 'main_phase'; }
  d.capabilities = [...new Set(['normal_layout_v1', sorcery ? 'cast_sorcery_v1' : 'cast_instant_v1', 'board_wipes_v1', family, ...keywords.map(k => keywordCaps[k])])];
  return { ...clone(frozen), definitions: [d] };
}
const probes = [], positives = [];
function positive(name, value) {
  const copy = clone(value);
  positives.push({ name, value: copy, exact: JSON.stringify(copy) });
  probes.push({ name, value: copy, expected: true });
  return copy;
}
function negative(name, base, change) {
  const value = clone(base); change(value.definitions[0]);
  probes.push({ name, value, expected: false });
}
for (const set of sets) for (const keywords of vectors) for (const modifier of [false, true]) {
  positive(`${set}:${keywords}:${modifier}`, control(set, keywords, modifier));
}
for (const set of sets) for (const modifier of [false, true]) {
  const base = control(set, Object.keys(keywordCaps), modifier);
  const name = `${set}:${modifier}`;
  positive(`${name}:sorcery`, control(set, Object.keys(keywordCaps), modifier, true));
  for (const cap of base.definitions[0].capabilities) negative(`${name}:missing:${cap}`, base, d => { d.capabilities = d.capabilities.filter(c => c !== cap); });
  for (const cap of ['creature_target_v1', 'temporary_creature_keyword_program_v1', 'temporary_flying_spell_v1',
    'temporary_deathtouch_spell_v1', 'temporary_indestructible_spell_v1', 'compound_indestructible_spell_v1',
    'temporary_power_toughness_v1', 'cast_sorcery_v1', 'flash_v1', 'reach_v1']) {
    negative(`${name}:spare:${cap}`, base, d => { d.capabilities.push(cap); });
  }
  for (const [label, change] of [
    ['empty', p => { p.instructions.at(-1).keywords = []; }],
    ['duplicate', p => { p.instructions.at(-1).keywords.push('haste'); }],
    ['unreviewed', p => { p.instructions.at(-1).keywords = ['reach']; }],
    ['first-strike-alias', p => { p.instructions.at(-1).keywords = ['first_strike']; }],
    ['unit-map', p => { p.instructions.at(-1).keywords = [{ haste: null }]; }],
    ['duration', p => { p.instructions.at(-1).duration = 'forever'; }],
    ['missing-duration', p => { delete p.instructions.at(-1).duration; }],
    ['block', p => { p.instructions.at(-1).source_block = 1; }],
    ['block-max', p => { p.instructions.at(-1).source_block = 4294967295; }],
    ['target-slot', p => { p.instructions.at(-1).target_slot = 0; }],
    ['target', p => { p.targets = clone(frozen.definitions.find(d => d.name === 'Giant Growth').spell.targets); }],
    ['zone', p => { p.functioning_zone = 'battlefield'; }],
    ['timing', p => { p.timing = 'main_phase'; }],
    ['resolution', p => { p.resolution = 'permanent'; }],
    ['additional-sacrifice', p => { p.additional_sacrifice = { types: ['artifact'], subtypes: [] }; }],
    ['extra-grant', p => { p.instructions.push(clone(p.instructions.at(-1))); }],
    ['modes', p => { const child = clone(p); p.instructions = []; p.modes = [{ program: child }, { program: clone(child) }]; }],
  ]) negative(`${name}:${label}`, base, d => change(d.spell));
  for (const set of ['creatures', 'nonland_permanents', 'enchantments', 'opponent_creatures', 'flying_creatures', 'nonflying_creatures']) {
    negative(`${name}:unsupported-set:${set}`, base, d => { for (const i of d.spell.instructions) i.set = set; });
  }
  for (const [label, change] of [
    ['kind', d => { d.characteristics.kind = 'creature'; }],
    ['power', d => { d.characteristics.power = '1'; }],
    ['toughness', d => { d.characteristics.toughness = '1'; }],
    ['basic', d => { d.characteristics.basic = true; }],
    ['legendary', d => { d.characteristics.legendary = true; }],
    ['intrinsic-zone', d => { d.intrinsic_zone = 'battlefield'; }],
    ['intrinsic-flash', d => { d.keywords = ['flash']; }],
    ['static', d => { d.static_abilities = [{ functioning_zone: 'battlefield', effect: 'cant_be_countered' }]; }],
  ]) negative(`${name}:context:${label}`, base, change);
  if (modifier) for (const [label, change] of [
    ['reverse', p => { p.instructions.reverse(); }],
    ['mixed-sets', p => { p.instructions[0].set = sets.find(s => s !== set); }],
    ['pump-block', p => { p.instructions[0].source_block = 1; }],
    ['dynamic-power', p => { p.instructions[0].power = { amount: 'source-power' }; }],
    ['noncanonical-quantity', p => { p.instructions[0].toughness = '-0'; }],
  ]) negative(`${name}:${label}`, base, d => change(d.spell));
  for (const host of ['Samite Healer', 'Blood Artist']) negative(`${name}:ability:${host}`, base, d => {
    const program = d.spell;
    const source = clone(frozen.definitions.find(d => d.name === host));
    if (source.activated?.length) source.activated[0].program = program;
    else source.triggered[0].program = program;
    Object.assign(d, source);
  });
}
for (const keyword of Object.keys(keywordCaps)) {
  const base = control('controller_creatures', [keyword], false);
  for (const [other, cap] of Object.entries(keywordCaps)) if (other !== keyword) negative(`${keyword}:spare-primitive:${other}`, base, d => { d.capabilities.push(cap); });
}
const own = control('controller_creatures', ['haste'], true);
for (const instruction of [
  { op: 'pump_set', set: 'controller_creatures', power: '1', toughness: '0', source_block: 0 },
  { op: 'move_set', set: 'controller_creatures', action: 'destroy', source_block: 0 },
  { op: 'damage_set', sets: ['controller_creatures'], each_player: false, amount: { amount: 'fixed', value: '1' }, source_block: 0 },
]) {
  negative(`own-selector-old-route:${instruction.op}`, own, d => {
    d.spell.instructions = [instruction];
    d.capabilities = ['normal_layout_v1', 'cast_instant_v1', 'board_wipes_v1'];
  });
  for (const host of ['Samite Healer', 'Blood Artist']) negative(`own-selector-old-route:${instruction.op}:${host}`, own, d => {
    const source = clone(frozen.definitions.find(d => d.name === host));
    const program = clone(d.spell); program.instructions = [instruction];
    if (source.activated?.length) source.activated[0].program = program; else source.triggered[0].program = program;
    Object.assign(d, source);
  });
}
for (const row of positives) assert.equal(JSON.stringify(row.value), row.exact);
const summary = { format_version: 1, status: 'prepared_only', probes: probes.length,
  positive: probes.filter(p => p.expected).length, negative: probes.filter(p => !p.expected).length,
  frozen_controls_sha256: createHash('sha256').update(JSON.stringify(positives)).digest('hex') };
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
  console.log(JSON.stringify({ ...summary, status: failures.length ? 'failed' : 'passed', schema_sha256: createHash('sha256').update(bytes).digest('hex'), failures }, null, 2));
  if (failures.length) process.exitCode = 1;
}
