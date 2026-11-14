# Contributing

Start at [AGENTS.md](AGENTS.md), identify the target package and read its manual.
The [README map](README.md#documentation-map) routes current contracts and setup.
Use the assigned orchestrator-bound checkout; inspect status, branch and HEAD
before edits. Preserve unrelated work and do not install over shared dependencies.

## Setup and checks

Use the existing pinned environment through `sh scripts/engine-env.sh COMMAND`.
Engine/parser bootstrap is `sh scripts/engine-bootstrap.sh`, only when the local
installation is missing or broken. It installs locked minimal dependencies and
never downloads native tools, builds Cargo or refreshes card data.
See [environment setup](docs/game-engine-development.md#toolchain-setup).

Product, backend, client and SDK have separate package-local locks and installs:
`npm --prefix packages/PACKAGE ci --workspaces=false --ignore-scripts` through
the environment launcher. Their manuals specify checks. No implicit Cargo,
generated SDK replacement, root dependency churn or full engine matrix follows
from a documentation/client change. Use `git diff --check` for whitespace.

For parser edits, preserve complete structural meaning and cursor/context rollback;
add meaningful positive and rejection cases, then run the package check. Corpus
validation is separate: compare source, scope and corpus hashes before claiming
coverage. `build-ir` deliberately fails; bulk per-set emission remains unimplemented.
For engine/server edits use [the agent workflow](docs/game-engine-agent-workflow.md)
and [focused verification](docs/game-engine-verification-workflow.md).

## Documentation

Keep one canonical owner per contract. Link precise sections rather than copying
rules into onboarding and release notes. Record supported/unsupported behavior,
commands, identities and omissions; use Git history for obsolete plans and
transcripts. Do not weaken reference/qualification tests when moving documentation.
Count every tracked Markdown file, including package manuals and skills, with:

```sh
git ls-files -z | python3 -c 'import sys; print(sum(p.lower().endswith((b".md", b".markdown")) for p in sys.stdin.buffer.read().split(b"\0") if p))'
```

Choose document boundaries by ownership and readability, reducing duplication and
obsolete guidance rather than optimizing an arbitrary count. Keep fewer than 100
tracked Markdown files; use `python3 scripts/check-docs.py` for count/local-link checks. Generated
outputs, dependencies, evidence, credentials, databases and worktrees are not
documentation cleanup targets. Publish a coherent branch and draft PR with actual
checks, omissions and any overlap with existing work; do not merge incidentally.
