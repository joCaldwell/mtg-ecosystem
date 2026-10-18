"""Synthetic pure validation data only; no admitted games or timing evidence."""
import copy
import hashlib
import importlib.util
from pathlib import Path
import tempfile
import unittest
from types import SimpleNamespace

HERE = Path(__file__).absolute().parent

def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m

c = load('choice_common_test', HERE / 'run-definition-diversity-v1.py')
host = load('choice_host_tested', HERE / 'run-shrinking-choices-v1.py')
ORDINARY = SimpleNamespace(catalog_load=lambda *_: None)  # native catalog decoding is tested by its existing shared validator.


def facts():
    deck = {'commander': 'commander-fixture', 'cards': [{'oracle_id': 'commander-fixture', 'count': 1}, {'oracle_id': 'edict-fixture', 'count': 1}, {'oracle_id': 'unit-fixture', 'count': 98}]}
    config = {'profile': 'pure-data-fixture-not-a-native-profile', 'starting_player': 0, 'seed': list(hashlib.sha256(host.SEED.encode()).digest()), 'decks': [deck] * 4}
    defs = [{'name': 'Cruel Edict', 'oracle_id': 'edict-fixture', 'characteristics': {'mana_cost': {'generic': '1', 'colored': ['B']}, 'kind': 'sorcery'}}, {'name': 'Fixture creature', 'oracle_id': 'unit-fixture', 'characteristics': {'kind': 'creature'}, 'keywords': []}]
    return {'config': config, 'catalog': {'definitions': defs}, 'fixture': {}}


def visible(handle, owner, zone, oracle='unit-fixture'):
    return {'handle': handle, 'oracle_id': oracle, 'owner': owner, 'controller': owner, 'zone': zone, 'tapped': False, 'marked_damage': '0', 'attack_ready': True, 'characteristics': {'kind': 'creature' if oracle == 'unit-fixture' else 'sorcery'}}


def make_root(live, seats, base):
    selected = ['a', 'b', 'c']
    objects = [visible(h, 0, {'zone': 'battlefield'}) for h in selected] + [visible('edict', 0, {'zone': 'hand', 'owner': 0}, 'edict-fixture')] + [visible('opp-' + str(p), p, {'zone': 'battlefield'}) for p in seats[1:]]
    view = {'combat': [], 'viewer': 0, 'active': 0, 'turn_owner': 0, 'turn': {'number': 9, 'step': 'precombat_main', 'lands_played': 0}, 'live_turn_order': seats, 'commanders': [], 'priority': 0, 'players': [{'player': p, 'status': 'live' if p in seats else 'left', 'life': '40', 'mana': {'B': '1', 'G': '1'} if p == 0 else {}, 'hand_count': 1, 'library_count': 90} for p in range(4)], 'objects': objects, 'stack': []}
    departures = [] if live == 4 else [2] if live == 3 else [2, 1]
    return {'living': live, 'journal': {'decisions': copy.deepcopy(base['decisions']), 'concessions': [{'after_decisions': len(base['decisions']), 'boundary': 100 + i, 'player': p} for i, p in enumerate(departures)]}, 'state_sha256': str(live) * 64, 'public_root': {'observation_version': 16, 'view': view, 'decision': {'request': 'priority', 'player': 0, 'token': 'root'}}, 'negative_oracles': {'rejected_without_mutation': ['duplicate_attacker'] + (['departed_defender'] if live < 4 else []), 'defender_witness_available': False, 'defender_negative_claim': False, 'departed_case_not_applicable_at4': live == 4}}


def meter(delta, requests, responses, final_boundary, rep=None, quantum=64):
    n = len(delta)
    resumes = n if quantum == 1 else 0
    counts = {'fork_started': 1, 'forked': 1, 'initial_queries': 2 * n + 1, 'query_resumes': resumes, 'validated_inputs': n, 'submitted_inputs': n, 'consumed_inputs': n, 'advance_calls': n + 1, 'rule_steps': final_boundary - 100, 'fully_advanced': 1, 'verified': int(rep is not None)}
    operations = ['fork'] + ['advance_with_processing_facts', 'observation_query', 'check_response', 'submit'] * n + ['advance_with_processing_facts', 'observation_query'] + ['query_resume'] * resumes + (['game_disposal'] if rep is not None else [])
    return {'counts': counts, 'segments': [{'operation': op, 'nanos': 1} for op in operations], 'requests': copy.deepcopy(requests), 'responses': copy.deepcopy(responses), 'processed': [{'identity': {'input': 'decision', 'journal_ordinal': 1 + i, 'decision_id': r['decision_id'], 'player': r['player']}, 'cause': 'rules_step', 'boundary': 101 + i} for i, r in enumerate(delta)]}


