# Hosted attachments profile

`commander-attachments-v1` and `commander-foundations-v1` are the reviewed hosted
profiles. This is admission scope, not a profile-specific rules implementation.
The registry discovers strict profile data and validates its `hosted` request
against reviewed capabilities, whole-pod construction and exact catalog/builds.
Other legal headless profiles remain rejected for hosting. No limit is relaxed.

## Contract

- Rust protocol DTOs embed core Observation/PlayerRequest/PlayerResponse. Generated
  SDK schemas/types and its exhaustive response consumer must cover every reachable
  variant; do not maintain parallel handwritten wire definitions.
- Authenticated seats see only their permitted projections/history. Search results,
  hands and libraries are private to their viewer; public attachments, granted
  keywords, counters and tokens remain permitted projections. Queries do not mint handles.
- The SDK example completion policy may consume only its seat observations and
  public manifest/catalog. It is not a browser control implementation or strategic
  acceptance. Core validates every response.
- Identity binding, credentials, clocks, queue/resource bounds, immutable retry
  dispositions, acceptance-before-acknowledgment and exact recovery retain the
  [common host contract](game-engine-server.md).

## Evidence and future changes

The historical P8.2 procedure required one real four-client default pod under the
unchanged 25-minute watchdog, natural typed win/draw at all clients, zero retries/
CoreConcede, final state/input/turn/memory records, and completed-game recovery
within 120 seconds with durable tables/head unchanged except generation. Worker
limits remained 4 GiB, swap zero, tasks 32. It also exercised registry/recovery/
strict decoding/search privacy and SDK types/variants. Historical proof locators
are in [engine evidence](game-engine-evidence.md); this is not a fresh acceptance run.

For a host/API edit choose focused relevant server/SDK tests under
[verification policy](game-engine-verification-workflow.md). A resource/time failure
remains evidence, never grounds to weaken limits. No server-side search, other
profile hosting, deployment/lobby/spectators or complete browser play is implied.
