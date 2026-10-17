# Foundations search capture and sampled-world contract

Status: **P7 verified for the finite foundations workload: primary targets, all
sixty independent replays, debug/release checks, fresh offline rebuild/replay and
retained-bundle audit passed**.
Implementation is in the engine/core and native simulator; development evidence
remains under `generated/search-staging`. The prerequisite P5 profile is released in `d9afc77`.
No new card or Magic mechanic is admitted by this slice. Search correctness does
not establish the measured P7 performance gate.

## Frozen identities and inputs

The gameplay profile is `commander-foundations-v1`, with the exact P5 IR11 catalog
of 34 definitions and its legal white/black-green construction restrictions.
Source fixtures, 49 named/offensive ban records, official policy bytes and the
September 25, 2026 rules remain the P5 inputs. Open decklists are an explicit session
information policy, not a Commander requirement.

| Identity | Frozen value |
| --- | --- |
| Sampling family | `foundations-priority-sampling-v1` |
| Reconstruction schema | `foundations-fresh-precombat-v1` |
| Information policy | `foundations-open-decklists-v1` |
| First sampler | `foundations-uniform-v2` |
| Workload | `commander-open-search-v1` |
| IR / catalog | 11 / unchanged P5 definitions |
| Real snapshot / simulator bundle | 18 / 19 |
| Observation and search context / query sidecar | 16 / 15 |
| Capture / world proposal / sample snapshot | 1 / 1 / 1 |