def make_cell(live, seats, cell):
    selected = ['a', 'b', 'c']
    requests, responses, delta = [], [], []
    def add(kind, player, response, **fields):
        i = len(delta)
        requests.append({'request': kind, 'player': player, 'token': 't' + str(i), **fields})
        responses.append(copy.deepcopy(response))
        raw = copy.deepcopy(response)
        if raw['response'] == 'cast': raw['card'] = 1
        if raw['response'] == 'sacrifice': raw['object'] = {'card': player * 100 + 2, 'incarnation': 0}
        if raw['response'] == 'attackers': raw['attacks'] = [[{'card': selected.index(h) + 2, 'incarnation': 0}, p] for h, p in raw['attacks']]
        delta.append({'decision_id': 20 + i, 'player': player, 'response': raw})
    typed, declarations, sacrifices, paid, cost = [], [], [], [], []
    if cell['choice'] == 'attack':
        for p in seats * 2: add('priority', p, {'response': 'pass'})
        attacks = [[selected[i], seats[d]] for i, d in enumerate(cell['digits']) if d]
        add('attackers', 0, {'response': 'attackers', 'attacks': attacks}, candidates=selected, defenders=seats[1:])
        declarations = [copy.deepcopy(delta[-1]['response']['attacks'])]
        typed = [{'outcome': 'attackers_declared', 'attacks': declarations[0]}]
        casts = 0
    else:
        target = cell['target']
        add('priority', 0, {'response': 'cast', 'card': 'edict'})
        add('player_targets', 0, {'response': 'player_targets', 'players': [target]}, occurrences=[{'source_domain_fixture': True}], candidates=seats[1:])
        mana_cost = {'generic': '1', 'colored': ['B']}
        add('mana', 0, {'response': 'finish_mana', 'undo_mana': True}, cost=mana_cost)
        add('spend_mana', 0, {'response': 'spend_mana', 'spending': {'B': '1', 'G': '1'}}, cost=mana_cost, available={'B': '1', 'G': '1'})
        for p in seats: add('priority', p, {'response': 'pass'})
        add('sacrifice', target, {'response': 'sacrifice', 'object': 'offered'}, candidates=['offered'])
        movement = {'previous': delta[-1]['response']['object'], 'successor': {'card': target * 100 + 2, 'incarnation': 1}, 'from': {'zone': 'battlefield'}, 'destination': {'zone': 'graveyard', 'owner': target}}
        sacrifices = [[target, movement]]
        typed = [{'outcome': 'mana_paid_selected', 'player': 0, 'cost': mana_cost, 'spending': {'B': '1', 'G': '1'}}, {'outcome': 'sacrificed', 'player': target, 'movement': movement}]
        casts, paid, cost = 1, [9], [mana_cost]
    boundary = 100 + len(delta) + 1
    proof = {'typed_outcomes': typed, 'spell_casts': casts, 'spell_completed': casts, 'paid_stacks': paid, 'resolved_stacks': paid, 'actual_payment_costs': cost, 'sacrifices': sacrifices, 'declarations': declarations, 'state_sha256': 'a' * 64, 'rule_boundary': boundary, 'delta_journal': delta, 'requests': requests, 'responses': responses}
    return {'living': live, 'root_state_sha256': str(live) * 64, 'cell': cell, 'phase': 'complete', 'reference': proof, 'reference_work': meter(delta, requests, responses, boundary), 'budget_oracles': [{'quantum': q, 'phase': 'verified', 'actual_work': meter(delta, requests, responses, boundary, quantum=q)} for q in [1, (1 << 64) - 1]], 'repetitions': [{'repetition': rep, 'warmup': rep == 0, 'status': 'verified', 'state_sha256': 'a' * 64, 'work': meter(delta, requests, responses, boundary, rep=rep)} for rep in range(6)]}


