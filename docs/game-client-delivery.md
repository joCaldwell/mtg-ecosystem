# Client delivery, verification and review

Status: planning only. See [architecture](game-client-plan.md) and
[contracts](game-client-contracts.md). Milestones are dependency/acceptance units,
not calendar estimates. Re-estimate after M0 proves the unknown integration costs.
The [responsibility boundaries](game-client-boundaries.md) are a mandatory design
and review gate for each milestone.

## 1. Work packages and dependencies

| Milestone | Deliverable and owners | Dependencies | Exit evidence |
| --- | --- | --- | --- |
| M0 Contract freeze and feasibility | Server/core/SDK owners: exact DTO drafts, request inventory, browser production-bundle experiment, supported browsers, source-pinned fixture manifest and budget choices | Existing reviewed host | Small browser proof against real host; no Node globals/private fixtures; strict validation/CSP proof; review settles action/query privacy and resume atomicity |
| M1 Join and read-only table | SDK/client owners: entrypoint, single-seat export/import, same-origin serving, lock ownership, manifest check, presentation metadata and table | M0 browser contract | Separate seats see correct boards/hands; missing images usable; duplicate tab blocked; client build from clean checkout documented |
| M2 Complete interaction infrastructure | Core/server: action beginnings and context; session owner: coherent resume and journal; UI owner: generic selection/payment/ordering controls | M0 DTO freeze; M1 wiring | Focused query/privacy tests; command and refresh fault cases; generated SDK parity; accessible fixture gallery for supported requests |
| M3 Foundations game | Client interaction owner covers all reachable foundations decisions; host/tooling owner supplies a human-paced bot runner; test owner orchestrates one browser + three SDK bots | M2 | Natural full match with consistent typed outcome; deliberate human-think-time delay; focused manual keyboard play; scripted browser integration plus actual host evidence |
| M4 Attachments alpha | Complete attachments workflows, inspector, dense boards, all four browser seats | M3 plus profile-specific request inventory | Every admitted request has focused evidence; natural attachments game and four-context privacy/recovery checks; supported browser smoke |
| M5 Private online design | Product/server owners define accounts, invitations, provisioning, remote supervision, capacity and operations | Useful M4 alpha | Separate approved deployment/session plan; not part of local release implementation |

M0 is a small experimental implementation phase in the FUTURE implementation task;
this planning task does not execute it. Any experiment must have explicit success
criteria, source pins and a recorded outcome. Resolve failed proofs before building
dependent code, rather than assuming a library or query is browser-ready.

Parallel implementation lanes after M0: UI fixtures/layout and pure draft reducers;
SDK packaging; core/server contract work. Session wiring follows DTO freeze. Keep
one owner for shared Rust protocol files and one heavy native compilation owner.
Each lane uses an orchestrator-bound worktree when assigned implementation; this
plan does not create implementation worktrees or start those tasks.

## 2. Agent handoffs

Each work package begins with root/target manuals, source/branch/status inspection,
owned paths and a smallest relevant verification plan. New independent work starts
from current origin/main; dependent changes use the explicit prerequisite head.
The planning source above is a reviewed reference, not a command to branch all
future work from a feature branch or adopt unaccepted engine capabilities.

Handoff includes source SHA, changed contract and generated schema digests, passed
focused checks, omitted checks, unresolved issues and artifact paths. A reviewer
checks semantics/privacy and one real integration path. No automatic full historical
engine matrix is added. Preserve all meaningful failed evidence and unrelated work.

Proposed package commands: `dev`, `build`, `typecheck`, `test`, `test:browser`,
`check`. Client checks never start Cargo silently. The native owner supplies the
specific compatible server binary; SDK generation is explicit and reproducible.

## 3. Decision coverage inventory

At M0 generate the request/response variant list from the selected Rust export.
Maintain a small reviewed table: variant, admitted profiles, source context needed,
control component, draft invariants, keyboard path, fixture and test locator.
All variants are exhaustively switched in TypeScript; unsupported profile variants
produce a visible stop rather than guessed responses. Fixtures must be permitted
observations from exact source-backed scenarios, never invented card facts.

