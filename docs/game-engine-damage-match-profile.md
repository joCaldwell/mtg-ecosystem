# Headless damage-trigger matches

`commander-damage-triggers-v1` is a finite development profile for four-player
headless Commander under the frozen rules and construction policy. It makes the
IR26–IR29 damage-trigger work available through ordinary legal-match admission
and simulator commands. It requests neither hosting nor sampled search.

The IR29 catalog `tests/fixtures/catalogs/damage-matches.json` contains 32
definitions, with canonical digest
`b7dae2b1f537f3c7abb4ed4e53a6dc1ae4b84f9b14eeb33e8f42f2736dda5bd0`.
Paths here are relative to `packages/game-engine/`. The isolated
`tests/fixtures/damage-matches/` provenance group references 27 existing source
records and the exact Thieving Otter, Neurok Commando, Dimir Cutpurse and Demon
of Loathing records. Their old files and metadata remain unchanged.

The only new source record is **Sivitri Scarzam**, Oracle ID
`333cc91b-01ab-4491-a993-d8304fc382c6`: a vanilla blue-black legendary creature
requiring no new rules. Ordinary `catalog extract` verified the full preserved
AST archive SHA-256
`d705222b280a71cb5a5362f657d530b22402ca51687bac3227c1d6db403d0282`
before extracting line 7624. The source SHA-256 is
`e56835fe3037f48c1b1b16adb69e91e4ac29795f805380f4776090048dcfb623`.
Selected source pins and exact archive headers are retained locally; the
historical origin retains its original per-record evidence qualification.

Each of four independently shuffled decks has one Sivitri as commander, one
each of the other 29 nonbasic catalog cards, 35 Islands and 35 Swamps. The
commander is included in the 100. The standard dated Commander policy applies;
color identity, singleton names, commander identity and count checks use the
ordinary admission path. These are finite test decks, not strategy advice or
claims of representative Commander play. Existing profiles are unchanged.

No rule, capacity, seed or policy branch selects this profile's mechanics.
The standard observation-based Develop policy handles the existing optional
draw, discard and sacrifice requests. Ordinary commands are:

```sh
mtg-sim match-game commander-damage-triggers-v1 0
mtg-sim match-replay commander-damage-triggers-v1 SAVED_GAME.json
```

Use the pinned environment and shared Cargo lock. Preserve the saved trusted
diagnostic game/journal outside Git. A single complete game and replay provide
focused integration evidence; they do not establish the full 18-game profile
matrix, combined engine acceptance, hosting/search admission or performance.

Source checkpoint `14698c1` passed all-target compilation, four new profile
tests, the existing construction-negative test, 108 compiler and 18 contracts
library tests, and the selected 32-record parser/catalog contract. The fixed
index-0 game finished naturally on turn 57 with seat 3 winning after 2,473
decisions. Explicit saved-report replay verified final state SHA-256
`7e5ee6b1d5e2cc51c811c7f8b4761a0630cdf1e34c294415da8880584ee59483`;
the measured source stayed unchanged. This does not assert that every catalog
instruction or all four new creatures were reached in that single game.

Logs, live journal, saved detailed game and summary are retained under
`~/.local/share/mtg-ecosystem/engine-verification/correctness-loop-20261110/`.
The detailed report is about 529 MiB; retain it rather than recapturing or treating
it as a disposable cache. The initial profile test's ordered color-identity
assumption is preserved in its failed log; it was corrected to set membership
using the unchanged pinned source and admission contract. Full core/sim/server
suites, Clippy, SDK, the 18-game profile matrix, search/hosting tests, quiet
performance measurements and retention rebuilds were omitted.
