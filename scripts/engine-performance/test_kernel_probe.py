import importlib.util
from pathlib import Path
import unittest

p=Path(__file__).absolute().parent/'probe-host-enforcement.py'
spec=importlib.util.spec_from_file_location('probe',p)
m=importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)

class ProbeProof(unittest.TestCase):
    def row(self,status):
        return {'status':status,'kernel_initial':{'memory_max':str(2*1024**3),'memory_swap_max':'0','pids_max':'128'},'audit':{'actual_child_kernel_caps_and_affinity_verified':True},'kernel_final':{'memory_peak_bytes':123,'memory_events':{'oom':0,'oom_kill':0,'max':0}},'cleanup':{'unit_empty':True,'launcher_reaped':True},'failure':{'type':'TimeoutError'}}
    def test_pass_requires_actual_success_explicit_timeout_and_closure(self):
        a,b=self.row('passed'),self.row('failed')
        self.assertFalse(m.validate_outcomes(a,b)['native_measurement_claim'])
        for change in [lambda a,b:b.update(status='passed'),lambda a,b:b['failure'].update(type='RuntimeError'),lambda a,b:b.pop('kernel_initial'),lambda a,b:a['cleanup'].update(unit_empty=False),lambda a,b:b['kernel_final']['memory_events'].update(oom_kill=1)]:
            a,b=self.row('passed'),self.row('failed')
            change(a,b)
            with self.assertRaises(ValueError):
                m.validate_outcomes(a,b)
    def test_helper_hash_checked_before_import(self):
        with self.assertRaises(ValueError):
            m.load_helper('0'*64)
    def test_actual_child_caps_and_affinity_not_boolean(self):
        class Host:
            MEMORY_BYTES=2*1024**3
            TASKS=128
        row={'scope':'tiny Python host-enforcement probe only; no native performance measurement','pid':123,'affinity':[0],'cgroup':'/mtg-measure-kernel-success-abc.service','memory_max':str(Host.MEMORY_BYTES),'swap_max':'0','pids_max':'128'}
        m.success_audit(row,[0],Host)
        for key,value in [('affinity',[1]),('memory_max','max'),('swap_max','1'),('pids_max','max'),('cgroup','/')]:
            bad=dict(row)
            bad[key]=value
            with self.assertRaises(ValueError):
                m.success_audit(bad,[0],Host)

if __name__=='__main__':
    unittest.main()
