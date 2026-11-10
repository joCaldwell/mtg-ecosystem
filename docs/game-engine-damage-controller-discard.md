# Controller discard in damage programs

IR31 adds `DamageControllerDiscardV1` for a one-card controller discard in a
`DamageDealt` trigger program. It composes the existing controller-discard
instruction and private resolution choice with frozen damage-trigger invocation
control. Omitted `recipient` retains its historical JSON meaning; explicit
`controller` is equivalent. Existing activation discard and event-player
discard retain their original capability/version gates. The contextual new gate
must reject older versions even with its capability label stripped, including
nested programs. Step/cast and other trigger families are not expanded.

The complete source witness is **Locke Cole**, Oracle ID
`bdcb7aed-3595-4c7e-b9da-543e92de919a`, retained in `cards-data-d2ce/`.
Its legendary creature characteristics, deathtouch and lifelink remain intact.
The combat/player self-source trigger draws one card, then requires its frozen
controller to discard one card. Archive line 28579 has source SHA-256
`eda8b26e4ed03247e75d50c75acd032213b9119d122b48e789cc347d1b1ac526`.
Whole-source lowering rejects qualified recipients, nonunit counts and
unsupported card-selection filters rather than dropping their semantics.

Drawing completes before the private discard choice. A failed draw still runs
the mandatory discard before state-based actions. A hidden Commander draw uses
the existing replacement continuation, then resumes at the discard once without
revealing another library card. Source or source-owner departure does not rebind
“you”; a departed invocation controller's personal actions are skipped.

No runtime opcode, public request, profile, historical catalog or hosting/search
admission is introduced. The existing headless damage-match catalog remains IR29
with its original digest and separately qualified match evidence.

Rules authority is the retained 2026-09-25 Comprehensive Rules, SHA-256
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`:
CR 109.5/603.3a bind the trigger controller; 608.2c preserves written order;
609.3 performs possible parts; 701.9a defines discarding; 117.5/704.4 defer
state-based actions during resolution; 903.9b governs the hand replacement.
Official source: <https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt>.

Focused native validation at source checkpoint `d85e757` passed formatting,
workspace all-target compilation, dependency/structure guards, 113 compiler tests,
20 contracts tests and 22 selected core tests. The core selection executes five
new source-backed Locke cases, six recipient-discard cases, six optional-draw
cases, one existing looter/activated-draw case and four damage-profile cases.
Tests cover lifelink, private checked responses, wrong/stale/foreign choices,
restore/replay, failed-draw ordering, hidden Commander replacement, departures,
forged choice actors and unchanged hosting/search rejection.

The executable source/schema preflight passed 33 existing cases. Its new case
initially failed because the synthetic modal witness had one mode instead of the
schema's required minimum two. That test fixture and its `action.json` catalog
path were corrected; the targeted IR31 rerun executed one test and passed. All
34 cases therefore have passing results across the two runs; no complete second
preflight or native rerun was needed for those test-only corrections. No source
record, expected card result or production guard changed to satisfy a failure.
An earlier formatting-only module-order failure is also retained.

Raw logs are retained in
`~/.local/share/mtg-ecosystem/engine-verification/correctness-loop-20261110/` as
`controller-discard-focused-01.log` (formatting failure),
`controller-discard-focused-02.log` (passed),
`controller-discard-schema-01.log` (33 passed, one test-fixture failure), and
`controller-discard-schema-02.log` (targeted pass).
Full core/sim/server suites, a fresh complete-match capture, SDK, hosting/search,
release/profile gameplay matrices and retention rebuilds were not run. Existing
complete-match evidence retains its original source identity; this is not renewed
combined acceptance.
