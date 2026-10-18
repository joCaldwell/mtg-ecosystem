import copy
import importlib.util
import json
from pathlib import Path
import unittest
from unittest.mock import patch
import tempfile
import sys
from types import SimpleNamespace

HERE=Path(__file__).absolute().parent
spec=importlib.util.spec_from_file_location('ordinary',HERE/'run-ordinary-diagnostic-v1.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)


def catalog():
    value={'schema_version':11,'definitions':[]}
    raw=json.dumps(value).encode()
    return {'value':value,'bytes':len(raw),'sha256':__import__('hashlib').sha256(raw).hexdigest()}


def load(fixture):
    return {'input_bytes':fixture['bytes'],'input_sha256':fixture['sha256'],'canonical_bytes':len(m.canonical(fixture['value'])),'catalog_sha256':m.digest(fixture['value']),'decode_ns':123,'source_compile_verify_ns':234}


def legal():
    fixture=catalog()
    decisions=[{'decision_id':i+1,'player':i%4,'response':{'response':'cast' if i<2 else 'pass','card':{'card':i,'incarnation':0}}} for i in range(1024)]
    journal={'decisions':decisions,'concessions':[]}
    order=[r['response']['response'] for r in decisions]
    s={'inputs':1024,'rule_steps':2000,'advances':1025,'query_calls':2048,'response_kinds':{'cast':2,'pass':1022},'event_kinds':{'committed':5,'other':3},'actual_outcome_kinds':{'life_gained':2,'other':3},'commander_casts':1,'noncreature_casts':1,'paid_command_casts':1,'paid_noncreature_casts':1,'survivor_counts':[4,3],'outcome':'fixed_prefix_horizon','final_state_sha256':'a'*64,'journal_sha256':m.digest(journal),'trace_sha256':'b'*64,'journal':journal}
    t={'admission_ns':1,'setup_advance_ns':2,'observation_ns':[3]*1024,'policy_ns':[4]*1024,'check_submit_advance_ns':[5]*1024,'response_order':order,'replay_verify_ns':6,'dispose_ns':7}
    return {'schema':'ordinary-legal-pod-diagnostic-v1','status':'passed','error':None,'build_id':'c'*64,'seed_index':1,'starting_player':1,'mulligans':[0]*4,'trace_mode':'off','quantum':1024,'warmups':1,'measured_repeats':5,'catalog_load':load(fixture),'reference_semantics':s,'warmup_timings':copy.deepcopy(t),'samples':[{'repetition':i,'semantics':copy.deepcopy(s),'timings':copy.deepcopy(t)} for i in range(5)]},fixture


def artifacts():
    fixture=catalog();cases=[];roots=[]
    for i in range(12):
        departures=[] if i<4 else [3] if i<8 else [2,3]
        case={'index':i,'departures_before_setup':departures}
        cases.append(case)
        snapshot={'snapshot_bytes':100,'snapshot_sha256':'a'*64,'state_sha256':'b'*64,'snapshot_ns':1,'restore_ns':2,'dispose_ns':3}
        batches=[]
        for size in [1,64,1024]:
            s={'size':size,'query_calls':size*2,'bytes':size*9+1,'serialized_sha256':'d'*64}
            t={'query_ns':size*100,'copy_ns':size*10,'serialize_ns':size*50,'dispose_ns':size}
            batches.append({'semantics':s,'warmup':dict(t),'samples':[{'repetition':j,'semantics':dict(s),'timings':dict(t)} for j in range(5)]})
        roots.append({'index':i,'case':case,'live_players':[p for p in range(4) if p not in departures],'state_sha256':'b'*64,'journal':{'decisions':[],'concessions':[{'after_decisions':0,'boundary':0,'player':p} for p in departures]},'prepare_ns':1,'snapshot_warmup':snapshot,'snapshots':[dict(snapshot) for _ in range(5)],'batches':batches})
    return {'schema':'ordinary-artifact-batch-v1','status':'passed','error':None,'build_id':'c'*64,'trace_mode':'off','warmups':1,'measured_repeats':5,'catalog_load':load(fixture),'roots':roots},fixture,cases


class Guards(unittest.TestCase):
    def test_legal_complete_repetitions_have_exact_denominators(self):
        r,f=legal();proof=m.validate_legal(r,'c'*64,f)
        self.assertEqual(proof['measured_inputs'],5120)
        self.assertEqual(proof['transition']['operations'],5120)
        self.assertFalse(proof['completed_game_claim'])
        self.assertEqual(proof['search_useful_trials_claimed'],0)
    def test_zero_test_missing_repetitions_and_failed_native_rejected(self):
        for change in [lambda r:r['samples'].clear(),lambda r:r.update(status='failed'),lambda r:r['reference_semantics'].update(inputs=0),lambda r:r['samples'][0]['timings']['check_submit_advance_ns'].clear()]:
            r,f=legal();change(r)
            with self.assertRaises(ValueError):m.validate_legal(r,'c'*64,f)
    def test_raw_journal_hist_sha_and_sample_state_must_all_match(self):
        for change in [lambda r:r['reference_semantics']['journal']['decisions'][0]['response'].update(response='pass'),lambda r:r['reference_semantics']['response_kinds'].update(cast=3),lambda r:r['samples'][0]['semantics'].update(final_state_sha256='f'*64),lambda r:r['reference_semantics']['actual_outcome_kinds'].update(other=4)]:
            r,f=legal();change(r)
            with self.assertRaises(ValueError):m.validate_legal(r,'c'*64,f)
    def test_paid_cast_witness_must_be_positive_and_not_exceed_proposals(self):
        for name in ['paid_command_casts','paid_noncreature_casts']:
            for value in [0,2,True]:
                r,f=legal();r['reference_semantics'][name]=value
                with self.assertRaises(ValueError):m.validate_legal(r,'c'*64,f)

    def test_rehashed_short_prefix_still_rejects(self):
        r,f=legal();s=r['reference_semantics'];s['journal']['decisions'].pop();s['journal_sha256']=m.digest(s['journal']);s['inputs']=1023;s['response_kinds']['pass']-=1
        with self.assertRaises(ValueError):m.validate_legal(r,'c'*64,f)
    def test_catalog_raw_and_canonical_byte_binding(self):
        for key in ['input_bytes','input_sha256','canonical_bytes','catalog_sha256']:
            r,f=legal();r['catalog_load'][key]=0
            with self.assertRaises(ValueError):m.validate_legal(r,'c'*64,f)
    def test_artifact_complete_twelve_and_batch_rates_count_same_root_copies(self):
        r,f,cases=artifacts();proof=m.validate_artifacts(r,'c'*64,f,cases)
        self.assertEqual(proof['snapshot_samples'],60)
        self.assertEqual(proof['batch_samples'],180)
        self.assertEqual(proof['measured_observation_copies'],12*5*(1+64+1024))
        self.assertEqual(proof['independent_games_claimed'],0)
    def test_artifact_wrong_batch_bytes_queries_or_repetition_rejected(self):
        for change in [lambda r:r['roots'][0]['batches'][1]['semantics'].update(bytes=999),lambda r:r['roots'][0]['batches'][1]['semantics'].update(query_calls=64),lambda r:r['roots'][0]['batches'][1]['samples'].pop(),lambda r:r['roots'][0]['batches'][1]['samples'][0]['semantics'].update(serialized_sha256='f'*64)]:
            r,f,cases=artifacts();change(r)
            with self.assertRaises(ValueError):m.validate_artifacts(r,'c'*64,f,cases)
    def test_artifact_omitted_root_false_survivors_snapshot_or_hidden_concession_rejected(self):
        for change in [lambda r:r['roots'].pop(),lambda r:r['roots'][8].update(live_players=[0,1,2]),lambda r:r['roots'][1]['snapshots'][0].update(snapshot_bytes=101),lambda r:r['roots'][1]['journal']['concessions'].append({'after_decisions':0,'boundary':0,'player':2})]:
            r,f,cases=artifacts();change(r)
            with self.assertRaises(ValueError):m.validate_artifacts(r,'c'*64,f,cases)
    def test_preflight_exception_retains_failed_attempt_without_resource_report(self):
        sys.path.insert(0,str(HERE))
        import measurement_host as h
        with tempfile.TemporaryDirectory() as temp:
            parent=Path(temp);parent.chmod(0o700);output=parent/'attempt'
            a={'test_executable':{'path':'/not/executed','sha256':'a'*64},'cpu_ids':[0],'engine_build_id':'b'*64}
            fixed=SimpleNamespace(load_host=lambda _:h)
            argv=['driver','--fixed-driver-sha256','0'*64,'--helper-sha256','0'*64,'--authority',str(parent/'authority.json'),'--authority-sha256','0'*64,'--mode','legal','--output',str(output),'--execute']
            with patch.object(sys,'argv',argv),patch.object(m,'load_fixed',lambda _:fixed),patch.object(m,'source_guard',lambda *args:(a,{})),patch.object(h,'run_bounded',side_effect=ValueError('invalid CPU authority')):
                self.assertEqual(m.main(),1)
            result=json.loads((output/'result.json').read_text())
            self.assertEqual(result['status'],'diagnostic_failed_or_truncated')
            self.assertIsNone(result['resource_report_sha256'])
            self.assertIn('CPU',result['failure']['reason'])

    def test_debug_assertions_numeric_zero_is_not_release_false(self):
        a={'stage_root':str(m.STAGE),'build_settings':{'profile':'release','debug_assertions':0}}
        fixed=SimpleNamespace(authority_check=lambda *args:a,require_target=lambda *args:None,SIM_TARGET={'name':'mtg-sim','kind':['bin']})
        with self.assertRaises(ValueError):
            m.source_guard(fixed,Path('/unused'),'0'*64,'legal')

    def test_authentication_happens_before_loading_fixed_authority_code(self):
        with self.assertRaises(ValueError):m.load_fixed('0'*64)
    def test_nan_and_zero_rate_denominators_not_accepted(self):
        r,f=legal();r['samples'][0]['timings']['check_submit_advance_ns']=[0]*1024
        with self.assertRaises(ValueError):m.validate_legal(r,'c'*64,f)
        r,f,cases=artifacts();r['roots'][0]['batches'][0]['samples'][0]['timings']['query_ns']=float('nan')
        with self.assertRaises(ValueError):m.validate_artifacts(r,'c'*64,f,cases)

    def test_native_proof_tmpdir_is_inside_owned_private_attempt(self):
        with tempfile.TemporaryDirectory() as directory:
            parent=Path(directory);parent.chmod(0o700);output=parent/'attempt'
            class Host:
                def real_path(self,*args,**kwargs):return parent
                def save_json(self,path,value):path.write_text(json.dumps(value))
                def run_bounded(self,**kwargs):
                    self.asserted=kwargs['env']['TMPDIR']
                    if self.asserted!=str(output/'tmp'):
                        raise AssertionError('proof TMPDIR escaped owned attempt')
                    if (output/'tmp').stat().st_mode&0o777!=0o700:
                        raise AssertionError('proof temporary parent not private')
                    raise ValueError('fixture stops before any child')
            h=Host();fixed=SimpleNamespace(load_host=lambda _:h)
            a={'test_executable':{'path':'/unused/exe','sha256':'a'*64},'cpu_ids':[0]}
            argv=['driver','--fixed-driver-sha256','0'*64,'--helper-sha256','0'*64,'--authority',str(parent/'authority.json'),'--authority-sha256','0'*64,'--mode','legal','--output',str(output),'--execute']
            with patch.object(sys,'argv',argv),patch.object(m,'load_fixed',lambda _:fixed),patch.object(m,'source_guard',lambda *args:(a,{})):
                self.assertEqual(m.main(),1)
            self.assertEqual(h.asserted,str(output/'tmp'))
            self.assertEqual((output/'tmp').stat().st_mode&0o777,0o700)

if __name__=='__main__':unittest.main()
