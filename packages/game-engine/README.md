# Game engine

Rust executable card contracts, semantic compiler, deterministic rules core and
native CLI for finite Commander matches, replay and sampled-world search.
Legal match profiles are data-driven; hosted/search capabilities remain explicitly
reviewed subsets. This is not a general Commander implementation.

Start with [the current architecture and evidence map](../../docs/game-engine-current.md).
Before editing, read [AGENTS.md](AGENTS.md).

From the repository root:

```sh
sh scripts/engine-bootstrap.sh
sh scripts/engine-env.sh npm run check:engine:source
sh scripts/engine-env.sh npm run check:engine -- --plan
```

Bootstrap selects existing pinned tools, installs minimal locked Node dependencies
and runs doctor. The environment launcher supplies PATH/LD_LIBRARY_PATH to commands
and their children. See [setup and command choices](../../docs/game-engine-development.md).
The default engine check deliberately omits runtime/full-matrix work; choose
focused additional checks for the behavior you change.

| Need | Read |
| --- | --- |
| Rules, privacy, state/recovery invariants | [runtime contracts](../../docs/game-engine-invariants.md) |
| Exact source records, catalog generation and generic CLI | [catalog workflow](../../docs/game-engine-catalogs.md) |
| Legal decks, hosting/search admission | [profile data](../../docs/game-engine-profiles.md) |
| Structural resource limits | [capacity policy](../../docs/game-engine-capacity-policy.md) |
| Local host and generated SDK | [server manual](../game-server/AGENTS.md) |
| Focused checks and shared native lock | [verification workflow](../../docs/game-engine-verification-workflow.md) |
| Tiny runtime probe and bounded replay artifact | [smoke](../../docs/game-engine-smoke.md) |
| Branch/worktree preservation and checkpoints | [lifecycle guide](../../docs/game-engine-worktrees.md) |
| Release evidence, prior scenarios and old onboarding | [historical index](../../docs/history/engine/index.md) |

Accepted releases and pending candidate work have different source boundaries.
Historical reports retain their exact identities and limitations; they do not
establish acceptance of the current checkout. Bulk `npm run build-ir` remains
unimplemented even though source-backed catalog compilation works.
