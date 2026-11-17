// Separate synthetic IR inventory: no printed-card/admission claims. Native and
// schema execution belong to the parent. --prepare-only loads no Ajv/schema.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { resolve, join } from 'node:path';
const frozen = JSON.parse(readFileSync(new URL('../../../tests/fixtures/expected-wipes-ir.json', import.meta.url), 'utf8'));
const clone = value => structuredClone(value);
// Bounded synthetic typed IR controls, independent from frozen source admission.
const keywordCaps = { indestructible: 'indestructible_v1', flying: 'flying_v1', lifelink: 'lifelink_v1',
  hexproof: 'attachments_v1', haste: 'haste_v1', 'first strike': 'first_strike_v1' };
function control(keywords) {
  const d = clone(frozen.definitions.find(d => d.name === 'Prodigal Sorcerer'));
  const program = clone(frozen.definitions.find(d => d.name === 'Giant Growth').spell);
  program.targets = [];
  program.instructions = [{ op: 'grant_keywords_set', set: 'controller_creatures',
    keywords: clone(keywords), duration: 'until_end_of_turn', source_block: 0 }];
  d.activated = [{ functioning_zone: 'battlefield', cost: 'sacrifice_source', program }];
  d.capabilities = ['normal_layout_v1', 'base_creature_v1', 'cast_creature_v1', 'activated_ability_v1',
    'sacrifice_source_cost_v1', 'board_wipes_v1', 'temporary_creature_set_keyword_program_v1', ...keywords.map(k => keywordCaps[k])];
  return { ...clone(frozen), definitions: [d] };
}
const probes = [], positives = [];
function positive(name, value) {
  const exact = JSON.stringify(value);
  probes.push({ name, value: clone(value), expected: true });
  positives.push({ name, value: clone(value), exact });
}
function negative(name, base, change) {
  const value = clone(base); change(value.definitions[0]);
  probes.push({ name, value, expected: false });
}
for (const keywords of [['indestructible'], ['flying', 'lifelink'], ['hexproof', 'haste', 'first strike']]) {
  const base = control(keywords);
  positive(`sacrifice_set:keywords:${keywords}`, base);
  for (const cap of base.definitions[0].capabilities) negative(`sacrifice_set:missing:${keywords}:${cap}`, base,
    d => { d.capabilities = d.capabilities.filter(c => c !== cap); });
}
const base = control(['indestructible']);
const nullModifier = clone(base); nullModifier.definitions[0].activated[0].program.instructions[0].modifier = null;
positive('sacrifice_set:explicit-null-modifier', nullModifier);
for (const cap of ['sacrifice_v1', 'cast_instant_v1', 'temporary_indestructible_spell_v1',
  'temporary_permanent_set_keyword_program_v1', 'creature_target_v1', 'flying_v1']) {
  negative(`sacrifice_set:spare:${cap}`, base, d => { d.capabilities.push(cap); });
}
for (const [label, change] of [
  ['tap', a => { a.cost = 'tap_source'; }],
  ['mana', a => { a.cost = { mana: { cost: { generic: '0', colored: [] } } }; }],
  ['zone', a => { a.functioning_zone = 'stack'; }],
  ['timing', a => { a.program.timing = 'main_phase'; }],
  ['set-attacking', a => { a.program.instructions[0].set = 'attacking_creatures'; }],
  ['set-permanent', a => { a.program.instructions[0].set = 'controller_permanents'; }],
  ['modifier-zero', a => { a.program.instructions[0].modifier = { power: '0', toughness: '0' }; }],
  ['block', a => { a.program.instructions[0].source_block = 1; }],
  ['duplicate', a => { a.program.instructions[0].keywords.push('indestructible'); }],
  ['empty', a => { a.program.instructions[0].keywords = []; }],
  ['unreviewed', a => { a.program.instructions[0].keywords = ['shroud']; }],
  ['extra-op', a => { a.program.instructions.push({ op: 'draw_cards', count: '1', source_block: 0 }); }],
  ['empty-program', a => { a.program.instructions = []; }],
  ['additional-cost', a => { a.program.additional_sacrifice = { types: ['creature'], subtypes: [] }; }],
  ['optional-cost', a => { a.program.optional_additional_costs = [{ identity: 'synthetic', source_block: 0, mana: { generic: '1', colored: [] } }]; }],
  ['target', a => { a.program.targets = clone(frozen.definitions.find(d => d.name === 'Giant Growth').spell.targets); }],
  ['mode', a => { const p = clone(a.program); a.program.instructions = []; a.program.modes = [{ program: p }]; }],
]) negative(`sacrifice_set:ability:${label}`, base, d => change(d.activated[0]));
for (const [label, change] of [
  ['artifact', d => { d.characteristics.kind = 'artifact'; }],
  ['combined-type', d => { d.characteristics.additional_types = ['artifact']; }],
  ['legendary', d => { d.characteristics.legendary = true; }],
  ['intrinsic-keyword', d => { d.keywords = ['indestructible']; }],
  ['intrinsic-zone', d => { d.intrinsic_zone = 'battlefield'; }],
  ['second-activation', d => { d.activated.push(clone(d.activated[0])); }],
  ['spell', d => { d.spell = clone(d.activated[0].program); }],
  ['trigger', d => { d.triggered = clone(frozen.definitions.find(d => d.name === 'Blood Artist').triggered); d.triggered[0].program = clone(d.activated[0].program); d.activated = []; }],
]) negative(`sacrifice_set:definition:${label}`, base, change);
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
