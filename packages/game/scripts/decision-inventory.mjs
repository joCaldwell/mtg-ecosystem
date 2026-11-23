// Explicit client-only inventory. Reads qualified generated SDK contracts and
// pinned public evidence; never invokes Cargo, installs, fetches or changes pins.
import { createHash } from 'node:crypto';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import ts from 'typescript';
const client = new URL('../', import.meta.url);
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
const openingProvenancePath = 'fixtures/foundations-opening/provenance.json';
const openingProvenanceBytes = await readFile(new URL(openingProvenancePath, client));
const provenance = JSON.parse(openingProvenanceBytes);
for (const file of provenance.files) if (hash(await readFile(new URL(`fixtures/foundations-opening/${file.path}`, client))) !== file.sha256) throw new Error('Inventory fixture provenance mismatch');
const policies = {};
function group(handler, names, control, required_context) {
  for (const name of names.split(' ')) policies[name] = { handler, control, required_context };
}
group('decisionSurface', 'mulligan', 'implemented-confirmation', 'Captured request can_mulligan, mulligans_taken, bottom_if_mulligan; no source semantics inferred.');
group('decisionSurface', 'bottom_opening', 'implemented-ordered-handles', 'Supplied candidates/count; permitted object-to-presentation mapping or explicit missing-metadata handle fallback.');
group('decisionSurface', 'discard resolve_discard', 'implemented-unordered-handles', 'Supplied candidates/count; permitted object labels. Initiating effect/source explanation is still absent.');
group('decisionSurface', 'legend_rule', 'implemented-single-handle', 'Supplied candidates and previous_choices; permitted object labels; no legend eligibility calculation.');
group('decisionSurface', 'sacrifice sacrifice_cost forced_sacrifice', 'implemented-single-handle', 'Supplied candidates; permitted object labels. Rich initiating source/cost/effect and previous choice presentation remain integration requirements.');
group('structuredDecisionSurface', 'announce_x', 'implemented-source-quantity', 'Requires producer spell source text; explicit canonical X input, never a derived maximum.');
group('structuredDecisionSurface', 'mana_color', 'implemented-source-color', 'Requires producer mana-ability source text and supplied colors. No automatic first color.');
group('structuredDecisionSurface', 'optional_effect', 'implemented-source-confirmation', 'Requires producer source and current instruction text before explicit perform/decline.');
group('structuredDecisionSurface', 'modes', 'implemented-source-mode', 'Requires producer source and one unambiguous compiler label per supplied mode candidate.');
group('structuredDecisionSurface', 'order_triggers', 'implemented-source-ordered-triggers', 'Requires compiler-bound labels for every supplied trigger handle; preserve explicit complete order.');
group('structuredDecisionSurface', 'damage_effect', 'implemented-source-damage-effect', 'Requires compiler-bound per-occurrence effect labels; preserve exact effect/damage pair and supplied amounts.');
group('structuredDecisionSurface', 'commander_return', 'implemented-commander-destination', 'Supplied object/designation/destination and previous choices only; explicit command-zone or destination choice.');
group(null, 'opening_placement graveyard_replacement_order additional_costs entry_copy entry_attachment hybrid_cost choose_mana_ability mana_combination entry_reveal counter_placement_effect', 'unsupported', 'The client has no reviewed draft for this request.');
group(null, 'priority mana', 'session-action-query', 'Bound authenticated query, complete original generation/scope and producer available labels/source. Pass needs priority/window; unexplained actions remain blocked. FinishMana preserves conditional payment-failure reversal.');
group('structuredDecisionSurface', 'spend_mana', 'implemented-source-payment', 'Requires producer source and supplied cost/available mana; explicit exact amount for every available color. Core validates joint payment legality.');
group('structuredDecisionSurface', 'targets entity_targets player_targets stack_targets', 'implemented-source-target-occurrences', 'Requires producer source/instruction; explicit candidate for each supplied ordered occurrence. Never interpret TargetSpec or derive joint eligibility.');
group('structuredDecisionSurface', 'attackers blockers', 'implemented-supplied-combat-mapping', 'Explicit per-candidate attacker/defender or blocker/attacker choices, including unused. Missing/ambiguous blocker eligibility fails closed. Core validates joint restrictions.');
group('structuredDecisionSurface', 'damage', 'implemented-supplied-damage-allocation', 'Supplied source, recipients, power, player recipient and lethal hints; exact total bookkeeping only, no derived lethal calculation.');
const observed = {
  mulligan: 'Historical authenticated Foundations opening and separately retained opening captures. Bounded captures only.',
  priority: 'Historical Foundations and attachments main-priority-before-land/main-priority-before-cast captures, plus core query report. Bounded prefixes only.',
  mana: 'Historical Foundations and attachments cast-mana captures, plus core query report. Bounded prefixes only.',
};
const reportPath = 'docs/game-client.md';
const prefixProvenancePath = 'fixtures/decision-controls/prefix-provenance.json';
const prefixProvenanceBytes = await readFile(new URL(prefixProvenancePath, client));
const prefixes = JSON.parse(prefixProvenanceBytes);
const capturedRequests = new Set(prefixes.captures.flatMap(capture => capture.stages.map(stage => stage.request)));
const capturedActions = new Set(prefixes.captures.flatMap(capture => capture.stages.flatMap(stage => stage.action_families)));

