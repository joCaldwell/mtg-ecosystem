# Historical engine evidence and recovery

This directory is the historical onboarding location. Use
[the current map](../../game-engine-current.md) for current status and commands.
The old reports below keep their paths so citations, evidence manifests and
recovery instructions continue to work. They are historical/source-qualified
evidence, not a sequence every new contributor must read or repeat.

## Prior onboarding at c62777b

- [Root manual](root-agents-20261109.md)
- [Engine manual](package-agents-20261109.md)
- [Engine README](package-readme-20261109.md)

These snapshots preserve the original prose, with an explicit historical header
and adjusted Markdown link bases. Their commands, counts, profile capacities,
version numbers and pending-review statements describe their original contexts.
Current rules use common capabilities/capacities; old per-profile gates do not
apply. The source originals are also recoverable with
`git show c62777b:packages/game-engine/AGENTS.md` (and equivalent paths).

## Report index

| Historical area | Source-qualified contracts and evidence |
| --- | --- |
| Chronological evidence | [Progress ledger](../../game-engine-progress.md), [fixture recovery index](../../../packages/game-engine/tests/fixtures/README.md) |
| Initial execution and recovery | [First scenario](../../game-engine-first-scenario.md), [P2](../../game-engine-p2-slice.md), [departures](../../game-engine-departure-slice.md), [events](../../game-engine-event-slice.md) |
| Stack, payment, turns and combat | [Stack](../../game-engine-stack-slice.md), [payment](../../game-engine-payment-slice.md), [turns](../../game-engine-turns-slice.md), [combat](../../game-engine-combat-games.md) |
| Commander and action slices | [Casting](../../game-engine-commander-casting.md), [damage/library](../../game-engine-commander-damage-library.md), [actions](../../game-engine-actions-payments.md), [stack costs](../../game-engine-stack-costs.md), [damage/life](../../game-engine-damage-life.md) |
| Trigger/interaction slices | [Triggers](../../game-engine-triggers.md), [effects](../../game-engine-effects.md), [choices](../../game-engine-choices.md) |
| Finite match/search/host releases | [Matches](../../game-engine-matches.md), [P7 search](../../game-engine-search.md), [P8 host](../../game-engine-server.md), [attachments hosting](../../game-engine-server-attachments.md) |
| Extended match catalogs | [Keywords](../../game-engine-keywords.md), [combat keywords](../../game-engine-combat-keywords.md), [tokens](../../game-engine-tokens-counters.md), [staples](../../game-engine-staples.md), [mana](../../game-engine-mana-permanents.md), [four colors](../../game-engine-blue-red.md), [attachments](../../game-engine-attachments.md), [removal](../../game-engine-removal.md), [sacrifice](../../game-engine-sacrifice.md), [wipes](../../game-engine-board-wipes.md) |
| Cards as data | [Audit](../../game-engine-cards-as-data-audit.md), [coverage](../../game-engine-cards-as-data-coverage.md), [D1c](../../game-engine-cards-as-data-d1c.md), [D2b](../../game-engine-cards-as-data-d2b.md) |
| Generic trigger additions | [D2c-a](../../game-engine-cards-as-data-d2c-a.md), [D2c-b](../../game-engine-cards-as-data-d2c-b.md), [accepted D2c-c](../../game-engine-cards-as-data-d2c-c.md), [accepted D2c-d](../../game-engine-cards-as-data-d2c-d.md), [candidate D2c-e](../../game-engine-cards-as-data-d2c-e.md) |
| Verification and ownership work | [Lifetimes](../../game-engine-verification-lifetimes.md), [trace digests](../../game-engine-verification-trace-digests.md), [November gardening](../../game-engine-gardening.md), [focused-check measurements](../../game-engine-fast-checks-20261109.md) |

## Preserve evidence boundaries

The accepted D2c-d gate at `394301d` includes the reviewed lifetime work and
excludes subsequent IR25/digest reuse. D2c-e's candidate-focused checks have their
own source freeze and pending parent/combined review. Earlier search performance
figures belong to their measured executables, not the current checkout.

The progress ledger is retained in place and is not rewritten to relocate every
release. Removed giant unused fixture reports remain recoverable through the
fixture index and Git history. External retained proof directories, failed logs,
source archives and binaries remain at their recorded paths. A Git source copy
does not substitute for retained executable or measurement evidence.
Do not delete/move evidence or worktrees as incidental onboarding cleanup.