Test representative combinations, not only isolated controls: nested mana payment,
multiple target occurrences, changing decision while drafting, same-name objects,
tokens and attachments, a player leaving, and ordering-dependent choices. A natural
match does not establish variants never encountered in that match.

## 4. Required focused acceptance cases

| Area | Required cases |
| --- | --- |
| Journal | Failure before persistence prevents send; crash after persistence/before send; response lost after commit; accepted acknowledgment followed by refresh before consumption; delayed original arrival; exact retry; definitive stale rejection; corrupted journal; wrong seat/build/catalog |
| Stream | Lifecycle-only page, backlog, future/malformed cursor, reconnect, outcome, observation/token replacement, abort, Strict Mode restart and late async completion after seat change |
| Ownership | Duplicate click; second tab; owner closes; browser refresh; different seat; bot/human collision gives explicit bounded failure without claiming rightful controller/takeover; unsupported lock API |
| Queries | Current/stale/foreign token, handle lifetime, budget exhaustion, cancellation, paging, purity, hidden-state paired cases, joint selection validation |
| Layer ownership | Core dependency guards; HTTP cannot run Game; SDK has no policy/journal/retries; pure client modules have no I/O; native/core and hosted semantic disposition agree after explicit host preconditions; forged client hints cannot bypass core |
| Error ownership | Distinct protocol/auth errors, host stale revision, core illegal response, incomplete query, host pause/fault and rules outcome; public error mapping preserves core visibility |
| Privacy | Four isolated browser contexts; hand/search candidates; no secrets in URLs/logs/export/cache; source context visibility |
| Controls | Every admitted decision family, keyboard operation, focus, reduced motion, large exact values and missing images |
| Host failure | 429/503, worker recovery, resource pause, fault, departure, incompatible build; no false win/draw or silent action |
| Browser packaging | Clean production build, exact generated schemas, standalone validation parity, CSP, no Node runtime or private fixture imports |
| Performance | Dense reviewed board, long history resume, bounded journal/history/cache, background-tab resume and idle CPU |

The test coordinator launches bots as separate SDK consumers with one private
single-seat bootstrap each. Human seat has no bot. Existing example consumers
have a 25-minute total acceptance-harness watchdog; they are not a sufficient
human-session runner unchanged. M3 provides a separately named runner reusing
their policies, retaining exact retry/idempotency and explicit bounded work, with
a documented human-session lifecycle. Preserve historical harness limits/evidence.
Proposed local runner total limit: four hours, with an explicit nonterminal stop
report and supervised process cleanup; never a fabricated concession. The existing
host's cumulative 30-minute per-seat ready/idle budget remains authoritative and
unchanged. Waiting for humans does not consume a bot-operation retry deadline:
separate idle polling from uncertain-command recovery and bound both explicitly.
Test a delayed human decision and a fake-clock run crossing the old 25-minute
limit, plus host resource pause and operator shutdown. Do not wait hours in tests.

Existing policies have in-memory plans; bot restart is not supported by current
evidence. A bot failure stops the run with retained diagnostics; it does not trigger
automatic takeover. Restart/resume of a bot policy is a separate enhancement if
alpha usability requires it. Human-browser refresh and host recovery are tested
separately from bot-process restart.

Four browser contexts can be driven by deterministic Playwright interaction scripts
to prove transport/privacy and workflow reachability. Label automation as automated;
include a separate actual human exploratory pass for usability. A fast scripted
game is not a benchmark of human pace or proof of all decision families.

## 5. Release evidence and budget decisions

Each alpha gate records exact client/server/core/catalog/schema identity, selected
profile/decks, browser versions, setup procedure, tests that actually ran, omissions,
outcome, resource measurements and recovery results. Keep raw generated evidence
outside Git under existing storage policy. Commit small fixtures and summaries.

