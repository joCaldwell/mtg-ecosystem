#!/usr/bin/env python3
"""Pure small report fixtures only. No native execution/systemd/measurements."""
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

P=Path(__file__).absolute().parent
spec=importlib.util.spec_from_file_location('dense_driver',P/'run-dense-diagnostic-v1.py')
d=importlib.util.module_from_spec(spec);spec.loader.exec_module(d)
BUILD='a'*64
k=d.constants(d.STAGE/'packages/game-engine/crates/core/src/engine/dense_kernel_tests.rs')
i=d.constants(d.STAGE/'packages/game-engine/crates/core/src/engine/dense_interaction_tests.rs')
contracts={'kernel':k['HOST_CONTRACT'],'kernel_fixture':k['CONTRACT'],'interaction':i['INTERACTION_CONTRACT']}
# Deliberately synthetic, tiny report fixtures: unit-test inputs only, NEVER
# retained measurement/correctness authority. The actual driver still requires
# explicit SHA-pinned real correctness reports in its execution authority.
def small_kernel_proof(spec):
    n=spec['battlefield_objects']
    return {'fixture':spec,'contract_sha256':contracts['kernel_fixture']['sha256'],
        'synthetic_profile_digest':'c'*64,'physical_cards':n+400,
        'initial_other_zone_cards':400,'root_other_zone_cards':400,
        'history_lengths':[max(10,spec['minimum_history_entries'])]*4,
        'setup_accepted_inputs':4,'root_rule_steps':10,'root_state_sha256':'d'*64,
        'state_serialized_bytes':1,'visible_rows_by_viewer':[n+7]*4,
        'first_write_state_sha256':'e'*64,'first_write_added_inputs':1,
        'public_admission_rejected':True,'snapshot_and_replay_verified':True,
        'timings_collected':False}

def small_interaction_proof(case):
    return {'row':case,'state_sha256':'f'*64,'work':{
        'advance_calls':2,'accepted_inputs':1,'committed_rule_steps':2,
        'explicit_batches_attempted':1,'explicit_occurrences_attempted':1,
        'explicit_batches_committed':1,'explicit_occurrences_committed':1,
        'initial_queries':1,'query_resumes':2,'samples':[]}}

refs={'kernel':{'proofs':[small_kernel_proof(s)for s in contracts['kernel']['value']['kernel_cases'][:4]]},
    'interaction':{'actual_proofs':[small_interaction_proof(c)for c in d.INTERACTION_CASES],
    'memory_phase_snapshots':[{'phase':phase,'history_lengths':[131 if index==0 else 835]*4,
        'kernel_counters':['VmRSS: 1024 kB','VmHWM: 2048 kB'],
        'note':'synthetic parser guard fixture, not a resource measurement'}for index,phase in enumerate([
        'after_all64_placements_before_oracles','after_all64_resolutions_before_oracle',
        'after_all64_resolution_oracle','after_exact_replay_and_disk_byte_comparison'])]}}
fixture=json.loads((d.STAGE/'packages/game-engine/tests/fixtures/expected-effects-ir.json').read_text())
instruction=next(x for x in fixture['definitions'] if x['oracle_id']=='5748ebf1-24e3-499d-ab7c-c2cebd462a24')['spell']['instructions'][0]


def segment(name,rep,n,index=None):return {'name':name,'repetition':rep,'warmup':rep==0,'nanos':10,'logical_operations':n,'operation_index':index}

