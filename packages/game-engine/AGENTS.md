# Engine development

Read [the current map](../../docs/game-engine-current.md),
[development](../../docs/game-engine-development.md), and affected
[runtime invariants](../../docs/game-engine-mechanics.md#runtime-invariants) before edits. This
package owns Rust contracts/compiler/core/simulator; server/parser have their own
manuals. [Mechanics](../../docs/game-engine-mechanics.md) owns supported semantics.

- Lower complete exact archived source or reject. Use frozen fixtures/pinned rules
  and meaningful positive/rejection tests; never bless a changed expected outcome
  without source/rules review. Card names/IDs identify witnesses, never mechanics.
- Deterministic core imports contracts only: no compiler/parser runtime dependency,
  I/O, clocks, OS randomness, async inference or unsafe code. Compiler test fixtures
  are development-only; [structure checks](../../docs/game-engine-catalogs.md#ir-and-build-identity)
  enforce declared boundaries.
- Rust IR types own executable contracts. Active schema stays 1 in place, with
  strict capabilities/shapes and exact build/catalog recovery fencing. Other format
  versions and historical evidence remain separate; no old decoder/downgrade matrix.
- A new resolving effect is a [Kernel IR](../../docs/game-engine-catalogs.md#kernel-ir)
  node: an audited `kir::check` cell and its interpreter arm. Extend the operand algebra when the complete source semantics require it.

- All admitted catalogs share rules/capacities. Profiles are admission data;
  hosting/search requests need explicit reviewed capabilities. New IR fields
  require reviewing capability mapping as well as exhaustive enum variants.
- Preserve identity, ownership/control, exact arithmetic, atomic events, budget
  suspension, privacy and recovery. Resource faults/truncation are not rules
  outcomes; queries never mint handles/history or mutate semantics.
- Use focused checks with actual executed selectors and explicit omissions.
  Default compilation/compiler-contract tests do not qualify runtime games/host.
  One native owner per worktree target holds its Cargo lock; runners own it internally.
  Compiler work uses the [inner loop](../../docs/game-engine-development.md#inner-loop-and-single-final-gate);
  whole-archive scans and `check:engine:fast` run once, before publishing.
- Preserve unrelated work and ignored source/evidence/cache paths. Commit/push
  coherent focused checkpoints; no incidental merge, mechanic or pool expansion.
  [Storage](../../docs/game-engine-storage.md) governs cleanup and retirement.

[Catalogs/profiles](../../docs/game-engine-catalogs.md),
[capacities](../../docs/game-engine-capacity-policy.md),
[search](../../docs/game-engine-search.md) and [hosting](../../docs/game-engine-server.md)
are topical contracts. [Evidence](../../docs/game-engine-evidence.md) retains the
accepted D2c-d `394301d` freeze and later focused qualification separately.
