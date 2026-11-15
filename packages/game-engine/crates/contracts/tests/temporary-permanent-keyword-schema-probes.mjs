// Separate synthetic IR inventory. Native/schema execution is UNRUN by A.
// --prepare-only constructs payloads without Ajv, schema export or native work.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { resolve, join } from 'node:path';
const frozen = JSON.parse(readFileSync(new URL('../../../tests/fixtures/expected-effects-ir.json', import.meta.url), 'utf8'));
const clone = value => structuredClone(value);
const family = 'temporary_permanent_keyword_program_v1';
const keywordCaps = { flying: 'flying_v1', deathtouch: 'deathtouch_v1', indestructible: 'indestructible_v1',
  lifelink: 'lifelink_v1', hexproof: 'attachments_v1', haste: 'haste_v1', trample: 'trample_v1', 'first strike': 'first_strike_v1' };
const all = Object.keys(keywordCaps);
function control(keywords, life, sorcery = false) {
  const d = clone(frozen.definitions.find(d => d.name === 'Giant Growth'));
  d.spell.targets = [{ slot: 0, selection_id: 'line:0:0', chooser: 'controller', stage: 'announcement',
    domain: 'controlled_battlefield_permanent', filter: { types: [], non_types: [], non_colors: [], opponent: false, tapped: false } }];
  d.spell.instructions = [{ op: 'grant_keywords', target_slot: 0, keywords: clone(keywords), duration: 'until_end_of_turn', source_block: 0 }];
  if (life !== undefined) d.spell.instructions.push({ op: 'gain_life', recipient: 'controller', amount: life, source_block: 0 });
  if (sorcery) { d.characteristics.kind = 'sorcery'; d.spell.timing = 'main_phase'; }
  d.capabilities = [...new Set(['normal_layout_v1', sorcery ? 'cast_sorcery_v1' : 'cast_instant_v1',
    'permanent_removal_v1', family, ...keywords.map(k => keywordCaps[k]), ...(life === undefined ? [] : ['life_gain_v1'])])];
  return { ...clone(frozen), definitions: [d] };
}
const probes = [], positives = [];
function positive(name, value) {
  const copy = clone(value); positives.push({ name, value: copy, exact: JSON.stringify(copy) });
  probes.push({ name, value: copy, expected: true });
}
function negative(name, base, change) {
  const value = clone(base); change(value.definitions[0]); probes.push({ name, value, expected: false });
}
const vectors = [...all.map(k => [k]), ['hexproof', 'indestructible'], ['indestructible', 'hexproof'], all, [...all].reverse()];
for (const keywords of vectors) for (const life of [undefined, '0', '2']) for (const sorcery of [false, true]) {
  const base = control(keywords, life, sorcery), label = `synthetic:${keywords}:${life}:${sorcery}`;
  positive(label, base);
  for (const cap of base.definitions[0].capabilities) negative(`${label}:missing:${cap}`, base, d => { d.capabilities = d.capabilities.filter(c => c !== cap); });
  for (const [other, cap] of Object.entries(keywordCaps)) if (!keywords.includes(other)) {
    negative(`${label}:spare-primitive:${other}`, base, d => { d.capabilities.push(cap); });
  }
  if (life === undefined) negative(`${label}:spare-life`, base, d => { d.capabilities.push('life_gain_v1'); });
}
const base = control(['hexproof', 'indestructible'], '2');
const pure = control(['hexproof', 'indestructible']);
const explicitNone = clone(base); explicitNone.definitions[0].spell.instructions[0].modifier = null;
positive('permanent:explicit-none', explicitNone);
const omittedTapped = clone(base); delete omittedTapped.definitions[0].spell.targets[0].filter.tapped;
positive('permanent:default-false-tapped-wire', omittedTapped);
positive('permanent:large-exact-life', control(['flying'], '999999999999999999999999999999999'));
for (const cap of ['temporary_creature_keyword_program_v1', 'temporary_creature_set_keyword_program_v1', 'temporary_permanent_set_keyword_program_v1',
  'temporary_flying_spell_v1', 'temporary_deathtouch_spell_v1', 'temporary_indestructible_spell_v1', 'compound_indestructible_spell_v1',
  'temporary_power_toughness_v1', 'creature_target_v1', 'flash_v1', 'board_wipes_v1', 'reach_v1', 'cast_sorcery_v1', 'sacrifice_v1']) {
  negative(`permanent:spare:${cap}`, base, d => { d.capabilities.push(cap); });
}
for (const [name, mutate] of [
  ['missing-filter', t => { delete t.filter; }], ['null-filter', t => { t.filter = null; }],
  ['types', t => { t.filter.types = ['creature']; }], ['non-types', t => { t.filter.non_types = ['land']; }],
  ['non-colors', t => { t.filter.non_colors = ['B']; }], ['opponent', t => { t.filter.opponent = true; }],
  ['tapped', t => { t.filter.tapped = true; }], ['unknown-filter', t => { t.filter.control = 'you'; }],
  ['null-tapped', t => { t.filter.tapped = null; }],
  ['slot', t => { t.slot = 1; }], ['empty-selection', t => { t.selection_id = ''; }], ['null-selection', t => { t.selection_id = null; }],
  ['chooser', t => { t.chooser = 'owner'; }], ['stage', t => { t.stage = 'resolution'; }],
  ['plain-permanent', t => { t.domain = 'battlefield_permanent'; }],
  ['controlled-creature', t => { t.domain = 'controlled_battlefield_creature'; t.filter = null; }],
  ['creature', t => { t.domain = 'battlefield_creature'; t.filter = null; }],
]) for (const [shape, input] of [['pure', pure], ['life', base]]) negative(`permanent:${shape}:target:${name}`, input, d => mutate(d.spell.targets[0]));
for (const modifier of [{ power: '0', toughness: '0' }, { power: '-1', toughness: '2' }, {}, { power: '1' }, { power: 1, toughness: '1' }]) {
  negative(`permanent:modifier:${JSON.stringify(modifier)}`, base, d => { d.spell.instructions[0].modifier = modifier; });
}
for (const [name, change] of [
  ['empty', p => { p.instructions = []; }], ['life-only', p => { p.instructions.shift(); }],
  ['reverse', p => { p.instructions.reverse(); }], ['extra-grant', p => { p.instructions.push(clone(p.instructions[0])); }],
  ['extra-life', p => { p.instructions.push(clone(p.instructions[1])); }],
  ['grant-block', p => { p.instructions[0].source_block = 1; }], ['life-block', p => { p.instructions[1].source_block = 1; }],
  ['grant-slot', p => { p.instructions[0].target_slot = 1; }], ['grant-duration', p => { p.instructions[0].duration = 'forever'; }],
  ['empty-keywords', p => { p.instructions[0].keywords = []; }], ['duplicate', p => { p.instructions[0].keywords.push('hexproof'); }],
  ['unknown-keyword', p => { p.instructions[0].keywords = ['shroud']; }], ['keyword-alias', p => { p.instructions[0].keywords = ['first_strike']; }],
  ['life-negative', p => { p.instructions[1].amount = '-1'; }], ['life-number', p => { p.instructions[1].amount = 2; }],
  ['life-x', p => { p.instructions[1].amount = { amount: 'x' }; }], ['life-null', p => { p.instructions[1].amount = null; }],
  ['life-opponent', p => { p.instructions[1].recipient = 'each_opponent'; }],
  ['life-target', p => { p.instructions[1].recipient = { target: { target_slot: 0 } }; }],
  ['life-lose', p => { p.instructions[1].op = 'lose_life'; }], ['life-extra', p => { p.instructions[1].optional = true; }],
  ['zero-target', p => { p.targets = []; }], ['second-target', p => { p.targets.push(clone(p.targets[0])); }],
  ['cost', p => { p.additional_sacrifice = { types: ['artifact'], subtypes: [] }; }],
  ['zone', p => { p.functioning_zone = 'battlefield'; }], ['resolution', p => { p.resolution = 'permanent'; }],
  ['timing', p => { p.timing = 'main_phase'; }],
  ['mode', p => { const child = clone(p); p.instructions = []; p.targets = []; p.modes = [{ program: child }, { program: clone(child) }]; }],
  ['split-pump', p => { p.instructions.unshift({ op: 'modify_power_toughness', target_slot: 0, power: '1', toughness: '1', duration: 'until_end_of_turn', source_block: 0 }); }],
]) negative(`permanent:program:${name}`, base, d => change(d.spell));
for (const [name, change] of [
  ['kind', d => { d.characteristics.kind = 'creature'; }], ['power', d => { d.characteristics.power = '1'; }],
  ['toughness', d => { d.characteristics.toughness = '1'; }], ['additional-type', d => { d.characteristics.additional_types = ['artifact']; }],
  ['basic', d => { d.characteristics.basic = true; }], ['legendary', d => { d.characteristics.legendary = true; }],
  ['intrinsic-zone', d => { d.intrinsic_zone = 'battlefield'; }], ['flash', d => { d.keywords = ['flash']; }], ['x', d => { d.x_mana_cost = true; }],
  ['static', d => { d.static_abilities = [{ functioning_zone: 'battlefield', effect: 'cant_be_countered' }]; }],
]) negative(`permanent:definition:${name}`, base, change);
for (const host of ['Samite Healer', 'Blood Artist']) for (const life of [undefined, '2']) {
  negative(`permanent:ability:${host}:${life}`, control(['hexproof'], life), d => {
    const program = clone(d.spell), source = clone(frozen.definitions.find(d => d.name === host));
    if (source.activated?.length) source.activated[0].program = program; else source.triggered[0].program = program;
    Object.assign(d, source);
  });
}
// Existing creature and set controls retain real narrow family meanings.
for (const domain of ['battlefield_creature', 'controlled_battlefield_creature']) {
  const c = control(['lifelink']); const d = c.definitions[0];
  d.spell.targets[0].domain = domain; delete d.spell.targets[0].filter;
  d.capabilities = ['normal_layout_v1', 'cast_instant_v1', domain === 'battlefield_creature' ? 'creature_target_v1' : 'attachments_v1', 'lifelink_v1', 'temporary_creature_keyword_program_v1'];
  positive(`creature-control:${domain}`, c);
  negative(`creature-life-forbidden:${domain}`, c, d => { d.spell.instructions.push(clone(base.definitions[0].spell.instructions[1])); d.capabilities.push('life_gain_v1'); });
  negative(`creature-spare-permanent:${domain}`, c, d => { d.capabilities.push(family); });
}
for (const set of ['controller_permanents', 'controller_creatures', 'attacking_creatures']) {
  const c = control(['lifelink']); const d = c.definitions[0]; d.spell.targets = [];
  d.spell.instructions = [{ op: 'grant_keywords_set', set, keywords: ['lifelink'], duration: 'until_end_of_turn', source_block: 0 }];
  d.capabilities = ['normal_layout_v1', 'cast_instant_v1', 'board_wipes_v1', 'lifelink_v1', set === 'controller_permanents' ? 'temporary_permanent_set_keyword_program_v1' : 'temporary_creature_set_keyword_program_v1'];
  positive(`set-control:${set}`, c);
  negative(`set-life-forbidden:${set}`, c, d => { d.spell.instructions.push(clone(base.definitions[0].spell.instructions[1])); d.capabilities.push('life_gain_v1'); });
}
for (const name of ['Sacred Nectar', 'Healing Hands', 'Soul Feast']) {
  const source = frozen.definitions.find(d => d.name === name);
  const c = { ...clone(frozen), definitions: [clone(source)] };
  positive(`ordinary-life-control:${name}`, c);
  source.spell.instructions.forEach((i, index) => {
    if (['gain_life', 'lose_life'].includes(i.op)) negative(`ordinary-zero-life:${name}:${index}`, c, d => { d.spell.instructions[index].amount = '0'; });
  });
}
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
  console.log(JSON.stringify({ ...summary, status: failures.length ? 'failed' : 'passed', schema_sha256: createHash('sha256').update(bytes).digest('hex'), failures }, null, 2));
  if (failures.length) process.exitCode = 1;
}
