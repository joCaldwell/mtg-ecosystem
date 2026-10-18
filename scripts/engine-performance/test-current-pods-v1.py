import copy,hashlib,importlib.util,json,tempfile,unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
P=Path(__file__).absolute().parent

def load(name,file):
 s=importlib.util.spec_from_file_location(name,file);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
m=load('pod',P/'run-current-pods-v1.py');o=load('ordinary',P/'run-ordinary-diagnostic-v1.py');h=load('h',P/'measurement_host.py')
BUILD='a'*64
# Parser-only fixtures, not card facts or real completed games.
DECKS=[{'commander':'white-fixture','cards':[{'oracle_id':'white-fixture','count':1},{'oracle_id':'basicW-fixture','count':99}]},{'commander':'BG-fixture','cards':[{'oracle_id':'BG-fixture','count':1},{'oracle_id':'basicBG-fixture','count':99}]}]
PROFILE='fixture-profile'

def fixture(index):
 j={'decisions':[{'decision_id':0,'player':index%4,'response':{'response':'pass'}}],'concessions':[]}
 winner=index%4
 return {'config':m.config(index,PROFILE,DECKS),'policy':'develop','mulligans':[(index+s)%3 for s in range(4)],'journal':j,'trace_mode':'detailed'if index==0 else'off','losses':[[p,'life']for p in range(4)if p!=winner]if index==0 else[],'checkpoints':[{'decisions':0 if k==0 else 1,'rule_boundary':0 if k==0 else 2,'snapshot':'fixture opaque checkpoint, real reconstruction remains native oracle'}for k in range(4)]if index==0 else[],'outcome':'finished','winner':winner,'decisions':1,'advances':2,'rule_steps':2,'query_calls':2,'work_limit':2000000,'quantum':1024,'turns':1,'final_state_sha256':'b'*64,'surviving_seats':[winner],'survivor_counts':[4,1],'elapsed_millis':0}

def create(directory):
 rows=[];generation=0;proof=0
 for i in range(18):
  e=fixture(i);encoded=o.canonical(e);path=directory/f'game-{i:02}.json';path.write_bytes(encoded)
  header={'kind':'header','build_id':BUILD,'config':e['config'],'policy':'develop','mulligans':e['mulligans']};footer={'kind':'footer','outcome':e['outcome'],'winner':e['winner'],'state_sha256':e['final_state_sha256'],'rule_boundary':e['rule_steps']}
  with(directory/f'game-{i:02}.jsonl').open('wb')as f:
   for x in [header,{'kind':'accepted','input':e['journal']['decisions'][0]},footer]:f.write(o.canonical(x)+b'\n')
  c={k:[1]*e[v]for k,v in m.VECTORS.items()};c.update({k:1 for k in m.SCALARS});c['generation_ns']=100;generation+=100;proof+=2
  rows.append({'index':i,**{k:e[k]for k in ['outcome','winner','survivor_counts','turns','decisions','advances','rule_steps','query_calls','final_state_sha256']},'journal_sha256':o.digest(e['journal']),'evidence_file':str(path),'evidence_sha256':hashlib.sha256(encoded).hexdigest(),'clocks':c,'evidence_encode_ns':1,'evidence_io_ns':1,'independent_replay_ns':2,'verified':True})
 wins=[sum(i%4==p for i in range(18))for p in range(4)]
 report={'schema':'current-fixed18-pod-throughput-v1','build_id':BUILD,'planned_pods':18,'completed_pods':18,'passed':True,'no_search_acceptance_or_strategy_quality_claim':True,'catalog_decode_ns':1,'compiler_verify_ns':1,'generation_sum_ns':generation,'independent_proof_sum_ns':proof,'whole_batch_ns':2000,'rows':rows,'outcome_histogram':{'finished':18},'winner_counts':wins,'failures':[],'loss_reason_coverage':True,'generation_pods_per_second':18e9/generation,'whole_batch_pods_per_second':18e9/2000}
 (directory/'report.json').write_bytes(o.canonical(report));return report

def check_mutation(mutate):
 with tempfile.TemporaryDirectory()as tmp:
  directory=Path(tmp);r=create(directory);mutate(r,directory)
  return m.validate_batch(o,h,directory,r,BUILD,(PROFILE,DECKS))

