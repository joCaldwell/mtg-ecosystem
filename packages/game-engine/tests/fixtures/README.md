# Engine fixtures

Keep exact card source records, archive/provenance manifests, reviewed IR catalogs,
profile/deck data, and small expected results here. Source facts remain checked in;
catalog regeneration uses `mtg-sim catalog build MANIFEST --write` and requires
review of the source and rules, not acceptance of whatever the compiler emits.

Current scenario expectations are the compact semantic hashes in
`d1b3-history-scenario-digests.json` plus explicit event, outcome, choice, and
privacy assertions in `../contracts.test.ts` and the Rust scenario suites.
The simulator checks trace/budget equivalence and replay separately. Do not
regenerate these expectations merely to make a failing test pass.

## Historical reports

The sixteen `expected-*scenario.json` / `expected-combat-games.json` reports were
obsolete, unconsumed historical outputs when removed in November 2026. Their
47,483,434 bytes and 1,719,863 lines mostly repeated complete observations, public
history, and events. They were not the current semantic baseline. Removing them
changes no runtime assertion or digest; it also stops the native source collector
from embedding those old reports into every new retained source bundle.

`historical-scenario-reports.json` records their exact Git revision, paths, byte
and line counts, and SHA-256 hashes. Historical documents naming those files refer
to that revision. Recover any report into a temporary location, for example:

```sh
git show 9257d1b692e05e402b2c9906d2b47d82e7049cbc:packages/game-engine/tests/fixtures/expected-effects-scenario.json > /tmp/expected-effects-scenario.json
sha256sum /tmp/expected-effects-scenario.json
```

Check the result against the manifest. The original bytes remain in Git history
and existing retained evidence; this cleanup does not rewrite either. A shallow
clone needs the named revision before recovery. New full demo reports are runtime
diagnostics, not fixtures to check in. No full demo, offline rebuild, or replay
acceptance claim follows from this file cleanup.