Use current stable Chromium as the development acceptance browser and Firefox as
the second-engine smoke target. M0 verifies Web Locks, IndexedDB, BigInt, ESM and
production SDK behavior on both; Safari is deferred until explicitly tested.
Browser versions are pinned in automation and recorded in release evidence.

In M0 set provisional bounded action-query work/page sizes and payloads, resume limits,
journal max record bytes, retained history and image-cache policy. Derive these from
existing host bounds and representative fixtures. Confirm or revise them with the
M0 feasibility evidence before M2, recording the reason for any revision. Choose measured UI targets before
performance runs; do not respond to failures by silently raising limits.

First local alpha requires all M1–M4 acceptance items. Refresh may require credential
reimport by design; preserving ambiguous actions is mandatory. Full public online
service, arbitrary decks, strategic strength and current-build engine-wide performance
claims do not follow from this gate.

## 6. Planning review record

### Separation-of-concerns follow-up

A further independent read-only boundary audit identified ambiguous host/core
wording, an overly broad read-owner rule and missing explicit metadata/SDK/policy
ownership. The follow-up adds a normative ownership matrix and command/query/resume
flows, separates core legality from host admission, assigns offline presentation
production, and adds dependency, semantic-parity and error-origin checks. This
refines planning only; the original runtime proof gates remain unexecuted.
The independent reviewer reread all four revised documents and approved the
separation of concerns with no remaining blocking ownership ambiguities. Local
links, whitespace and code fences were checked; no application code or runtime
tests were changed/run for this documentation follow-up.

Three independent read-only agents assessed integration, client architecture and
failure/privacy risks, then each reviewed the assembled documents. Review closure
concerns plan completeness only; implementation proof gates remain unexecuted.

Findings already incorporated:

- Action enumeration is missing in core query APIs, not only the HTTP router.
- Several decisions need source-backed prompt context in addition to candidate IDs.
- SDK browser packaging and runtime Ajv compilation require a CSP/validator gate.
- Existing full credential output must never be passed to the browser; use one seat.
- Unknown outcomes require the exact persisted envelope and cannot be replaced on 404.
- Same-seat tabs and bots need explicit ownership; browser locks do not fence devices.
- Bounded coherent resume is distinct from historical event replay.
- Current streams do not provide a complete rules log or authoritative countdown.
- Existing bot recovery and public deployment are separate unproven capabilities.

Second-pass corrections and disposition:

| Review area | Finding | Resolution |
| --- | --- | --- |
| Integration and recovery | Accepted acknowledgment could be mistaken for journal completion | Exact envelope retained through accepted/pending; terminal-disposition rule and refresh-before-consumption test added |
| Recovery | Abandonment or changed build key could bypass an uncertain command | Stopping locally preserves unresolved binding; scan by origin/match/seat before selecting a new build binding |
| Ownership | Generic 429 cannot identify the secondary controller | Bounded failure without primary/secondary claims; cross-device fencing remains deferred |
| Browser lifecycle | Late callbacks or Strict Mode remount could mutate replacement session | Idempotent lifecycle and generation fences explicitly required and tested |
| Accessibility | Remote decision update could steal keyboard focus | Announcement plus explicit jump-to-decision; user-initiated focus transitions only |
| Integration setup | Development bootstrap origin could bypass same-origin proxy | Explicit allowlisted upstream mapping and identity verification |
| Delivery | Budget freeze milestone wording conflicted | M0 provisional bounds, feasibility-qualified confirmation before M2 |
| Human play | Existing bots stop at the historical 25-minute watchdog | Separate human-paced runner, explicit duration/idle contract and delay tests; final risk reviewer rechecked and cleared this blocker |

Final outcome: integration and client reviewers approved subject to the corrections
above, which were applied; the risk reviewer confirmed the final bot-runner change
and reported no remaining blocking findings. The coordinator checked all local
document links, whitespace and code fences. No native, SDK or browser tests were
run for documentation-only planning. Future implementation begins with M0 and
must obtain its own source-qualified evidence; this review is not runtime approval.
