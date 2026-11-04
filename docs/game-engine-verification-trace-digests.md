# Trace verifier digest reuse — staged for combined acceptance

Production source `f9a051530077029456e82889709e602fc933382f` passed independent
source review and is staged at `f0be4ac47325b7f29b9035459cb999f1ec3217ed`.
Only the production `main.rs` change is integrated. The diagnostic harness is
excluded. Full acceptance on the next reviewed combined freeze remains pending;
accepted `394301d` does not cover these changed verifier bytes.

The verifier computes each immutable expected Game digest once and lazily caches
the reference digest with the exact three-mode group Game. The cache resets when
that reference is replaced. Mode guards, fallible short-circuit order, all
comparisons, and independent restore/replay remain. The normal full workload
still requires all 540 trace executions. Static successful-loop accounting removes
900 explicit calls (2,160 to 1,260); transitive hashing and total runtime work are
outside that count.

The source-reviewed feature-only component harness uses four real initial and
resolved land-return/stack-response states and twelve production-generated trace
runs. Forward/reverse independent snapshots, traces and journals agree. All 51
malformed-input results match exactly, including three envelope-decode failures
before component execution. The representative successful component records 48
versus 28 explicit calls. Six alternating paired observations are retained;
they establish no production-verifier, full-gate, quiet P7 or overlap-budget
result. These copied component functions do not execute the complete production
`verify_bundle` proof. Final acceptance must run that unchanged proof path.

The original `650e786ce63f40f3d985592ef8453b1e339f6264` attempt passed phases
01–11: normal/feature compilation, exact discovery and existing reuse execution,
and all nine bounded native phases including equivalence and timing. Each native
phase used the unchanged helper at 2 GiB cgroup memory plus a separately checked
native RSS guard, zero swap, 128 tasks, one fixed physical CPU and a 120-second
total deadline. Applied limits, full outputs, identities and empty/reaped cleanup
passed. Clippy then stopped on two test-only styles; formatting was unrun.
That stopped status is preserved.

Parent commit `9e2c4e843c93f156da0838656282154fd3b87a15` changes only the
UTF-8-safe assertion byte-slice spelling and the nonzero-divisor `is_multiple_of`
spelling. Production code and copied component bodies are unchanged. A separately
authorized completion passed only feature Clippy and formatting on those new
bytes. No native diagnostic, timing, discovery, or successful gate was repeated.
Original runtime passes remain bound to the original source/binaries; this is
not a single-freeze focused acceptance claim.

The parent independently hashed the original 445 retained files (653,939,475
bytes), checked all 1,393 Git archive members, all nine native envelope/output
proofs, exact rejection results and complete forward/reverse receipts. It also
hashed the 28-file completion (691,179 bytes) and verified its exact two commands,
exit status and unchanged source. Parent reader corrections for component-result
field names and pre-component decode receipts are preserved separately; no engine
work was rerun.

Original read-only proof:
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/trace-digest-f9a0515-650e-stopped-20261104T033217Z/`
Manifest SHA256 `349698e2e620037b769015d54067ea0391baded887e6f71b19098048c5ec089a`.
Feature test SHA256 `3b6620da30229a193d1a2fe241402d70cc4e996a631f51eccd10c7cb47bb3d7a`;
release source identity `e969645fd09866b754ad9fdf0d522deeb76771f8ea1bc9b68195564a0be14a44`.

Separate read-only completion:
`/home/josh/.local/share/mtg-ecosystem/engine-evidence/trace-digest-9e2c4e8-lint-completion-20261104T033728Z/`
Manifest SHA256 `14c3f6d392d01733e1f794f5b5f016580946d7c30774680965f2ad688ae5357c`.