Rules bytes SHA256 remains
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
Relevant rules are [CR103.5/.5c/.8c,117.3–.4,302.6,400.2/.7,401.2–.3,
402.3,701.24a,704.3 and733.1–.2](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.pdf).
The [search specification](game-engine-spec.md#81-initial-information-model-and-sampled-worlds)
and [P7 workload](game-engine-plan.md#p7--search-api-and-measured-optimization)
remain authoritative. P5 retained snapshots17 and prior source/build evidence are
separate historical identities; they are not rewritten as P7 evidence.

## Supported root family

A supported root is a stable ordinary Priority boundary for a living active viewer
in PrecombatMain after setup. The active viewer holds priority and the consecutive
pass round is empty. There is no accepted pending input, concession, stack entry,
pending trigger, effect, damage transaction, combat declaration/choice, announcement,
resolution or mandatory private continuation. Creature marked damage is zero for
this first reconstruction schema. The interpreter has completed ordinary SBAs
and trigger placement before capture.

Support 4, 3 and 2 living seats with original seating and departed-player facts.
No-active-player turns, nonacting viewers, nonfresh pass rounds, other steps,
setup, paid stacks, effects and unresolved continuations reject explicitly.
`priority == active` alone does not prove an empty pass round. Unsupported roots
are not replaced with a different root or an authoritative full-state fork.

The family includes arbitrary-turn public battlefields, completed casts, own paid
mulligans, known bottoms, current public mana and physical Commander histories.
It is not limited to a no-history opening. Every future-relevant State field must
come from permitted facts, complete hypotheses, or a documented invariant of this
exact schema. Missing history semantics return UnsupportedHistory.

## Permitted capture and information constraints

`Game::capture_search(viewer,Budget)` returns Complete(SearchCapture),
Incomplete(CaptureWork,required_minimum), or Rejected(SearchRejection).
`resume_search_capture` binds opaque work to its actual executor and state.
SearchRejection distinguishes UnsupportedRoot, UnsupportedHistory, InvalidProposal,
StaleWork and Invariant failures.

SearchCapture is sealed, immutable, cloneable authority with a public DTO and
context_id. It contains no State, actual Game/fork, real snapshot, real journal,
private physical/incarnation map, true RNG or secret Prepared.profile_digest.
The latter binds the actual MatchConfig seed and cannot serve as a public identity.
The private source authority does not enter deterministic sampler inputs or hashes.

PublicSearchProfile contains engine/catalog/rules/legality/information/reconstruction
identities, normalized open decklists, original starter and designated commanders.
CapturedSearchContext preserves the exact current Observation and complete permitted
view history. SearchCaptureData also includes the applied owner-only knowledge
ledger, remaining known bottom and public remembered-hand constraints. Its public identity is independent of the true hidden world and source executor
token. Context identity version1 hashes the canonical tuple
`("capture-identity-v1",capture_version,profile,observation,own_knowledge,known_bottom,known_hands,history_digest)`,
where history_digest is SHA256 of the complete ordered canonical permitted-history
array, streamed one view at a time. No history entry is omitted or summarized.

The owner-local applied ledger records OpeningRedraw, OpeningBottom with ordered
RememberedCard(memory_ref,oracle_id), and Drawn(oracle_id). Memory references are
not ObjectHandles. Only actually applied setup/draw actions are recorded; abandoned
accepted inputs do not become knowledge of an action that never occurred.

A later own mulligan shuffle invalidates earlier position constraints but does
not erase remembered observations. The final kept bottom is an ordered suffix.
BottomOpening appends cards in response order: the first listed card is the first
card of that suffix reached by future top draws. At current library length L,
the remaining known suffix is the last min(original_bottom_length,L) cards.
Observed own draws crossing that boundary must match the consumed prefix; a
counts-only proposal may not silently move those known cards earlier.

A failed hand-origin spell announcement is another knowledge source. Its face
was public on the stack before CR 733 returned it to a hidden hand. Reversal does
not erase that observation. In this exact P5 pool, nonbasic singleton cards,
absence of postsetup shuffle/bounce/library moves/recursion, and public deaths
make the following inference sound: a living opponent's historically public
nonbasic card absent from current public zones must be remembered in that owner's
hand. Key this constraint by(owner,Oracle); another owner's same-named card is
not a substitute. Current public membership or owner departure removes that hand
constraint. No current hidden-hand permission is minted. Ambiguous history rejects;
future profile expansions cannot inherit this inference automatically.

Public current life/status/mana, turn/land-use, taps/readiness, ownership/control,
commander casts and combat damage remain exact. Continuous-control readiness is
not a generic default: a newborn untapped creature remains unable to attack/pay
a tap cost. Commander ledgers bind physical designations, not just Oracle names.
Departed seats are never reseated or given new hidden objects.

## Complete hypotheses and executable sample origin

WorldProposal version 1 binds context_id, exact sampler version, world_index,
four SeatHypothesis values and an explicit independent 32-byte future_seed.
Each hypothetical card has a unique local symbol and authenticated Oracle ID.
Hands are unordered; library arrays are ordered. The viewer's hidden_hand is
empty because its exact current hand comes from the capture. Both hidden zones
are empty for departed seats. All remaining live hidden slots are supplied;
there is no authoritative fallback for omitted values.

`SampledGame::build(catalog,&capture,proposal,Budget)` returns a sealed sampled game
plus RootHandleMap, incomplete opaque SampleBuildWork, or explicit rejection.
Build work resumes with its retained catalog/capture/proposal. The builder has
no actual Game, State, Prepared, snapshot, real RNG or callback for missing data.
It validates identities, legal deck multiplicities, complete local-symbol coverage,
own/public objects, zone counts, remembered opponent hands, own draw/bottom history
and canonical physical Commander binding. Proposal fields cannot override public
root facts, change the catalog or supply their own executable handle map.

Construction uses the canonical 400-card public deck table with a fresh local
namespace, reserving designated commanders and mapping visible rows injectively.
No real hidden IDs are copied. Chooser-facing handle/token namespaces contain no
proposal, hidden-assignment or future-RNG digest: worlds with identical permitted
facts must yield identical complete observations before those worlds diverge
visibly. Cross-executor receipt/work isolation uses authority, not a public hidden
world fingerprint. Root current fields are reconstructed from public
facts; excluded captures are empty by certification. Old event/stack/revision
counters start in the independent sample namespace. Stable-root failed-draw flags,
consumed Commander-return opportunities and departed inert fields require explicit
future-equivalence arguments, not an unreviewed State::default.

The original permitted history is retained as knowledge, not mislabeled as a
hypothetical accepted-input journal. Other players receive sampled current views
and later sampled observations; their real private histories are never copied.
First-version rollout policies are stateless current-view policies. Nested
history-aware sampling during a rollout is outside this schema.

## Mapping, execution and recovery

RootHandleMap derives a bijection from captured CURRENT object handles to sampled
handles and a separate root DecisionToken mapping. Historical memory references
cannot map to current permissions. Repeated basic objects still require an
injective current map even though hidden-copy permutations are strategically
equivalent. `mapped_observation` inversely renames the sampled root Observation
and must reproduce the capture exactly, including every repeated reference.

RootActionPlans are generated from captured visible information before inspecting
sampled opponents' secrets. A plan includes ordinary staged modes, targets, mana
activation and exact spending; Cast alone is not a completed action. Every stage
uses the sample's ordinary player-facing validation. Future draws and choices use
the sampled chooser's own observation. An infeasible candidate is not replaced
with a different action or silently removed from the trial denominator.

SampledGame wraps the same rules interpreter and exposes budgeted observe/decision/
check/resume, scoped validated submission, advance, fork, concession, digest,
rule-boundary and snapshot operations. Query-work save/restore preserves the usual
executor/state/proved-prefix checks. A receipt or work from the real game, another
sample, another fork or a previous revision does not authorize this sample.

Sample snapshots have a strict separate version 1 origin envelope binding capture,
complete proposal, derived origin identity, claimed final state hash and subsequent
hypothetical journal.
`SampledGame::restore(catalog,&capture,json,Budget)` revalidates the origin,
rebuilds it independently, replays the future inputs at recorded boundaries and
checks the complete state hash. The sample interpreter uses fixed Off tracing;
no unproved diagnostic trace is imported. Its opaque restore work resumes budgetedly.
The envelope cannot import a forged capture as authority. Ordinary Game::restore
continues rejecting injected sample origins. A sample token cannot authorize a
real action and a real token cannot authorize a sampled action.

## Sampler, roots and outcome accounting

The first sampler assumes uniform remaining definition assignments subject to the
capture constraints. It does not certify an opponent keep-policy posterior.
Assignment digest ignores local symbols, hand order, world_index and future_seed;
libraries retain Oracle-definition order. Merely relabeling two hidden Plains or
changing an unused RNG seed does not create another useful world.

Derive sampling, future game and rollout-agent streams from a separate public job
seed and versioned domains. Real match seed/cursor and worker scheduling are
forbidden inputs. Report the actual number of useful distinct assignments and any
shortfall below 8; duplicate samples do not fill the minimum. Exhausting a finite
retry budget establishes a sampling shortfall, not the cardinality of compatible
worlds. Do not claim that fewer than 8 worlds exist without a separate proof.

The primary root suite has indices 0..11 with seed SHA256 of the ASCII string
`commander-open-search-v1/root/` followed by the decimal index. Each uses the
default W/BG/W/BG pod, starting/viewer seat index%4, mulligans(floor(index/4)+seat)%4 and
reversed own BottomOpening candidate order. Only observation-based land plays
and passes develop roots. Cleanup discard and bottom responses reverse the offered
candidate order and take the required count; attackers/blockers are empty, owner
Commander returns accept, and every unlisted request fails that named root. The
workload manifest fixes these choices before running. No actual hidden state selects a root.

| Indices | Live seats and capture |
| --- | --- |
|0..3 |Four live, second own PrecombatMain before land |
|4..7 |Three live after recorded departure of(start+2)%4, sixth own PrecombatMain after available land attempt |
|8..11 |Two live after recorded departures of(start+1)%4 and(start+3)%4, second own PrecombatMain before land |

Use up to 32 visible-information candidates and 32 distinct worlds; every generated
candidate receives its first 8 useful distinct worlds before extras. Fewer legal
candidates do not justify invented actions. Trial horizon is 256 accepted inputs, including nested choices, evaluated at
the next exposed decision or an earlier rules terminal result. Rule steps are
counted separately. The frozen fixture retains the key
`rollout_decision_boundary_horizon`; its value bounds accepted inputs. Return
per-player result vectors.
Terminal games, valid horizon evaluations, infeasible candidates, unsupported
roots/samples, faults, cancellation, resource stops and missed minimums are distinct.

Frozen performance requirements remain 5s hard end-to-end/root, 3s p95, 2GiB total
process RSS including four workers on distinct physical cores, and 8 useful worlds
per generated candidate. All capture/sampling/validation/mapping/staged choice/
rollout/aggregation/disposal costs count. Measure the isolated release workload;
concurrent P5 gates and a cheap fork microbenchmark cannot close this gate.

## Required independent correctness evidence

- Paired real roots with different true hidden orders/seeds but identical complete
  permitted context produce identical capture payloads and seeded world proposals.
  Positive roots include real own choices and later public turns.
- Opposite paid bottom orders change knowledge while unchanged old views stay
  equal; a later shuffle removes old positional constraints; suffix-edge draws
  consume the right known prefix. Focused private ledger tests supplement public
  integration where a full100-card late draw would be disproportionately large.
- Reversed announced-card hand membership is enforced, or its history rejects
  explicitly. Another owner's same Oracle cannot satisfy the memory.
- Missing/duplicate/wrong cards, invalid symbols/positions/identities, wrong own
  hand, invisible current-handle mappings and rehashed origin forgeries reject.
- Root projection matches exactly under the trusted mapping; future draw comes
  from the proposed library; each chooser sees only its own sampled private hand.
- Real restore rejects sample injection; sample restore proves full future replay;
  forks, budget partitions, query-work recovery and receipts remain isolated.
- 4/3/2-seat histories, real public mana/readiness/Commander continuity and every
  UnsupportedRoot condition receive explicit tests. No unsupported case falls
  back to true hidden state or disappears from performance accounting.


## Development measurements and entropy revision

The first concurrent development diagnostic used sampler v1, bound sampling
entropy to the exact executable identity, and missed both the minimum and hard
latency targets: 5,099,557µs,158,900,224B peak RSS and8/7/4 useful worlds for
three candidates. Exact replay verified19 completed horizon trials; three deadline
truncations and two not-started trials were not counted as useful. Its162-file
source closure, executable and report are retained in ignored reference evidence.
This is not a quiescent acceptance result.

Before optimization comparison, workload manifest revision2 changes only the
sampler entropy identity: it binds every permitted capture field and the complete
ordered-history digest while excluding executable BuildID. Proposal permissions
and restore remain bound to the complete current capture ID. Otherwise every
native optimization would change the sampled worlds and prevent an identical-work
comparison. The original sampler/measurement is preserved. Roots, public job seed,
budgets, candidate policy,256-input horizon and four workers are unchanged.

The unoptimized sampler-v2 diagnostic also missed:5,013,020µs,156,381,184B and
8/8/4 useful worlds;20 completed horizons replayed exactly. Summed worker time
(overlapping, not elapsed wall time) was10.58s advance,3.50s queries,3.08s apply,
2.56s exact full-State hashing and0.16s disposal. Sampling is not the measured
bottleneck. Optimization must retain an exact semantic and query-budget reference.


The first measured optimization caches only derived query work and traverses
object keys by range. An exact copy of the original query algorithm remains a
unit-by-unit reference, including SavedWork bytes and budget1/64/MAX recovery.
Its root0 development diagnostic completed the8×3 minimum in4,443,807µs at
156,295,168B; all20 completed trials shared with the unoptimized v2 run retained
identical full semantic State SHA, counts and candidate identity. Query worker time
fell from3.50s to0.19s; summed worker times overlap and are not wall-clock sums.

The next optimization shares individual immutable History entries, including the
partial tail. It preserves the ordinary ordered JSON array and copy-on-write
mutation. Clone-counter tests prove zero deep entry clones on forks and exactly
one selected shared entry clone on indexed mutation; independent Vec/canonical/
SHA oracles cover every length0..132, chunk edges, flat caches, append/clear and
restore. Thirty-nine focused tests, Clippy and formatting pass. The root0
concurrent diagnostic completed in2,689,014µs at173,690,880B, with30 useful horizon
trials (12/10/8 worlds), and all24 first-eight trial results/state hashes/counts
matched the preceding query-cache build. Replay verified all30 completed trials;
66 not-started extras remained non-useful diagnostics. This single development
root does not close the twelve-root, sixty-job quiescent performance gate.

The next derived-only digest optimization caches canonical visible-object bytes
between adjacent views only when the complete typed object is equal. The cache
contains only one view's object membership and replaces it at each view; every
history row and other view field is emitted in its original order. A64KiB
buffer coalesces SHA writes and explicitly flushes before finalization. Eight
state-hash oracle tests cover object/characteristic changes, duplicate handles,
removal/reinsertion, reordered objects, other view fields, Unicode, chunk edges
and writer failures. Forty-three focused tests plus Clippy/formatting pass.
A separate schedule test runs the same24 first-eight trials with one worker in
ascending order and four workers in a reordered queue; fullStateSHA, counts,
evaluations, aggregates and selection match exactly.

The resulting concurrent twelve-root development diagnostic meets the minimum
useful-trial count and5,000ms maximum at all roots. End-to-end times range from
2,569,089µs to3,062,663µs; peak process memory is264,798,208B. Root7 is the sole
root above3,000ms. These are development measurements while other work runs,
not the sixty-job quiescent acceptance benchmark. Replay verifies all617
completed trials; all437 completed trials shared with the prior history-sharing
build retain identical fullStateSHA, counts, candidate identity and results.
Two prior optional trials were not started before the new run's soft admission
cutoff; they are reported as coverage differences, not compared as completed
results. First-eight coverage was complete at all roots. At that checkpoint, the
primary performance and complete offline-retention gates remained open.

## Primary sixty-job measurement

The first quiet primary measurement completed all twelve frozen roots five times
on the reference Ryzen 9 9950X. It used the unchanged 166-file release source
closure, Build `255fbcd727ef3b22a613a23c717b2c363402bdb0c64663cb817f9d2cc829cb17`,
and executable SHA256
`1d1e7e56d972b9e29d57f0836bb031459356d3ef001585e6041be37580e08732`.
All sixty attempts remain in the denominator. Every generated candidate met its
eight-useful-world minimum; all workers were pinned to four distinct physical
cores, and no engine, policy or job fault occurred.

| Measurement | Observed | Requirement |
| --- | --- | --- |
| p95, nearest rank 57 of 60 | 2,719,864 microseconds | At most 3,000,000 |
| Slowest complete decision | 2,990,379 microseconds | At most 5,000,000 |
| Fastest complete decision | 2,517,747 microseconds | Diagnostic |
| Entire native process high-water RSS | 311,537,664 bytes | At most 2 GiB |
| Whole harness elapsed | 167.977 seconds | Diagnostic |

The external wrapper collected post-exit `wait4` resource usage and sampled
VmRSS/VmHWM throughout the entire sixty-job process. Acceptance requires the
maximum of these and every internal job peak to fit 2 GiB. This stronger upper
bound includes final capture/report disposal and artifact encoding after the
internal memory sample. No competing build/test/native process was detected;
source and executable hashes were rechecked after exit. Timing measures the
native search lifecycle; authoritative fixture development and trusted evidence
encoding are outside the consumer decision stopwatch.

The frozen workload retains its original `sampler` label alongside explicit
`sampler_version: foundations-uniform-v2` and a revision reason. The explicit
version, compiled sampler and supplemental wrapper manifest identify the actual
v2 measurement. The historical field does not select an algorithm.

The raw measurement report SHA256 is
`2c347162a553b8797204db2e08ccd6f367bf614524da154c6aac5706d9fb56dd`;
the complete external wrapper report is
`2ff823acc3e43ed8cce9f1109f3e75b9a49b7b2346568462c4b67a861503c61d`.
Reports are under `generated/search-verification/primary-60-run-1` within the
isolated search candidate. Independent replay passed all sixty reports: 3,859
useful trials (3,834 horizon evaluations and 25 terminal results) reproduced exact
counts, outcomes and full State hashes. All 3,341 NotStarted diagnostics remained
non-useful and were not represented as verified rollouts. Across repetitions,
all twelve first-eight sets/selection results and 3,022 overlapping useful trial
comparisons matched exactly. Independent report SHA256:
`ddd656560e2dae3b024859fdec0879ce1d74bb8ed1417c9846566882997545cc`.
Formatting, warnings-denied Clippy, 417 Rust debug tests and 31 Node checks passed;
the separate release suite also passed all 417 Rust tests. Fresh retained-source
offline rebuild/replay and the independent retained-bundle audit passed.
These measurements apply to the exact original build above. A later consumer or
engine build requires its own evidence and cannot inherit the numbers by profile
name alone.


The final gates used the unchanged 166 native files / 52,121,033 bytes and nine
host inputs. Development checks took 1,756.302 seconds, the separate release
suite 912.376 seconds, and fresh offline retention 1,404.952 seconds. These are
verification durations, separate from the decision latency above.

Retained release identities:

- Store: `/home/josh/.local/share/mtg-ecosystem/engine-evidence/p2-a70d95584dbb10cc-hgUTKX`.
- Native bundle: `a70d95584dbb10cc8e13eaf1e6caca80e3862757096edb79be2109903bd27bb0` (2,087,128,814 bytes).
- Offline executable: `a9894e427db72e1b4c319d5c845b9cf21f3c171434d3a90580a2684f928ba572`.
- Dependency manifest: `9b3e439c94ceaf07c828ebf37a4bcd513a345248345c0f84ad0893a8536e7989` (43 external packages / 5,596 files).
- Independent bundle audit: `095837621170a5b1996b9ccca28c425418a28799b2de767618e03c78ac4f6a77`.

The audit independently checked all eleven catalogs, 41 source records, 42
historical configurations, 540 trace executions, 727 snapshot/hash checks,
17,627 recovery boundaries and all nineteen complete legal-match journals.
The historical initial-scenario State hash remains
`58a0a241c4662d403dec8944430c1fe402c7fd15d22a2ce242c9a0b01d54f657`.
The parser compatibility capture remains the separately identified historical
199-file parser capture; no current-main parser result is inferred from it.

The offline executable and measured executable have different file hashes while
binding the exact same release Build and native source closure. Performance
claims remain attached to the measured executable above. The original process
inventory matched exact tool names; the later current-build wrapper additionally
recognizes Node26's `node-MainThread` name. Original raw measurements remain
unchanged.


The exact measured executable, all sixty raw reports, every independent replay
output, the reviewed drivers and host inputs are retained outside the worktree
under the store's `search-acceptance/` directory. Its 211-file / 25,056,417-byte
manifest SHA256 is
`baea4998acdcc7d3f034cbe8a4b5f67562a6d7f8b3a9c2f9cf733c1cfeaa034c`.
All archived bytes were verified against the reviewed evidence before and after
copying; the parent store retains the native source/vendor/offline replay closure.


## Current hosting-build search verification

The P8 hosting candidate changes shared history/hash paths and therefore has a
separate search measurement. Build
`7e77708cad1f80dfa7ad3be82d198f3ad6eae7f62b9c127879a335206bbc8da5`
uses 192 native files / 52,558,543 bytes; measured executable SHA256 is
`6d347fbeff9ceeea58883066c348b5cf2dfe5d94368c05d83fef696a076a2069`.
The unchanged twelve-root workload ran five repetitions on four distinct
physical cores. All sixty jobs met the useful-trial minimum and resource gates.

| Measurement | Observed | Requirement |
| --- | --- | --- |
| p95, nearest rank 57 of 60 | 2,662,119 microseconds | At most 3,000,000 |
| Slowest complete decision | 2,690,433 microseconds | At most 5,000,000 |
| Whole-process kernel high-water upper bound | 394,698,752 bytes | At most 2 GiB |
| Whole measurement harness elapsed | 166.806886 seconds | Diagnostic |

The separate whole-harness batch diagnostic is 0.359697 roots/second and
25.106877 useful trials/second (60 roots and 4,188 useful trials over
166.806886 seconds). The process consumed 622.992141 user CPU seconds and
1.644808 system CPU seconds. These are horizon-limited search work units, not
complete pods or one/two/four-worker speedup evidence.

The wrapper recognizes Node26's `node-MainThread` process name. Three existing
Node processes were admitted only after at least 600 seconds of age, a zero-CPU
two-second preflight and continued identity/status/CPU monitoring. All observed
self and waited-child CPU counters stayed unchanged through the measurement;
166 monitoring samples plus final endpoints were retained. This is an explicit
measured idle-process policy, not an arbitrary process allowlist.

Independent semantic replay verified all sixty reports: **4,188 useful trials**
(4,163 horizon evaluations and 25 terminal outcomes). All 3,012 NotStarted
entries remain excluded diagnostics. All twelve first-eight work sets and
selection results matched across repetitions; 3,348 overlapping optional useful
trials also matched exactly. Replay does not remeasure acceptance latency.

Current wrapper report SHA256:
`b4f6a510db4e0d988b3c912c366f3b7a6b0dbb90a9c4203363bfb5c4e252ee2c`.
Independent all-sixty review SHA256:
`4650f87e12bf94f2ec35c2427c7d5aa4db463b91320253a049c05371af1527e4`.
The raw reports are under
`generated/server-staging/generated/server-verification/current-search-primary/`.
Their final external archive is the server release store’s `server-proof-archive/`
directory, whose manifest SHA256 is
`a11f203b1cff1334c57e13655a7f93549c138e77dd00c69596238085d600952a`.
The original P7 reports above retain their original measurements and scope.

Retrospective CPU/allocation profiling is separate from quiet acceptance. The
same recorded root report passed ordinary, CPU-sampled and allocator-instrumented
replay with identical full semantic results: 68 useful trials verified and 28
diagnostic stops excluded. These runs profile reconstruction and replay, not the
live sampler/scheduler. GNU memusage counts intercepted malloc/calloc/realloc/free
calls; its heap peak is not kernel RSS or a count of every possible allocation.
GNU gprofng provides statistical program-counter samples with limited inline and
library attribution. Instrumented durations cannot replace the quiet numbers.
Separate actual `search-diagnostic 0` CPU and allocator runs also finished,
including capture, hypotheses, root mapping, candidate generation, worker jobs
and aggregation. Each emitted worklist was independently replayed: the CPU job
verified 68 horizon trials and excluded 28 NotStarted entries; the allocator job
verified 20 horizon trials and excluded four resource-truncated entries. The
allocator job hit the unchanged five-second deadline and did not meet the
minimum useful-work gate. Instrumentation changes completed work; these reports
are diagnostics and cannot be compared as identical work or used for acceptance.

The live allocator report counted 67,396,551 malloc, 18 calloc, 1,112,649 realloc
and 67,396,583 free calls, with no failed intercepted calls. Its intercepted heap
peak was 200,757,364 bytes, separate from kernel RSS. The CPU experiment sampled
10.24 CPU seconds across all four worker threads and the main thread; waited
owned collector/target CPU accounting totaled 10.588678 seconds. Unresolved
library frames and statistical attribution remain. The collector explicitly
warned: “Collection interval timer period was changed (10000 -> 0); profile data
may be unreliable.” Preserve that limitation; these samples are exploratory
hotspot evidence, not complete CPU attribution. Independent raw/source/cleanup
review SHA256 is
`cb9fe2b8046e6f8303d403c7f2e403eeb30b0fe252f6c6d3aaeccf78bb7b093f`.
Actual reports and all raw experiments are in
`retrospective-live-search-profile-run-1` under the server verification directory.

No historical preoptimization CPU/allocation profile has been recovered; these
current-build measurements cannot establish that historical process requirement
or a causal before/after ratio. Ordinary/dense performance coverage still needs
its own explicit completion audit; the quiet search suite alone does not prove
all synthetic scaling dimensions in the broader performance plan.


The final hosting retention subsequently corrected two host-only files for
npm 12 cache preparation and repository-relative SDK test-fixture layout.
All 192 native sources, all 23 SDK sources, the measured executable and its
Build remained unchanged. The current benchmark's original host endpoints
remain recorded as measured; the final release records the exact two-file
host delta separately. These retention-tool corrections cannot be presented
as engine performance improvements.
