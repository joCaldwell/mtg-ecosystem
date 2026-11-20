# Client and product acceptance evidence

[Client contracts](game-client.md) own behavior;
[application launch gates](../packages/multiverse/web/ARCHITECTURE.md#game-launch)
own launch. Evidence binds exact producer, source and schema, not moving HEAD.
Later hosts, producers or consumers need fresh proof.

## Active baselines

| Baseline | Exact proof root |
| --- | --- |
| Observation 21 native producer `2305e2f8` | [Engine evidence](game-engine-evidence.md#combined-client-native-producer-observation-21) |
| Metadata-delivery SDK bridge (SDK 69 extends 66; observation 21 builds and schema unchanged) | `~/.local/share/mtg-ecosystem/engine-evidence/product-presentation-sdk-20261115/` |
| Application boundary extraction `de785a97` (relocation bridge and Native21 bootstrap/proxy/retry/lease witness) | `~/.local/share/mtg-ecosystem/engine-evidence/game-application-boundary-20261116/` |

## Hosted completion

Chromium `774cad38` finished a foundations game in 38.9 minutes under 240
Actor requests/minute with zero 429s, 935 consumed/reconciled commands and
three joined bot leases. The visible owner win matches native ledger outcomes
for all four seats; the browser did not directly observe the native outcome.
Only the tested release identity is approved. Evidence:
`product-shared-budget-completion.potzlXV6/completion-evidence.json`
SHA-256 `ec06be154b07eef380499fba4319a06fe179694d233436657ae4549d2438f814`.

## Core action discovery evidence

Native source `d1d039539d63aa17cd411c654e22086ccf3c9322` retains
`generated/core-queries/attempt02/`; the
[immutable core record](https://github.com/joCaldwell/mtg-ecosystem/blob/33dcbe9df5c0cc7fef5f8d67f97adb85a3af4cdb/docs/game-client-core-queries.md#executed-checks-and-retained-evidence)
owns commands, closure, hashes and preserved failures. Its schema is not SDK input.

## Historical records

Earlier producer-context, opening/recovery, observation 16 lifecycle, shared
display, live-discovery, frontend and hosted-prefix records keep their exact
identities, failures and proof roots in the
[immutable client evidence](https://github.com/joCaldwell/mtg-ecosystem/blob/13704bf5781362a4fcf51848dc8a2f3e398fd87b/docs/game-client-evidence.md).
The [immutable transport/delivery reports](https://github.com/joCaldwell/mtg-ecosystem/tree/1c3eca4c37b90de02decca316fefab5c8035140e/docs)
and [package reports](https://github.com/joCaldwell/mtg-ecosystem/tree/1c3eca4c37b90de02decca316fefab5c8035140e/packages/game-client)
retain earlier prerequisites. Raw evidence stays preserved.