function schemaDigest(variant) {
  // Preserve ordered arrays such as tuple items. Required field names are a set.
  return hash(JSON.stringify(variant, (key, value) => {
    if (Array.isArray(value)) return key === 'required' ? [...value].sort() : value;
    if (value && typeof value === 'object') {
      return Object.fromEntries(Object.keys(value).sort().map(name => [name, value[name]]));
    }
    return value;
  }));
}
const variants = (definition, tag) => definition.oneOf.map(variant => ({
  variant: variant.properties[tag].const,
  fields: Object.keys(variant.properties).sort(),
  required: [...variant.required].sort(),
  schema_sha256: schemaDigest(variant),
}));
const requests = variants(schema.resume.definitions.PlayerRequest, 'request');
const responses = variants(schema.command.definitions.PlayerResponse, 'response');
const sameVariants = (actual, expected) => JSON.stringify([...actual].sort()) === JSON.stringify([...expected].sort());
const requestNames = requests.map(row => row.variant);
if (!sameVariants(requestNames, Object.keys(policies))) throw new Error('New or removed wire request needs explicit coverage review');
const extraResponses = ['activate_ability', 'trample_damage', 'mode_subset', 'spend_mana_allocated', 'bestow', 'play_back_land', 'play_land', 'activate_mana', 'cast', 'pass', 'finish_mana'];
if (responses.some(row => !policies[row.variant] && !extraResponses.includes(row.variant))) throw new Error('New wire response needs explicit coverage review');

async function verifyRequestSwitches(path, handlers, expected) {
  const source = ts.createSourceFile(path, await readFile(new URL(path, client), 'utf8'), ts.ScriptTarget.Latest);
  for (const handler of handlers) {
    const declaration = source.statements.find(node => ts.isFunctionDeclaration(node) && node.name?.text === handler);
    const parameter = declaration?.parameters[0]?.name;
    const requestSwitch = declaration?.body?.statements.find(node => {
      if (!ts.isSwitchStatement(node) || !ts.isPropertyAccessExpression(node.expression)) return false;
      const { expression: receiver, name } = node.expression;
      return parameter && ts.isIdentifier(parameter) && ts.isIdentifier(receiver) && receiver.text === parameter.text && name.text === 'request';
    });
    const cases = requestSwitch?.caseBlock.clauses.filter(ts.isCaseClause);
    if (!cases || !cases.every(clause => ts.isStringLiteralLike(clause.expression)) ||
        !sameVariants(cases.map(clause => clause.expression.text), expected)) {
      throw new Error(`${path}#${handler} request switch needs explicit coverage review`);
    }
  }
}
await verifyRequestSwitches('web/src/decisions/controls.ts', ['decisionSurface', 'completeDecision'], requestNames);
await verifyRequestSwitches('web/src/decisions/structured-controls.ts', ['structuredDecisionSurface', 'completeStructuredDecision'],
  requestNames.filter(name => policies[name].handler === 'structuredDecisionSurface'));

