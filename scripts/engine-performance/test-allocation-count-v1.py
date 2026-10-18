import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
P=Path(__file__).absolute().parent
s=importlib.util.spec_from_file_location('alloc',P/'run-allocation-count-v1.py');m=importlib.util.module_from_spec(s);s.loader.exec_module(m)

class Guards(unittest.TestCase):
    def raw(self):
        return '\x1b[1;32mMemory usage summary: heap total: 900\x1b[0m\nfunction | calls | total memory | failed calls\n'+''.join(f'{name} | {n} | 300 | 0\n' for n,name in enumerate(['malloc','realloc','calloc','free','mmap','mmap64','mremap','munmap'],1))
    def test_counts_and_emitted_mmap_rows(self):
        x=m.parse_api_counts(self.raw());self.assertEqual(x['intercepted_api_rows']['mmap64']['calls'],6);self.assertFalse(x['complete_Rust_allocation_count_claim']);self.assertEqual(x['unemitted_api_rows'],['mmap(a)','mmap(r)','mmap(w)'])
    def test_installed_format_single_separator_and_ansi(self):
        raw='\x1b[00;34m malloc|\x1b[0m  196108086    20133284147              0\nrealloc|3459447 4199267207 0 (nomove:1077666, dec:154068, free:0)\ncalloc|1 152 0\nfree|196108087 23589811497\n'
        x=m.parse_api_counts(raw);self.assertEqual(x['intercepted_api_rows']['malloc']['calls'],196108086);self.assertEqual(x['intercepted_api_rows']['free']['total_memory_bytes_as_reported'],23589811497)
    def test_required_missing_or_duplicate_reject(self):
        for text in [self.raw().replace('free | 4 | 300 | 0\n',''),self.raw()+'malloc | 1 | 3\n']:
            with self.assertRaises(ValueError):m.parse_api_counts(text)
    def test_unknown_or_malformed_API_reject(self):
        for extra in ['newapi | 3 | 9\n','malloc | nan | 3\n','\x1b[2J']:
            with self.assertRaises(ValueError):m.parse_api_counts(self.raw()+extra)
    def test_installed_parenthesized_mmap_labels_not_skipped(self):
        x=m.parse_api_counts(self.raw()+'mmap(r)|2 100 0\nmmap(w)|3 200 0\nmmap(a)|4 300 0\n');self.assertEqual(x['intercepted_api_rows']['mmap(a)']['calls'],4)
        with self.assertRaises(ValueError):m.parse_api_counts(self.raw()+'mmap(z)|9 300 0\n')
    def test_mmap_absent_not_invented_zero(self):
        raw=''.join(f'{n}|1|3\n' for n in ['malloc','realloc','calloc','free']);x=m.parse_api_counts(raw);self.assertNotIn('mmap',x['intercepted_api_rows']);self.assertIn('mmap',x['unemitted_api_rows'])
    def test_zero_work_reject(self):
        with self.assertRaises(ValueError):m.parse_api_counts(''.join(f'{n}|0|0\n' for n in ['malloc','realloc','calloc','free']))
    def test_warning_preserved_and_no_segment_claim(self):
        x=m.parse_api_counts(self.raw()+'warning: intercepted API scope\n');self.assertTrue(any('intercepted API scope' in w for w in x['warnings']));self.assertFalse(x['isolated_core_or_per_segment_count_claim'])
    def test_exact_direct_native_env_no_signal_entry(self):
        a={'test_executable':{'path':'/native'}}
        with patch.object(m,'tools_guard'):
            cmd,env,trace=m.command_environment(a,Path('/private/run'))
        self.assertEqual(cmd[0],'/native');self.assertNotIn('ordinary_core',str(cmd));self.assertNotIn('MTG_GPROFNG_PAUSED',env);self.assertEqual(env['LD_PRELOAD'],'/usr/lib/libmemusage.so');self.assertEqual(env['MEMUSAGE_NO_TIMER'],'yes');self.assertEqual(env['MEMUSAGE_TRACE_MMAP'],'yes');self.assertEqual(trace,Path('/private/run/execution/allocation-api.bin'));self.assertNotIn('SECRET',env)
    def test_wrong_authority_before_import(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'a';p.write_text('{}')
            with patch.object(m,'load',side_effect=AssertionError('must not import')):
                with self.assertRaises(ValueError):m.guard(p,'0'*64)
    def test_paused_ordinary_forbidden(self):
        a={'allocation_tools':{}}
        with self.assertRaises(ValueError):m.tools_guard(a)
    def test_helper_output_names_are_actual(self):
        source=(P/'run-allocation-count-v1.py').read_text();self.assertIn('execution/workload.stdout',source);self.assertIn('execution/workload.stderr',source);self.assertNotIn('execution/stdout.log',source)

if __name__=='__main__':unittest.main()
