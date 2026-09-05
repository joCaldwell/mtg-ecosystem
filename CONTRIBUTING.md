# Contributing to Project Multiverse

Start with the root [AGENTS.md](AGENTS.md), identify the package, and read its
manual. The deck-builder is active and standalone. The oracle-parser is a
limited supported subset; the rules engine and game server are future work.

## Oracle parser setup

Use Node ≥ 23, npm, and Git. Install workspace dependencies at the root:

```sh
npm install
npm run check --workspace=@mtg-ecosystem/oracle-parser
```

The parser runs TypeScript directly. `build` means `tsc --noEmit`; tests use
`node --test`. No Java, generated grammar, or visitor is required. Package
commands and constraints are in
[packages/oracle-parser/AGENTS.md](packages/oracle-parser/AGENTS.md).

## Changing the parser

1. Check actual card text in card data and consult the relevant Comprehensive
   Rules. Cite the rule/source for externally defined behavior in code.
2. Decide the complete AST shape in `src/ast.ts`. Preserve distinctions in
   logic, event grouping, ownership, targets, costs, restrictions, and scope.
   Reject unsupported constructs rather than accepting an approximation.
3. Extend the relevant parser module. Returning `null` must restore the cursor;
   effect parsing must also restore contextual state after failed attempts.
4. Add complete structural assertions and negative cases. Pair similar inputs
   when a small wording change affects meaning. Avoid success-only tests and
   unchecked snapshot updates.
5. Run the package check. With a local corpus, run `npm run validate` at the
   root and inspect accepted/rejected examples and changed ASTs. A correctness
   fix may lower acceptance. Compare reports only under the same corpus and
   scope; `--json` includes the identities needed for that comparison.
6. Update the current contract documentation. `src/ast.ts` is authoritative;
   future IR design must be labeled as a proposal.

Pipeline tests use temporary directories and simulated HTTP responses. They
must not need a bulk download or mutate the working cache.

Do not commit downloaded/generated artifacts or modify another package's
ongoing work. Do not assume root test, build, or dependency conventions match
another package's commands. No repository-wide formatter or linter command is
currently prescribed; follow the local TypeScript style and use `git diff
--check` for whitespace errors.

See [parser design](docs/oracle_parser.md), [data contracts](docs/data_schemas.md),
and [Scryfall ingestion](docs/scryfall-integration.md). `npm run build-ir` fails
intentionally until semantic validation and the emitter are implemented.
