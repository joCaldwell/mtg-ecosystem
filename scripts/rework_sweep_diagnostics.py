"""Diagnostic attribution with the original Sweep 5 family boundaries.

The archived Sweep 5 family rules preserve the sole-family Other bucket of
1,838 cards and 570 slots. The sub-row patterns partition that bucket to match
the plan's approximate card shares for continuous effects (35%), references
and operands (25%), verbs (11%), and token specifications (11%).
"""
import re

TARGET = 'Target and selection operands'
EVENT = 'Observed events and trigger predicates'
CONTROL = 'Control flow and step results'
STATIC = 'Continuous effects and granted abilities'
ENTRY = 'As-enters choices and entry replacements'
REPLACE = 'Replacement and prevention'
CAST = 'Casting and activation costs'
LIBRARY = 'Search, reveal and library operations'
ZONE = 'Zone movement and return-to-hand operands'
COUNTERS = 'Counter kinds and counter movement'
STACK = 'Stack operations and cascade'
DELAYED = 'Delayed and reflexive triggers'
PERMISSION = 'Other-zone play and linked permissions'
TOKENS = 'Token definitions, copies and token verbs'
ACTIONS = 'Other keyword actions'
DESIGNATION = 'Monarch and Initiative'
LAYOUT = 'MDFC, transform and split'
OTHER_LAYOUT = 'Other layouts'
SAGA = 'Sagas and Classes'
WALKER = 'Planeswalkers'
CHAR = 'Characteristic shells'
MANA = 'Mana costs, production and spending'
KEYWORD = 'Other keyword abilities'
ATTACH = 'Attachments'
HAND = 'Draw, discard and hand selection'
UNASSIGNED = 'Other or underspecified Rust diagnostics'


def walk(node):
    if isinstance(node, dict):
        yield node
        for child in node.values():
            yield from walk(child)
    elif isinstance(node, list):
        for child in node:
            yield from walk(child)


def shell(source):
    layout, types = source['layout'], source['type_line']
    if 'Planeswalker' in types.split(' // ')[0].split(' — ')[0].split():
        return WALKER
    if layout in {'saga', 'class'} or re.search(r'\b(?:Saga|Class)\b', types.partition(' — ')[2]):
        return SAGA
    if layout in {'modal_dfc', 'transform', 'split'}:
        return LAYOUT
    return OTHER_LAYOUT


