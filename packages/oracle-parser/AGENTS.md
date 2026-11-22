# Oracle parser

Read the root manual and [parser guide](../../docs/oracle_parser.md). This package
owns the TypeScript syntax AST and diagnostic reference analysis; root scripts own
ingestion/archive tooling. Rust owns complete-source semantic lowering/executable
IR. Broad grammar expansion is paused; `build-ir` deliberately fails. See
[system ownership](../../README.md#system-architecture) and
[current engine scope](../../docs/game-engine-current.md).

## Constraints

- Compile the complete supported syntax into typed nodes or fail; no lossy raw-text
  fallback, unknown keyword/character dropping or `any` AST. Wrong success is worse
  than rejection. Prefer a clear experimental AST change over compatibility shims.
- Preserve line/ability boundaries and modal grouping. Every `null` parser result
  restores cursor and effect context. Keep complete source and explicit diagnostics.
- `src/ast.ts` owns shapes; `parser/keywords.ts` owns accepted keyword vocabulary.
  Source/archive contracts are in the guide, not parallel module/type inventories.
- Reference-bound/fully-bound/parsed remain separate from compiler/runtime support.
  Use exact data and cited rules; preserve unresolved scopes and target provenance.
- The archive's `resolution` (closed `Ref` roles, step obligations, span
  certificate) is the compiler's contract; map a new binder node to the
  vocabulary or leave it a named residual. Changing `ast.ts` or
  `semantic/resolution.ts` requires `node scripts/generate-rust-ast.ts`.
- Downloaded corpora, raw snapshots and generated archives/reports remain ignored
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

Fix false positives before expanding grammar; read complete source/rules, extend
AST/parser or reject, add meaningful structural positives and paired negatives,
then run the gate. Typechecking and Node tests include tooling fixtures without
network/cache access. Source/report changes can invalidate coverage evidence.
Use [corpus comparison commands](../../docs/oracle_parser.md#development-and-verification)
only when needed, with matching corpus/scope/source hashes. Acceptance may fall
when correctness improves. Existing reports are historical until their identities
match; never derive execution support from percentages or filenames.
