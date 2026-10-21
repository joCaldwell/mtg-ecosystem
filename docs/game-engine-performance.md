# Finite engine performance diagnostics

This is a candidate performance follow-up, separate from the released P7/P8
acceptance evidence. Current Build `3118eff2…d4d8eee` verifies the ordinary
legal prefix, dense-trigger retry and an isolated allocation-request phase.
The earlier fixed-work scaling, twelve-root artifact and actual-world diagnostics
retain their own measured Build identities below. Genuine dense-pod preparation
hits the memory cap; legal branching, persistent-worker/full-pod measurements,
profile-driven changes and final release closure are still in progress.
No new optimization, admission expansion, general Magic performance claim or
playing-strength claim follows from these measurements.

The measured native source closure has 197 files and Build ID
`303ee7e20255d4d98fde0bf35ce41b114ea52d1cd13a78b6f7f5dae24045324e`.
[Compiled release evidence](../generated/performance-verification/compiled-release-evidence-v1.json)
pins the source collector, effective build settings, exact actual Cargo
compiler-artifact rows, complete build logs and test executables. The sim test
executable SHA is
`686360acc1df8ae5983d7af4c0cd2852454a7cdd866568f1c29e8c8b6d3b1632`;
the core test executable SHA is
`da90c20fce81a01c2510341d3f48eb9b45b4e887f0a6a6f738cd01b20f635b1e`.

The [focused release correctness audit](../generated/performance-verification/release-focused-correctness-audit.json)
passed 22 tests: 12 dense-core, four fixed-work, two actual-world diagnostic
guards and four ordinary adapters. Six explicit diagnostic/profile entries were
ignored. Formatting and workspace Clippy passed. These are the current chunk's
checks for that measured source closure; they do not claim a new full-workspace
test/offline-retention release.

## Fixed work across physical cores

The diagnostic uses unchanged `commander-open-search-v1` root 0. The permitted
capture generates its three complete production candidates and freezes 32
complete, genuinely distinct hidden assignments in the production sampler's
world-index/attempt order. Every candidate executes in every world: **96 trials
per schedule**, with the existing 256-accepted-input horizon, policy, evaluator,
query/rules calls and full State hashing. This avoids making the work list depend
on a scheduling deadline. Thirty-two worlds provide work for all 16 available
physical cores; the separate eight-world support test is not this scaling row.

Each worker-count process first executes its serial oracle, then one warmup and
five measured schedules. Counts are 1, 2, 4 and 16 physical-core representatives.
All processes inherit the same full physical CPU mask; each native worker pins
to its declared representative. No horizon, source fixture, candidate, assignment
or policy changes between rows.

| Workers | Median pool time | Observed ratio to one worker | Useful trials per pool second |
| ---: | ---: | ---: | ---: |
| 1 | 13.082194 s | 1.0000× | 7.3382 |
| 2 | 6.753055 s | 1.9372× | 14.2158 |
| 4 | 3.501744 s | 3.7359× | 27.4149 |
| 16 | 1.026941 s | 12.7390× | 93.4815 |

These are medians of five complete pool executions, not product search latency
or full-game throughput. Pool clocks include worker creation/pinning, world
construction, all trials/queries/rules/State hashing, disposal, joins and sorted
aggregation. They exclude the common producer, serial oracle and parity audit.
Each process's whole wall time includes those excluded activities as well.

The [independent raw-result review](../generated/performance-verification/independent-fixed-root0-run1-review.json)
(SHA256 `6d1b51afef1db15a8c1b063baa77f2d86c3b1bee3181a37cb094becacdc4e37c`)
re-derived candidate and assignment digests, every candidate/world pair, actual
work counts, all four exact utility values, outcomes, full State hashes and
aggregates. All **2,688 executions** matched: 1,920 timed trials, 384 warmup trials
and 384 serial-oracle trials. The raw
[run status](../generated/performance-verification/measurement-runs/fixed-root0-run-1/status.json)
retains all four processes and every sample. A fault, truncated row, missing pair
or changed semantic result cannot contribute a speedup.

## Resource and provenance boundaries

The root-reviewed fixed authority pins guard source `fa98a77d…daab018`, shared
OS helper `f426db8f…1001648`, the exact sim test executable, complete native
source inventory and contract hashes. The shared guard recomputes the existing
core Build ID from source bytes, rustc and its historical six build-environment
fields. The separately pinned actual Cargo artifact proves release/opt3,
debug/assertions off and overflow checks on; overflow checks are not falsely
claimed to be part of that historical Build-ID algorithm.

Every row used a fresh private user-systemd service with actual cgroup-v2
MemoryMax 2 GiB, swap 0 and TasksMax 128, and a fixed 120-second deadline covering
startup, native work, final audit and cleanup. The lifetime backstop also stops a
service if its controller dies. Actual kernel properties are checked before the
command is released. All four groups were independently observed empty/absent
and their units inactive after completion; no new native trial was needed for
that audit.

The maximum whole-group **charged-memory peak** was 944,635,904 bytes. The
separately recorded direct native process RSS high-water was at most 924,127,232
bytes. Charged memory is not an RSS upper bound: shared-page ownership, page
cache and kernel charges differ. The RSS guard is independently checked after
exit and is not a tree-summed RSS measurement. All four rows had zero recorded
max/OOM events. The longest whole-host row was 92.0301 seconds, below 120 seconds.