def families(code, kind, block, source, abilities):
    """Return independently required observed scopes. Ambiguous codes stay broad or unassigned."""
    if code.startswith(('resolution:', 'archive_excluded:', 'status:')) or code in {'not in archive', 'archive:missing'}:
        return set()
    if any(code.startswith(f'ref:{x}') for x in (
        'unsupported-', 'unresolved-', 'ambiguous-', 'missing-', 'cardinality-mismatch',
    )):
        return set()
    # Raw unresolved cross-block/ref names have no lowering-context tail and
    # are excluded by the caller whenever the R5 row lists that exact residual.
    ability = abilities[block] if block is not None and block < len(abilities) else {}
    if code.startswith('compose.block.'):
        if ability.get('kind') in {'activated', 'keywords', 'additional-cost'}:
            return {CAST}
        if ability.get('kind') == 'triggered':
            return {EVENT}
    if code == 'compose.step.target' and any(n.get('effect') == 'fight' for n in walk(ability)):
        return {'Fight'}
    if code.startswith(('source.layout:', 'layout:', 'multi_face_layout:')):
        return {shell(source)}
    if code.startswith(('source.power:', 'source.toughness:', 'source.type_line', 'cell:shell', 'subtype:')):
        return {CHAR}
    if code in {'compose.step.keyword-action', 'compose.step:keyword_action_shape_outside_the_lowering_mirror'}:
        actions = {n['action'] for n in walk(ability if block is not None else abilities)
                   if n.get('effect') == 'keyword-action' and isinstance(n.get('action'), str)}
        return {action(a) for a in actions} or {ACTIONS}
    if re.search(r'become-monarch|take-initiative', code):
        return {DESIGNATION}
    if code == 'static:conditional:designation':
        marks = {str(v).lower() for node in walk(ability) for v in node.values() if isinstance(v, str)}
        return {DESIGNATION} if marks & {'monarch', 'initiative'} else {ACTIONS}
    if code == 'keyword:ascend':
        return {ACTIONS}
    if re.search(r'(^|[.:_-])(proliferate|populate|investigate|fight)([.:_:-]|$)', code):
        return {action(re.search(r'(proliferate|populate|investigate|fight)', code)[1])}
    if re.search(r'(delayed|reflexive)[-_]trigger', code):
        return {DELAYED}
    if re.search(r'planeswalker|loyalty', code):
        return {WALKER}
    if code in {'compose.step.predefined', 'compose.step.abilities'} and any(n.get('effect') == 'create-token' for n in walk(ability)):
        return {TOKENS}
    if re.search(r'(create-token|create_token|copyOf|token_controller|token_product|amass|offspring|token_definition)', code):
        return {TOKENS}
    if re.search(r'(copy-stack|copy_stack|choose-new-targets|keyword:cascade|verb:counter:|counter_shape|^compose\.step\.counter$|^compose\.step\.nonTypes$)', code):
        return {STACK}
    if re.search(r'(play-card|play_card|play-permission|linked-exile|hideaway|^static:look-permission|^ref:linked)', code):
        return {PERMISSION}
    if re.search(r'(entry-choice|entry-copy|entry-replacement|enters-with-counters|enters-tapped|as-enters|entering-controller|entering-object|opening-hand|cross-block:value-choice)', code):
        return {ENTRY}
    if re.search(r'(replacement|prevent-damage|redirect-damage|regenerate)', code):
        return {REPLACE}
    if re.search(r'(^ref:attached-to|keyword:(equip|enchant|reconfigure|bestow|fortify)|compose\.step\.attach|compose\.block\.(equipped|enchanted|attached))', code):
        return {ATTACH}
    if re.search(r'(source\.mana_cost|^mana:|^cell:mana|taps?-for-mana|verb:add-mana|compose\.step\.combination)', code):
        return {MANA}
    if re.search(r'(additional-cost|alternative-cost|cost-modification|cost_modification|costModifications|^cost:|^compose\.cost|^cell:cost|^restriction:|keyword:(affinity|convoke|improvise|delve|overload|kicker|multikicker|flashback|evoke|buyback|emerge|escalate|replicate|retrace|suspend|rebound|gift|ward|escape|typecycling|cycling|morph|foretell|madness|splice|jump-start|spectacle|blitz|dash|mutate))', code):
        return {CAST}
    if re.search(r'(verb:(search|look-at|reveal|shuffle|additional-land-plays)|compose\.step\.(scan-library|shuffle-into-library|look|reveal|separate-piles)|shared-land-type-search|operand:destination:library)', code):
        return {LIBRARY}
    if re.search(r'(put[_-]counters|remove[_-]counters|move[_-]counters|get[_-]counters|compose\.step\.(adapt|monstrosity)|verb:.*:counter_kind)', code):
        if not code.startswith('compose.block.'):
            return {COUNTERS}
    if re.search(r'(verb:(return-to-hand|move-zone|exile)|compose\.step\.(exile-until|controller|entry)$|operand:destination:(battlefield|graveyard|exile)|operand:exile_zone_owner)', code):
        return {ZONE}
    if code == 'abilities.condition:unsupported_complete_intervening_if_condition':
        return {EVENT}
    if re.search(r'(ref:(event-object|event-player)|^trigger:|^cell:trigger|^abilities\.trigger|^compose\.block\.(enters|leaves|dies|blocks|discard|gains-life|player-attacks|counter-change|exile|move-zone|tap-objects|occurrence|create-token|draws|expends|casts|becomes-target|is-dealt-damage|discards|return|subtypes|taps?|state|put-into-zone|player-draws|event-gated)|^static:additional-triggers|^compose\.step:step_table_rows_require_a_table_event)', code):
        return {EVENT}
    if re.search(r'(ref:((source|evaluated|target):static|controller:[^:]+:static)|^static:|^cell:static|^static\.|^abilities\.static|^compose\.step\.(modify-characteristics|gain-control|become|cant|combat-rule|untap-step-suppression|player-gain-abilities)|^compose\.block\.player-combat-rule)', code):
        return {STATIC}
    if re.search(r'(draw:|discard:|discard_shape|^verb:discard|^verb:loot|^compose\.step\.discard)', code):
        return {HAND}
    if re.search(r'(ref:(step-result|bound-var|linked-pool|derived|evaluated|commander-set)|compose\.step\.(optional|forEach|for-each|condition|pay-cost|upTo|choose-value|choose-random|allowRepeat|otherwise)|^compose\.(effects|control)|^abilities\.condition|^bindings|^operand:bindings|^forEach:|verb:(modal|simultaneous)|modal_shape|:(?:optional|required_condition)$|^lines:parsed_block_grouping)', code):
        return {CONTROL}
    if re.search(r'(^keyword:|^cell:keyword|^source.keywords|^abilities\.keywords)', code):
        return {KEYWORD}
    if code == 'ref:source:triggered':
        return {EVENT}
    if re.search(r'(^operand:(target|creature_target|set|filter|amount|player|damage_recipient)|^compose\.step\.(declare-target|choose-object|attribute|divided|target|noun)|^(target|targets|filter|amount|set|who):|sacrifice_shape|pump_shape|^ref:(source|target|controller):|^abilities\.effects\.source)', code):
        return {TARGET}
    if re.search(r'(^compose\.step\.(explore|goad|connive|suspect|support|surveil|scry|learn|venture|discover|adapt)|^verb:(explore|goad|connive|suspect|support|surveil|scry|learn|venture|discover))', code):
        return {ACTIONS}
    return {UNASSIGNED}