class Guards(unittest.TestCase):
 def test_complete_all18_fixture_no_actual_games(self):
  r=check_mutation(lambda *_:None);self.assertTrue(r['all18_complete_verified']);self.assertEqual(len(r['rows']),18);self.assertEqual(len(r['retained_files']),37)
 def test_seed_start_mulls_and_two_extra_deck_modes(self):
  for i in range(18):
   c=m.config(i,PROFILE,DECKS);self.assertEqual(c['seed'],list(hashlib.sha256((m.DOMAIN+str(i)).encode()).digest()));self.assertEqual(c['starting_player'],i%4)
  self.assertEqual(m.config(16,PROFILE,DECKS)['decks'],[DECKS[0]]*4);self.assertEqual(m.config(17,PROFILE,DECKS)['decks'],[DECKS[1]]*4)
  with self.assertRaises(ValueError):m.config(18,PROFILE,DECKS)
 def test_wrong_seed_deck_policy_mulls_trace_checkpoint(self):
  for mutation in [lambda e:e['config']['seed'].__setitem__(0,0),lambda e:e['config']['decks'].__setitem__(1,DECKS[0]),lambda e:e.update(policy='draw_only'),lambda e:e.update(mulligans=[0]*4),lambda e:e.update(trace_mode='off'),lambda e:e['checkpoints'].pop()]:
   e=fixture(0);mutation(e)
   with self.assertRaises(ValueError):m.evidence(o,e,0,PROFILE,DECKS)
 def test_concessions_or_invented_counts_reject(self):
  for mutation in [lambda e:e['journal']['concessions'].append({'player':3,'after_decisions':0,'boundary':0}),lambda e:e.update(decisions=2),lambda e:e.update(quantum=1),lambda e:e.update(work_limit=10),lambda e:e.update(query_calls=0),lambda e:e.update(rule_steps=True)]:
   e=fixture(1);mutation(e)
   with self.assertRaises(ValueError):m.evidence(o,e,1,PROFILE,DECKS)
 def test_all18_denominator_no_omission_or_duplicate(self):
  for mutation in [lambda r:r['rows'].pop(),lambda r:r['rows'][1].update(index=0),lambda r:r.update(completed_pods=17)]:
   with self.assertRaises(ValueError):check_mutation(lambda r,d:mutation(r))
 def test_wrong_evidence_bytes_and_stream_body_reject(self):
  for mutation in [lambda r,d:(d/'game-01.json').write_text('{}'),lambda r,d:(d/'game-01.jsonl').write_text('{}\n')]:
   with self.assertRaises(ValueError):check_mutation(mutation)
 def test_wrong_clock_vectors_overlap_and_bool_reject(self):
  for mutation in [lambda c:c['advance_ns'].pop(),lambda c:c.update(generation_ns=1),lambda c:c['policy_ns'].__setitem__(0,True)]:
   with self.assertRaises(ValueError):check_mutation(lambda r,d:mutation(r['rows'][0]['clocks']))
 def test_wrong_rate_or_fault_cannot_pass(self):
  for mutation in [lambda r:r.update(generation_pods_per_second=float('inf')),lambda r:r.update(generation_pods_per_second=42),lambda r:r['rows'][0].update(verified=False),lambda r:r.update(loss_reason_coverage=False),lambda r:r.update(failures=[{'bad':True}])]:
   with self.assertRaises(ValueError):check_mutation(lambda r,d:mutation(r))
 def test_false_loss_duplicate_or_fake_winner_reject(self):
  for mutation in [lambda e:e['losses'].append(e['losses'][0]),lambda e:e.update(winner=3),lambda e:e.update(survivor_counts=[4,3,4,1])]:
   e=fixture(0);mutation(e)
   with self.assertRaises(ValueError):m.evidence(o,e,0,PROFILE,DECKS)
 def test_extra_retained_member_reject(self):
  with self.assertRaises(ValueError):check_mutation(lambda r,d:(d/'foreign.json').write_text('{}'))
 def test_incomplete_batch_must_suppress_rates(self):
  with tempfile.TemporaryDirectory()as tmp:
   directory=Path(tmp);r=create(directory);r.update(passed=False,completed_pods=17,failures=[{'index':0,'replay_error':'fixture'}],generation_pods_per_second=None,whole_batch_pods_per_second=None);r['rows'][0]['verified']=False
   v=m.validate_batch(o,h,directory,r,BUILD,(PROFILE,DECKS));self.assertFalse(v['all18_complete_verified']);self.assertEqual(v['completed_pods'],17)
   r['generation_pods_per_second']=0
   with self.assertRaises(ValueError):m.validate_batch(o,h,directory,r,BUILD,(PROFILE,DECKS))
 def test_source_decks_are_parsed_not_recalled(self):
  with tempfile.TemporaryDirectory()as tmp:
   stage=Path(tmp);base=stage/'packages/game-engine/crates/core';(base/'src/engine').mkdir(parents=True)
   (base/'src/engine/choices.rs').write_text('')
   (base/'src/engine.rs').write_text('const C1: &str="id1"; const C2: &str="id2"; const B: &str="basic";')
   text='const MATCH_PROFILE: &str="source-profile";\n'
   for name,comm in [('white_deck','C1'),('black_green_deck','C2')]:text+=f'pub fn {name}() -> Decklist {{\n Decklist {{ commander: {comm}.into(), cards: entries(&[({comm},1),(B,99)]), }}\n}}\n'
   (base/'src/engine/match_admission.rs').write_text(text);profile,decks=m.source_decks(stage);self.assertEqual(profile,'source-profile');self.assertEqual(decks[1]['commander'],'id2')
   (base/'src/engine/match_admission.rs').write_text(text.replace('(B,99)','(B,dynamic())'))
   with self.assertRaises(ValueError):m.source_decks(stage)
 def test_wrong_helper_pin_before_import(self):
  with tempfile.TemporaryDirectory()as tmp:
   directory=Path(tmp);(directory/'run-ordinary-diagnostic-v1.py').write_text('raise RuntimeError("executed")')
   with patch.object(m,'P',directory):
    with self.assertRaisesRegex(ValueError,'before import'):m.load('0'*64)
 def test_preflight_failure_preserves_attempt_without_resource_claim(self):
  with tempfile.TemporaryDirectory()as tmp:
   parent=Path(tmp);parent.chmod(0o700);out=parent/'attempt'
   a={'test_executable':{'path':'/never/executed','sha256':'f'*64},'stage_root':str(parent),'cpu_ids':[0],'engine_build_id':BUILD}
   fixed=SimpleNamespace(load_host=lambda _:h)
   argv=['host','--ordinary-driver-sha256','0'*64,'--fixed-driver-sha256','0'*64,'--helper-sha256','0'*64,'--authority',str(parent/'authority'),'--authority-sha256','0'*64,'--output',str(out),'--execute']
   with patch.object(m.sys,'argv',argv),patch.object(m,'load',lambda _:SimpleNamespace(load_fixed=lambda _:fixed)),patch.object(m,'auth',lambda *args:(a,(PROFILE,DECKS),{})),patch.object(h,'run_bounded',side_effect=ValueError('bad CPU')):
    self.assertEqual(m.main(),1)
   result=json.loads((out/'result.json').read_text());self.assertEqual(result['status'],'failed_or_truncated_no_throughput');self.assertIsNone(result['host_report_sha256']);self.assertIsNone(result['generation_pods_per_second']);self.assertIn('bad CPU',result['failure']['reason'])
 def test_feature_on_or_unknown_features_not_baseline(self):
  m.baseline_features({'features':[]})
  for a in [{},{'features':None},{'features':['diagnostic-allocations']},{'features':['default']}]:
   with self.assertRaises(ValueError):m.baseline_features(a)
 def test_histogram_winner_float_or_bool_rejected(self):
  for mutation in [lambda r:r['winner_counts'].__setitem__(0,float(r['winner_counts'][0])),lambda r:r['outcome_histogram'].update(finished=18.0)]:
   with self.assertRaises(ValueError):check_mutation(lambda r,d:mutation(r))
 def test_checkpoint_boundaries_must_each_be_monotone(self):
  e=fixture(0);e['journal']['decisions']=[{'decision_id':i,'player':0,'response':{'response':'pass'}}for i in range(3)];e.update(decisions=3,advances=4,query_calls=6,rule_steps=5)
  for point,dec,step in zip(e['checkpoints'],[0,1,2,3],[0,2,1,5]):point.update(decisions=dec,rule_boundary=step)
  with self.assertRaises(ValueError):m.evidence(o,e,0,PROFILE,DECKS)
 def test_shared_target_and_bounds_required(self):
  fixed=SimpleNamespace(authority_check=lambda *_:{},require_target=lambda *_:(_ for _ in()).throw(ValueError('SIM required')),SIM_TARGET='sim')
  with self.assertRaisesRegex(ValueError,'SIM'):m.auth(o,fixed,Path('/unused'),'pin')

if __name__=='__main__':unittest.main()