def counts(objects,obs):return {'initial_queries':obs+1,'query_resumptions':obs*((objects+3)//64),'submits':1,'advance_calls':1,'committed_rule_steps':2}

def memory(rss=64*1024**2,peak=None):return {'rss_bytes':rss,'high_water_bytes':peak or rss}

def kernel_report():
    proof=copy.deepcopy(refs['kernel']['proofs'][0]);spec=proof['fixture'];n=spec['battlefield_objects'];samples=[]
    for rep in range(6):
        samples.extend(segment(name,rep,4 if j==0 else n+400 if j==1 else 1)for j,name in enumerate(d.SEGMENTS))
    return {'schema':'synthetic-dense-diagnostic-result-v1','build_id':BUILD,'contract':contracts['kernel']['value'],'contract_sha256':contracts['kernel']['sha256'],'outer_guard_declared_not_verified_here':True,'acceptance_latency_claim':False,'legal_match_admission_claim':False,'result':{'case':spec,'oracle':proof,'physical_cards':408,'other_zone_cards':400,'commanders':0,'lives':['6','2','2','2'],'timed_oracle_work':False,'all_segment_inputs_and_results_verified':True,'samples':samples,'work_counts_by_repetition':[counts(408,4)for _ in range(6)],'memory_including_setup_and_oracles':memory()}}


def interaction_events(case='combat'):
    ref=copy.deepcopy(next(x for x in refs['interaction']['actual_proofs'] if x['row']==case));events=[]
    for rep in range(6):
        row=copy.deepcopy(ref);w=row['work'];ops={'advance':w['advance_calls'],'validate_and_accept_input':w['accepted_inputs'],'observe_initial':w['initial_queries'],'observe_resume':w['query_resumes'],'atomic_event_group':w['explicit_batches_attempted']}
        if case=='damage':ops['prepare_simultaneous_damage']=2
        w['samples']=[{'operation':name,'nanos':10}for name,n in ops.items()for _ in range(n)]
        events.append(('DENSE_INTERACTION_START',{'case':case,'repetition':rep,'warmup':rep==0}))
        if case=='triggers':
            events.extend(('DENSE_INTERACTION_MEMORY',x)for x in refs['interaction']['memory_phase_snapshots'][:4])
        events.append(('DENSE_INTERACTION_RESULT',{'schema':'synthetic-dense-interaction-result-v2','build_id':BUILD,'contract_sha256':contracts['interaction']['sha256'],'contract':contracts['interaction']['value'],'repetition':rep,'warmup':rep==0,'result':row,'outer_guard_only_declared_here':True,'acceptance_latency_claim':False}))
    return events


def retained(rep,size,full=False):
    n=size if full else 0;stop=None if full else 'rss_reserve_before_next_fork';selected=sorted({0,n//2,max(0,n-1)}&set(range(n)))
    samples=[segment('retained_fork',rep,1,index)for index in range(n)]+[segment('retained_selected_first_write',rep,1,index)for index in selected]+[segment('dispose_retained_branches',rep,n)]
    baseline=refs['kernel']['proofs'][0]['root_state_sha256'];rss=64*1024**2 if full else d.MEMORY-64*1024**2
    return {'repetition':rep,'warmup':rep==0,'requested':size,'retained':n,'resource_shortfall':stop,'requested_count_reached':full,'accepted_full_size_sample':full,'baseline_state_sha256':baseline,'samples':samples,'selected_first_write_counts':[{'index':index,'counts':counts(408,1)}for index in selected],'sweep_lifecycle_nanos_including_guards_and_field_assertions':sum(x['nanos']for x in samples)+100,'memory_before_retention':memory(rss),'memory_at_retention':memory(rss),'memory_after_disposal':memory(64*1024**2,rss)}


def retained_events():
    rows=[retained(rep,n,n==64)for rep in range(6)for n in [64,256,1024,10000]];events=[]
    for r in rows:
        events.append(('DENSE_RETAINED_SWEEP_START',{k:r[k]for k in ['repetition','warmup','requested','baseline_state_sha256']}));events.append(('DENSE_RETAINED_SWEEP_RESULT',r))
    flags=['individual_fork_samples_are_not_independent_sweep_repetitions','no_tail_precision_claim_from_five_lifecycle_samples','internal_rss_guard_is_not_outer_process_cap_proof','rss_difference_is_not_exact_live_heap_allocation','resource_shortfall_is_not_proof_target_can_never_fit']
    value={'case':'retained_branches','fixture':contracts['kernel']['value']['kernel_cases'][0],'declared_total_sweeps':24,'warmup_sweeps':4,'measured_sweeps':20,'rows':rows,**dict.fromkeys(flags,True)}
    r=kernel_report();r['result']=value;events.append(('DENSE_DIAGNOSTIC',r));return events


def parsed(events):return {'events':events}

def validate(events,family='interaction',case='combat'):
    return d.validate_output(parsed(events),family,case,BUILD,contracts,refs,instruction)


def stdout(events,family):
    entry=d.ENTRIES[family];rows=['running 1 test']
    for index,(name,value)in enumerate(events):rows.append(('test '+entry+' ... 'if index==0 else '')+name+'='+json.dumps(value,separators=(',',':')))
    return '\n'.join(rows+['ok','','test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 228 filtered out; finished in 0.01s',''])


class Guards(unittest.TestCase):
    def test_actual_named_entry_and_case_scope(self):
        self.assertEqual(d.ENTRIES['kernel'],'engine::dense_kernel_tests::diagnostic::dense_kernel_diagnostic_host_adapter')
        self.assertEqual(len(d.ALL_CASES),12)
        self.assertEqual(d.KERNEL_CASES[1],'moderate')
    def test_valid_small_kernel_fixture(self):
        self.assertTrue(validate([('DENSE_DIAGNOSTIC',kernel_report())],'kernel','ordinary')['regular_full_size_pass'])
    def test_extended_history_exact_visibility_after_natural_draw_discard(self):
        for case in ['dense_history','dense_combined']:
            spec=next(s for s in contracts['kernel']['value']['kernel_cases']if s['name']==case);n=spec['battlefield_objects'];r=kernel_report();v=r['result'];v['case']=spec;v['oracle'].update(fixture=spec,history_lengths=[257]*4,visible_rows_by_viewer=[n+9,n+9,n+9,n+10],physical_cards=n+400,setup_accepted_inputs=115);v['physical_cards']=n+400;v['work_counts_by_repetition']=[counts(n+400,4)for _ in range(6)]
            for s in v['samples']:
                if s['name']=='characteristics_all_objects':s['logical_operations']=n+400
            got=validate([('DENSE_DIAGNOSTIC',r)],'kernel',case);self.assertIn('current compiled untimed oracle',got['validated']['proof_scope'])
            for bad in [[n+7]*4,[n+9]*3,[n+9,n+9,n+9,n+10,n+10],[n+9]*4]:
                changed=copy.deepcopy(r);changed['result']['oracle']['visible_rows_by_viewer']=bad
                with self.assertRaises(ValueError):validate([('DENSE_DIAGNOSTIC',changed)],'kernel',case)
        base=kernel_report();base['result']['oracle']['visible_rows_by_viewer']=[17,17,17,18]
        with self.assertRaises(ValueError):validate([('DENSE_DIAGNOSTIC',base)],'kernel','ordinary')
    def test_missing_or_duplicate_historical_proof_rejected(self):
        for proofs in [[],refs['kernel']['proofs']+[refs['kernel']['proofs'][0]]]:
            changed=copy.deepcopy(refs);changed['kernel']['proofs']=proofs
            with self.assertRaises(ValueError):d.validate_output(parsed([('DENSE_DIAGNOSTIC',kernel_report())]),'kernel','ordinary',BUILD,contracts,changed,instruction)
    def test_bad_build_contract_hash_or_full_contract(self):
        for key,value in [('build_id','b'*64),('contract_sha256','0'*64),('contract',{**contracts['kernel']['value'],'warmup_repetitions':True})]:
            r=kernel_report();r[key]=value
            with self.assertRaises(ValueError):validate([('DENSE_DIAGNOSTIC',r)],'kernel','ordinary')
    def test_missing_kernel_repetition_and_actual_work(self):
        for mutation in [lambda r:r['result']['samples'].pop(),lambda r:r['result']['work_counts_by_repetition'][0].update(committed_rule_steps=1),lambda r:r['result']['samples'][1].update(logical_operations=8)]:
            r=kernel_report();mutation(r)
            with self.assertRaises(ValueError):validate([('DENSE_DIAGNOSTIC',r)],'kernel','ordinary')
    def test_interaction_counts_and_full_state_reference(self):
        self.assertTrue(validate(interaction_events())['regular_full_size_pass'])
        for mutation in [lambda r:r['result']['work'].update(accepted_inputs=19),lambda r:r['result'].update(state_sha256='0'*64),lambda r:r['result']['work']['samples'].pop()]:
            events=interaction_events();mutation(events[1][1])
            with self.assertRaises(ValueError):validate(events)
    def test_interaction_boolean_number_and_missing_progress(self):
        events=interaction_events();events[1][1]['result']['work']['explicit_batches_committed']=False
        with self.assertRaises(ValueError):validate(events)
        for bad in [interaction_events()[1:],interaction_events()[:-1],interaction_events()+interaction_events()[:2]]:
            with self.assertRaises(ValueError):validate(bad)
    def test_memory_records_and_history_prefix(self):
        self.assertEqual(validate(interaction_events('triggers'),case='triggers')['classification'],'complete_declared_row_verified')
        events=interaction_events('triggers');events.pop(1)
        with self.assertRaises(ValueError):validate(events,case='triggers')
    def test_retention_shortfall_never_becomes_10000_pass(self):
        value=validate(retained_events(),'kernel','retained_branches')
        self.assertFalse(value['regular_full_size_pass']);self.assertEqual(value['classification'],'verified_resource_shortfall')
        self.assertEqual(value['validated']['resource_shortfall_attempts'],18)
        bad=retained(0,10000);bad['accepted_full_size_sample']=True
        with self.assertRaises(ValueError):d.retained_row(bad,0,10000,refs['kernel']['proofs'][0]['root_state_sha256'])
    def test_retained_start_missing_repetition_or_identity(self):
        events=retained_events()
        with self.assertRaises(ValueError):validate(events[1:],'kernel','retained_branches')
        events=retained_events();events[0][1]['baseline_state_sha256']='0'*64
        with self.assertRaises(ValueError):validate(events,'kernel','retained_branches')
        events=retained_events();events.pop(3)
        with self.assertRaises(ValueError):validate(events,'kernel','retained_branches')
    def test_partial_progress_keeps_denominator_and_torn_line(self):
        text=stdout(retained_events(),'kernel');prefix=text[:text.index('DENSE_RETAINED_SWEEP_RESULT')]+ 'DENSE_RETAINED_SWEEP_RESULT={"torn":'
        with tempfile.TemporaryDirectory()as temp:
            p=Path(temp)/'stdout';p.write_text(prefix)
            result=d.parse_stdout(p,'kernel','retained_branches',partial=True)
            prog=d.progress(result['events'],'kernel','retained_branches',False)
            self.assertEqual((prog['started'],prog['completed'],prog['expected']),(1,0,24));self.assertTrue(result['excluded_torn_final_line'])
            with self.assertRaises(ValueError):d.parse_stdout(p,'kernel','retained_branches')
    def test_strict_stdout_unknown_duplicate_json_extra_final(self):
        text=stdout([('DENSE_DIAGNOSTIC',kernel_report())],'kernel')
        with tempfile.TemporaryDirectory()as temp:
            p=Path(temp)/'stdout';p.write_text(text);self.assertTrue(d.parse_stdout(p,'kernel','ordinary')['successful_test_trailer'])
            for bad in [text+'unexpected\n',text.replace('"build_id":','"build_id":"b","build_id":',1),text.replace('1 passed','0 passed'),text.replace('DENSE_DIAGNOSTIC=','DENSE_INTERACTION_RESULT=',1)]:
                p.write_text(bad)
                with self.assertRaises(ValueError):d.parse_stdout(p,'kernel','ordinary')
    def test_wrong_fixed_helper_sha_rejects_before_import(self):
        with self.assertRaises(ValueError):d.load_fixed('0'*64)
    def test_complete_retained_row_work_and_lifecycle(self):
        r=retained(3,64,True);self.assertTrue(d.retained_row(r,3,64,refs['kernel']['proofs'][0]['root_state_sha256'])['full_size_sample'])
        for mutation in [lambda r:r['selected_first_write_counts'][0]['counts'].update(initial_queries=1),lambda r:r['samples'][0].update(operation_index=1),lambda r:r.update(sweep_lifecycle_nanos_including_guards_and_field_assertions=0)]:
            bad=retained(3,64,True);mutation(bad)
            with self.assertRaises(ValueError):d.retained_row(bad,3,64,refs['kernel']['proofs'][0]['root_state_sha256'])


if __name__=='__main__':unittest.main()