def action(action_name):
    if action_name in {'proliferate', 'populate', 'investigate', 'fight'}:
        return action_name.capitalize()
    if action_name in {'become-monarch', 'take-initiative', 'monarch', 'initiative'}:
        return DESIGNATION
    return ACTIONS


PHASES = ('read', 'resolve', 'elaborate', 'check', 'admission')


def archive_residuals(row):
    resolution = row.get('resolution') or {}
    residuals = set(resolution.get('residuals', []))
    for face in resolution.get('faces', []):
        for block in face.get('blocks') or []:
            residuals.update(block['residuals'])
            residuals.update(f"ref:{ref['unresolved']}" for ref in block['refs'] if 'unresolved' in ref)
    if row['status'] == 'unsupported':
        residuals.add('status:unparsed')
    if row['status'] == 'excluded':
        residuals.add(f"archive_excluded:{row['reason']}")
    if row['status'] == 'parsed' and 'resolution' in row and not resolution.get('complete') and not residuals:
        residuals.add('resolution:incomplete-without-enumerated-residual')
    return residuals


def archive_diagnostic(code, residuals):
    return (code in residuals or code.startswith(('resolution:', 'archive_excluded:', 'status:'))
            or code in {'not in archive', 'archive:missing'}
            or code.startswith(('ref:unsupported-', 'ref:unresolved-', 'ref:ambiguous-',
                                'ref:missing-', 'ref:cardinality-mismatch'))
            or code in {'compose.effects:a_printed_number_is_absent_from_the_ast',
                        'abilities.abilityWord:empty_or_unprinted_annotation'}
            or (code.startswith('lines:') and not code.startswith('lines:parsed_block_grouping')))


def rejection_phase(code, residuals):
    """Use archive evidence before the primary reason's phase or Rust code."""
    if 'status:unparsed' in residuals or any(r.startswith('archive_excluded:') for r in residuals):
        return 'read'
    if residuals:
        return 'resolve'
    label = code.partition(':')[0]
    if label in PHASES:
        return label
    # A Rust reference/operand error after complete archive resolution is an
    # elaboration failure unless the producer explicitly tags its phase.
    if (code.startswith(('cell:', 'definition:', 'validation:'))
            or any(marker in code for marker in ('kernel_cell_is_not_audited', 'kernel_operand_type',
                                                'kernel_count_over_a_filtered_hidden_zone'))):
        return 'check'
    return 'elaborate'


