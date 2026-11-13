# Damage source identity during compilation

The bounded spell/ability damage instructions execute using the invocation's
source. A triggering creature, selected sacrifice or newly created token can be
a different object. Lowering must prove the source reference matches the object
the instruction will use before admitting the complete program.

The source-binding boundary is:

| Context | Supported source proof |
| --- | --- |
| Ordinary spell, including every modal option | The existing explicit `self` spell reference |
| Activated ability | Existing explicit source forms; `it` only as the first top-level effect step after an exact self-sacrifice cost, without sentence bindings |
| Triggered ability | Existing explicit source forms; first-step `it` only for exact self entry, death, attack or damage events, without sentence bindings |
| Other event/cost/result pronouns, later steps or modal pronouns | Explicit unsupported-case rejection until their object binding and execution are implemented |

These are source-proof requirements within the existing opcode and context
constraints. Proving a source reference does not admit an otherwise unsupported
operation. Every modal branch is inspected recursively; entering one does not
inherit a proof for a pronoun. The Card IR schema remains 1, with unchanged
capabilities and runtime damage-source semantics.

Two coherently rebound synthetic source mutations demonstrate the boundary:

- A Goblin Bombardment mutation changes its effect's source to `it`. The actual
  parser/binder binds that pronoun to the selected sacrificed creature. Emitting
  observer-source damage would attribute the damage to the enchantment.
- A modal Healing Salve mutation creates a token and makes `it` deal damage.
  The parser/binder identifies the created token as the source. A shallow check
  of the root steps misses that nested effect.

Both mutations were passed through the ordinary native compiler API before the
correction and returned `Lowered` with invocation-source damage instructions.
Their exact inputs, parser results, reference graphs and native results are
retained under
`~/.local/share/mtg-ecosystem/engine-verification/correctness-loop-20261113/activated-damage-source-native-probe-01/`.
They are synthetic rejection witnesses, separate from archived card facts and
catalog membership. Existing explicit source references and Mogg Fanatic's
first-step self-sacrifice damage retain their supported meaning.

The pinned 2026-09-25 Comprehensive Rules establish the source/object distinction:
CR 113.7a, 120.2b, 120.7 and 608.2h. The retained rules SHA-256 is
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`;
the [official rules text](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt)
is the authority. The earlier [trigger-source correction](game-engine-filtered-damage-sources.md#current-compiler-source-binding)
uses the same source-identity principle.

Focused validation passed all 135 compiler library tests, including the four new
regressions and freshness of every committed catalog, plus formatting, the
structure guard and production compiler Clippy. Actual parser/binder reruns
reproduced the retained negative and explicit-self source proofs byte for byte.
The original Bombardment, Mogg Fanatic, Fling and Siege-Gang Commander definitions
still match their committed catalog definitions. No card source, expected IR,
catalog digest, capability, runtime or public schema changed.

Passing receipts and the complete captured proofs use `activation-source-*` under
the same external verification directory. Runtime/replay, server/SDK/browser,
full source-tooling and release/profile matrices were omitted for this
compiler-only correction. Existing runtime and consumer producers retain their
own exact source qualification.
