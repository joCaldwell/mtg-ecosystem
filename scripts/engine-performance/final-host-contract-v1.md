# Portable final measurement hosts — preparation only

No authority is approved here. No native workload, Cargo command, systemd unit,
benchmark, profile, replay or real pod was launched. The tests use tiny fabricated
JSON/source trees and mocked process leases; they establish guard behavior only.
The original T201/303 and P8 sources are byte-preserved under `originals/`.
The shared `run-fixed-work.py` remains untouched. The host-only pod Evidence
read correction changes `measurement_host.py` and requires fresh host pins.

This directory contains three thin executable hosts and one shared input/lease
module. The primary's idle monitoring and the reviewer's independent raw-CPU
arithmetic/utility aggregation are retained from the reviewed P8 hosts. Native
sampling, candidates, policies, seeds, roots, workloads and core APIs are unchanged.
Build-salted captures/proposals must agree **within the actual measured Build**;
no cross-Build normalization or transferred performance result is introduced.

## Commands (unresolved roles, not approved invocations)

```text
python3 run-final-pod-series-v1.py --authority POD_SERIES_AUTHORITY --authority-sha256 SHA --inputs-sha256 HOST_INPUTS_SHA --output FRESH_SERIES --execute
python3 run-final-search-primary-v1.py --authority SEARCH_AUTHORITY --authority-sha256 SHA --inputs-sha256 HOST_INPUTS_SHA --run-directory FRESH_PRIMARY --quiescent
python3 review-final-search-primary-v1.py --authority SEARCH_AUTHORITY --authority-sha256 SHA --inputs-sha256 HOST_INPUTS_SHA --run-directory EXISTING_PRIMARY --expected-wrapper-sha256 ACTUAL_WRAPPER_REPORT_SHA --execute
```

All source/authority/file paths must be absolute and free of symlink ancestry.
Each fresh run is a direct child of its authority's existing0700 `run_parent`.
There is no overwrite/resume/retry option. Original failed outputs survive.
Reports/files are0600, directories0700. A wrapper source or dependency change
requires new source pins and review, not a fallback to currently existing bytes.

## Pod-series authority

`final-pod-series-authority-v1` requires:

- `engine_build_id`, `run_parent`, `warmup_batches:1`, `measured_batches:10`,
  `batch_indices:[0,...,10]`, `seed_indices:[0,...,17]`, and
  `retry_policy:"fail-stop-no-retry-v1"`.
- File pins `{path,sha256}` named `single_batch_driver`,
  `single_batch_authority`, `ordinary_driver`, `fixed_driver`; plus
  `measurement_helper_sha256` for the authenticated helper adjacent to fixed.
- `host_sources`: absolute script path→SHA map containing this wrapper,
  `host_inputs.py` and every dependency above. Pins are authenticated before
  imported Python executes.

The existing single-batch driver's own `auth()` validates its actual baseline
SIM test executable, Cargo `test:true/features:[]`, complete compiled source
manifest/Build/settings, source-bound correctness, exact18 seed/deck/policy
cases and one physical CPU. Its **120-second / 2GiB charged / swap0 / Tasks128**
guard, positive independent child RSS≤2GiB, pressure-event rejection and verified
service cleanup are unchanged. The series also rechecks those emitted resource
facts, executable/affinity and their exact linked host-report hash.

There are exactly11 sequential batches, not11 attempts until one passes:
18 warmup pods followed by180 measured pods. Every attempted controller is
recorded before invocation. The first failure preserves the attempted result,
stdout/stderr and still-unstarted batch indices; no throughput summary follows.
The inherited150-second controller guard wraps each existing120-second batch;
it is not a larger native-work budget. Full Evidence comparison, one game at a
time, omits **only** `elapsed_millis` and requires all other fields to equal the
same seed's warmup Evidence. It retains the original cross-batch metadata scope
outside the individual native batch timer; the native complete replay and each
batch's source/resource audit remain inside its120-second envelope. Both native
generation and generation-plus-proof/persistence rates retain their old clocks.
Ten raw measured totals, variance and median/quartiles remain visible.

Only the single-batch and series **full pod Evidence** reads allow 256 MiB
(268,435,456 bytes), retaining the full detailed seed0 trace and all four
checkpoint strings. Both strict JSON readers default to 128 MiB for control,
report and authority inputs. They open once, check descriptor size, read at most
cap+1 bytes and reject excess bytes before parsing, including growth after the
size check. Duplicate keys and nonfinite constants still reject. Each full game
is released before reading the next. This is an input byte bound, not a Python
heap bound or native RSS proof; canonical full-field comparison still omits only
validated top-level `elapsed_millis`. Existing failed attempts and their unstarted
batches remain unchanged. This correction creates no new authorities or native
acceptance/performance evidence; fresh host pins and a fresh complete series
remain required.

## Search authority

`final-search-host-authority-v1` requires:

- `stage_root`, actual `build_id`, `native_files`, `native_bytes`, and
  `{path,sha256}` pins `native_manifest`, `archive_manifest`, `cli`,
  `compiler_artifact`, `workload`, `fixed_driver`, `primary_wrapper`.
