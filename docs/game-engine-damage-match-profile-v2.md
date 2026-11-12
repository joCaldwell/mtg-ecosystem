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
Full-game capture and saved-report replay have not been run for v2. Full runtime
suites, SDK, hosting/search, 18-game/release matrices and retention rebuilds were
also omitted. V1's completed game/replay remains evidence for its original source;
these admission/setup checks do not renew combined acceptance.
