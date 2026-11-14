// Source-only prepared probes; parent executes against its new exported schema.
// Typed synthetic IR controls, never assertions about invented card sources.
// --prepare-only checks fixture/control construction without Ajv/schema execution.
// Otherwise: --schema ABSOLUTE_SCHEMA [--ajv-root NODE_MODULES_DIRECTORY].
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { resolve, join } from 'node:path';

const effects = JSON.parse(readFileSync(new URL('../../../tests/fixtures/expected-effects-ir.json', import.meta.url), 'utf8'));
const clone = value => structuredClone(value);
function freeze(value) {
  if (value && typeof value === 'object') {
    Object.values(value).forEach(freeze);
    Object.freeze(value);
  }
  return value;
}
const keywordCaps = {
  flying: 'flying_v1', deathtouch: 'deathtouch_v1',
  indestructible: 'indestructible_v1', lifelink: 'lifelink_v1',
};
const subsetCaps = {
  flying: 'temporary_flying_spell_v1',
  deathtouch: 'temporary_deathtouch_spell_v1',
  indestructible: 'temporary_indestructible_spell_v1',
};
const family = 'temporary_creature_keyword_program_v1';
const pt = 'temporary_power_toughness_v1';
function control(keywords, modifier = false) {
  const d = clone(effects.definitions.find(d => d.name === 'Giant Growth'));
  const grant = { op: 'grant_keywords', target_slot: 0, keywords: clone(keywords), duration: 'until_end_of_turn', source_block: 0 };
  const pump = { op: 'modify_power_toughness', target_slot: 0, power: '-2', toughness: '99999999999999999999999999999999999', duration: 'until_end_of_turn', source_block: 0 };
  d.spell.instructions = modifier ? [pump, grant] : [grant];
  let capability = family;
  if (keywords.length === 1) {
    if (!modifier && subsetCaps[keywords[0]]) capability = subsetCaps[keywords[0]];
    if (modifier && keywords[0] === 'indestructible') capability = 'compound_indestructible_spell_v1';
  }
  d.capabilities = ['normal_layout_v1', 'cast_instant_v1', 'creature_target_v1', capability, ...keywords.map(k => keywordCaps[k]), ...(modifier ? [pt] : [])];
  return freeze({ ...clone(effects), definitions: [d] });
}
const probes = [];
const positives = [];
function positive(name, value) {
  const frozen = freeze(clone(value));
  positives.push({ name, value: frozen, exact: JSON.stringify(frozen) });
  probes.push({ name, value: frozen, expected: true });
  return frozen;
}
function negative(name, base, mutate) {
  const value = clone(base);
  mutate(value.definitions[0], value);
  probes.push({ name, value: freeze(value), expected: false });
}
function vectors(prefix = [], remaining = Object.keys(keywordCaps)) {
  const result = prefix.length ? [prefix] : [];
  for (const keyword of remaining) result.push(...vectors([...prefix, keyword], remaining.filter(k => k !== keyword)));
  return result;
}
assert.equal(vectors().length, 64);
for (const keywords of vectors()) {
  for (const modifier of [false, true]) {
    positive(`${modifier ? 'modifier' : 'pure'}:${keywords.join('+')}`, control(keywords, modifier));
  }
}
const pure = positive('frozen pure lifelink control', control(['lifelink']));
const compound = positive('frozen compound four-keyword control', control(Object.keys(keywordCaps), true));
for (const [name, base] of [['pure', pure], ['compound', compound]]) {
  for (const cap of base.definitions[0].capabilities) {
    negative(`${name}:missing:${cap}`, base, d => { d.capabilities = d.capabilities.filter(c => c !== cap); });
  }
  for (const cap of [...Object.values(keywordCaps), ...Object.values(subsetCaps), 'compound_indestructible_spell_v1', pt, 'reach_v1', 'cast_sorcery_v1']) {
    if (!base.definitions[0].capabilities.includes(cap)) negative(`${name}:spare:${cap}`, base, d => { d.capabilities.push(cap); });
  }
  for (const [label, mutation] of [
    ['empty', k => { k.length = 0; }],
    ['duplicate', k => { k.push(k[0]); }],
    ['unknown', k => { k.push('future-keyword'); }],
    ['unreviewed', k => { k.push('reach'); }],
  ]) negative(`${name}:keywords:${label}`, base, d => mutation(d.spell.instructions.at(-1).keywords));
  for (const [label, mutation] of [
    ['target-slot', p => { p.targets[0].slot = 1; }],
    ['target-domain', p => { p.targets[0].domain = 'blocking_battlefield_creature'; }],
    ['empty-selection', p => { p.targets[0].selection_id = ''; }],
    ['chooser', p => { p.targets[0].chooser = 'opponent'; }],
    ['stage', p => { p.targets[0].stage = 'resolution'; }],
    ['filter', p => { p.targets[0].filter = { types: [], non_types: [], non_colors: [], opponent: false, tapped: false }; }],
    ['cardinality', p => { p.targets[0].count = 2; }],
    ['extra-target', p => { p.targets.push(clone(p.targets[0])); }],
    ['no-target', p => { p.targets = []; }],
    ['instruction-slot', p => { p.instructions.at(-1).target_slot = 1; }],
    ['duration', p => { p.instructions.at(-1).duration = 'forever'; }],
    ['extra-op', p => { p.instructions.push({ op: 'draw_cards', count: '1', source_block: 0 }); }],
    ['extra-property', p => { p.instructions.at(-1).unreviewed = true; }],
    ['additional-cost', p => { p.additional_sacrifice = { types: ['artifact'], subtypes: [] }; }],
    ['functioning-zone', p => { p.functioning_zone = 'battlefield'; }],
    ['resolution', p => { p.resolution = 'permanent'; }],
    ['mode', p => { const child = clone(p); p.instructions = []; p.targets = []; p.modes = [{ program: clone(child) }, { program: clone(child) }]; }],
  ]) negative(`${name}:${label}`, base, d => mutation(d.spell));
  negative(`${name}:definition-context`, base, d => { d.characteristics.kind = 'creature'; });
  for (const [sourceName, kind] of [['Samite Healer', 'activated'], ['Blood Artist', 'triggered']]) {
    const d = clone(effects.definitions.find(d => d.name === sourceName));
    // Preserve a fresh exact non-grant program as a paired context control.
    const context = positive(`${name}:${kind}:unmodified-control`, { ...clone(effects), definitions: [d] });
    negative(`${name}:${kind}`, context, d => { d[kind][0].program = clone(base.definitions[0].spell); });
  }
}
for (const [name, change] of [
  ['order', p => p.instructions.reverse()],
  ['modifier-slot', p => { p.instructions[0].target_slot = 1; }],
  ['modifier-provenance', p => { p.instructions[0].source_block = 1; }],
  ['grant-provenance', p => { p.instructions[1].source_block = 1; }],
  ['split-provenance', p => { p.instructions[0].source_block = 1; p.instructions[1].source_block = 1; }],
  ['modifier-duration', p => { p.instructions[0].duration = 'forever'; }],
  ['numeric-quantity', p => { p.instructions[0].power = 2; }],
  ['nonfixed-quantity', p => { p.instructions[0].power = { amount: 'source-power' }; }],
  ['noncanonical-quantity', p => { p.instructions[0].toughness = '-0'; }],
]) negative(`compound:${name}`, compound, d => change(d.spell));
for (const keyword of Object.keys(keywordCaps)) {
  const base = control([keyword]);
  const preserved = clone(base);
  preserved.definitions[0].spell.instructions[0].source_block = 4294967295;
  positive(`pure:${keyword}:preserved-provenance`, preserved);
}
for (const [keywords, modifier] of [[['flying'], false], [['deathtouch'], false], [['indestructible'], false], [['indestructible'], true]]) {
  const base = control(keywords, modifier);
  assert(!base.definitions[0].capabilities.includes(family));
  negative(`legacy:${keywords}:${modifier}:spare-family`, base, d => { d.capabilities.push(family); });
}
const sorcery = clone(compound);
sorcery.definitions[0].characteristics.kind = 'sorcery';
sorcery.definitions[0].spell.timing = 'main_phase';
sorcery.definitions[0].capabilities = sorcery.definitions[0].capabilities.map(c => c === 'cast_instant_v1' ? 'cast_sorcery_v1' : c);
positive('sorcery compound family', sorcery);
for (const row of positives) assert.equal(JSON.stringify(row.value), row.exact, `mutated positive: ${row.name}`);
const summary = {
  format_version: 1,
  status: 'prepared_only',
  probes: probes.length,
  positive: probes.filter(p => p.expected).length,
  negative: probes.filter(p => !p.expected).length,
  frozen_controls_sha256: createHash('sha256').update(JSON.stringify(positives)).digest('hex'),
};
if (process.argv.includes('--prepare-only')) {
  console.log(JSON.stringify(summary));
} else {
  function option(name) { const index = process.argv.indexOf(name); return index < 0 ? undefined : process.argv[index + 1]; }
  const schemaPath = option('--schema');
  assert(schemaPath, '--schema PATH is required (or use --prepare-only)');
  const packageRoot = option('--ajv-root');
  const require = createRequire(packageRoot ? join(resolve(packageRoot), '__schema_probe__.cjs') : import.meta.url);
  const Ajv2020 = require('ajv/dist/2020.js').default;
  const ajvVersion = require('ajv/package.json').version;
  const schemaBytes = readFileSync(schemaPath);
  const ajv = new Ajv2020({ strict: false, allErrors: true });
  ajv.addFormat('uint32', { type: 'number', validate: n => Number.isInteger(n) && n >= 0 && n <= 4294967295 });
  const validate = ajv.compile(JSON.parse(schemaBytes));
  const failures = [];
  for (const probe of probes) {
    const actual = validate(clone(probe.value));
    if (actual !== probe.expected) failures.push({ name: probe.name, expected: probe.expected, actual, errors: clone(validate.errors) });
  }
  for (const row of positives) assert.equal(JSON.stringify(row.value), row.exact, `mutated positive: ${row.name}`);
  console.log(JSON.stringify({ ...summary, status: failures.length ? 'failed' : 'passed', ajv_version: ajvVersion, schema_sha256: createHash('sha256').update(schemaBytes).digest('hex'), failures }, null, 2));
  if (failures.length) process.exitCode = 1;
}
