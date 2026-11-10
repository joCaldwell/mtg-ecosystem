# Engine development

Read [the current map](../../docs/game-engine-current.md) first. This package
contains Rust contracts/compiler/core/simulator: finite legal Commander matches,
public-observation policies, replay and sampled search. It is not arbitrary Magic.
Server and parser edits also require their own package manuals.

## Start and verify

Run from repository root:

```sh
sh scripts/engine-bootstrap.sh
sh scripts/engine-env.sh npm run check:engine:source
sh scripts/engine-env.sh npm run check:engine -- --plan
```

[Bootstrap/doctor/environment instructions](../../docs/game-engine-development.md)
cover exact existing toolchains and minimal locked dependencies. No automatic
toolchain installation or system downgrade. Preserve explicit cache overrides.

Josh's 2026-11-09 preference is substantially faster focused verification, even
with reduced coverage. The default engine check compiles all targets and runs
tooling/source plus compiler/contracts library tests; it omits core/sim/server
runtime tests, executable contracts, SDK, demo, release/profile matrices and
retention. Add the smallest relevant test for the change, verify it actually ran,
and report omissions. No mandatory full acceptance matrix follows each edit.
See [verification modes and cadence](../../docs/game-engine-verification-workflow.md).

One heavy owner coordinates native compilation. Standalone Cargo uses the shared
`~/.local/share/mtg-ecosystem/engine-verification/cargo.lock`. The check runner owns
that lock internally; do not acquire it twice. Work on source/docs while busy.
Do not delete or move the preserved performance cache or evidence paths.
Commit and push coherent checkpoints after focused checks; do not wait for a
full gate. Preserve exact failed/interrupted results and unrelated work.

## Non-negotiable boundaries

- Core's runtime dependency is contracts; no compiler/parser, network, filesystem, clocks,
  OS randomness, async work, inference or unsafe code in deterministic core.
  Compiler-backed test fixtures are development dependencies; see [boundary checks](../../docs/game-engine-builds.md).
- Complete archived card source must lower or reject. Use exact local fixtures,
  pinned rules citations and paired positive/unsupported cases. Never change an
  expected outcome without source/rules review.
- Rust IR types are authoritative; schema exports are generated. AST archive,
  IR, snapshots, public projections and query recovery have separate versions.
  Historical catalogs and evidence keep their original version/source pins.
- All admitted catalogs execute common rules. Profiles are admission data;
  names/lineage/IR versions do not select mechanics or capacity ceilings.
  Hosting/search requests must fit their explicit reviewed capability sets.
- Keep identity, ownership/control, exact arithmetic, atomic events, budget
  suspension, query privacy and recovery contracts intact. Read the affected
  sections of [runtime invariants](../../docs/game-engine-invariants.md) before
  editing those paths; they are part of this manual's constraints.
- Do not weaken guards or substitute resource faults/truncation for rules
  outcomes. Queries do not mutate semantics or mint public handles/history.
- Commit small fixtures and summaries; downloaded corpora, bulk IR, schema
  exports, bundles, timings and build outputs stay ignored. Ignored evidence is
  not permission to delete it. Do not expand a pool/mechanic incidentally.

## Find the relevant contract

[Catalog/source workflow](../../docs/game-engine-catalogs.md),
[profile admission](../../docs/game-engine-profiles.md),
[capacity policy](../../docs/game-engine-capacity-policy.md),
[search](../../docs/game-engine-search.md), [hosting](../../docs/game-engine-server.md),
and [storage](../../docs/game-engine-storage.md) are topical entry points.

Accepted D2c-d at `394301d` excludes later IR25 and digest reuse. Candidate-focused
checks are not combined acceptance or a renewed P7 claim. Use the
[current evidence table](../../docs/game-engine-current.md#evidence-boundaries-at-the-platform-baseline)
and [history index](../../docs/history/engine/index.md). The old 522-line manual
is preserved there as historical context, not current release authority.
