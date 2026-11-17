// Separate synthetic IR inventory. Native/schema execution is UNRUN by A.
// --prepare-only constructs payloads without Ajv, schema export or native work.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { resolve, join } from 'node:path';
const frozen = JSON.parse(readFileSync(new URL('../../../tests/fixtures/expected-effects-ir.json', import.meta.url), 'utf8'));
const clone = value => structuredClone(value);


const probes=[],positives=[];
function positive(name,value) {const copy=clone(value);positives.push({name,value:copy,exact:JSON.stringify(copy)});probes.push({name,value:copy,expected:true});}
function negative(name,base,change) {const value=clone(base);change(value.definitions[0]);probes.push({name,value,expected:false});}
function control(domain,modifier,keywords){const d=clone(frozen.definitions.find(d=>d.name==='Giant Growth'));let set=domain.includes('_creatures')||domain==='controller_permanents';d.spell.targets=set?[]:[{slot:0,selection_id:'line:0:0',chooser:'controller',stage:'announcement',domain:domain==='artifact_or_creature'?'battlefield_permanent':domain}];const i={op:set?'grant_keywords_set':'grant_keywords',keywords:clone(keywords),duration:'until_end_of_turn',source_block:0};if(set)i.set=domain;else i.target_slot=0;if(modifier)i.modifier={power:'-1',toughness:'3'};if(domain==='artifact_or_creature'||domain==='controlled_battlefield_permanent')d.spell.targets[0].filter={types:domain==='artifact_or_creature'?['creature','artifact']:[],non_types:[],non_colors:[],opponent:false};d.spell.instructions=[i];d.capabilities=['normal_layout_v1','cast_instant_v1','reach_v1'];if(set)d.capabilities.push('board_wipes_v1',domain==='controller_permanents'?'temporary_permanent_set_keyword_program_v1':'temporary_creature_set_keyword_program_v1');else if(domain==='artifact_or_creature'||domain==='controlled_battlefield_permanent')d.capabilities.push('permanent_removal_v1','temporary_permanent_keyword_program_v1');else{d.capabilities.push('temporary_creature_keyword_program_v1',domain==='battlefield_creature'?'creature_target_v1':'attachments_v1');if(modifier)d.capabilities.push('temporary_power_toughness_v1');}if(keywords.includes('flying'))d.capabilities.push('flying_v1');if(keywords.includes('hexproof'))d.capabilities.push('attachments_v1');d.capabilities=[...new Set(d.capabilities)];return{...clone(frozen),definitions:[d]};}
for(const domain of ['battlefield_creature','controlled_battlefield_creature','artifact_or_creature','controlled_battlefield_permanent','controller_creatures','attacking_creatures','controller_permanents'])for(const keywords of [['reach'],['reach','flying'],['hexproof','reach']])for(const modifier of [false,true]){if(modifier&&['artifact_or_creature','controlled_battlefield_permanent','controller_permanents'].includes(domain))continue;const base=control(domain,modifier,keywords),label=`reach:${domain}:${modifier}:${keywords}`;positive(label,base);for(const cap of base.definitions[0].capabilities)negative(`${label}:missing:${cap}`,base,d=>d.capabilities=d.capabilities.filter(c=>c!==cap));negative(`${label}:duplicate`,base,d=>d.spell.instructions[0].keywords.push('reach'));negative(`${label}:unsupported`,base,d=>d.spell.instructions[0].keywords.push('vigilance'));negative(`${label}:spare-subset`,base,d=>d.capabilities.push('temporary_flying_spell_v1'));negative(`${label}:duration`,base,d=>d.spell.instructions[0].duration='forever');}
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