def fixture():
    f = facts()
    base = {'decisions': [{'decision_id': 0, 'player': 0, 'response': {'response': 'mulligan', 'take': False}}], 'concessions': []}
    roots = [make_root(live, seats, base) for live, seats in host.LIVE]
    counts = {key: 0 for key in host.COUNT_KEYS.split()}
    counts.update(validated_inputs=1, submitted_inputs=1, consumed_inputs=1, initial_queries=3, advance_calls=2, rule_steps=100)
    report = {'schema': 'legal-shrinking-choice-costs-v1', 'status': 'complete105_cells630_repetitions_verified', 'build_id': 'b' * 64, 'seed_domain': host.SEED, 'config': f['config'], 'acceptance_claim': False, 'planned_attack_cells': [64, 27, 8], 'planned_edict_cells': [3, 2, 1], 'planned_total_cells': 105, 'warmups_per_cell': 1, 'measured_repeats_per_cell': 5, 'planned_actual_repetitions': 630, 'oracle_work_scope': 'pure test fixture not evidence', 'cells': [make_cell(live, next(seats for n, seats in host.LIVE if n == live), cell) for live, cell in host.expected_cells()], 'in_progress_cell': None, 'roots': roots, 'preparation_progress': {'phase': 'prepared_first_public_condition', 'setup_inputs': 1, 'rule_boundary': 100, 'public_root': roots[0]['public_root'], 'counts': counts}, 'no_search_latency_or_quality_or_definition_diversity_claim': True, 'catalog_load': {}, 'root_journal': base, 'frozen_root_state_sha256': '4' * 64, 'selected_three': ['a', 'b', 'c'], 'error': None}
    return report, f