# First-match code classes within the original sole-Other family. Count a
# multi-code card once. Unanimous in-scope classes earn a subrow, with mixed
# and other diagnostics grouped in the remainder.
OTHER_CLASSES = {
    'Continuous effects created at resolution': re.compile(
        r'(^static[:.]|^cell:static|^abilities\.static|^compose\.step\.(modify-characteristics|gain-control|become|cant|combat-rule|untap-step-suppression|player-gain-abilities|set-pt|switch-pt|multiply-pt|colors|change-text|exchange-control|phase-out|player-rule|detain|remove-from-combat|base|lose-counters)$'
        r'|(become|modify_characteristics|gain_abilities|lose_abilities|combat_rule|cant)_shape|^verb:(gain-abilities|lose-abilities|become|modify-characteristics|combat-rule|gain-control|cant)|^operand:granted_|keyword_grant|combat_prohibition|characteristic_setter|^compose\.block\.player-combat-rule)'),
    'References and operands': re.compile(
        r'(^ref:|^bindings|^operand:|trigger\.what:event_object_|abilities\.effects\.source|damage\.source|verb:damage:source'
        r'|^compose\.step\.(noun|who|from|choose-object|declare-target|attribute|divided|target|count|product|minus|plus|half|minimum|maximum|of|each|counters|controller|distinct-count|opponent-count|counter-count|within|chosen|control|enchanted|attacked|plural|factors|different-colors|multicolored|colorless|nonColors|color-among|manaCostContains|abilityKind|abilitySource|maxTotal|damage-history-total|trigger-event-color|that|keptUntil)$'
        r'|^(target|targets|filter|amount|set|who):|_shape_outside_the_lowering_mirror|^verb:(sacrifice|draw|discard|destroy|damage|mill|tap|untap|put-counters|remove-counters|gain-life|lose-life|exile|return-to-hand|move-zone|pump|loot|attach|fight|transform|scry|surveil|keyword-action)|^composition:|^cell:)'),
    'Verbs with no IR': re.compile(r'^(compose\.step\.|verb:)'),
    'Token specifications': re.compile(
        r'(create-token|create_token|copyOf|token_controller|token_product|amass|offspring|token_definition|^operand:token|^effects\.token|compose\.step\.(predefined|abilities|create-role|incubate|copy-card))'),
    'Remainder': None,
}

# Diagnostics that the RFC classifies outside these four subrows. Match
# before the broad operand and unknown-verb patterns.
REMAINDER = re.compile(
    r'(^operand:(duration|characteristic_duration|mana|chosen_cards)|^compose\.block[.:]|^compose\.(kind|control|effects)|^abilities\.(condition|trigger)|^trigger[:.]|^cell:trigger|outside_the_block_tables'
    r'|delayed[-_]trigger|reflexive[-_]trigger|replacement|prevent-damage|redirect-damage|regenerate|prevention_shape'
    r'|copy-stack|copy_stack|choose-new-targets|change-targets|compose\.step\.(countered|modify-spells-cast|scan-library|shuffle-into-library|look|reveal|separate-piles|manifest|manifest-dread|cloak|faceDown|turn-face|combination|produced|scaled-symbol|lose-unspent-mana|double-mana-pool)$'
    r'|^verb:(search|look-at|reveal|shuffle|additional-land-plays|add-mana|modal|simultaneous|choose-value)|^compose\.step\.(optional|forEach|for-each|condition|pay-cost|upTo|choose-value|choose-random|allowRepeat|otherwise|choice|repeat-process|repeatWhile|afterConditional|anyOf|vote|secret-choice-damage|flip-coin|roll-dice|random|unpreventable)$)')


def other_class(codes):
    continuous, references, verbs, tokens, remainder = OTHER_CLASSES

    def classify(code):
        if code.startswith('trigger.what:event_object_'):
            return references
        if REMAINDER.search(code):
            return remainder
        for label in (tokens, continuous, references, verbs):
            if OTHER_CLASSES[label].search(code):
                return label
        return remainder

    classes = {classify(code) for code in codes}
    return classes.pop() if len(classes) == 1 else remainder


def sole_other(row, observations, primary, residuals):
    if residuals:
        return None
    abilities = [ability for face in row.get('faces', [])
                 for ability in face.get('result', {}).get('abilities', [])]
    if all(code != primary for code, _, _ in observations):
        observations = [*observations, (primary, 'primary', None)]
    codes = set()
    for code, kind, block in observations:
        if archive_diagnostic(code, residuals):
            return None
        if families(code, kind, block, row['source'], abilities) - {UNASSIGNED}:
            return None
        codes.add(code)
    return other_class(codes)
