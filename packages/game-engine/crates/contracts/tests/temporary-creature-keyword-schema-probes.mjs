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
// Separate lookup: keywordCaps also defines the frozen legacy permutation set.
const combatKeywordCaps = {
  haste: 'haste_v1', trample: 'trample_v1', 'first strike': 'first_strike_v1',
};
const subsetCaps = {
  flying: 'temporary_flying_spell_v1',
  deathtouch: 'temporary_deathtouch_spell_v1',
  indestructible: 'temporary_indestructible_spell_v1',
};
const family = 'temporary_creature_keyword_program_v1';
const pt = 'temporary_power_toughness_v1';
function control(keywords, modifier = false, domain = 'battlefield_creature') {
  const d = clone(effects.definitions.find(d => d.name === 'Giant Growth'));
  const grant = { op: 'grant_keywords', target_slot: 0, keywords: clone(keywords), duration: 'until_end_of_turn', source_block: 0 };
  const pump = { op: 'modify_power_toughness', target_slot: 0, power: '-2', toughness: '99999999999999999999999999999999999', duration: 'until_end_of_turn', source_block: 0 };
  if (modifier) grant.modifier = { power: pump.power, toughness: pump.toughness };
  d.spell.instructions = [grant];
  let capability = family;
  d.spell.targets[0].domain = domain;
  if (domain === 'battlefield_creature' && keywords.length === 1) {
    if (!modifier && subsetCaps[keywords[0]]) capability = subsetCaps[keywords[0]];
    if (modifier && keywords[0] === 'indestructible') capability = 'compound_indestructible_spell_v1';
  }
  d.capabilities = [...new Set(['normal_layout_v1', 'cast_instant_v1', domain === 'battlefield_creature' ? 'creature_target_v1' : 'attachments_v1', capability, ...keywords.map(k => k === 'hexproof' ? 'attachments_v1' : keywordCaps[k] ?? combatKeywordCaps[k]), ...(modifier ? [pt] : [])])];
  return freeze({ ...clone(effects), definitions: [d] });
}
// Reconstruct the historical split representation only for byte/hash comparison;
// it is never submitted as a valid control to the current schema.
function priorSplitHash(rows) {
  const prior = rows.map(({ name, value }) => {
    const copy = clone(value);
    // Presentation labels were added after these historical controls. Keep the
    // old executable-IR hash pinned while current schema probes retain labels.
    for (const definition of copy.definitions) delete definition.source_labels;
    const program = copy.definitions[0].spell;
    if (program?.instructions.length === 1 && program.instructions[0].op === 'grant_keywords' && program.instructions[0].modifier) split(program);
    return { name, value: copy, exact: JSON.stringify(copy) };
  });
  return createHash('sha256').update(JSON.stringify(prior)).digest('hex');
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
function boundaries(name, base, extended = false) {
  for (const cap of base.definitions[0].capabilities) {
    negative(`${name}:missing:${cap}`, base, d => { d.capabilities = d.capabilities.filter(c => c !== cap); });
  }
  for (const cap of [...Object.values(keywordCaps), ...Object.values(subsetCaps), 'compound_indestructible_spell_v1', pt, 'reach_v1', 'cast_sorcery_v1', ...(extended ? ['creature_target_v1', 'attachments_v1', family] : [])]) {
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
for (const [name, base] of [['pure', pure], ['compound', compound]]) boundaries(name, base);
function split(program, reverse = false) {
  const grant = program.instructions[0];
  const modifier = grant.modifier;
  delete grant.modifier;
  const pump = { op: 'modify_power_toughness', target_slot: grant.target_slot,
    power: modifier.power, toughness: modifier.toughness, duration: grant.duration, source_block: grant.source_block };
  program.instructions = reverse ? [grant, pump] : [pump, grant];
}
function compoundBoundaries(name, compound) {
  for (const [label, change] of [
    ['old-reversed-split', p => split(p, true)],
    ['old-ordered-split', p => split(p)],
    ['atomic-provenance', p => { p.instructions[0].source_block = 1; }],
    ['missing-power', p => { delete p.instructions[0].modifier.power; }],
    ['missing-toughness', p => { delete p.instructions[0].modifier.toughness; }],
    ['unknown-modifier-field', p => { p.instructions[0].modifier.extra = true; }],
    ['numeric-quantity', p => { p.instructions[0].modifier.power = 2; }],
    ['nonfixed-quantity', p => { p.instructions[0].modifier.power = { amount: 'source-power' }; }],
    ['noncanonical-quantity', p => { p.instructions[0].modifier.toughness = '-0'; }],
  ]) negative(`${name ? name + ':' : ''}compound:${label}`, compound, d => change(d.spell));
}
compoundBoundaries('', compound);
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
// The prior 230-probe stage used split compound IR. Counts persist; its hash
// is historical, while the atomic controls below receive a new measured hash.
const legacy = {
  probes: probes.length,
  frozen_controls_sha256: createHash('sha256').update(JSON.stringify(positives)).digest('hex'),
};
assert.equal(legacy.probes, 230);
legacy.prior_split_controls_sha256 = priorSplitHash(positives);
assert.equal(legacy.prior_split_controls_sha256, '7619a3fda04a6e23567b9a415e7ae13385ed64597e1901c46ccc6f0cbb251107');
const reviewed = [...Object.keys(keywordCaps), 'hexproof'];
assert.equal(vectors([], reviewed).length, 325);
for (const keywords of vectors([], reviewed)) {
  for (const domain of ['battlefield_creature', 'controlled_battlefield_creature']) {
    if (domain === 'battlefield_creature' && !keywords.includes('hexproof')) continue;
    for (const modifier of [false, true]) {
      positive(`${domain}:${modifier ? 'modifier' : 'pure'}:${keywords.join('+')}`, control(keywords, modifier, domain));
    }
  }
}
for (const [name, keywords, modifier, domain] of [
  ['plain-hexproof', ['hexproof'], false, 'battlefield_creature'],
  ['plain-hexproof-modifier', ['hexproof'], true, 'battlefield_creature'],
  ['controlled-indestructible', ['indestructible'], false, 'controlled_battlefield_creature'],
  ['controlled-indestructible-modifier', ['indestructible'], true, 'controlled_battlefield_creature'],
  ['controlled-mixed', reviewed, true, 'controlled_battlefield_creature'],
]) {
  const base = control(keywords, modifier, domain);
  boundaries(name, base, true);
  if (modifier) compoundBoundaries(name, base);
  const otherDomain = domain === 'battlefield_creature' ? 'controlled_battlefield_creature' : 'battlefield_creature';
  // A domain-only edit keeps the old capability set and must reject whenever
  // that changes requirements (Hexproof can supply Attachments on both sides).
  negative(`${name}:domain-capability-parity`, base, d => { d.spell.targets[0].domain = otherDomain; });
}
for (const domain of ['battlefield_creature', 'controlled_battlefield_creature']) {
  for (const keyword of reviewed) {
    const preserved = clone(control([keyword], false, domain));
    preserved.definitions[0].spell.instructions[0].source_block = 4294967295;
    positive(`${domain}:${keyword}:preserved-provenance`, preserved);
  }
}
for (const [name, extra] of [['bare', false], ['other-composition', true]]) {
  const base = control(['hexproof'], true, 'controlled_battlefield_creature');
  negative(`controlled-pump:${name}`, base, d => {
    split(d.spell);
    d.spell.instructions.pop();
    if (extra) d.spell.instructions.push({ op: 'draw_cards', count: '1', source_block: 0 });
    d.capabilities = ['normal_layout_v1', 'cast_instant_v1', 'attachments_v1', pt, ...(extra ? ['controller_draw_v1'] : [])];
  });
}
negative('plain-lifelink:spare-attachments', pure, d => { d.capabilities.push('attachments_v1'); });
// Retain stage counts, explicitly distinguishing atomic hashes from prior split IR.
const legacyFamily = {
  probes: probes.length,
  positive: probes.filter(p => p.expected).length,
  negative: probes.filter(p => !p.expected).length,
  frozen_controls_sha256: createHash('sha256').update(JSON.stringify(positives)).digest('hex'),
};
assert.deepEqual([legacyFamily.probes, legacyFamily.positive, legacyFamily.negative], [1657, 1331, 326]);
legacyFamily.prior_split_controls_sha256 = priorSplitHash(positives);
assert.equal(legacyFamily.prior_split_controls_sha256, '9e558adbff21f0f56d3ba9c898b1d9ff21b115449497c348a17c1988a5f22fc9');
const combatVectors = [
  ['haste'], ['trample'], ['first strike'],
  ['first strike', 'haste'], ['haste', 'first strike'],
  ['first strike', 'trample'], ['trample', 'first strike'],
  ['haste', 'trample'], ['trample', 'haste'],
  ['haste', 'flying'], ['deathtouch', 'trample'],
  ['indestructible', 'first strike'], ['lifelink', 'haste'], ['hexproof', 'trample'],
  [...reviewed, ...Object.keys(combatKeywordCaps)],
  [...reviewed, ...Object.keys(combatKeywordCaps)].reverse(),
];
assert.equal(combatVectors.length, 16);
for (const keywords of combatVectors) {
  for (const domain of ['battlefield_creature', 'controlled_battlefield_creature']) {
    for (const modifier of [false, true]) {
      positive(`combat:${domain}:${modifier ? 'modifier' : 'pure'}:${keywords.join('+')}`, control(keywords, modifier, domain));
    }
  }
}
for (const domain of ['battlefield_creature', 'controlled_battlefield_creature']) {
  for (const modifier of [false, true]) {
    const name = `combat:${domain}:${modifier ? 'modifier' : 'pure'}`;
    const base = control(['first strike', 'haste'], modifier, domain);
    boundaries(name, base, true);
    if (modifier) compoundBoundaries(name, base);
    for (const capability of Object.values(combatKeywordCaps)) {
      if (!base.definitions[0].capabilities.includes(capability)) {
        negative(`${name}:spare:${capability}`, base, d => { d.capabilities.push(capability); });
      }
    }
    for (const invalid of [null, 'haste', [{ haste: null }], ['first strike', 'haste', 'first strike'], ['haste', 'defender'], ['haste', 'double strike']]) {
      negative(`${name}:malformed-keywords:${JSON.stringify(invalid)}`, base, d => { d.spell.instructions.at(-1).keywords = clone(invalid); });
    }
    for (const keyword of Object.keys(combatKeywordCaps)) {
      const preserved = clone(control([keyword], false, domain));
      preserved.definitions[0].spell.instructions[0].source_block = 4294967295;
      positive(`${name}:${keyword}:preserved-provenance`, preserved);
    }
  }
}
// The 1,955 prior-format probes are not claimed byte-identical: compound
// positives now use atomic IR and split-only mutations became genuine negatives.
assert.equal(probes.length, 1955);
const atomicPriorStages = { probes: probes.length,
  prior_split_controls_sha256: priorSplitHash(positives),
  frozen_controls_sha256: createHash('sha256').update(JSON.stringify(positives)).digest('hex') };
for (const domain of ['battlefield_creature', 'controlled_battlefield_creature']) {
  const keywords = ['flying', 'lifelink'];
  const absent = clone(control(keywords, false, domain));
  absent.definitions[0].spell.instructions[0].source_block = 1;
  positive(`${domain}:pure:absent-modifier:block1`, absent);
  const explicitNone = clone(absent);
  explicitNone.definitions[0].spell.instructions[0].modifier = null;
  positive(`${domain}:pure:null-modifier:block1`, explicitNone);
  negative(`${domain}:pure:null-modifier:spare-pt`, explicitNone, d => { d.capabilities.push(pt); });
  for (const [power, toughness] of [['0', '0'], ['-2', '3'], ['999999999999999999999999999999999999999', '-999999999999999999999999999999999999999']]) {
    const value = clone(control(keywords, true, domain));
    value.definitions[0].spell.instructions[0].modifier = { power, toughness };
    positive(`${domain}:atomic:exact:${power}:${toughness}`, value);
  }
  const atomic = control(keywords, true, domain);
  for (const invalid of [{}, { power: '1' }, { toughness: '1' }, { power: '1', toughness: '1', extra: true }, [], '1', 1, false]) {
    negative(`${domain}:atomic:bad-modifier:${JSON.stringify(invalid)}`, atomic, d => { d.spell.instructions[0].modifier = clone(invalid); });
  }
  for (const reverse of [false, true]) for (const blocks of [[0, 0], [0, 1], [1, 0], [1, 1]]) {
    negative(`${domain}:old-split:${reverse}:${blocks}`, atomic, d => {
      split(d.spell, reverse);
      d.spell.instructions.forEach((instruction, index) => { instruction.source_block = blocks[index]; });
    });
  }
}
assert.equal(probes.length, 1999);
for (const row of positives) assert.equal(JSON.stringify(row.value), row.exact, `mutated positive: ${row.name}`);
const summary = {
  format_version: 2,
  prior_split_format: { format_version: 1, probes: 1955 },
  atomic_prior_stages: atomicPriorStages,
  status: 'prepared_only',
  legacy,
  legacy_family: legacyFamily,
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
