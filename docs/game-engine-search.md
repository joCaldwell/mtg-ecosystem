# Foundations search capture and sampled-world contract

Search admits only `commander-foundations-v1` under its open-decklist session
policy (not a Commander requirement). It admits no card or mechanic itself. Exact
types, versions and tests live in `crates/core` and `crates/sim`
(`mtg_sim::search`). Rules:
[CR 103.5, 117.3–117.4, 302.6, 400.2, 400.7, 401.2–401.3, 402.3, 701.24a,
704.3 and 733.1–733.2](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.pdf).

## Supported root family

A supported root is a stable ordinary Priority boundary for the living active
viewer in PrecombatMain after setup: the viewer holds priority, the consecutive
pass round is empty, SBAs and trigger placement are complete, and there is no
pending input, concession, stack entry, pending trigger, effect, damage
transaction, combat declaration, announcement, resolution or private continuation.
Creature marked damage is zero for this reconstruction schema.

Four, three and two living seats are supported with original seating and departed
facts; roots may carry arbitrary public battlefields, completed casts, own
mulligans, known bottoms, public mana and physical Commander histories. Every other
root rejects explicitly (`priority == active` alone does not prove an empty pass
round) and is never replaced by another root or an authoritative full-state fork.
Every future-relevant field must come from permitted facts, complete hypotheses or
a documented schema invariant; missing history semantics reject.

## Permitted capture and information constraints

Capture is budgeted and resumable, returning a complete capture, incomplete work or
a typed rejection. A sealed capture contains no real state, game, fork, snapshot,
journal, private physical map, true RNG or secret seed-bound digest, and private
authority never enters sampler inputs or hashes. Its public identity hashes the
profile, current observation, complete ordered permitted history (nothing omitted
or summarized), own knowledge ledger, known bottom and remembered-hand constraints.

The owner-local ledger records only applied redraws, bottoms and draws; memory
references are not object handles. A later own shuffle invalidates positional
constraints without erasing observations. The kept bottom is an ordered suffix;
draws crossing it consume its known prefix, and proposals may not move known cards
earlier.

A failed hand-origin announcement was public before CR 733 returned it to hand. In
this exact pool (nonbasic singletons, no postsetup shuffle/bounce/library
moves/recursion, public deaths), a living opponent's historically public nonbasic
card absent from current public zones must be in that owner's hand, keyed by owner
and Oracle. Current public membership or departure removes the constraint; no
hidden-hand permission is minted; ambiguous history rejects. Future profiles do not
inherit this inference automatically.

Public life, status, mana, turn/land use, readiness, ownership/control, commander
casts and combat damage stay exact; readiness is never defaulted. Commander ledgers
bind physical designations. Departed seats are never reseated or given hidden
objects.

## Complete hypotheses and executable sample origin

A world proposal binds the capture identity, sampler version, world index, one
hypothesis per seat and an independent future seed. Every remaining live hidden
slot needs an authenticated Oracle identity; there is no authoritative fallback.
The viewer's hand comes from the capture; departed seats have empty hidden zones.

The sample builder cannot reach the real game, state, snapshot, RNG or a
missing-data callback. It validates identities, deck multiplicities, complete
coverage, public objects, zone counts, remembered hands, own history and Commander
binding; proposals cannot override public facts, change the catalog or supply a
handle map. Construction uses a fresh namespace with no real hidden IDs, and
chooser-facing namespaces hold no proposal or future-RNG digest, so worlds with
identical permitted facts yield identical observations until they diverge visibly.
Non-public root fields need explicit future-equivalence arguments, not defaults.
Permitted history is knowledge, never a hypothetical journal; other players' real
private histories are never copied. Rollout policies are stateless current-view.

## Mapping, execution and recovery

The root map is a bijection from captured current handles and decision tokens to
sampled ones; historical memory cannot map to current permissions. The mapped
sampled root observation must reproduce the capture exactly. Root action plans come
from visible information before sampled secrets are inspected and use ordinary
validation; an infeasible candidate is never replaced or dropped from the trial
denominator.

Sampled games wrap the same interpreter. Receipts and work from the real game,
another sample, fork or revision never authorize a sample, and vice versa. Sample
snapshots carry a separate origin envelope; restore rebuilds it independently,
replays future inputs and checks the state hash with tracing off. Ordinary restore
rejects sample origins, and a forged capture cannot become authority.

## Sampler roots and outcome accounting

The first sampler assumes uniform remaining definition assignments subject to
capture constraints; it does not certify an opponent keep-policy posterior.
Assignment identity ignores local symbols, hand order, world index and future seed,
so relabeling identical hidden cards or changing an unused seed is not a new world.
Sampling, future-game and rollout streams derive from a public job seed and
versioned domains; real match seeds and worker scheduling are forbidden inputs.
Report actual useful distinct assignments and any shortfall below 8; an exhausted
retry budget is a shortfall, not proof that fewer worlds exist.

Jobs use up to 32 visible-information candidates and 32 distinct worlds, giving
each candidate its first 8 useful worlds before extras. The horizon is 256 accepted
inputs, evaluated at the next exposed decision or an earlier terminal result, with
per-player result vectors. Terminal games, horizon evaluations, infeasible
candidates, unsupported roots, faults, cancellation, resource stops and missed
minimums are reported distinctly. Measurement runs through `engine:decks`.

Profile capability admission and exact [build/catalog identity](game-engine-catalogs.md#ir-and-build-identity)
fence search; new engine/compiler support does not admit it. No performance,
playing-strength, arbitrary-root or resource-fit claim follows from this contract
or historical [evidence](game-engine-evidence.md#search-hosting-and-performance).

## Paired deck experiments

`mtg-sim bench-pair` admits both decks at the HeadlessQualified tier. Supply
`--bulk-artifact DIR --manifest-sha SHA --pod-decks A.json`, one or more
`--swap SLOT:REMOVE:ADD` flags for the same zero-based slot, `--blocks N` and
`--out NEW_DIR`. Each block uses the indexed `bench-pair-v1` seed, starter
`index % 4`, and all four rotations; one worker runs all eight games, and output
order is independent of thread count. The Python bench wrapper also writes
`report.json` and `summary.md`.

`pairs.jsonl` records the first unequal accepted input (zero-based, including a
length mismatch) and compares the physical slots in the focal initial seven. The
focal win indicator counts a terminal draw as zero. The primary estimate averages
within-block win differences across fully observed four-rotation blocks;
complete-case estimates use all jointly terminal pairs. Percentile intervals
resample whole seed blocks 10,000 times from a run-identity-bound ChaCha seed; the
exact two-sided sign test excludes tied blocks; undefined values are null. The censoring table indexes A rows and A′ columns, complete then
censored; worst-case bounds assign each missing win indicator both extremes. A
rules or policy failure invalidates the run. Confounding is flagged by a censoring
rate above 2%, a censoring-difference interval excluding zero, or bounds spanning
zero alongside a complete-case interval excluding zero.