For the 16-worker process, native wall/user/system time was respectively
19.7421/107.0042/1.08142 seconds. That includes its producer, serial oracle,
warmup and five samples and must not be compared directly with the 1.026941-second
pool median. Reference-machine observations were recorded during this diagnostic;
this is not an assertion of the separate quiet-primary acceptance conditions.
RSS differences are not exact live-heap allocation counts.

## Dense observations and retained host correction

The first dense batch executed all twelve declared attempts. Its raw
[batch status](../generated/performance-verification/measurement-runs/dense-run-1/batch-status.json)
is terminal, rather than a blanket pass. At this draft checkpoint, nine attempts
reported complete rows and three reported failures. Two history-row failures
were subsequently reviewed against a corrected host predicate without
remeasuring or changing those original failed statuses. The trigger resource
failure remains unresolved; a complete final batch audit remains separate.

The four base synthetic kernel geometries, modifier evaluator, retained-branch
row and damage/target/combat rows reported complete outputs. These are private
synthetic/domain fixtures, not newly admitted Commander positions. In particular,
modifier counts above the admitted effect capacity measure the pure evaluator;
they do not demonstrate creation of that many effects or pending triggers.
The existing retained-branch row uses synthetic ordinary-eight geometry, not an
evolved legal pod or dense-256 branch workload.

The two extended history rows were rejected by the original host's
history/visible-density predicate. Actual evolved roots have viewer row counts
`N+[9,9,9,10]`; the four shorter original geometries retain `N+7` for every viewer.
A narrow host-only correction verified both retained native outputs and their
original no-pressure resource proofs. This is a post-hoc validation, not new
measurement or an upgrade of the original failed host statuses. The
[post-hoc review](../generated/performance-verification/dense-history-posthoc-review-v1.json)
and [independent correction/portability review](../generated/performance-verification/independent-dense-host-density-portability-review-runtime.json)
retain exact source and raw-result hashes; fifteen small host guards passed.

The trigger attempt exited its native test with code zero but failed the outer
memory-pressure guard: charged peak reached 2,147,483,648 bytes with 1,206
`memory.events.max` events and zero OOM kills. Its direct process RSS high-water
was 1,101,672,448 bytes. The group was cleaned up. Proof serialization and file
cache are part of the guarded process envelope; these counters alone do not
establish their causal allocation contribution. This attempt is a resource
failure, not a successful complete resource row or a rules-defined outcome.
No cap waiver or omitted replay proof repairs it.

## Remaining work

The actual ordinary legal-prefix attempt failed its kernel memory envelope
approximately three seconds after launch; the root retained the exact unit's
kernel OOM/2-GiB evidence and verified cleanup. The simultaneous original/replay
snapshot Strings, generic Values and repeated canonicalization are a candidate
proof-memory cause outside the timed core segment, not an established causal
profile. A [sequential full-byte proof proposal](../generated/performance-verification/ordinary-sequential-proof-proposal-v1.md)
keeps every State byte, history, replay, journal, trace and event check while
holding only one generic envelope at a time and releasing the original Game
before constructing the Detailed replay. The applied change and its bounded
retry are recorded below. The original failed attempt and its measured Build
remain unchanged; no cap waiver, smaller prefix or dropped history repairs it.

## Twelve-root artifact and actual-world diagnostics

The [independent raw audit](../generated/performance-verification/independent-sim-diagnostics-run1-review-runtime.json)
(SHA256 `6e788c8549a025df64372ac82b698128822fa6d9fbd9da6b5280f95403d7df4f`)
verified all 197 native source files, actual release identity, authorities,
raw-report/resource links, all declared root cases and post-run cgroup emptiness.
It did not rerun native workloads or alter original reports.

The artifact row completed twelve unchanged legal-pod roots with four, three or
two living seats. It retained 60 measured snapshot/restore samples and 180
measured query/copy/serialization batches: five repetitions at sizes 1, 64 and
1,024 per root, after warmups. The **65,340 measured observation copies** repeat
the same root within each batch. They are not independent games, network calls
or FFI measurements. The host independently checked raw journal ordering,
concession roots, complete snapshot identity/count fields, repeated batch
semantics and exact byte/query-call denominators. Native source oracles establish
full snapshot/restore State equality and actual encoded observation bytes;
these unavailable raw bytes were not independently reconstructed from hashes.
Catalog load measures already-mapped embedded IR decoding, with source compiler
validation separately clocked.

Artifact whole-host time was 87.3231 seconds; direct native wall/user/system time
was 87.0379/86.4190/0.198506 seconds. Peak charged memory was 568,922,112 bytes;
direct process RSS high-water was 566,505,472 bytes. All twelve roots completed
under the unchanged 120-second/2-GiB guard with no memory-pressure events.

The same-policy actual-world comparison completed all twelve fixed roots and
all 45 production candidates. Each candidate had eight genuinely distinct
sampled hypotheses and eight deterministic repeats of the **one actual Game**:
360 sampled trials plus 360 actual-world-repeat trials, all retaining complete
counts, outcomes, full State hashes and four exact utility quantities. The
actual forks preserve their real origin and RNG. Both branches share the
production strategy/evaluator and unchanged 256-input horizon; the trusted
actual Game never enters the permitted sampler or product candidate generator.

The host re-derived every candidate/assignment identity, all candidate/slot
pairs, all utility aggregates and first-tie selections. Sampled and actual-world
branches selected the same candidate in nine roots and differed in roots 3, 7
and 11. This is one finite same-strategy information diagnostic, **not evidence
of playing strength or a causal value-of-information estimate**. Sampled-then-
actual ordering and shared caches were not controlled. The 360 actual repeats
remain one distinct world per root and cannot satisfy a product eight-world
minimum. These diagnostic trials are separate from primary useful-trial counts.

