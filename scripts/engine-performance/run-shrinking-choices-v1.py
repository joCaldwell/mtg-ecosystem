#!/usr/bin/env python3
"""One fixed legal 4/3/2-seat choice matrix; no implicit native execution."""
import argparse
from collections import Counter
import hashlib
import importlib.util
import os
from pathlib import Path
import re
import sys

HERE = Path(__file__).absolute().parent
STAGE = HERE.parents[1]
ENTRY = 'match_games::ordinary_diagnostic::shrinking_choice_costs::shrinking_pod_choice_diagnostic'
MODULE = 'packages/game-engine/crates/sim/src/match_games/ordinary_diagnostic/shrinking_choice_costs.rs'
SEED = 'foundations-shrinking-choices-v1/legal-BG-W-BG-W'
LIVE = [(4, [0, 1, 2, 3]), (3, [0, 1, 3]), (2, [0, 3])]
COUNT_KEYS = 'fork_started forked initial_queries query_resumes validated_inputs submitted_inputs consumed_inputs advance_calls rule_steps fully_advanced verified'
METER_KEYS = 'counts segments requests responses processed'
REQUEST_FIELDS = {'priority': '', 'attackers': 'candidates defenders', 'mana': 'cost', 'spend_mana': 'cost available', 'player_targets': 'occurrences candidates', 'sacrifice': 'candidates', 'order_triggers': 'candidates', 'commander_return': 'object designation destination'}
RESPONSE_FIELDS = {'pass': '', 'cast': 'card', 'activate_mana': 'object', 'finish_mana': 'undo_mana', 'spend_mana': 'spending', 'player_targets': 'players', 'sacrifice': 'object', 'order_triggers': 'order', 'commander_return': 'to_command', 'attackers': 'attacks'}


def load_common(expected):
    path = HERE / 'run-definition-diversity-v1.py'
    if path.is_symlink() or not path.is_file() or hashlib.sha256(path.read_bytes()).hexdigest() != expected:
        raise ValueError('authenticated common source BEFORE import')
    spec = importlib.util.spec_from_file_location('shrinking_common', path)
    common = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = common
    spec.loader.exec_module(common)
    return common


def source_guard(args, c):
    sources = [MODULE, 'packages/game-engine/crates/sim/src/match_games/ordinary_diagnostic.rs', 'packages/game-engine/crates/sim/src/match_games.rs', 'packages/game-engine/crates/core/src/engine.rs', 'packages/game-engine/crates/core/src/engine/match_admission.rs', 'packages/game-engine/crates/core/src/engine/choices.rs']
    a, host, fixed = c.authority_guard(args.authority, args.authority_sha256, args.fixed_driver_sha256, args.helper_sha256, {'name': 'mtg-sim', 'kind': ['bin']}, Path(__file__).absolute(), sources)
    for p in [HERE / 'run-definition-diversity-v1.py', HERE / 'run-current-pods-v1.py', HERE / 'run-ordinary-diagnostic-v1.py']:
        c.need(a['host_scripts'].get(str(p)) == c.sha(c.plain(p)), 'all helper source pins')
    text = c.plain(STAGE / MODULE).read_text()
    c.need(re.findall(r'const CHOICE_SEED: &\[u8\] = b"([^"]*)";', text) == [SEED], 'one fixed source seed; no scan')
    for key, value in [('SETUP_INPUT_LIMIT', 30_000), ('CALL_LIMIT', 100_000), ('CHOICE_REPETITIONS', 6)]:
        c.need(re.findall(r'const ' + key + r': usize = ([0-9_]+);', text) == [f'{value:_}' if value >= 1000 else str(value)], 'fixed source limits/repetitions')
    pod = c.authenticated('shrinking_source_decks', HERE / 'run-current-pods-v1.py', a['host_scripts'][str(HERE / 'run-current-pods-v1.py')])
    ordinary = c.authenticated('shrinking_ordinary', HERE / 'run-ordinary-diagnostic-v1.py', a['host_scripts'][str(HERE / 'run-ordinary-diagnostic-v1.py')])
    profile, decks = pod.source_decks(STAGE)
    config = {'profile': profile, 'starting_player': 0, 'seed': list(hashlib.sha256(SEED.encode()).digest()), 'decks': [decks[1], decks[0], decks[1], decks[0]]}
    declaration = {'entrypoint': ENTRY, 'seed_domain': SEED, 'config_sha256': c.digest(config), 'live_orders': [order for _, order in LIVE], 'attack_cells': [64, 27, 8], 'edict_cells': [3, 2, 1], 'total_cells': 105, 'warmups': 1, 'measured_repetitions': 5, 'budget_oracles': [1, (1 << 64) - 1], 'reference_policy': 'complete_native_typed_proof_budget_and_repeat_equality_v1'}
    c.need(c.canonical(a['choice_diagnostic']) == c.canonical(declaration), 'explicit approved worklist/source contract, no generated reference trust')
    fixture_path = STAGE / 'packages/game-engine/tests/fixtures/expected-match-ir.json'
    catalog = c.read(fixture_path)
    fixture = {'value': catalog, 'sha256': c.sha(fixture_path), 'bytes': fixture_path.stat().st_size}
    return a, host, ordinary, {'config': config, 'catalog': catalog, 'fixture': fixture}


