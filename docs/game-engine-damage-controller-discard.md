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