Whole-host time was 108.9161 seconds; direct native wall/user/system time was
108.6089/107.9528/0.147579 seconds. Peak charged memory was 347,127,808 bytes;
direct process RSS high-water was 341,909,504 bytes. The row had no memory-pressure
events and all owned groups were independently observed absent/inactive.

## Ordinary legal-prefix retry on the separate v2 build

The proof-only follow-up compiled as Build
`8946cc6b5753c2b52fb352f80804b1b36c9e62f8f69c4ab0b8e6ed6afffc94ac`,
with sim test executable SHA
`9ac9fcb08ca8f938f833c1d4422ec03e0014fcdbfe7482782d85410e64a32edc`.
It preserves the same seed-1/start-seat-1 four-hundred-card pod, public-view-only
Develop policy, 1,024-input prefix and one warmup plus five measured repetitions.
Every repetition retained exactly the same complete semantics and accepted-input
journal. It is a horizon prefix with all four players still living, not a
completed game.

The [independent v2 raw audit](../generated/performance-verification/independent-ordinary-legal-run2-review-runtime.json)
(SHA256 `e4a27c907eca3e3d70fab24588817c2c2f865a707ef14336f070be8d2985287d`)
checked the complete current 197-file source/executable/authority binding,
all 6,144 actual inputs, all 5,120 measured entries in each timing vector,
raw journal SHA/order/response histogram and all full repeated semantics.
Per repetition, native evidence records 3,001 total rule boundaries, 1,025
advance calls, 2,048 query calls, 1,994 committed outcomes, six paid command-origin
casts, three paid noncreature casts, ten total paid spells and two completed
triggers. The host validates consistency of these event/paid-cast witnesses;
Detailed native replay and its source-checked proposal-to-paid-event join remain
the authority for raw event/trace data that the report does not export.

The five sums of accepted-input check/submit/automatic-advance segments were
0.403276, 0.406287, 0.403033, 0.405832 and 0.406101 seconds. Their rates were
2,520.38–2,540.74 **accepted input transitions per core second**. They are not
committed-rule-boundaries per second or full-game throughput. Observation,
policy, admission, setup, full-State proof, replay, serialization, I/O and disposal
have separate clocks. Across the 5,120 measured input segments, p50/p95/p99 were
310,599/809,759/1,805,837 nanoseconds. Five repetitions do not establish precise
tail probabilities for complete-pod costs.

The complete guarded process took 29.7820 seconds including host audit/cleanup;
direct native wall/user/system time was 29.5171/26.3551/0.646301 seconds. Peak
charged memory was 1,617,461,248 bytes and native process RSS high-water was
1,510,965,248 bytes, independently below 2 GiB, with zero memory-pressure/OOM
events. The exact unit was independently observed inactive, its cgroup absent
and the attempt-owned temporary proof directory empty. The 120-second deadline,
2-GiB cap, zero swap and single physical CPU were unchanged.

The generic oracle now streams every parsed State byte to private files and
compares complete lengths, bytes and both EOFs; matching SHA values never replace
the full comparison. It drops the original Game before building Detailed replay
and removes proof files before event/trace work. This is a proof-memory lifetime
change, not a gameplay optimization. Proof-phase clocks changed scope, so their
old/new values are not an identical-scope speedup. The snapshot implementation
and final trace digest still use their existing generic serializers; this result
does not prove those paths fit every larger history.

The v2 ordinary seven-test group and workspace Clippy passed. One unrelated new
core comparator test had an incorrect empty-write assertion and remained failed
at the time of this diagnostic. The successful bounded prefix does not upgrade
that unresolved test or establish a new full release; its correction and a new
verified source identity remain required.

## Remaining closure

CPU and allocation attribution for ordinary and dense work remains separate from
RSS/charged-memory observations. Profiling overhead and measurement scope must be
reported before proposing another optimization. Historical missing profiles are
not retroactively supplied by a new diagnostic. Retained branches for a genuine
legal ordinary pod and dense-256 geometry, evolved legal histories, final source
closure/retention and the current chunk's required verification remain open.
The finite product budgets are unchanged: 5 seconds hard, p95 at most 3 seconds,
2 GiB, four physical-core workers and at least eight distinct useful worlds per
candidate. The fixed-work pool table neither replaces nor relaxes those gates.

## Current 201-file diagnostic candidate

Build `3118eff2f2f4abcc2df5f50bacb001c5510a7761d4edeb8d4093f2df8d4d8eee`
adds private legal-branch, persistent-worker, full-pod and allocation diagnostics.
All-features workspace Clippy and formatting passed. There are 37 unique passing
focused normal tests, plus the counter arithmetic guard. A separate same-counter-
executable known-vector test passed inside the OS resource guard. These checks
are not the full workspace debug/release or offline-retention gate.

The exact 201 native inputs, four actual executables and build/test logs are
preserved under `performance-native201-3118eff2/manifest.json` in the external
store named by `game-engine-server.md`; manifest SHA-256 is
`f329420c48c87711f14e571f00937a769ac849b22852927afaee3b481db6ab16`.
Actual Cargo artifacts distinguish the ordinary sim executable
`3bae56d1638a8784dfb89191487ac1553af0208330d2ef96bc57fbb1e57f3556`
with features `[]` from the allocation executable
`a2f0250c62c4737bc4031d87644e9bf52e1ff05f37168c7418b81d9e7ef3beb6`
with `diagnostic-allocations`. The historical core Build hash does not encode
Cargo features. Every new sim diagnostic host therefore checks actual features
as well as source, compiler profile and executable SHA.