- `archive_files_prefix`: `"files"` for retained-copy manifests, or `""` for a
  release archive whose rows address the manifest's own directory;
  `archive_native_prefix`, normally `"source"`, addresses native rows beneath
  that container. The manifest's `files` map must bind actual stored bytes/size
  of the native manifest, each source, CLI and Cargo artifact. The archived
  manifest's `engine_build_id` must match. No directory-layout guessing occurs.
- `build_settings` with all eight exact values: rustc, release profile, target,
  encoded rustflags, opt_level3, debugfalse, debug_assertionsfalse,
  overflow_checkstrue. The actual raw Cargo artifact must name production
  `mtg-sim/bin`, **profile.test:false**, features`[]`, opt3/debug0/assertfalse/
  overflowtrue. Its original executable and archived CLI must have the same
  pinned bytes. A SIM test artifact or allocation-instrumented artifact rejects.
- `host_sources` absolute path→SHA map containing both hosts, this input module,
  the pinned fixed guard and authenticated measurement helper;
  `measurement_helper_sha256` binds the adjacent helper before import.
- `run_parent` and explicit `prerequisites`, including unique roles
  `functional_gates` and `independent_audit`. Each entry has `file:{path,sha256}`
  and `expected_status:"passed"`; optional `expected_build_id` is checked against
  its `build_id`. The authority author must choose real completed required
  predecessor evidence and its declared Build; these host scripts do not create
  substitute success proofs or infer current full-release acceptance from an
  older prerequisite. At least both roles are mandatory.

The unchanged fixed helper independently enumerates the complete native namespace
and recomputes the original Build algorithm. Every live and archived source byte
and length must match the actual compiled manifest. Every host pin, authority,
archive row, predecessor and executable is checked before and after. The precise
workload SHA is fixed to `75f1665a6c41dfc285cb4cb46b74923dd1760d753783ec0a71b1d0b2b4834d45`
from the actual `search-workload.json`; it is not a Build-derived capture hash.
The benchmark child receives and records only PATH=`/usr/bin:/bin`, LANG=`C`
and LC_ALL=`C`; ambient `LD_PRELOAD`, library paths and instrumentation variables
are not inherited. The pod controller and replay leases use the same explicit
minimal environment.

## Primary/replay scopes preserved

The primary executes the production CLI's exact60 jobs (12 roots × five
repetitions) once. It retains original **600-second native-suite guard**, not a
new120-second whole-batch limit. Each native root still has5seconds hard,
p95≤3seconds at rank57/60, whole-processRSS≤2GiB, four actual distinct physical
cores0/1/2/3, minimum8 distinct useful worlds **for every candidate**, desired32,
maximum32 candidates and256 accepted-input horizon. Capture→sampling→candidate
queries→fork/apply→rollout→aggregation→disposal retains the native clock. Fixture
preparation and trusted artifact encoding/audits retain their original separate
scopes. The wrapper's suite guard measures the native child lifetime/monitor;
final metadata/source audits are separately included in wrapper elapsed time.

`--quiescent` is a caller declaration, not magical proof. The original two-second
preflight/600-second-old sleeping Node baseline and each-window/whole-window
aggregate≤1% of **one** core are retained, including waited-child CPU. New heavy
jobs, vanished/reused PIDs, active/recent nodes and counter regressions reject.
No unrelated process is signaled. Raw inventories contain no argv/secrets.

The reviewer independently reconstructs the original idle proof, all60 job
denominators, exact first-eight per-candidate work/selection, overlapping useful
trial equality, full utility sums, measured p95 and actual whole-process peaks.
It then runs **all60** native semantic replays, at mosttwo concurrently, each
under its original300-second replay guard. The useful/unverified-stop counts
must exactly match; no second timing measurement or useful claim for diagnostic
stops is introduced. Every replay has its own session/lease and bounded
TERM/KILL/reap handling, with input/executable hashes checked at both ends.

SIGINT/SIGTERM handlers record cancellation rather than raising during `Popen`;
cleanup begins only after the caller owns the lease. Handlers are restored.
No accepted failure automatically retries. Cleanup that cannot prove reaping/
owned-group emptiness remains failed with the PID/raw attempt retained. The
primary no longer falls back to an unbounded final `wait4`; it fails explicitly
after the two bounded cleanup windows. This cleanup hardening does not extend
the native5second root budget or turn an incomplete row into a pass.
Known cancellation through cleanup or the final success write withdraws the
success publication and writes failed status. Pure callback tests exercise that
boundary in each host. The unchanged single-batch `auth()` API returns
`(authority,decks,limits)`; the series guard explicitly unpacks it and has a
guard-level tuple-returning API regression in addition to its mocked series tests.

These hosts do not alter the separate fixed-work1/2/4/16-physical-worker
diagnostic, the D48 rows/105 cells, the artifact1/64/1024 matrix, omniscient
comparison or original P7/P8 reports. Those remain distinct measurement roles.

## Local checks

```text
python3 test-final-hosts-v1.py -v
python3 -m py_compile host_inputs.py run-final-pod-series-v1.py run-final-search-primary-v1.py review-final-search-primary-v1.py test-final-hosts-v1.py
```

Guard logs, original source pins, exact patches and preparation inventory are
retained here. Pure guard success is neither an authority approval nor evidence
that any future final candidate meets the actual resource/performance limits.
