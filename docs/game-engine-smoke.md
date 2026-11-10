# Tiny engine runtime smoke and bounded reproduction

`mtg-sim smoke` is a focused developer check, not engine acceptance. It compiles
five exact archived source fixtures into the existing base catalog and admits
`ScenarioConfig::land_return()` (three physical cards, four seats). It activates
Island's mana ability and passes priority through public observations, decision
tokens, response validation and submission. It asserts the initial state, then
Island tapped with one blue mana, then priority at player 1. Each input prefix is
replayed through the existing engine journal API and compared by semantic digest.
Stale public tokens must reject without mutating state. Trace retention is off.

Run the current executable, with the pinned toolchain runtime libraries available:

```sh
/path/to/current/mtg-sim smoke
/path/to/current/mtg-sim smoke --artifact /new/path/smoke.json
/path/to/current/mtg-sim smoke-replay /new/path/smoke.json
```

Build via the coordinated Cargo workflow (`cargo build --locked -p mtg-sim`);
do not mistake another worktree's cached binary for the current source. Focused
Rust tests are `cargo test --locked -p mtg-sim smoke::tests`. Standalone Cargo
commands must acquire the shared lock documented in
[the verification workflow](game-engine-verification-workflow.md).
The coordinator owns npm/check-runner integration.

The default writes only a small JSON result to stdout; failure returns nonzero.
The result includes an exact shell-quoted rerun command with working directory
and runtime library path. For an existing saved artifact, use `smoke-replay`
rather than repeating its create-new write command.
The optional artifact preserves both successful and failed probes, including
partial journals/checkpoints when ordinary errors occur. It uses create-new
semantics: existing files and symlinks are never replaced, and missing parent
directories are errors. A write failure remains a failure; it cannot mask a failed
probe as passed. Input and output artifacts are limited to 65,536 bytes. A process
panic or kill can prevent artifact creation; this is not a crash recorder.

Artifacts are explicitly **trusted host diagnostics, not player payloads**.
They contain raw journal identities, a version, a fixed scenario name, null seed
(no random setup), core build/toolchain settings, smoke/CLI source digests,
source-manifest/record digests, catalog/configuration digests, and three tiny
checkpoints. They do not include a source archive, binary, snapshots, hidden
libraries, a full trace, or a source-retention closure. Preserve the corresponding
source and executable separately for long-term reproduction.

Replay reruns the fixed probe and its ordinary engine journal replays, then
compares the saved identity, input journal, checkpoints and failure. It reports
one stable JSON pointer with bounded expected/actual values. Pathological long
field names are truncated and marked explicitly. Input and checkpoint
indices are zero-based; `after_inputs` counts accepted decisions. A different
build is reported as an identity mismatch before state comparisons. Saved
arbitrary journals are never executed: this tool only reproduces this fixed
scenario. A changed internal state digest does not reveal which internal state
field changed, and no universal mismatch locator is claimed. A saved failing
probe remains a failing command even if its failure reproduces exactly.

`runtime_us` measures time inside the CLI, including compilation of source data,
admission, public queries, semantic digest/replay work and optional artifact IO.
It excludes process startup and Cargo compilation. Measure wall-clock process
startup/run separately from Cargo build and from warm repeats. No arbitrary
latency threshold, full game completion, match profile, search, server, SDK,
release matrix or retention rebuild is part of this check.