The current ordinary retry preserves all six complete 1,024-input prefixes,
including exact State, journal, trace, typed outcomes and paid-cast witnesses.
Its full semantics equal the prior 8946 build's reference. The five core-segment
rates are about 2,292–2,535 accepted inputs/second; the 10,000 initial calibration
hypothesis remains unmet. Whole-host wall time is 29.6033 seconds, native peak
RSS 1,511,092,224 bytes and charged peak 1,617,227,776 bytes, with zero max/OOM
counters and verified cleanup. The independent current-prefix/known-vector
review is `independent-ordinary-known-vector-v4c-runtime-review.json`, SHA
`c1562e17cab3cc1c7a0d5fdcd74a5927d6cfac64e97fee61eac46b0c69a7f9a5`.

The dense-trigger retry completes all six repetitions with the same complete
semantic proofs as the 303 build: each repetition retains 64 trigger placements
and resolutions, 324 accepted inputs, 327 advance calls and 842 committed rule
steps. Whole-host time is 48.5460 seconds, direct RSS high-water 1,102,487,552
bytes and charged peak 1,843,908,608 bytes. Zero pressure/OOM counters and cleanup
passed. The bounded comparator eliminates the extra actual-State proof file;
these measurements do not establish a causal page-cache allocation breakdown.
The original 303 memory-pressure failure remains unchanged.

### Isolated allocation requests and CPU evidence

The pinned `stats_alloc` 0.1.10 dev dependency forwards to `System` only in the
explicit test-plus-feature build. The registry archive checksum is
`5c0e04424e733e69714ca1bbb9204c1a57f09f5493439520f9f68c132ad25eec`;
all 11 archived package files match the actual compiled dependency source.
No authored unsafe code or production allocator policy was introduced.

The exact existing raw core loop consumes 1,024 precloned accepted inputs between
counter snapshots. Setup, policy, initial advance, full-State/journal/event
proofs, report serialization and disposal lie outside that counter region.
The complete result matches the current feature-off baseline. Requests are:

| Counter | Actual phase total |
| --- | ---: |
| Allocation requests, including zeroed allocation | 9,243,136 |
| Reallocation requests | 100,869 |
| Deallocation requests | 7,788,833 |
| Requested allocation bytes, including realloc growth | 1,140,452,873 |
| Requested deallocation bytes, including realloc shrink | 1,000,558,399 |
| Signed net realloc-byte change | 169,570,950 |

That is 9,026.5 allocation requests per accepted input. Counters cover the whole
process, including the waiting libtest main thread. Stable thread identities and
two exact-zero empty controls reject observed interference; they do not make
these counters thread-local. There is no noise subtraction, successful-allocation
census, C-allocation coverage or live/peak-heap claim. Realloc-byte change overlaps
its growth/shrink contributions and must not be added again to allocated bytes.
Instrumented wall times are not baseline latency acceptance.

The independent counter/dense-trigger audit is
`independent-allocation-dense-triggers-v4c-runtime-review.json`, SHA
`b9f4a425424cfbd1460cb5409460f1fc7e964b0b553141d6ec9351e0f3c53bf6`.
A separate current feature-off CPU capture encloses the unchanged raw core phase
(404,334,067 ns). Its function table samples 0.400 CPU seconds and attributes
0.210 inclusive seconds to visibility refresh. All collector warnings remain,
including the unreliable timer-period warning. Forty samples, inclusive overlap
and unresolved libc frames cannot establish precise causal cost shares. The
capture and allocation counts motivate investigation of immutable configuration
and State copying; they are not themselves an optimized release.

### Legal branching and the natural dense-history limit

The first genuine ordinary legal root completed all 24 selected-detail sweeps:
64, 256, 1,024 and 10,000 retained handles, one warmup plus five measurements.
Only three selected branches per sweep are mutated and verified. Ten thousand
handles are therefore not ten thousand useful first-write branches.

The native test completed in 97.2926 seconds, with RSS 1,448,132,608 bytes,
charged peak 1,502,613,504 bytes and zero pressure/OOM events. The initial host
rejected its mulligan bookkeeping: three mulligans bottom one card after the
second round and two after the third, so the total is three, not the final kept
hand's two. Pinned CR 103.5 and 103.5c and the actual journal confirm this.
Eighteen host guards now cover the correction. The independent corrected parser
verified all 24 retained schedules, but did so after the original measurement.
The original failed status is preserved; this is qualified posthoc evidence,
not a retroactive successful in-deadline host audit. Its audit SHA is
`b2ad95431eec92f3346737f93bc980e73873960ab38590ff00079f3530971215`.

The separate natural dense root did not reach its requested 256-object board.
The last recorded progress shows P0 main visit 36, 4,864 accepted-input counters,
143 battlefield objects and 11,009 history entries per viewer. This is a partial
observation, not the exact interruption boundary or a completed root proof.
The exact unit journal records a kernel OOM kill, a human-formatted 2G memory
peak and 5.172 seconds wall time; final cgroup counters and direct RSS were lost
when the unit disappeared. Cleanup passed. The retained journal SHA is
`670b5970e2829d1c828657159446874d9e413e8e1b2c3977e7f2d5d4a5763d6f`.
No branch sweep ran. A smaller synthetic board, reset history or larger memory
cap does not close this missing legal-root measurement.

