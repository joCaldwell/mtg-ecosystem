# Engine development

This package owns the Rust contracts, compiler, core and simulator; server and
parser have their own manuals. Read [the current map](../../docs/game-engine-current.md),
[development](../../docs/game-engine-development.md) (environment, checks,
publication) and the affected [runtime invariants](../../docs/game-engine-mechanics.md#runtime-invariants)
before edits. [Core](../../docs/game-engine-mechanics.md) and [board](../../docs/game-engine-board-mechanics.md) mechanics own supported semantics.

- Lower complete exact archived source or reject, with positive and rejection
  tests; never bless a changed expected outcome without source/rules review. Card
  names/IDs identify witnesses, never mechanics.
- Core imports only IR and permitted-view contracts: no compiler/parser
  dependency, I/O, clocks, OS randomness, async inference or unsafe code.
  Compiler test fixtures are development-only. [Structure checks](../../docs/game-engine-catalogs.md#ir-and-build-identity)
  enforce declared boundaries.
- Rust IR types own executable contracts; active schema stays 1, evolved in place
  with strict capabilities/shapes and exact build/catalog recovery fencing. No old
  decoder or downgrade matrix.
- A new resolving effect is a [Kernel IR](../../docs/game-engine-catalogs.md#kernel-ir)
  node. Resolving actions use a checked verb signature and the shared
  evaluator. Extend the operand algebra when the complete source semantics
  require it.
- Profiles are admission data over shared rules/capacities; hosting, search and
  analytics need the derived [tier](../../docs/game-engine-mechanics.md#derived-capability-tiers).
  New IR fields and cells require reviewing tier rows as well as exhaustive enum
  variants.
- Preserve identity, ownership/control, exact arithmetic, atomic events, budget
  suspension, privacy and recovery. Resource faults/truncation are not rules
  outcomes; queries never mint handles/history or mutate semantics.
- Put rules assertions in `rules`, the journal corpus in `recovery`, frozen-source
  witnesses in `cards` and server integration in `host`; prefer `testkit::Board`
  and `Driver` for private boards and keep adversarial modules on public APIs
  ([test topology](../../docs/game-engine-development.md#engine-test-topology)).
- Opt-in Rust families prove both directions of coverage through the
  [printed-evidence API](../../docs/game-engine-catalogs.md#rust-printed-evidence).
  Select the authority before lowering and use the pinned comparison for its migration.
- Compile whole archives through the [frozen-input command](../../docs/game-engine-catalogs.md#frozen-input-compile);
  use the R6 input manifest for cached or fresh reproduction. `build-ir`
  remains its byte-identical legacy control.
- Iterate and gate per [development](../../docs/game-engine-development.md#inner-loop-and-one-gate).
  No incidental mechanic or pool expansion.

Topical contracts: [catalogs/profiles](../../docs/game-engine-catalogs.md),
[capacities](../../docs/game-engine-capacity-policy.md),
[search](../../docs/game-engine-search.md), [hosting](../../docs/game-engine-server.md),
[storage](../../docs/game-engine-storage.md) and immutable historical
[evidence](../../docs/game-engine-evidence.md).
