# Oracle parser

Read the root manual and [parser guide](../../docs/oracle_parser.md), which owns
AST, binding, archive and ingestion contracts. This package owns the TypeScript
syntax AST and diagnostic reference analysis; root scripts own ingestion/archive
tooling; Rust owns semantic lowering and executable IR
([system ownership](../../README.md#system-architecture),
[current engine scope](../../docs/game-engine-current.md)). Broad grammar
expansion is paused.

## Constraints

- Compile complete supported syntax into typed nodes or fail; no lossy raw-text
  fallback, dropped keywords/characters or `any` AST. Wrong success is worse than
  rejection. Prefer a clear experimental AST change over compatibility shims.
- Keep [tracked normalization](../../docs/oracle_parser.md#pipeline-and-public-api) byte-identical to ordinary normalization.
- Preserve line/ability boundaries and modal grouping. Every `null` parser result
  restores cursor and effect context. Keep complete source and explicit diagnostics.
- `src/ast.ts` owns shapes; `parser/keywords.ts` owns accepted keyword vocabulary.
  Do not write parallel module/type inventories.
- Parsed/reference-bound/fully-bound remain separate from compiler/runtime
  support. Use exact data and cited rules; preserve unresolved scopes and target
  provenance.
- The archive's `resolution` is the compiler's contract; map a new binder node to
  its vocabulary or leave a named residual. Changing `ast.ts` or
  `semantic/resolution.ts` requires `node scripts/generate-rust-ast.ts`.
- Downloaded corpora, raw snapshots and generated archives/reports stay ignored
  and protected. Never refresh a frozen corpus to improve a comparison percentage.

## Focused workflow

Use pinned Node through `sh scripts/engine-env.sh` from repository root. Reuse
healthy dependencies; missing/broken installs use the
[conditional bootstrap](../../docs/game-engine-development.md#fresh-worktree).
No root `npm install`, implicit toolchain download, Java or emitted build step.
Node executes explicit `.ts` imports with erasable syntax only.

```sh
sh scripts/engine-env.sh npm --prefix packages/oracle-parser run check --workspaces=false
```

Fix false positives before expanding grammar: read complete source/rules, extend
AST/parser or reject, add structural positives with paired negatives, then run
the gate. Run [corpus comparison](../../docs/oracle_parser.md#development-and-verification)
only when needed, with matching corpus/scope/source hashes; acceptance may fall
when correctness improves, and percentages never establish execution support.