The corrected host also ran the ordinary four-seat every-branch-first-write
baseline. The warmup completes 64, 256 and 1,024 useful mutations. At 10,000
retained handles, 6,137 branches complete a verified first write before the
native RSS reserve stops further mutation. All twenty measured schedules then
report zero created handles because the process remains above its reserve.
The native test emits all 24 explicit attempt records, but the outer resource
guard rejects the run: charged peak is 2,147,483,648 bytes with 63 max events,
zero OOM events and a separately measured direct-process RSS high-water of
2,134,224,896 bytes. Cleanup passes after 44.1267 seconds. Complete partial rows
are audited after failure; none are a successful bounded measurement or a
10,000-useful-branch result.

### Current persistent workers and full-pod attempts

The same-build persistent-worker diagnostic completes its declared 1, 2, 4 and
16 physical-worker rows, each with one warmup and five measured generations.
Each generation executes the same 32 sampled worlds and three candidates, with
full trial outcomes, utility vectors, counts and State identities checked
against the serial oracle. Median steady execution is 13,210,846, 6,813,704,
3,536,090 and 1,037,449 microseconds respectively. Four workers provide a
3.7360-times within-build diagnostic speedup. Startup and shutdown are reported
separately; the independently checked whole-process resource envelope still
includes the oracle and complete lifecycle. The independent raw-result review
passes (`independent-persistent-root0-run1-v4c-runtime-review.json`, SHA-256
`17e0d4e1ab3fbe837b89029497471beca6b9a4c0fc4902626592cb5d6d5f94ca`).
Different Build identities salt sampled contexts, so this table is not
a causal comparison with the older recreated-worker table.

The predeclared full-pod series requires one warmup and ten measured complete
eighteen-pod batches with the released seeds, policies, limits, detailed trace
and checkpoints. Its first warmup is killed before the first game completes.
The retained game journal contains a header and 1,394 accepted input records;
there is no complete Evidence row or rules-defined game outcome. The unit
journal proves an OOM kill and a human-formatted 2G peak in 2.441 seconds, but
final charged counters and direct RSS were lost with the unit. Cleanup passes.
No later batch starts, and no full-game throughput rate can be computed.
The last retained input does not prove the exact failing operation: visibility
retention, canonical memos and whole-State snapshot materialization remain
source-backed investigation leads, not measured causal attributions.

The independent CPU/failure audit is
`independent-current-profile-failures-v4c-runtime-review.json`, SHA-256
`2f374d1b85cc6c752507be4f75cae03561754a65481522b8af5bae65d7f68481`.
The immutable current measurement archive is
`performance-third-measurements-3118eff2/manifest.json`, SHA-256
`f00332750de3c9fadb2a68e4a62390c712326ff473e025388887af19395a0663`
(298 files, 98,582,592 bytes). It preserves every attempted run's raw output,
original status, authority, host source and available resource receipts.
It is diagnostic evidence, not a new full release or a revised product budget.


### Prepared sharing component verification

A separate 202-file candidate changes internal immutable admission ownership
to `Arc<Prepared>`; public owned admission APIs and complete State/snapshot
values remain covered by independent old-owned oracles. Build
`1f57d2e2fcedecde548abef7e5cd348b1c711269cc1d36a5a640df7051e2974d`
passes all 246 release core tests (five new component tests included), 17
distinct simulation tests, a separate counter arithmetic guard, and core
Clippy. These are correctness results, not timing or memory measurements. The
independent review preserves its narrow qualification that per-filter SIM
invocation mapping is parent-reported; compiled artifacts, executable bytes,
features and source closure are independently verified.

The external `performance-native202-prepared-1f57d2e2/manifest.json` has SHA-256
`8984dfbce62314b911295b589a1fd447f59663d1f63844d122dba75a09db7f74`
(243 files, 222,793,970 bytes). The measured 201-file baseline and this component
remain frozen while compact history and streaming snapshot integration proceeds
in another staging copy. Neither the source changes nor their tests close the
dense-root, full-pod or 10,000-useful-branch resource gaps without actual bounded
measurements.


### Compact history and streaming snapshot component verification

Build `cde7cdfa57922f14b4582cd3cf2b7561239838a67ee7a00cebf5b0ac1f1ed799`
retains every historical public value and canonical byte while sharing equal
immutable rows. Snapshot Write and seekable Read APIs preserve the complete
identity, State-hash, duplicate-field, replay and origin checks. All 262 release
core tests, four strict-reader contract tests and 19 public integration tests
pass, as do formatting and affected-target Clippy. The earlier 64 focused core
passes are included in the 262. Actual production, baseline-test and
allocation-test executables are separately pinned; compilation alone is not
simulator runtime verification.

The independent source review and all source/executable/check evidence are in
`performance-native209-memory-cde7cdfa/manifest.json`, SHA-256
`9019167f4984a97c71de52b8b317600b04fc4e98ac4038429718a70e2fe708d9`
(269 files, 226,733,688 bytes). No timing, memory-fit, complete-workspace or
public-schema result is inferred. Complete owned SearchContext exports,
full-String convenience APIs, typed restore/replay and proof-file page cache
remain actual resource costs. The measured 201 and prepared-only 202 baselines
remain frozen for subsequent controlled comparison.


