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
The exact archived files now match both fixture hashes; a retention-only retry
is running. The original failed status and raw log remain unchanged.
