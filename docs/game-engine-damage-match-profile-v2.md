# Headless damage profile through IR32

`commander-damage-triggers-v2` is a separate finite development profile, with a
36-definition IR32 catalog. It adds Research Thief, Alpha Myr, Locke Cole and
Fear of Failed Tests to the 32-definition IR29 v1 pool. The new catalog digest is
`06466aeb1c18d3b75352c9ab276c046ff07b931f1aadf3f86fde7c5e74dc5bda`.
The [v1 profile and its evidence](game-engine-damage-match-profile.md) retain their
original catalog digest and bytes. Neither profile requests hosting or search.

The four identical decks use Sivitri Scarzam as commander. Each contains 34
singleton nonbasic cards including the commander, 33 Islands and 33 Swamps:
100 physical cards, with 99 initially in the library. All source color identities
fit the commander's blue-black identity. Construction uses the existing frozen
2026-10-15 Commander policy; the profile does not claim current universal legality
or introduce a new policy. Ordinary common rules, resource limits and public
observation policies execute these cards; no profile selects different mechanics.

The new `damage-matches-v2/` provenance group references the original 36 source
records. Twenty-seven retain their historical archive origin; nine use the pinned
current archive. It retains exact source/AST/raw record pins and both original
headers without changing the old record groups. The current archive is SHA-256
`d705222b280a71cb5a5362f657d530b22402ca51687bac3227c1d6db403d0282`.
No extraction, card-source edit, copied source row or generated expected-result
adjustment is needed. The committed small IR fixture and digest come from the
ordinary catalog builder.

This profile makes the IR30 filtered source, IR31 controller discard and IR32
actual damage-quantity draw available together in legal headless matches.
Compiler support, profile admission, completed gameplay and replay are separate
evidence boundaries. Source-backed kernel tests establish the new operations;
a naturally completed game alone does not prove every card or operation fired.

Focused validation passed formatting/dependency guards, workspace all-target
compilation, 115 compiler tests, 21 contracts tests and 17 selected core tests:
four v2 admission/setup cases, four unchanged v1 cases, eight shared profile-rule
cases and one generic invalid-construction case. The two selected executable
source/catalog cases passed, verifying all catalog regeneration and fresh parser
agreement for the 36 source records. Selected tests executed with no skips.

Logs are retained under
`~/.local/share/mtg-ecosystem/engine-verification/correctness-loop-20261112/` as
`damage-profile-v2-focused-01.log` and `damage-profile-v2-source-01.log`.
One ordinary fixed-index game and explicit saved-report replay subsequently
passed at clean published source `580e862`, engine build
`66eb93de4f6505440b9e1d1701e648992967e9c09b4155aa0fff687fc0f85292`.
Seat 0 won naturally on turn 45 after 2,293 decisions, 6,477 rule steps and
4,586 query calls. Replay reproduced final state
`86964a838754112623cdfb759c03a4a12311f0f0de99b356c8db9936338f98d8`;
source identity remained unchanged throughout capture and replay.

The final detailed trace contains seven resolved Locke Cole triggers, seven
Research Thief triggers and six Fear of Failed Tests triggers. Research was its
own damager in all eight collected witnesses: the distinct observer/damager case
is established by the earlier exact-source kernel tests, not by this game.
The trace has 14 Locke instruction batches and 12 Fear instruction batches;
batch counts are event receipts, not a general opcode-completion metric.

The approximately 535 MiB saved report `damage-profile-v2-game-0.json`, replay
report `damage-profile-v2-replay-01.json`, game summary and
`damage-profile-v2-trace-audit-01.json` remain in the same external evidence
folder, with the live journal under `damage-profile-v2-artifacts/`. No repeated
seed search, 18-game/release matrix, full runtime suite, SDK, hosting/search or
retention rebuild was run. V1's completed game/replay remains evidence for its
original source. These focused results do not renew combined acceptance.