The compact-history candidate subsequently passes 17 distinct simulation tests
and the separate counter arithmetic guard. Fresh same-executable known-vector
controls pass for both the prepared-only and compact-history counter variants;
these were correctness controls during concurrent functional jobs, not quiet
performance measurements. The complete permitted-history export component
separately passes 271 release core tests and 19 public integration tests with
an independent review. Its archive is
`performance-native213-shared-c9f5f594/manifest.json`, SHA-256
`e7ae515bdad0d8a094e4c88c15110b15b328252714602ec46dbaa7a9233c7470`.
Complete large-query reference comparison still materializes full Values in that
component; a separate proof-storage change is required and unmeasured.


The 201-file baseline passes the composite full debug/release checks (563 Rust
tests in each mode, 45 Node checks), plus 40 SDK checks and strict TypeScript
with identical public schema bytes. Its first fresh retention attempt stopped
before offline compilation because the staging copy lacked pinned policy HTML.
After both exact archived policy files matched their fixture hashes, the
retention-only retry passed its fresh frozen offline rebuild and complete replay,
retaining 10,716 dependency files. The successful raw log SHA-256 is
`73af24a603d0d81d2e3acd07c688abc020a5ff93bf01f6e3826ab85a7848645d`.
The original failed status and raw log remain unchanged. This retention result
belongs to Build3118eff2, not the later combined source.


### Definition diversity and complete query proof storage

The separate 205-file diagnostic component retains its corrected source-backed
combat probe, all eight normal correctness checks and 49 independently reviewed
host guards in `performance-native205-diversity-choice-fca4063e/manifest.json`,
SHA-256 `b54c85f158629477efb667f91c7ece7e3c4f8dac642e61d4c4e4fb09d2503762`.
The guarded definition workload subsequently completes all 48 rows: eight
configurations, one warmup and five measured repetitions, with complete native
and host semantic checks. No work or resource limit was reduced. The actual
resource report records 48,443,392 bytes peak charged memory and a separate
43,253,760-byte direct-process RSS high-water, no pressure/OOM events, and
successful cleanup. Concurrent functional jobs make this a diagnostic
correctness integration, not a quiet benchmark or causal comparison. The full
raw authority/report/output archive is
`performance-definition-instrumentation-fca4063e-v1/manifest.json`, SHA-256
`c2ab1c7b4ad95693adf4378624e33aaafea9c97ad4900921d15139e8501468df`.

The combined 218-file candidate adds complete per-prefix query proof spooling
and standalone file snapshot restore in test-host code. All 284 normal core
tests pass in the optimized test profile; a separately compiled release Build
`d472788f8c8cd5c15137b3f08b29e304b7495cfe363614cbda53cc304725c903`
passes 23 affected core and 21 simulator normal checks. Independent review
passes. All original query units, partitions, full prefix/SavedWork/State/trace
bytes and standalone replay checks remain. No performance/resource-fit result
is inferred from these correctness tests. The full 105-cell/630-repetition
shrinking-choice matrix and final-source broader measurements remain pending.


### Combined-source functional verification

The integrated 220-input source passes 622 Rust tests in each of debug and
release, 45 Node checks, and 40 SDK checks with strict TypeScript and unchanged
public schema bytes. Its debug Build is
`d45d878d4fa8bcddc1298f73f2e104f4ba41c6c592039eedfceff20ae17d8edc`;
its release Build is
`0398e42f215517220e4411d73e908c52c8132220196c082dd0f7c030b67e56c8`. Twelve explicit
diagnostics remain ignored in each normal Rust run. Format, Clippy, source guards
and owned-process cleanup pass. The terminal functional receipt SHA-256 is
`7c6468ec443e3d8fea1394cb052ee66b01c81a1933cc68108bacc74956401a64`.
Fresh offline retention also passes for this exact release Build: all 220 native
inputs, 10,716 vendored files from 152 packages, and the complete retained replay
were verified after rebuilding with a fresh Cargo home and target directory.
The independent audit checked all 11,461 retained files; its SHA-256 is
`ac76e36969e550a033a8e5c5b39e7527b67c501940f03486e9d4c919a1bd15b8`.
Functional retention uses its separate verification envelope and establishes no
product memory or timing result. The subsequent baseline measurements below
belong to this exact release Build.

### Combined-source measurement baseline

The baseline sixty-job quiet primary search run and all sixty independent semantic
replays pass. Its p95 is 2,628,907 microseconds, maximum 2,635,936 microseconds,
and kernel whole-process RSS high-water 358,547,456 bytes. It records 5,202 useful
trials and excludes 1,998 unverified diagnostic stops. The original limits and
per-candidate minimum remain unchanged. The independent review SHA-256 is
`c551f265fdf826e800b955bc59e370eb3a037c234647332c6b7319157bf0c110`.

The ordinary legal-input and artifact workloads, eight dense kernels, four dense
interaction cases, and all 48 definition-diversity rows pass their complete
declared work and semantic checks. Persistent fixed-work search also passes
with 1, 2, 4 and 16 physical workers, identical trial results and full State
proofs. Median steady-work times are respectively 10,628,021, 5,467,986,
2,810,374 and 799,019 microseconds. These are fixed-work diagnostics with
separate startup/shutdown costs, not product decision latency.

The separately labeled omniscient diagnostic passes all twelve roots, comparing
360 sampled trials with 360 trials repeating one actual world. Nine roots select
the same candidate. This establishes the recorded same-policy comparison, not
strategic quality, eight distinct actual worlds, or additional product trials.