def expected_cells():
    result = []
    for live, seats in LIVE:
        defenders = seats[1:]
        base = len(seats)
        result += [(live, {'choice': 'attack', 'digits': [n % base, (n // base) % base, n // (base * base)]}) for n in range(base**3)]
        result += [(live, {'choice': 'edict', 'target': p}) for p in defenders]
    return result


def journal(c, value, concessions=None):
    c.shape(value, 'decisions concessions', 'full root journal')
    c.need(type(value['decisions']) is list and type(value['concessions']) is list, 'journal arrays')
    previous = -1
    for row in value['decisions']:
        c.shape(row, 'decision_id player response', 'accepted journal row')
        previous_id = c.number(row['decision_id'], 'decision id')
        c.need(previous_id > previous, 'strict accepted input order')
        previous = previous_id
        c.number(row['player'], 'input player', high=3)
        c.need(type(row['response']) is dict and type(row['response'].get('response')) is str, 'complete raw typed response')
        if row['response']['response'] in ['play_land', 'cast']:
            c.number(row['response']['card'], 'raw card response', high=399)
    for row in value['concessions']:
        c.shape(row, 'after_decisions boundary player', 'recorded concession')
        c.number(row['after_decisions'], 'concession journal position', high=len(value['decisions']))
        c.number(row['boundary'], 'concession boundary')
        c.number(row['player'], 'departing seat', high=3)
    if concessions is not None:
        c.need([r['player'] for r in value['concessions']] == concessions, 'source predeclared departure order')
    return value['decisions']


def physical_cards(config):
    cards = {}
    for seat, deck in enumerate(config['decks']):
        index = seat * 100
        cards[index] = (seat, deck['commander'])
        index += 1
        for row in deck['cards']:
            if row['oracle_id'] != deck['commander']:
                for _ in range(row['count']):
                    cards[index] = (seat, row['oracle_id'])
                    index += 1
        if index != (seat + 1) * 100:
            raise ValueError('exact physical100 per seat')
    return cards


def root(c, row, live, seats, base_journal, selected, facts):
    c.shape(row, 'living journal state_sha256 public_root negative_oracles', 'surviving root')
    c.need(c.number(row['living'], 'living') == live and c.hexsha(row['state_sha256']), 'source root identity')
    decisions = journal(c, row['journal'], [] if live == 4 else [2] if live == 3 else [2, 1])
    c.need(c.canonical(decisions) == c.canonical(base_journal['decisions']), 'same entire accepted history, no root reconstruction')
    c.need(all(r['after_decisions'] == len(decisions) for r in row['journal']['concessions']), 'departures after exactly frozen prefix')
    o = row['public_root']
    c.shape(o, 'observation_version view decision', 'own observation')
    c.need(c.number(o['observation_version'], 'projection version', low=1) == 16, 'current public projection')
    v = o['view']
    fields = 'combat viewer active turn_owner turn live_turn_order commanders priority players objects stack' + (' setup' if 'setup' in v else '') + (' effects' if 'effects' in v else '')
    c.shape(v, fields, 'complete public root view')
    c.shape(v['turn'], 'number step lands_played', 'public turn')
    c.number(v['turn']['number'], 'turn number', low=1)
    c.number(v['turn']['lands_played'], 'land count', high=1)
    c.number(v['turn_owner'], 'turn owner', high=3)
    c.number(v['priority'], 'priority holder', high=3)
    for visible in v['objects']:
        c.shape(visible, 'handle oracle_id owner controller zone tapped marked_damage attack_ready' + (' characteristics' if 'characteristics' in visible else ''), 'complete visible row')
        c.need(type(visible['handle']) is str and type(visible['tapped']) is bool and type(visible['attack_ready']) is bool, 'typed row handle/readiness')
        c.number(visible['owner'], 'owner', high=3)
        c.number(visible['controller'], 'controller', high=3)
        c.quantity(visible['marked_damage'])
        zone = visible['zone']
        c.need(type(zone) is dict and zone.get('zone') in ['battlefield', 'stack', 'exile', 'hand', 'library', 'graveyard', 'command'], 'typed visible zone')
        owned_zone = zone['zone'] in ['hand', 'library', 'graveyard', 'command']
        c.shape(zone, 'zone owner' if owned_zone else 'zone', 'complete zone')
        if owned_zone:
            c.number(zone['owner'], 'zone owner', high=3)
    c.need([p['player'] for p in v['players']] == [0, 1, 2, 3] and all(p['status'] == ('live' if p['player'] in seats else 'left') for p in v['players']), 'all four original seat facts retained, departed status explicit')
    for player in v['players']:
        c.shape(player, 'player status life mana hand_count library_count', 'complete visible player')
        c.number(player['player'], 'visible seat', high=3)
        c.quantity(player['life'])
        c.number(player['hand_count'], 'hand count')
        c.number(player['library_count'], 'library count')
    c.need(v['viewer'] == 0 and v['active'] == 0 and v['turn']['step'] == 'precombat_main' and v['stack'] == [] and v['live_turn_order'] == seats and o['decision']['request'] == 'priority' and o['decision']['player'] == 0, 'same active fresh root and living public order')
    c.number(v['viewer'], 'view player', high=3)
    c.number(v['active'], 'active player', high=3)
    c.number(o['decision']['player'], 'priority actor', high=3)
    c.need(c.canonical(v['live_turn_order']) == c.canonical(seats), 'exact typed living seats')
    definitions = {d['oracle_id']: d for d in facts['catalog']['definitions']}
    ready = [x['handle'] for x in v['objects'] if x['controller'] == 0 and x['zone'] == {'zone': 'battlefield'} and x['tapped'] is False and x['attack_ready'] is True and (x.get('characteristics') or {}).get('kind') == 'creature' and 'defender' not in definitions[x['oracle_id']].get('keywords', [])]
    c.need(ready[:3] == selected and len(set(selected)) == 3, 'exact first3 actual ready nonDefender witnesses')
    edict = c.one_definition(facts['catalog'], 'Cruel Edict')
    c.need(any(x['owner'] == 0 and x['zone'] == {'zone': 'hand', 'owner': 0} and x['oracle_id'] == edict['oracle_id'] for x in v['objects']), 'same held archived Edict')
    funds = {color: c.quantity(q) for color, q in next(p for p in v['players'] if p['player'] == 0)['mana'].items()}
    for x in v['objects']:
        d = definitions[x['oracle_id']]
        if x['controller'] == 0 and x['zone'] == {'zone': 'battlefield'} and x['tapped'] is False and d['characteristics']['kind'] == 'land':
            identity = d['characteristics']['color_identity']
            if identity:
                funds[identity[0]] = funds.get(identity[0], 0) + 1
    required = Counter(edict['characteristics']['mana_cost']['colored'])
    c.need(all(funds.get(color, 0) >= n for color, n in required.items()) and sum(funds.values()) - sum(required.values()) >= c.quantity(edict['characteristics']['mana_cost']['generic']), 'actual public source-funded cost, no seed retry')
    c.need(all(any(x['controller'] == p and x['zone'] == {'zone': 'battlefield'} and (x.get('characteristics') or {}).get('kind') == 'creature' for x in v['objects']) for p in seats[1:]), 'each target has actual sacrifice domain')
    negative = row['negative_oracles']
    c.shape(negative, 'rejected_without_mutation defender_witness_available defender_negative_claim departed_case_not_applicable_at4', 'negative source oracles')
    c.need(type(negative['defender_witness_available']) is bool and negative['defender_negative_claim'] is negative['defender_witness_available'] and negative['departed_case_not_applicable_at4'] is (live == 4), 'available/unavailable negative coverage honest')
    expected = ['duplicate_attacker'] + (['departed_defender'] if live < 4 else []) + (['defender_permanent'] if negative['defender_witness_available'] else [])
    c.need(negative['rejected_without_mutation'] == expected, 'all actual source negative checks in order')
    return row


def public_vectors(c, requests, responses, delta, seats):
    c.need(type(requests) is list and type(responses) is list and len(requests) == len(responses) == len(delta), 'every complete request and response retained')
    tokens = []
    for request, response, accepted in zip(requests, responses, delta):
        kind = request.get('request')
        c.need(kind in REQUEST_FIELDS, 'unexpected finite branch request')
        fields = 'request token player ' + REQUEST_FIELDS[kind]
        if kind == 'commander_return' and 'previous_choices' in request:
            fields += ' previous_choices'
        c.shape(request, fields, 'complete source request')
        c.need(type(request['token']) is str and request['token'] and request['token'] not in tokens and c.number(request['player'], 'chooser', high=3) in seats, 'actual unique chooser token')
        tokens.append(request['token'])
        response_kind = response.get('response')
        c.need(response_kind in RESPONSE_FIELDS, 'unexpected branch response')
        c.shape(response, 'response ' + RESPONSE_FIELDS[response_kind], 'complete permitted response')
        if response_kind in ['cast', 'activate_mana', 'sacrifice']:
            handle = response['card' if response_kind == 'cast' else 'object']
            c.need(type(handle) is str and handle, 'actual permitted object handle')
        c.shape(accepted['response'], 'response ' + RESPONSE_FIELDS[response_kind], 'complete raw typed response')
        c.need(accepted['player'] == request['player'] and accepted['response']['response'] == response_kind, 'accepted journal/own response alignment')
        allowed = {'priority': ['pass', 'cast'], 'attackers': ['attackers'], 'mana': ['activate_mana', 'finish_mana'], 'spend_mana': ['spend_mana'], 'player_targets': ['player_targets'], 'sacrifice': ['sacrifice'], 'order_triggers': ['order_triggers'], 'commander_return': ['commander_return']}
        c.need(response_kind in allowed[kind], 'response belongs to actual request')
        if kind == 'attackers':
            c.need(type(request['defenders']) is list and all(type(p) is int and p in seats for p in request['defenders']), 'typed offered defender seats')
            for attacks in [response['attacks'], accepted['response']['attacks']]:
                c.need(type(attacks) is list, 'typed attack list')
                for attack in attacks:
                    c.need(type(attack) is list and len(attack) == 2, 'typed attack pair')
                    c.number(attack[1], 'declared defender', high=3)
        if kind in ('sacrifice', 'attackers'):
            c.need(type(request['candidates']) is list and len(set(request['candidates'])) == len(request['candidates']), 'complete unique offered object domain')
        if kind == 'sacrifice':
            c.need(response['object'] == request['candidates'][0], 'source policy selects actual first sacrifice')
        elif kind == 'player_targets':
            c.need(type(request['candidates']) is list and all(type(p) is int and p in seats for p in request['candidates']), 'typed target candidate seats')
            c.need(len(response['players']) == len(request['occurrences']) and all(type(p) is int and p in request['candidates'] for p in response['players']), 'complete target occurrences/offered seats')
            c.need(c.canonical(response) == c.canonical(accepted['response']), 'raw player target exact')
        elif kind == 'order_triggers':
            c.need(type(accepted['response']['order']) is list and all(type(t) is int and t >= 0 for t in accepted['response']['order']), 'typed raw trigger IDs')
            c.need(response['order'] == [t['handle'] for t in request['candidates']], 'full offered trigger order')
        elif kind == 'commander_return':
            c.need(response['to_command'] is True and c.canonical(response) == c.canonical(accepted['response']), 'mandatory source return policy')
        elif response_kind == 'finish_mana':
            c.need(response['undo_mana'] is True, 'source mana completion flag')
        elif kind == 'spend_mana':
            c.need(c.canonical(response) == c.canonical(accepted['response']), 'actual exact spending response')
            spending = response['spending']
            c.need(type(spending) is dict and all(color in 'WUBRGC' and len(color) == 1 and c.quantity(q) > 0 for color, q in spending.items()), 'canonical positive mana spending')
            needed = Counter(request['cost']['colored'])
            c.need(all(c.quantity(spending.get(color, '0')) >= n for color, n in needed.items()) and sum(c.quantity(q) for q in spending.values()) == sum(needed.values()) + c.quantity(request['cost']['generic']), 'exact paid color/generic cost')
    return len(delta)


def meter(c, m, delta, root_inputs, final_boundary, verified, disposal=False):
    c.shape(m, METER_KEYS, 'full meter')
    counts = m['counts']
    c.shape(counts, COUNT_KEYS, 'work counts')
    for key in COUNT_KEYS.split():
        c.number(counts[key], key)
    n = len(delta)
    c.need(counts['fork_started'] == counts['forked'] == counts['fully_advanced'] == 1 and counts['verified'] == verified, 'fork/advance/verification lifecycle')
    c.need(counts['validated_inputs'] == counts['submitted_inputs'] == counts['consumed_inputs'] == n and counts['initial_queries'] == 2 * n + 1 and counts['advance_calls'] >= n + 1 and counts['rule_steps'] > 0, 'branch work/input/query conservation')
    c.need(len(m['processed']) == n, 'one actual processing fact per accepted response')
    previous = -1
    for index, (fact, accepted) in enumerate(zip(m['processed'], delta)):
        c.shape(fact, 'identity cause boundary', 'processing fact')
        expected = {'input': 'decision', 'journal_ordinal': root_inputs + index, 'decision_id': accepted['decision_id'], 'player': accepted['player']}
        c.need(c.canonical(fact['identity']) == c.canonical(expected) and fact['cause'] == 'rules_step', 'actual consumed identity/ordinal/cause')
        boundary = c.number(fact['boundary'], 'processing boundary', high=final_boundary)
        c.need(boundary > previous, 'ordered committed processing facts')
        previous = boundary
    c.need(type(m['segments']) is list, 'all segment clocks')
    hist = Counter()
    for s in m['segments']:
        c.shape(s, 'operation nanos', 'segment')
        c.need(type(s['operation']) is str, 'segment label')
        c.number(s['nanos'], 'segment ns', high=120_000_000_000)
        hist[s['operation']] += 1
    expected = {'fork': 1, 'observation_query': n + 1, 'check_response': n, 'submit': n, 'advance_with_processing_facts': counts['advance_calls'], 'query_resume': counts['query_resumes'], 'game_disposal': int(disposal)}
    c.need(hist == Counter({key: value for key, value in expected.items() if value}), 'each source operation call has one timing segment')
    c.need(m['segments'][0]['operation'] == 'fork' and (not disposal or m['segments'][-1]['operation'] == 'game_disposal'), 'fork/disposal segment endpoints')
    return counts


def effect(c, proof, cell, seats, selected, config, catalog):
    c.shape(proof, 'typed_outcomes spell_casts spell_completed paid_stacks resolved_stacks actual_payment_costs sacrifices declarations state_sha256 rule_boundary delta_journal requests responses', 'full source effect proof')
    c.need(c.hexsha(proof['state_sha256']), 'full final State SHA')
    boundary = c.number(proof['rule_boundary'], 'final rule boundary', low=1)
    delta = journal(c, {'decisions': proof['delta_journal'], 'concessions': []}, [])
    c.need(delta, 'nonempty actual branch')
    public_vectors(c, proof['requests'], proof['responses'], delta, seats)
    for k in ('spell_casts', 'spell_completed'):
        c.number(proof[k], k)
    c.need(type(proof['typed_outcomes']) is list and all(type(o) is dict and type(o.get('outcome')) is str for o in proof['typed_outcomes']), 'complete typed native outcome objects')
    actual_attacks = [o['attacks'] for o in proof['typed_outcomes'] if o['outcome'] == 'attackers_declared']
    actual_sacrifices = [[o['player'], o['movement']] for o in proof['typed_outcomes'] if o['outcome'] == 'sacrificed']
    c.need(c.canonical(actual_attacks) == c.canonical(proof['declarations']) and c.canonical(actual_sacrifices) == c.canonical(proof['sacrifices']), 'all actual typed declarations/sacrifices retained')
    if cell['choice'] == 'attack':
        defenders = seats[1:]
        wanted = [[selected[i], defenders[d - 1]] for i, d in enumerate(cell['digits']) if d]
        pub = [(r, s) for r, s in zip(proof['requests'], proof['responses']) if r['request'] == 'attackers']
        c.need(len(pub) == 1 and pub[0][0]['player'] == 0 and pub[0][0]['candidates'][:3] == selected and c.canonical(pub[0][0]['defenders']) == c.canonical(defenders) and c.canonical(pub[0][1]['attacks']) == c.canonical(wanted), 'complete planned attack assignment and actual opponent domain')
        raw = [r['response']['attacks'] for r in delta if r['response']['response'] == 'attackers']
        for declared_object, _ in raw[0] if raw else []:
            c.shape(declared_object, 'card incarnation', 'declared raw object')
            c.number(declared_object['incarnation'], 'declared incarnation')
            c.need(physical_cards(config).get(c.number(declared_object['card'], 'declared physical card', high=399), (None,))[0] == 0, 'declared object owned by source attacker')
        c.need(c.canonical(raw) == c.canonical(proof['declarations']) and len(raw) == 1 and len(raw[0]) == len(wanted) and c.canonical([p for _, p in raw[0]]) == c.canonical([p for _, p in wanted]), 'actual committed declaration equals entire accepted assignment')
        c.need([r['response']['response'] for r in delta] == ['pass'] * (2 * len(seats)) + ['attackers'] and [r['player'] for r in delta] == seats * 2 + [0], 'source precombat and beginning-combat priority cycles complete')
        c.need(proof['sacrifices'] == [] and proof['spell_casts'] == proof['spell_completed'] == 0 and proof['paid_stacks'] == [] and proof['resolved_stacks'] == [] and proof['actual_payment_costs'] == [], 'attack row does not claim spell completion')
    else:
        target = cell['target']
        edict = c.one_definition(catalog, 'Cruel Edict')
        casts = [r['response']['card'] for r in delta if r['response']['response'] == 'cast']
        c.need(len(casts) == 1 and physical_cards(config).get(c.number(casts[0], 'cast physical card', high=399)) == (0, edict['oracle_id']), 'actual physical P0 source is archived Edict')
        targets = [(r, s) for r, s in zip(proof['requests'], proof['responses']) if r['request'] == 'player_targets']
        c.need(targets and targets[0][0]['player'] == 0 and c.canonical(targets[0][0]['candidates']) == c.canonical(seats[1:]) and len(targets[0][0]['occurrences']) == 1 and c.canonical(targets[0][1]['players']) == c.canonical([target]), 'complete Edict target subset/order')
        c.need(proof['spell_casts'] == proof['spell_completed'] == 1 and len(proof['paid_stacks']) == 1 and c.canonical(proof['paid_stacks']) == c.canonical(proof['resolved_stacks']) and c.canonical(proof['actual_payment_costs']) == c.canonical([edict['characteristics']['mana_cost']]), 'paid and actually resolved source stack/cost facts')
        for stack in proof['paid_stacks']:
            c.number(stack, 'stack id')
        paid = [o['cost'] for o in proof['typed_outcomes'] if o['outcome'] == 'mana_paid_selected' and o['player'] == 0]
        c.need(c.canonical(paid) == c.canonical(proof['actual_payment_costs']), 'typed payment facts exactly conserved')
        chosen = [r['response']['object'] for r in delta if r['response']['response'] == 'sacrifice']
        c.need(len(proof['sacrifices']) == 1 and c.number(proof['sacrifices'][0][0], 'sacrificing player', high=3) == target and len(chosen) == 1 and proof['declarations'] == [], 'one actual selected-target sacrifice')
        movement = proof['sacrifices'][0][1]
        c.shape(movement, 'previous successor from destination', 'actual sacrifice movement')
        c.need(movement['previous'] == chosen[0] and movement['from'] == {'zone': 'battlefield'} and c.canonical(movement['destination']) == c.canonical({'zone': 'graveyard', 'owner': target}), 'chosen incarnation actually reaches owner graveyard')
        for key in ['previous', 'successor']:
            c.shape(movement[key], 'card incarnation', 'object incarnation')
            c.number(movement[key]['card'], 'physical card', high=399)
            c.number(movement[key]['incarnation'], 'incarnation')
        c.need(movement['successor']['card'] == movement['previous']['card'] and movement['successor']['incarnation'] == movement['previous']['incarnation'] + 1 and physical_cards(config)[movement['previous']['card']][0] == target, 'exact successor and source-owned physical card')
    return delta, boundary


def validate_report(c, report, build, facts, ordinary):
    c.native_numeric_tree(report)
    c.shape(report, 'schema status build_id seed_domain config acceptance_claim planned_attack_cells planned_edict_cells planned_total_cells warmups_per_cell measured_repeats_per_cell planned_actual_repetitions oracle_work_scope cells in_progress_cell roots preparation_progress no_search_latency_or_quality_or_definition_diversity_claim catalog_load root_journal frozen_root_state_sha256 selected_three error', 'choice report')
    c.need(report['schema'] == 'legal-shrinking-choice-costs-v1' and report['status'] == 'complete105_cells630_repetitions_verified' and report['error'] is None and report['build_id'] == build and report['seed_domain'] == SEED and c.canonical(report['config']) == c.canonical(facts['config']), 'same fixed admitted config and successful whole report')
    c.need(report['acceptance_claim'] is False and report['no_search_latency_or_quality_or_definition_diversity_claim'] is True and report['in_progress_cell'] is None, 'no pending or broader acceptance claim')
    for key, value in {'planned_attack_cells': [64, 27, 8], 'planned_edict_cells': [3, 2, 1], 'planned_total_cells': 105, 'warmups_per_cell': 1, 'measured_repeats_per_cell': 5, 'planned_actual_repetitions': 630}.items():
        c.need(c.canonical(report[key]) == c.canonical(value), 'immutable denominator contract')
    ordinary.catalog_load(report['catalog_load'], facts['fixture'])
    setup = journal(c, report['root_journal'], [])
    cards = physical_cards(facts['config'])
    definitions = {d['oracle_id']: d for d in facts['catalog']['definitions']}
    for accepted in setup:
        response = accepted['response']
        if response['response'] == 'mulligan':
            c.need(response.get('take') is False, 'fixed keep-all preparation policy')
        elif response['response'] == 'attackers':
            c.need(response.get('attacks') == [], 'no setup attacks or changed root history')
        elif response['response'] == 'cast':
            physical = c.number(response.get('card'), 'setup cast physical card', high=399)
            c.need(cards[physical][0] == accepted['player'] and definitions[cards[physical][1]]['characteristics']['kind'] == 'creature', 'source public setup casts only own creatures, no one-shot spells')
    c.need(0 < len(setup) < 30_000 and c.hexsha(report['frozen_root_state_sha256']) and type(report['selected_three']) is list and len(report['selected_three']) == len(set(report['selected_three'])) == 3 and all(type(h) is str and h for h in report['selected_three']), 'complete fixed root setup and three witnesses')
    progress = report['preparation_progress']
    c.shape(progress, 'phase setup_inputs rule_boundary public_root counts', 'first public-condition progress')
    c.need(progress['phase'] == 'prepared_first_public_condition' and c.number(progress['setup_inputs'], 'setup input count') == len(setup), 'no later/easier substituted root')
    c.number(progress['rule_boundary'], 'setup boundary', low=1)
    c.shape(progress['counts'], COUNT_KEYS, 'preparation counts')
    for value in progress['counts'].values():
        c.number(value, 'preparation counter')
    c.need(progress['counts']['validated_inputs'] == progress['counts']['submitted_inputs'] == progress['counts']['consumed_inputs'] == len(setup), 'preparation accepted versus committed counts')
    c.need(progress['counts']['initial_queries'] == 2 * len(setup) + 1 and progress['counts']['advance_calls'] >= len(setup) + 1 and progress['counts']['rule_steps'] == progress['rule_boundary'] and all(progress['counts'][key] == 0 for key in ['fork_started', 'forked', 'fully_advanced', 'verified']), 'complete preparation source call and lifecycle counts')
    c.need(len(report['roots']) == 3, 'all living root variants')
    root_by_live = {}
    for r, (live, seats) in zip(report['roots'], LIVE):
        root_by_live[live] = root(c, r, live, seats, report['root_journal'], report['selected_three'], facts)
        concessions = r['journal']['concessions']
        if concessions:
            c.need(concessions[0]['boundary'] == progress['rule_boundary'] and all(a['boundary'] < b['boundary'] for a, b in zip(concessions, concessions[1:])), 'departures derive from same frozen rule boundary in exact order')
    c.need(c.canonical(progress['public_root']) == c.canonical(report['roots'][0]['public_root']) and report['roots'][0]['state_sha256'] == report['frozen_root_state_sha256'], 'first-public-condition observation/root preserved')
    worklist = expected_cells()
    c.need(len(report['cells']) == len(worklist) == 105, 'all105 cells retained')
    base_boundaries = {}
    measured_segments = Counter()
    for row, (live, cell) in zip(report['cells'], worklist):
        c.shape(row, 'living root_state_sha256 cell phase reference reference_work budget_oracles repetitions', 'complete cell')
        c.need(c.number(row['living'], 'cell living') == live and c.canonical(row['cell']) == c.canonical(cell) and row['phase'] == 'complete' and row['root_state_sha256'] == root_by_live[live]['state_sha256'], 'fixed complete ordered cell identity/root')
        seats = next(seats for n, seats in LIVE if n == live)
        delta, boundary = effect(c, row['reference'], cell, seats, report['selected_three'], facts['config'], facts['catalog'])
        m = row['reference_work']
        c.need(c.canonical(m['requests']) == c.canonical(row['reference']['requests']) and c.canonical(m['responses']) == c.canonical(row['reference']['responses']), 'full reference meter/domain vectors')
        counts = meter(c, m, delta, len(setup), boundary, 0)
        start_boundary = boundary - counts['rule_steps']
        c.need(start_boundary >= 0 and base_boundaries.setdefault(live, start_boundary) == start_boundary, 'all cells fork exact same root boundary')
        c.need(len(row['budget_oracles']) == 2, 'both1/MAX oracle runs')
        for alternate, q in zip(row['budget_oracles'], [1, (1 << 64) - 1]):
            c.shape(alternate, 'quantum phase actual_work', 'budget oracle')
            c.need(c.number(alternate['quantum'], 'quantum') == q and alternate['phase'] == 'verified', 'complete budget partitions in source order')
            other = alternate['actual_work']
            oc = meter(c, other, delta, len(setup), boundary, 0)
            for key in ['requests', 'responses', 'processed']:
                c.need(c.canonical(other[key]) == c.canonical(m[key]), 'budget complete domains and processing facts preserved')
            c.need(oc['rule_steps'] == counts['rule_steps'], 'budget partition actual rule transitions')
        c.need(len(row['repetitions']) == 6, 'one warmup+five samples per cell')
        for rep, sample in enumerate(row['repetitions']):
            c.shape(sample, 'repetition warmup status state_sha256 work', 'sample')
            c.need(c.number(sample['repetition'], 'repetition') == rep and sample['warmup'] is (rep == 0) and sample['status'] == 'verified' and sample['state_sha256'] == row['reference']['state_sha256'], 'exact complete sample order/full State equality')
            actual = sample['work']
            sc = meter(c, actual, delta, len(setup), boundary, 1, disposal=True)
            c.need(c.canonical({**sc, 'verified': 0}) == c.canonical(counts), 'all same-quantum counts equal full reference')
            for key in ['requests', 'responses', 'processed']:
                c.need(c.canonical(actual[key]) == c.canonical(m[key]), 'all complete same-work data and input facts equal')
            if rep:
                for segment in actual['segments']:
                    measured_segments[segment['operation']] += 1
    return {'status': 'complete105_cells630_repetitions_full_proofs_verified', 'cells': 105, 'warmup_executions': 105, 'measured_executions': 525, 'all_repetitions': 630, 'extra_reference_executions': 105, 'extra_partition_oracle_executions': 210, 'extra_Detailed_replays': 105, 'measured_segment_counts': dict(measured_segments), 'full_rule_executor_claim': False, 'scope': 'Actual full generic State, normal root restore, independent Detailed replay and paid/sacrifice proofs are native source-bound oracles. Host verifies emitted complete work/data, source config, domains, identities, cardinalities, ordering and conservation; no new rules evaluator.', 'fit_latency_or_quality_claim': False}


def main():
    parser = argparse.ArgumentParser()
    for name in ['authority', 'output']:
        parser.add_argument('--' + name, type=Path, required=True)
    for name in ['authority-sha256', 'fixed-driver-sha256', 'helper-sha256', 'common-driver-sha256']:
        parser.add_argument('--' + name, required=True)
    parser.add_argument('--execute', action='store_true')
    args = parser.parse_args()
    c = load_common(args.common_driver_sha256)
    c.need(args.execute, 'explicit root execution release required')
    os.umask(0o077)
    a, host, ordinary, facts = source_guard(args, c)
    host.real_path(args.output.parent, private=True)
    c.need(not args.output.exists() and not args.output.is_symlink(), 'fresh attempt')
    args.output.mkdir(mode=0o700)
    (args.output / 'tmp').mkdir(mode=0o700)
    report = args.output / 'execution/native-report.json'
    def audit():
        c.need(source_guard(args, c)[0] == a, 'source endpoint')
        result = validate_report(c, c.read(report), a['engine_build_id'], facts, ordinary)
        c.need(c.stdout_records(args.output / 'execution/workload.stdout', ENTRY, []) == [], 'no unexpected native stdout')
        c.need(source_guard(args, c)[0] == a, 'postsemantic source endpoint')
        return {**result, 'raw_report_sha256': c.sha(report), 'raw_stdout_sha256': c.sha(args.output / 'execution/workload.stdout')}
    try:
        row = host.run_bounded(command=[a['test_executable']['path'], '--exact', '--ignored', '--nocapture', '--test-threads=1', ENTRY], cwd=STAGE, env={'PATH': '/usr/bin:/bin', 'LANG': 'C', 'LC_ALL': 'C', 'TMPDIR': str(args.output / 'tmp'), 'MTG_SHRINKING_CHOICE_OUTPUT': str(report)}, directory=args.output / 'execution', label='shrinking-choices', cpus=a['cpu_ids'], timeout_seconds=120, expected_executable_sha256=a['test_executable']['sha256'], audit=audit)
    except BaseException as error:
        row = {'status': 'failed', 'failure': {'type': type(error).__name__, 'reason': str(error)[:2048]}, 'resource_report_created': False}
    try:
        c.need(source_guard(args, c)[0] == a, 'final source endpoint')
    except Exception as error:
        row['status'] = 'failed'
        row['failure'] = {'type': type(error).__name__, 'reason': str(error)[:2048]}
    host.save_json(args.output / 'result.json', {'schema': 'shrinking-choices-host-result-v1', 'status': 'verified_all_declared_work' if row['status'] == 'passed' else 'failed_or_partial_no_denominator_reduction', 'engine_build_id': a['engine_build_id'], 'authority_sha256': args.authority_sha256, 'resource_report_sha256': c.sha(args.output / 'execution/host-report.json') if (args.output / 'execution/host-report.json').exists() else None, 'audit': row.get('audit') if row['status'] == 'passed' else None, 'failure': row.get('failure'), 'partial': c.partial_diagnostic(args.output, report) if row['status'] != 'passed' else None, 'throughput_on_incomplete_or_acceptance_claim': False})
    return 0 if row['status'] == 'passed' else 1


if __name__ == '__main__':
    sys.exit(main())
