// Explicit client-only inventory. Reads qualified generated SDK contracts and
// pinned public evidence; never invokes Cargo, installs, fetches or changes pins.
import { createHash } from 'node:crypto';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
const client = new URL('../', import.meta.url);
const repo = new URL('../../../', import.meta.url);
const sdk = new URL('../../game-server/sdk/', import.meta.url);
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const readJSON = async url => JSON.parse(await readFile(url, 'utf8'));
const pins = await readJSON(new URL('sdk-qualification.json', client));
const schemaBytes = await readFile(new URL('generated/protocol.schema.json', sdk));
if (hash(schemaBytes) !== pins.protocol_schema_sha256) throw new Error('Inventory requires the currently qualified protocol export');
const generation = await readFile(new URL('generated/generation.json', sdk));
const build = await readJSON(new URL('dist/build.json', sdk));
if (hash(generation) !== build.generation_sha256 || build.schema_sha256 !== pins.protocol_schema_sha256 || JSON.parse(generation).bootstrap_schema_sha256 !== pins.bootstrap_schema_sha256) throw new Error('Inventory SDK generation/build qualification mismatch');
for (const file of build.files) if (hash(await readFile(new URL(`dist/${file.path}`, sdk))) !== file.sha256) throw new Error('Inventory SDK artifact changed');
for (const source of build.sources) if (hash(await readFile(new URL(source.path, sdk))) !== source.sha256) throw new Error('Inventory SDK source changed');
const schema = JSON.parse(schemaBytes);
const provenance = await readJSON(new URL('fixtures/foundations-opening/provenance.json', client));
for (const file of provenance.files) if (hash(await readFile(new URL(`fixtures/foundations-opening/${file.path}`, client))) !== file.sha256) throw new Error('Inventory fixture provenance mismatch');
const policies = {};
function group(names, control, required_context) { for (const name of names.split(' ')) policies[name] = { control, required_context }; }
group('mulligan', 'implemented-confirmation', 'Captured request can_mulligan, mulligans_taken, bottom_if_mulligan; no source semantics inferred.');
group('bottom_opening', 'implemented-ordered-handles', 'Supplied candidates/count; permitted object-to-presentation mapping or explicit missing-metadata handle fallback.');
group('discard resolve_discard', 'implemented-unordered-handles', 'Supplied candidates/count; permitted object labels. Initiating effect/source explanation is still absent.');
group('legend_rule', 'implemented-single-handle', 'Supplied candidates and previous_choices; permitted object labels; no legend eligibility calculation.');
group('sacrifice sacrifice_cost forced_sacrifice', 'implemented-single-handle', 'Supplied candidates; permitted object labels. Rich initiating source/cost/effect and previous choice presentation remain integration requirements.');
group('search_lands', 'implemented-bounded-handles', 'Supplied maximum and VisibleSearchLand handle/oracle_id; caller must map permitted search-result metadata, never inspect library/IR.');
group('announce_x', 'context-missing', 'Core permitted source/action and meaning of X, plus compiler source-label mapping; quantity syntax alone is insufficient.');
group('mana_color', 'context-missing', 'Supplied colors plus permitted mana_ability_source and source/ability meaning. No automatic first color.');
group('optional_effect optional_recursion', 'context-missing', 'Core permitted effect/source and producer source-block mapping; boolean alone does not explain what is accepted.');
group('modes', 'context-missing', 'Core mode candidates plus compiler mode-index-to-source-label mapping. Numeric indices are not human choices.');
group('order_triggers', 'context-missing', 'VisiblePendingTrigger identity plus compiler ability/source-label mapping. Generic ordered selection exists, no trigger gameplay adapter.');
group('damage_effect damage_allocation', 'context-missing', 'Core recipient/effect/damage identities, permitted source labels and allocation constraints. No internal IR interpretation or derived damage amounts.');
group('commander_return', 'context-missing', 'Designation/object/destination and previous choices need reviewed permitted designation/zone explanation; no name-based mapping.');
group('priority mana', 'context-missing', 'Bound authenticated core action query, completeness/continuation handling and source labels. ActionLabel is Unavailable; missing mana source remains explicit.');
group('spend_mana', 'unsupported', 'Core cost/available/source context, reviewed joint payment flow and reversal semantics. No local payment-legality calculation.');
group('targets entity_targets player_targets stack_targets', 'unsupported', 'Core ordered occurrences/candidates and permitted target/source labels with joint validator; no local TargetSpec interpretation.');
group('attackers blockers damage', 'unsupported', 'Core candidate/defender/assignment and supplied lethal/legal-block context; joint combat validation and source mapping. No menace/lethal/eligibility calculation.');
const observed = {
  mulligan: 'Historical authenticated foundations opening plus fresh separately validated openings in both admitted profiles. Bounded captures only.',
  priority: 'Fresh separately validated foundations and attachments main-priority-before-land/main-priority-before-cast captures, plus core query report. Bounded prefixes only.',
  mana: 'Fresh separately validated foundations and attachments cast-mana captures, plus core query report. Bounded prefixes only.',
};
const reportPath = 'docs/game-client-core-queries.md';
const prefixes = await readJSON(new URL('fixtures/decision-controls/prefix-provenance.json', client));
const capturedRequests = new Set(prefixes.captures.flatMap(capture => capture.stages.map(stage => stage.request)));
const capturedActions = new Set(prefixes.captures.flatMap(capture => capture.stages.flatMap(stage => stage.action_families)));
const sourcePaths = [reportPath, 'docs/game-engine-current.md', 'docs/game-engine-server-attachments.md', 'packages/game-engine/crates/core/src/engine/visibility.rs', 'packages/game-client/src/decisions/controls.ts', 'packages/game-client/src/features/decisions/LocalControls.tsx', 'packages/game-client/src/features/decisions/DecisionPanel.tsx', 'packages/game-client/src/features/decisions/DecisionGallery.tsx', 'packages/game-client/src/features/decisions/decisions.module.css', 'packages/game-client/src/app/fixture.ts', 'packages/game-client/scripts/decision-inventory.mjs', 'packages/game-client/tests/decision-controls.test.ts', 'packages/game-client/tests/decision-inventory.test.ts', 'packages/game-client/tests/browser/decision-controls.spec.ts', 'packages/game-client/fixtures/foundations-opening/provenance.json'];
const evidence = [];
for (const path of sourcePaths) evidence.push({ path, sha256: hash(await readFile(new URL(path, repo))) });
for (const path of ['packages/game-client/scripts/decision-prefixes.mjs', 'packages/game-client/fixtures/decision-controls/prefix-provenance.json']) evidence.push({ path, sha256: hash(await readFile(new URL(path, repo))) });
const variants = (definition, tag) => definition.oneOf.map(variant => ({ variant: variant.properties[tag].const, fields: Object.keys(variant.properties).sort(), required: [...variant.required] }));
const requests = variants(schema.resume.definitions.PlayerRequest, 'request');
const responses = variants(schema.command.definitions.PlayerResponse, 'response');
if (JSON.stringify(requests.map(row => row.variant).sort()) !== JSON.stringify(Object.keys(policies).sort())) throw new Error('New or removed wire request needs explicit coverage review');
const extraResponses = ['activate_ability', 'trample_damage', 'play_land', 'activate_mana', 'cast', 'pass', 'finish_mana'];
if (responses.some(row => !policies[row.variant] && !extraResponses.includes(row.variant))) throw new Error('New wire response needs explicit coverage review');
const requestRows = requests.map(row => {
  const policy = policies[row.variant];
  return { ...row,
    known_admitted_reachability: observed[row.variant] ?? 'Not established by retained permitted captures or bounded admitted-prefix evidence. Schema presence is not admission.',
    reachability_evidence: observed[row.variant] ? [reportPath, ...(row.variant === 'mulligan' ? ['packages/game-client/fixtures/foundations-opening/provenance.json'] : []), ...(capturedRequests.has(row.variant) ? ['packages/game-client/fixtures/decision-controls/prefix-provenance.json'] : [])] : [],
    implemented_control_draft: policy.control, required_core_context_and_source_mapping: policy.required_context,
    permitted_fixture: row.variant === 'mulligan' ? 'fixtures/foundations-opening/opening-seat0-http.json (exact historical capture); fresh opening prefixes in prefix-provenance.json (evidence only)' : capturedRequests.has(row.variant) ? 'Fresh isolated admitted prefix observations/queries in prefix-provenance.json (evidence only; no runtime query integration)' : 'No captured request in this slice. Structural tests, where present, do not establish reachability.',
    test_locator: policy.control.startsWith('implemented') ? 'tests/decision-controls.test.ts; tests/browser/decision-controls.spec.ts (gallery covers mulligan, synthetic discard/bottom, generic quantity only)' : 'src/decisions/controls.ts exhaustive generated-type switch; missing-context unit/browser cases cover representative gaps only',
    gallery_locator: row.variant === 'mulligan' ? '/fixtures/decisions#captured' : ['discard', 'bottom_opening'].includes(row.variant) ? '/fixtures/decisions#structural (synthetic only)' : row.variant === 'modes' ? '/fixtures/decisions#blocked (synthetic only)' : null,
  };
});
const output = {
  inventory_version: 1, implementation_base: '144fe059ab2707877109d7e9dbe8751f204681dd',
  qualification: { ...pins, sdk_build_manifest_sha256: hash(await readFile(new URL('dist/build.json', sdk))), sdk_generation_sha256: hash(generation) },
  capture_qualification: { native_source: provenance.native_source, protocol_schema_sha256: provenance.protocol_schema_sha256, catalog_sha256: provenance.catalog_sha256, note: 'Historical bytes/provenance preserved; validated against current SDK schema separately.' },
  evidence, scope: 'Local drafts only. No send, legality, whole-profile reachability, full M2 or playability claim.',
  fresh_producer_evidence: { provenance: 'fixtures/decision-controls/prefix-provenance.json', producer_protocol_schema_sha256: prefixes.producer_protocol_schema_sha256, public_prefixes_sha256: prefixes.public_prefixes_sha256, native_source_manifest_sha256: prefixes.native_source_manifest_sha256, note: 'Separately validated producer query extension, not installed SDK/runtime. Existing qualification unchanged. Only mulligan/priority/mana requests observed; action beginnings do not prove accepted responses.' },
  requests: requestRows,
  responses: responses.map(row => ({ ...row, request_variant: policies[row.variant] ? row.variant : null,
    implemented_control_draft: policies[row.variant]?.control ?? 'unsupported-action-or-combat-response',
    known_admitted_reachability: capturedActions.has(row.variant) ? 'Fresh isolated admitted prefix queries expose this initiation family (prefix-provenance.json); no accepted-response or client-control proof follows.' : observed[row.variant] ?? 'Not established by this slice; response schema does not prove admission.',
    required_core_context_and_source_mapping: policies[row.variant]?.required_context ?? 'Core query/decision context and source labels; response variant alone is not an action menu.',
    permitted_fixture: row.variant === 'mulligan' ? 'Captured request only; completion is local human draft, not an accepted native command.' : null,
    test_locator: policies[row.variant]?.control.startsWith('implemented') ? 'tests/decision-controls.test.ts' : 'Generated SDK exhaustive consumer/typecheck; no control acceptance claimed.',
  })),
};
const bytes = JSON.stringify(output, null, 2) + '\n';
const destination = new URL('fixtures/decision-controls/coverage.json', client);
if (process.argv.includes('--write')) { await mkdir(new URL('fixtures/decision-controls/', client), { recursive: true }); await writeFile(destination, bytes); }
else if (await readFile(destination, 'utf8') !== bytes) throw new Error('Coverage inventory is stale. Review contracts/evidence, then run this script with --write explicitly.');
console.log(`Qualified inventory: ${requests.length} requests, ${responses.length} responses; ${requestRows.filter(row => row.implemented_control_draft.startsWith('implemented')).length} local adapters. No admission inferred.`);