const requestRows = requests.map(row => {
  const policy = policies[row.variant];
  return { ...row,
    known_admitted_reachability: observed[row.variant] ?? 'Not established by retained permitted captures or bounded admitted-prefix evidence. Schema presence is not admission.',
    reachability_evidence: observed[row.variant] ? [reportPath, ...(row.variant === 'mulligan' ? [`packages/game/${openingProvenancePath}`] : []), ...(capturedRequests.has(row.variant) ? [`packages/game/${prefixProvenancePath}`] : [])] : [],
    implemented_control_draft: policy.control, local_control_handler: policy.handler, required_core_context_and_source_mapping: policy.required_context,
    permitted_fixture: row.variant === 'mulligan' ? 'fixtures/foundations-opening/opening-seat0-http.json (exact historical capture); retained opening prefixes in prefix-provenance.json (evidence only)' : capturedRequests.has(row.variant) ? 'Historical admitted prefix observations/queries in prefix-provenance.json (evidence only; no runtime query integration)' : 'No captured request in this slice. Structural tests, where present, do not establish reachability.',
    test_locator: policy.control.startsWith('implemented') ? 'tests/decision-controls.test.ts; tests/structured-controls.test.ts; tests/browser/decision-controls.spec.ts (captured opening and synthetic controls only)' : 'src/decisions/controls.ts exhaustive generated-type switch; missing-context unit/browser cases cover representative gaps only',
    gallery_locator: row.variant === 'mulligan' ? '/fixtures/decisions#captured' : ['discard', 'bottom_opening'].includes(row.variant) ? '/fixtures/decisions#structural (synthetic only)' : row.variant === 'modes' ? '/fixtures/decisions#blocked (synthetic only)' : null,
  };
});
const output = {
  inventory_version: 2, implementation_base: 'ca18eb653384b3f9a58758f596c6634051b67f78',
  qualification: { ...pins, sdk_build_manifest_sha256: hash(await readFile(new URL('dist/build.json', sdk))), sdk_generation_sha256: hash(generation) },
  capture_qualification: { provenance: openingProvenancePath, provenance_sha256: hash(openingProvenanceBytes), native_source: provenance.native_source, protocol_schema_sha256: provenance.protocol_schema_sha256, catalog_sha256: provenance.catalog_sha256, note: 'Historical bytes/provenance preserved; validated against current SDK schema separately.' },
  scope: 'Local drafts only. No send, legality, whole-profile reachability, full M2 or playability claim.',
  fresh_producer_evidence: { provenance: prefixProvenancePath, provenance_sha256: hash(prefixProvenanceBytes), producer_protocol_schema_sha256: prefixes.producer_protocol_schema_sha256, public_prefixes_sha256: prefixes.public_prefixes_sha256, native_source_manifest_sha256: prefixes.native_source_manifest_sha256, note: 'Historical producer capture with its own schema and source pins. Only mulligan/priority/mana requests observed; action beginnings do not prove accepted responses.' },
  requests: requestRows,
  responses: responses.map(row => ({ ...row, request_variant: policies[row.variant] ? row.variant : null,
    implemented_control_draft: policies[row.variant]?.control ?? 'unsupported-action-or-combat-response',
    known_admitted_reachability: capturedActions.has(row.variant) ? 'Historical admitted prefix queries expose this initiation family (prefix-provenance.json); no accepted-response or client-control proof follows.' : observed[row.variant] ?? 'Not established by this slice; response schema does not prove admission.',
    required_core_context_and_source_mapping: policies[row.variant]?.required_context ?? 'Core query/decision context and source labels; response variant alone is not an action menu.',
    permitted_fixture: row.variant === 'mulligan' ? 'Captured request only; completion is local human draft, not an accepted native command.' : null,
    test_locator: policies[row.variant]?.control.startsWith('implemented') ? 'tests/decision-controls.test.ts; tests/structured-controls.test.ts' : 'Generated SDK exhaustive consumer/typecheck; no control acceptance claimed.',
  })),
};
const bytes = JSON.stringify(output, null, 2) + '\n';
const destination = new URL('fixtures/decision-controls/coverage.json', client);
if (process.argv.includes('--write')) { await mkdir(new URL('fixtures/decision-controls/', client), { recursive: true }); await writeFile(destination, bytes); }
else if (await readFile(destination, 'utf8') !== bytes) throw new Error('Coverage inventory is stale. Review contracts/evidence, then run this script with --write explicitly.');
console.log(`Qualified inventory: ${requests.length} requests, ${responses.length} responses; ${requestRows.filter(row => row.implemented_control_draft.startsWith('implemented')).length} local adapters. No admission inferred.`);