class Guards(unittest.TestCase):
    def validate(self, report):
        return host.validate_report(c, report, 'b' * 64, facts(), ORDINARY)

    def reject(self, edit):
        report, _ = fixture()
        edit(report)
        with self.assertRaises((ValueError, KeyError, TypeError, IndexError)):
            self.validate(report)

    def test_all105_630_positive_and_fixed_order(self):
        r, _ = fixture()
        checked = self.validate(r)
        self.assertEqual(checked['all_repetitions'], 630)
        self.assertEqual([r['cells'][i]['cell'] for i in [0, 63, 64, 66, 67, 93, 94, 96, 103, 104]], [{'choice': 'attack', 'digits': [0, 0, 0]}, {'choice': 'attack', 'digits': [3, 3, 3]}, {'choice': 'edict', 'target': 1}, {'choice': 'edict', 'target': 3}, {'choice': 'attack', 'digits': [0, 0, 0]}, {'choice': 'attack', 'digits': [2, 2, 2]}, {'choice': 'edict', 'target': 1}, {'choice': 'attack', 'digits': [0, 0, 0]}, {'choice': 'attack', 'digits': [1, 1, 1]}, {'choice': 'edict', 'target': 3}])
    def test_omitted_cell(self): self.reject(lambda r: r['cells'].pop())
    def test_duplicate_cell(self): self.reject(lambda r: r['cells'].__setitem__(1, copy.deepcopy(r['cells'][0])))
    def test_reordered_cells(self): self.reject(lambda r: r['cells'].__setitem__(slice(0, 2), list(reversed(r['cells'][:2]))))
    def test_omitted_repetition(self): self.reject(lambda r: r['cells'][0]['repetitions'].pop())
    def test_fake_verified_phase(self): self.reject(lambda r: r['cells'][0]['budget_oracles'][0].__setitem__('phase', 'awaiting_state_oracle'))
    def test_missing_MAX_oracle(self): self.reject(lambda r: r['cells'][0]['budget_oracles'].pop())
    def test_wrong_full_state_after_success(self): self.reject(lambda r: r['cells'][0]['repetitions'][5].__setitem__('state_sha256', '9' * 64))
    def test_missing_actual_processing_fact(self): self.reject(lambda r: r['cells'][0]['reference_work']['processed'].pop())
    def test_duplicate_actual_fact_rehashed_state_irrelevant(self): self.reject(lambda r: r['cells'][0]['reference_work']['processed'].__setitem__(1, copy.deepcopy(r['cells'][0]['reference_work']['processed'][0])))
    def test_consumed_counter_bool(self): self.reject(lambda r: r['cells'][0]['reference_work']['counts'].__setitem__('consumed_inputs', True))
    def test_non_consumed_input(self): self.reject(lambda r: r['cells'][0]['reference_work']['counts'].__setitem__('consumed_inputs', 0))
    def test_segment_call_omission(self): self.reject(lambda r: r['cells'][0]['repetitions'][2]['work']['segments'].pop())
    def test_segment_deadline_overflow(self): self.reject(lambda r: r['cells'][0]['repetitions'][0]['work']['segments'][0].__setitem__('nanos', 120_000_000_001))
    def test_sacrifice_different_target(self): self.reject(lambda r: r['cells'][64]['reference']['sacrifices'][0].__setitem__(0, 3))
    def test_paid_source_not_edict(self): self.reject(lambda r: r['cells'][64]['reference']['delta_journal'][0]['response'].__setitem__('card', 2))
    def test_missing_resolved_stack(self): self.reject(lambda r: r['cells'][64]['reference'].__setitem__('resolved_stacks', []))
    def test_incomplete_target_domain(self): self.reject(lambda r: r['cells'][64]['reference']['requests'][1]['candidates'].pop())
    def test_forged_late_root_or_different_deck(self): self.reject(lambda r: r['config'].__setitem__('starting_player', 2))
    def test_root_history_not_reconstructed(self): self.reject(lambda r: r['roots'][1]['journal']['decisions'].__setitem__(0, {'decision_id': 1, 'player': 0, 'response': {'response': 'pass'}}))
    def test_bad_frozen_root_order(self): self.reject(lambda r: r['roots'].reverse())
    def test_extra_reference_or_unknown_field(self): self.reject(lambda r: r['cells'][0].__setitem__('ignored_extra_reference', {}))
    def test_false_denominator_reduction(self): self.reject(lambda r: r.__setitem__('planned_actual_repetitions', 600))
    def test_nested_numeric_aliases(self):
        edits = [
            lambda r: r['roots'][0]['public_root']['view'].__setitem__('viewer', False),
            lambda r: r['roots'][0]['public_root']['view']['players'][1].__setitem__('player', True),
            lambda r: r['preparation_progress'].__setitem__('setup_inputs', True),
            lambda r: r['cells'][64]['reference']['delta_journal'][0]['response'].__setitem__('card', True),
            lambda r: r['cells'][64]['reference']['sacrifices'][0].__setitem__(0, True),
            lambda r: r['cells'][64]['reference']['requests'][1].__setitem__('candidates', [True, 2, 3]),
            lambda r: r['cells'][1]['reference']['requests'][-1].__setitem__('defenders', [True, 2, 3]),
            lambda r: r['cells'][1]['reference']['delta_journal'][-1]['response']['attacks'][0].__setitem__(1, True),
            lambda r: r['cells'][1]['reference']['declarations'][0][0][0].__setitem__('incarnation', False),
            lambda r: r['cells'][64]['reference']['resolved_stacks'].__setitem__(0, 9.0),
            lambda r: (r['cells'][64]['reference'].__setitem__('paid_stacks', [1]), r['cells'][64]['reference'].__setitem__('resolved_stacks', [True])),
            lambda r: r['cells'][0]['repetitions'][1]['work']['segments'][0].__setitem__('nanos', 1.0),
        ]
        for index, edit in enumerate(edits):
            with self.subTest(case=index): self.reject(edit)

    def test_unknown_and_torn_stdout(self):
        raw = '\nrunning 1 test\ntest ' + host.ENTRY + ' ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 42 filtered out; finished in 0.0s\n'
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp) / 'stdout'
            p.write_text(raw)
            self.assertEqual(c.stdout_records(p, host.ENTRY, []), [])
            for bad in [raw[:-1], raw + 'DEFINITION_DIVERSITY_ROW_RESULT={}\n', raw.replace('1 passed', '2 passed')]:
                p.write_text(bad)
                with self.assertRaises(ValueError): c.stdout_records(p, host.ENTRY, [])
    def test_common_module_sha_before_import(self):
        with self.assertRaisesRegex(ValueError, 'BEFORE import'): host.load_common('0' * 64)


if __name__ == '__main__':
    unittest.main()
