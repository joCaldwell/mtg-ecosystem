# Verification runtime audit and optimization plan

Requested by Josh on 2026-11-03. This audit reads the settled D2c-b phase records,
not a new performance benchmark. Source freeze is
`02f0d4df92343be43241065b916cbcfe7cfd2cc9`; the immutable baseline is retained at
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/d2c-b-02f0d4df9234-20261103/`.

## Measured wall time

| Phase | Wall time |
| --- | ---: |
| Full engine check | 37m 19.5s |
| Release/build/SDK/identity/coverage | 2m 55.4s |
| Twelve batches and 216 explicit replays | 44m 11.1s |
| Successful path total | 84m 26.0s |

Inside the full gate, Rust suite execution sums about 23m17s (core 7m55s,
simulator 14m34s). Node takes 12m07s, of which the single shared release demo takes
12m02s. SDK verification alone takes 4.9s. The first failed full gate spent 38m06s
before exposing a cheap nested-schema error, then required a corrected full run.

The logs record child elapsed time but no user/system CPU or peak RSS. They
therefore locate execution windows but cannot precisely distinguish CPU, I/O or
other blocked time. More than 96% of the successful path is in the large runtime
phases; compiler and orchestration micro-optimizations alone will not solve it.

## Concrete repeated work

1. `sim/src/match_games.rs::verify_replay` constructs and checks a Detailed Game.
   The batch then calls `interactions::game`, which replays the same journal again
   only to scan its trace. Return an internal verified result and reuse its owned
   Game for reporting. Retain checkpoint, loss, survivor, final digest, terminal
   and standalone CLI replay checks. Preserve diagnostics on failing evidence.
2. `sim/src/main.rs::trace_runs` serializes the same unchanged snapshot three
   times per execution: restore, byte count and retention. There are 540 trace
   executions. Cache immutable bytes/digest while preserving every equality and
   recovery check.
3. Cheap Node schema checks after the first demo-backed case wait about12 minutes
   for that demo, and the entire Node phase follows the expensive Rust suite.
   Introduce an explicit cheap preflight before runtime work. Keep all acceptance
   checks and their complete normal counts.
4. `match_games::suite` verifies 18 games again after `batch` verified them; bundle
   verification later runs its own required independent proof. Investigate typed
   process-local verified-result reuse separately, preserving aggregate loss
   checks and standalone bundle verification.

Construction across 216 games sums 20m38s of elapsed time. Batch commands sum 60m16s,
and explicit replays sum 20m44s. The batch is close to construction plus two
comparable replay executions. Avoiding the reporting-only replay could save
roughly 10 minutes wall at two workers; this is an estimate requiring A/B testing.

## Next measured iterations

- Instrument wall/user/system CPU, correctly labeled RSS, exit status, source and
  executable identity, and lock waiting. Do not call a sampled sum or maximum
  child RSS a kernel process-tree peak.
- Use representative attachments/removal/wipes evidence for quick paired A/B
  checks, then one full gate on the final independently reviewed freeze.
- Compare complete interaction/site summaries, accepted inputs, every manifest
  field except build identity, outcomes/winners, state hashes, all deterministic
  counters and required explicit replay outputs. Preserve historical references.
- Measure peak memory before increasing the current two runtime workers. This
  host reports 32 logical CPUs and about 30.4GiB RAM; that alone is not a measured
  concurrency budget. Longest-first scheduling predicts about 40m32s versus 44m11s;
  four workers predict about 21m21s. Both are scheduling estimates, not measured
  gains.
- A source-verified prebuilt release CLI can remove repeated `cargo run` overhead
  and enable Node runtime checks to overlap Rust tests after compilation. First
  audit shared outputs, cancellation and measured memory. Preserve both debug and
  release evidence and keep Cargo compilation serialized.

Implementation agents now use focused checks and source review before one full
acceptance owner, as documented in
[the workflow](game-engine-verification-workflow.md). A separate optimization
thread will repeat profile/implement/review/measure cycles. Its independent
subtasks can run concurrently; their expensive final verification must not be
duplicated. No measured optimization gain is claimed by this baseline audit.
