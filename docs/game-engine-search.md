# Foundations search capture and sampled-world contract

Historical P7 acceptance covers only its exact finite foundations source/build.
See [evidence](game-engine-evidence.md#search-hosting-and-performance) for measured
identities, retained proofs and later failed diagnostics. The contract below
continues to bound search; historical version numbers are not active schema selectors.
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
The supported root and frozen workload below remain authoritative. P5 retained snapshots17 and prior source/build evidence are
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



## Measurement and consumer qualification

Full historical search measurement/source tables are linked from [evidence](game-engine-evidence.md).
New engine/compiler support does not admit new search capabilities. Current
requirements are checked through profile capability admission and exact build/
catalog identity; [build policy](game-engine-catalogs.md#ir-and-build-identity) owns the active schema.
No current-Build quiet P7 result follows from older measurements. No playing-strength,
arbitrary-root, full-game-browser or resource-fit claim follows from this contract.