The broader gate remains open. Ordinary-four selected-detail branching completes,
but the subsequent ordinary all-first-write and smaller-seat cases hit the
original 120-second deadline. Dense-four legal root setup is OOM-killed before
its ready-root proof; its last retained progress has 171 battlefield objects.
Shrinking-choice setup likewise OOMs before any of its 630 repetitions complete.
The remaining five dense legal cases were left unstarted after these failures.
No shortened denominator, incomplete-work throughput, or full legal-suite pass
is claimed. The full-pod series fails before native startup because its controller
omits the user-bus environment; zero pods ran. Its original failed cleanup report
is preserved alongside a separate exact-unit absence check.

Baseline CPU and GNU libc allocation profiles have independently verified search
semantics. The CPU collector warns that its profile data may be unreliable;
reported samples do not establish precise causal percentages or historical
preoptimization profiling. The libc profile and the separate Rust request counter
measure different scopes and worklists. Equality and retained-vector capacity
fixes require new compiled identities, complete correctness checks and unchanged
workload/resource-limit measurements before any benefit is claimed.

The source/functional archive remains separate from the measurement supplement
`performance-native220-measurements-pass1-v1/manifest.json` under
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-11fdee9d9b50bb8d-2Y0CLl`.
The supplement retains 902 files / 339,147,720 bytes, including passed
and failed attempts. Its manifest SHA-256 is
`e5b1498ab5883c9c2d4143e14cfdce25c4b96828a5d23996a5540ab50b4d9637`;
the independent archive audit SHA-256 is
`6663058ecd73eac168f7fac6b106df21119c9d4f92db634ee736e49cd6f2100b`.
These records do not turn failed workloads into passes.

### Capacity/equality Build a742: retained results

Release Build `a742650f4d4cfadbec67480901732880e29095a053ff5bd07f3e2b7975df06c3`
passes 630 Rust tests in each debug and release mode, 45 Node checks and 40 SDK
tests with strict TypeScript. Its independent functional audit is `394de68c…`;
fresh offline reconstruction and complete replay also pass (`0d2b624a…`).
These are new-source proofs, separate from the earlier 0398 results above.

All sixty quiet primary search jobs and sixty independent semantic replays pass:
p95 2,610,138µs, maximum 2,633,648µs, whole-process RSS upper bound 213,286,912
bytes, 5,218 useful trials and 1,982 unstarted trials excluded. The unchanged
per-candidate eight-world minimum and product budgets hold. Review
`candidate-primary-runs-a742-v1/run-1-independent-primary-review.json` has SHA-256
`c6d5e84de3f0c3743b74965067f6b3220427074bee0aa0b84ece8241e269c39c`.
The original wrapper's replay-pending status stays immutable; that separate
review establishes completed replay.

The broader program completes cases 0–16: ordinary 1,024-input work, all twelve
artifact roots, twelve dense kernel/interaction cases, and three legal-branch
cases with all twenty-four sweeps through 10,000 branches. The terminal independent audit verifies all seventeen complete cases and
preserves both failures and eleven unstarted cases; it is not a full thirty-case
pass. Its SHA-256 is
`e85d599e948fdbd2743d9e3277663c35f9f7251bf7f7d5561da7e0b3170986db`. Ordinary-three/all-first-write case 17 times out after nineteen
complete sweeps; no complete-work rate or final process RSS is available.
Dense-four/selected case 20 times out during real setup, before any sweep or root
proof: 8,704 accepted inputs and 19,705 histories per viewer were last observed.
Its charged peak reaches 2 GiB with memory.max events but no OOM; native RSS is
unavailable. The other eleven cases and the separate omniscient run remain
unstarted. All original limits, histories and partial denominators remain.

The separate instrumented Rust phase verifies the same full 1,024-input semantics
and zero empty controls. It records 7,171,042 allocations, 100,869 reallocations
and 6,973,743 deallocations; requested bytes are 1,068,999,650 allocated and
1,041,127,923 deallocated, with signed realloc-byte change 169,570,950. This is
process-global intercepted request accounting over the declared raw loop, not
C allocation coverage, live heap or production latency. Independent subset audit
`independent-candidate-allocation-a742-v2/audit.json` has SHA-256
`fde4ad993838cbecb0636724c229fc46db621557daa75386169c5c60e00fcbaf`.

A separate original-work legal17 hardware profile reaches eighteen of twenty-four
complete sweeps before its 120-second deadline. The independent review maps the
repeated 65,536-byte zero-fill call in the complete-byte comparator, motivating
a scratch-lifetime experiment; sampled hardware events are not exact CPU-time
percentages or a promised speedup. Profile audit SHA-256 is
`a80e1469fb1a2b01a7d90b7cedfcd1bf2d650ac9e9867197260f7bc5ad01a5ce`.
Neither profile nor memory-pressure evidence shortens the workload or supplies a
resource-fit pass. A separately identified successor must earn its own source,
functional, retention and measurement proofs.

Three private archives preserve these exact a742 results under the external
evidence root named above. Retention status verifies bytes and classifications,
including failures; it does not convert the program into a pass.

| Archive directory | Payload files / bytes | Manifest SHA-256 |
| --- | --- | --- |
| `performance-candidate-source-functional-a742650f4d4cfadb-v1` | 1,033 / 1,129,452,452 | `8dfed18be105f92f252326d3c00c11529402d5ddac0b8e77269c18d3df8253d2` |
| `performance-candidate-measurements-a742-pass1-v1` | 600 / 575,089,457 | `d8301adacdac9752f506fa24866b44dc21f15a9580fdedeaba903d1abb32c720` |
| `performance-candidate-profile-a742-v1` | 167 / 263,537,591 | `f9318c0d91135469ff79d768f5756644207da7eec7551a6ed672ef03489d2358` |

The profile payload count includes its copy plan/producer; manifest and execution
metadata are additional. Its raw experiment, partial stdout, all forensic tool
records and source/tool authorities remain intact, linked to the old source
archive and executable. Independent archive audits are `51b2be51…`, `090321b2…`
and `3bc37cda…`, respectively.

### Shared collections and comparator scratch: Build faf9

Source commit `92a7863` has release Build
`faf9ca241b7e1bf9469a77c9d88080ef18501bc851dd107811a758c24afa7318`.
Its separately identified run passed 637 Rust debug tests, 45 Node checks,
637 Rust release tests and 40 SDK tests with strict TypeScript. Independent
functional audit `a56edc5c…` and compiled identity `6f857743…` bind this source
and its actual executable roles. Local proof locators below are relative to
`generated/final-engine-verification`.

This change shares complete immutable object-slot, player and commander arrays
with the immediately preceding history view for the same viewer, after full
ordered typed equality. Public JSON retains every value and its original array
order; public exports remain owned values, with COW isolation before mutation.
The test-host complete-byte comparator now owns one 64 KiB scratch buffer per
comparison and reuses it across serializer fragments. Seven new normal tests
cover these mechanisms, full wire/State equality, sibling isolation and
adversarial comparator I/O. They establish correctness, not a promised speedup
or dense-root memory fit.

Fresh offline reconstruction and full replay also passed, independently audited
in `shared-collections-scratch-independent-retention-v1/audit.json`
(`82f6d9be…`). The private source/functional archive
`performance-shared-collections-scratch-source-functional-faf9ca241b7e1bf9-v1`
preserves 893 files / 945,588,802 bytes under the external evidence root above.
Its manifest SHA-256 is
`860a6a4092765a0a44ee1b049b121e9e1ffb907ea0658d57c9120a8a3eb6093f`;
independent archive audit `d4e77633…` verifies the complete byte/mode/namespace
closure. These source and replay proofs do not establish performance acceptance.

The current sixty-job primary search measurement and all sixty independent
semantic replays now pass the unchanged product budgets: p95 2,619,498µs,
maximum 2,630,390µs and whole-process RSS upper bound 174,325,760 bytes on
four distinct physical cores under the reviewed bounded-idle baseline.
There are 5,501 useful trials (5,461 horizon evaluations and 40 terminal rollouts);
1,699 unstarted trials are excluded. Every candidate meets the first-eight
distinct-world minimum. The replay checks recorded semantics, not new timings.
Independent review
`current-search-primary-shared-collections-scratch-v1/run-1-independent-primary-review.json`
has SHA-256
`68f460b8458c02e6f3c52f57bd1a55b21fb12dd5f875937d1d3c2004cf00529c`.
It binds wrapper `f37a27f2…`, source/authority endpoints and owned-process cleanup;
the original wrapper's `measurement_passed_replay_pending` status stays unchanged.

Current ordinary case 0 completes one warmup and five measured repetitions of
the unchanged 1,024-input legal prefix. All six retain full journal, payment,
event/trace and exact State/replay proofs. The five measured core segments report
2,771–2,795 accepted inputs/second; these clocks cover check/submit/advance and
exclude observation, policy, admission, replay, serialization and I/O. Whole-host
elapsed time is 33.4742 seconds, target process peak RSS is 112,848,896 bytes,
and cgroup charged peak is 508,719,104 bytes. These are separate memory metrics;
the prefix is not a completed game or full-pod throughput measurement.

The raw result is `measurement-shared-collections-scratch-ordinary-legal-run-1/result.json`
(`c9c9a2de…`). Independent audit
`shared-collections-scratch-independent-row0-v1/audit.json`, SHA-256
`2d782f416367ba55bc3025c91c631e9f3bec622c5a6eee1b76562ddb4af1ec5a`,
verifies this complete declared case with source/resource/cleanup joins. Its
status is explicitly a verified subset, not a full thirty-case program pass.

The separate instrumented Rust phase also passes all 1,024 accepted inputs with
full semantic equality to the current feature-OFF reference and two zero empty
controls. It records 6,944,647 allocations, 6,913,186 deallocations and 100,869
reallocations; requested bytes are 1,049,289,836 allocated and 1,044,587,925
deallocated, with signed realloc-byte change 169,570,950. These are process-global
Rust allocator requests during the declared raw loop, including possible
libtest-main interference; there is no control subtraction, thread-local proof,
C-allocation coverage, live-heap census or production-latency claim. The raw
result is `measurement-shared-collections-scratch-allocation-phase-run-1/result.json`
(`65269c1c…`), independently verified by
`shared-collections-scratch-independent-allocation-v1/audit.json`, SHA-256
`a9377d549f009a2b8ff8f34d37423b0d411306cdb96a879a172d0e03058f999e`.

The broader diagnostic program completes cases 0–17. Case 18, ordinary-two
selected-detail branching, reaches 22 of 24 full sweeps before the unchanged
120-second deadline; independent review `independent-ordinary2-partial-v1/audit.json`
(`f3d1a818…`) preserves that failure without inferring missing work or a full
rate. Cases 19–29, the separate twelve-root omniscient comparison and final
measurement evidence retention remain pending completion and review. This is
not a full thirty-case program pass.
The earlier a742 measurements keep their original Build, partial-work and profile
qualifications; no prior pass or predicted resource-fit/speedup transfers here.
