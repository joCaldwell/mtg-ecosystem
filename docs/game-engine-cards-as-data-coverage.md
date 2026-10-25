# Cards-as-data corpus coverage

D2a measures complete-card lowering and IR validation, not Commander deck
admission or complete rules support. Every count uses the record's own
`source.legalities.commander == "legal"` value.

Run from the repository root with the pinned engine toolchain:

```sh
T=/home/josh/.local/share/mtg-ecosystem/engine-toolchains/rust1990-node26111-v1/usr
export PATH=$T/bin:$PATH LD_LIBRARY_PATH=$T/lib
cargo run --locked --release -p mtg-sim -- catalog coverage \
  /home/josh/.local/share/mtg-ecosystem/engine-evidence/archives/oracle-ast-d705222b280a71cb.jsonl \
  --out generated/cards-as-data-coverage/d2a/final
```

The accepted-archives data must pin both the exact manifest and full archive
SHA-256. Unknown archives and changed bytes produce typed errors. No manifest
substitution occurs. The input is read-only. Output defaults to
`generated/cards-as-data-coverage/` if `--out` is omitted.

`summary.json` reports all-record and Commander-legal totals, the legal lowered
count/percentage, legal status counts and descending rejection counts (ties by
reason then status). The top 30 reasons include up to five distinct card names,
sorted alphabetically. `records.tsv` classifies **every** archive record, including
nonlegal cards; its columns are oracle_id, name, status, reason. Backslash, tab,
CR and LF are escaped. Summary counts include only legal records.

Statuses are `lowered_validated`, `unsupported`, `invalid`, and `unparsed`.
Reasons identify an AST discriminator (`trigger:phase`, `effect:gain-abilities`,
`keyword:ward`) or a compiler field path and specific boundary. Field-path
reasons normalize diagnostic punctuation to underscores; serde locations and
expected-variant lists are excluded. These codes describe the first failed
boundary, so fixing one can expose another without admitting a card. Unparsed
means the archive does not contain a complete successful card parse. Unknown
semantics are always rejected for the entire card.

## D2a baseline

Measured on base `317e759` with the committed command, before intake fixes:
38,721 records; 32,116 Commander-legal; **1,719 lowered and validated (5.35247%)**
at IR v20. Legal statuses: 24,694 Unsupported, 3,222 Invalid, 2,481 unparsed.
This reproduces the audit's 1,719 result without substituting a header: D1a
already accepted the current archive through data. Accepted-archive fix delta: 0.

Archive SHA-256:
`d705222b280a71cb5a5362f657d530b22402ca51687bac3227c1d6db403d0282`;
parser revision `327b3360ea8e5cffb75b136223e75a348e903cbe`.
Raw baseline: `generated/cards-as-data-coverage/d2a/00-baseline/` (ignored).
