# Ramos revision-17 source support

This is the single per-definition matrix and dependency backlog for
[the Ramos plan](game-engine-ramos-plan.md). M0 freezes Jo's Cascade, deck 11,
revision 17/edit version 0, unchanged: **85 definitions, 100 cards, 37 lands**.
The LF export with final newline hashes to
`f9eb5bc8ea479e036fc24c3fba27383ee2f030191d1a689de80bdb30e2278ee1`.
There is no discrepancy against the plan's digest, card count or land count.

## Frozen authority and executable identity

- Main: `bbdba9f5add0bfd0c75c0ec18eb2986581c3e534`.
  Audit source/base: `46ef08faedb8c5fa9c1c1fa9a8afefd1f85e1f6c`, its planning-only child.
  No engine production code changed in M0.
- Debug core BUILD_ID:
  `770125c36502febc3a1f1276094666b34109809e32a4429673a8e7e342aad619`.
  Rust 1.99.0, x86_64-unknown-linux-gnu, opt-level 0, debug assertions enabled.
  This agrees with the integrated code identity; it is not a new acceptance gate.
- The built `mtg-sim` binary SHA-256 is
  `ed810294f8c395ced02960d468ce9e844169d6d817d3c52f40e219de5951c4ef`.
  The copied Cargo build output and metadata are retained with the raw evidence.
- [Target manifest](../packages/game-engine/tests/fixtures/ramos/revision-17.manifest.json)
  pins every Oracle ID, selected printing ID, quantity, commander designation,
  archive line, exact fixture bytes, source/AST/binding hashes and milestone.
  Null AST hashes mean a complete successful parse is absent, never an empty AST.
- The accepted retained archive is
  `/home/josh/.local/share/mtg-ecosystem/engine-evidence/archives/oracle-ast-d705222b280a71cb.jsonl`,
  SHA-256 `d705222b280a71cb5a5362f657d530b22402ca51687bac3227c1d6db403d0282`.
  Its [unchanged header](../packages/game-engine/tests/fixtures/ramos/archive-manifest-current.json)
  pins corpus, paper-printing evidence and parser
  `327b3360ea8e5cffb75b136223e75a348e903cbe`. The local
  [accepted-origin pins](../packages/game-engine/tests/fixtures/ramos/accepted-archives.json)
  and [provenance](../packages/game-engine/tests/fixtures/ramos/provenance.json)
  follow the existing catalog format; no main fixture registry was expanded.
- Rules: 2026-09-25, SHA-256
  `8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`.
  Legality is the **frozen record's** `source.legalities.commander`; all 85 say
  `legal`. The existing 2026-10-15 Commander banned policy is pinned by file hash
  in the manifest. These are historical input identities, not a fresh live
  legality ruling or an executed deck/pod admission check.

The discovery-app comparison is **unverified**. The connected read attempt
`mtg_deck_builder.start_session` returned `UNAUTHORIZED`, reauthentication
required (`oauth_refresh_token_missing`). No discovery text, live export or new
revision was read, and no deck mutation occurred. Future reconciliation must
compare discovery text with these frozen fixtures, record any differences, and
keep revision 17 fixed unless a new target is explicitly authorized.

The [M3a announcement/payment foundation](game-engine-ramos-casting-contract.md#m3a-announcementpayment-foundation-basic-cascade-still-rejected)
changes shared ordinary casting/payment boundaries and the 100-card sampler,
with no source admission. Shardless Agent and Bloodbraid Elf remain the same
whole-source rejections in the matrix; basic cascade and the full M3 exit are open.

## Measured baseline and matrix

The production `catalog extract` and `catalog verify` commands verified all 85
records, including original line terminators, against the complete accepted
archive hash. All records contain frozen source. There are 78 complete parses,
7 parser-stage failures, 68 `fullyBound` bindings and 17 incomplete bindings.
Sixty-nine report `referencesBound`; that is a separate archive fact.

A single locked audit pass invoked production `catalog build --write` on one
singleton per definition. **11 lowered and passed catalog/capability validation;
74 returned Unsupported; 0 returned Invalid or an unexpected catalog error.**
Seven of the 74 stop at `status` because parsing failed. A first rejection is
not an exhaustive mechanic inventory: fixing it can expose later obligations.
There is no executable catalog digest for the full target.

[The compact immutable compiler baseline](../packages/game-engine/tests/fixtures/ramos/baseline-46ef08fa.json)
preserves every diagnostic's exact path and reason, each successful singleton's
capabilities and catalog digest, executable identity, and raw receipt hash.
The unmodified stdout/stderr and command per definition remain in
`generated/ramos/m0-46ef08fa/compiler/results.json` (SHA-256
`3e5a036fe0a3ccfc4dd535e7f88853c16c45f3d18b4871f7965e39405d30aa28`).
Source links below identify the exact per-card Oracle text, AST and complete
binding diagnostics; the manifest supplies all identity hashes.

Matrix legend: source **V** = archive-byte verified; parse **P/U** = complete
parse/parser unsupported; binding **F/I** = archived fullyBound true/false;
lowering **L/U** = complete lowering/Unsupported; capability **V/—** = executable
catalog validation passed/not reached (M0 singletons; M2 source-witness catalog).
Scenario **R** = the bounded M1a/M1b private paid source witnesses (not admitted pods),
qualified in the [M1 contract](game-engine-ramos-m1-contract.md#bounded-m1b-counter-placement-family);
**NR** = no per-source runtime witness; **S** = the bounded M2
private paid source witness, qualified in the
[M2 contract](game-engine-ramos-m2-contract.md#bounded-basic-search-checkpoint).
Admission **—** = exact target not admitted; consumer **—/—** =
exact target not hosted/search-enabled. Existing other pools are not requalified
by these rows. Rows retain M0 evidence except the three R rows and two S rows, which add the
separately qualified M1a/M1b/M2 lowering/capability/runtime checks. The immutable M0
baseline and its 11/74 counts are unchanged; no full target re-audit or executable
target catalog is claimed. The owner milestone remains the completion/witness owner.

| Definition × quantity | Source | Parse | Binding | Lowering | Capability | Scenario | Admission | Host/search | Owner | First diagnostic reason |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| [Ramos, Dragon Engine (commander)](../packages/game-engine/tests/fixtures/ramos/3ed41d2d-211b-4013-8562-8c64d54cc43a.json) ×1 | V | P | F | L | V | R | — | —/— | M1 complete; admission separate | `—` |
| [Abzan Charm](../packages/game-engine/tests/fixtures/ramos/4137a22c-f793-4e27-a9b2-72740a6e2121.json) ×1 | V | P | I | U | — | NR | — | —/— | M6 | `effect:distribute-counters` |
| [Akroma's Will](../packages/game-engine/tests/fixtures/ramos/fd949f82-fc10-4e37-8aa9-6c7569fe3c55.json) ×1 | V | P | F | U | — | NR | — | —/— | M7 | `abilities[*].effects[*].steps[*].conditionalCount` |
| [Anguished Unmaking](../packages/game-engine/tests/fixtures/ramos/ad09b3c3-c8e7-481c-8c45-e7f234935117.json) ×1 | V | P | F | L | V | NR | — | —/— | M7 | `—` |
| [Apex Devastator](../packages/game-engine/tests/fixtures/ramos/b4d6747c-3516-4e71-be30-af007c6b1dc4.json) ×1 | V | P | F | U | — | NR | — | —/— | M4 | `keyword:cascade` |
| [Aragorn, the Uniter](../packages/game-engine/tests/fixtures/ramos/3f31c423-d45c-4c3e-b7d4-ca125be0f92c.json) ×1 | V | P | F | U | — | NR | — | —/— | M6 | `abilities[*].trigger.colors` |
| [Arcane Signet](../packages/game-engine/tests/fixtures/ramos/0bc7f093-bef0-4f1a-852c-4b75ebf54838.json) ×1 | V | P | F | L | V | NR | — | —/— | M2 | `—` |
| [Avacyn, Angel of Hope](../packages/game-engine/tests/fixtures/ramos/216cb26e-8da9-478b-bfbc-8030f7adee72.json) ×1 | V | P | F | U | — | NR | — | —/— | M7 | `unsupported modifier scope` |
| [Averna, the Chaos Bloom](../packages/game-engine/tests/fixtures/ramos/23695bc2-86d3-4d78-b374-9e83c4cc9cc4.json) ×1 | V | U | I | U | — | NR | — | —/— | M4 | `complete-card parsing is required` |
| [Bloodbraid Elf](../packages/game-engine/tests/fixtures/ramos/3f0c9466-5ab9-4205-a84f-b4b27b5a678e.json) ×1 | V | P | F | U | — | NR | — | —/— | M3 | `keyword:cascade` |
| [Breeding Pool](../packages/game-engine/tests/fixtures/ramos/20283c4a-f1f0-42f0-bc08-6da87474426b.json) ×1 | V | P | F | U | — | NR | — | —/— | M2 | `static:entry-choice` |
| [Breya, Etherium Shaper](../packages/game-engine/tests/fixtures/ramos/d460a9e2-5a7d-4562-880e-45174be19a9d.json) ×1 | V | P | F | U | — | NR | — | —/— | M10 | `unsupported token characteristic shape` |
| [Call Forth the Tempest](../packages/game-engine/tests/fixtures/ramos/0623c51a-e829-413f-a3a0-817c0902821e.json) ×1 | V | U | I | U | — | NR | — | —/— | M4 | `complete-card parsing is required` |
| [Child of Alara](../packages/game-engine/tests/fixtures/ramos/70dbe8a9-505d-41c2-9b5b-a991d13ab459.json) ×1 | V | P | F | U | — | NR | — | —/— | M7 | `regeneration-forbidden mass destruction is outside this slice` |
| [Chromanticore](../packages/game-engine/tests/fixtures/ramos/695ee4fe-356d-47fb-9442-d171b618151c.json) ×1 | V | P | F | U | — | NR | — | —/— | M9 | `keyword:bestow` |
| [Chromatic Lantern](../packages/game-engine/tests/fixtures/ramos/539f5396-d99a-417d-a84c-dff7930b5900.json) ×1 | V | P | F | U | — | NR | — | —/— | M5 | `unsupported modifier scope` |
| [Cinder Glade](../packages/game-engine/tests/fixtures/ramos/dfac0258-e148-4d7d-8ded-fc2466d9caa6.json) ×1 | V | P | F | U | — | NR | — | —/— | M2 | `static:enters-tapped:conditional` |
| [City of Brass](../packages/game-engine/tests/fixtures/ramos/f25351e3-539b-4bbc-b92d-6480acf4d722.json) ×1 | V | P | F | U | — | NR | — | —/— | M2 | `trigger:becomes-tapped` |
| [Command Tower](../packages/game-engine/tests/fixtures/ramos/0895c9b7-ae7d-4bb3-af17-3b75deb50a25.json) ×1 | V | P | F | L | V | NR | — | —/— | M2 | `—` |
| [Conflux](../packages/game-engine/tests/fixtures/ramos/32ebb029-9f03-49c9-b9bb-cb1954e2a324.json) ×1 | V | P | I | U | — | NR | — | —/— | M6 | `effect:reveal` |
| [Corpsejack Menace](../packages/game-engine/tests/fixtures/ramos/ca0cc02b-b106-4eca-9388-d4b48dd3be49.json) ×1 | V | P | I | L | V | R | — | —/— | M1b complete; admission separate | `—` |
| [Creative Technique](../packages/game-engine/tests/fixtures/ramos/c336b735-3e6b-471e-be7a-84d7402ae2f5.json) ×1 | V | U | I | U | — | NR | — | —/— | M8 | `complete-card parsing is required` |
| [Cultivate](../packages/game-engine/tests/fixtures/ramos/8b755881-a72d-4e21-a369-d2924eb4585a.json) ×1 | V | P | F | L | V | S | — | —/— | M2 | `—` |
| [Dauntless Escort](../packages/game-engine/tests/fixtures/ramos/c0516c2c-d7c9-4763-9180-980f60205a28.json) ×1 | V | P | F | U | — | NR | — | —/— | M7 | `unsupported activated operator, controller or cardinality` |
| [Door to Nothingness](../packages/game-engine/tests/fixtures/ramos/b919cca5-88c0-4104-b54e-49644fbf78a5.json) ×1 | V | P | F | U | — | NR | — | —/— | M10 | `effect:lose-game` |
| [Double Major](../packages/game-engine/tests/fixtures/ramos/ece44a82-dcf0-4439-bdd9-a09c99a6f159.json) ×1 | V | U | I | U | — | NR | — | —/— | M8 | `complete-card parsing is required` |
| [Enigma Sphinx](../packages/game-engine/tests/fixtures/ramos/6fb84ecd-ab60-4c2c-987d-8a0749db5777.json) ×1 | V | P | F | U | — | NR | — | —/— | M4 | `keyword:cascade` |
| [Esper Charm](../packages/game-engine/tests/fixtures/ramos/d40ea3d9-8c48-4846-b1f4-99018d4fd789.json) ×1 | V | P | F | U | — | NR | — | —/— | M6 | `unsupported complete spell operator, destination, chooser or cardinality` |
| [Ethersworn Sphinx](../packages/game-engine/tests/fixtures/ramos/71366469-04ea-4526-bf24-7abc02fdc5ab.json) ×1 | V | P | F | U | — | NR | — | —/— | M4 | `keyword:affinity` |
| [Exotic Orchard](../packages/game-engine/tests/fixtures/ramos/27b047e3-0d41-45e2-98e9-9391d7923a1e.json) ×1 | V | P | F | U | — | NR | — | —/— | M2 | `mana:could-produce` |
| [Exterminatus](../packages/game-engine/tests/fixtures/ramos/8d90ffb5-63da-48d5-a5a2-7b61062556f6.json) ×1 | V | P | F | U | — | NR | — | —/— | M7 | `effect:lose-abilities` |
| [Faeburrow Elder](../packages/game-engine/tests/fixtures/ramos/70a6f08e-854d-4e2f-9d8c-c45ec3231157.json) ×1 | V | P | I | U | — | NR | — | —/— | M5 | `mana:each-color-among` |
| [Farewell](../packages/game-engine/tests/fixtures/ramos/4eb813fd-2d5a-4b02-8193-662681ef4e7d.json) ×1 | V | P | F | U | — | NR | — | —/— | M7 | `abilities[*].effects[*].steps[*].atLeast` |
| [Farseek](../packages/game-engine/tests/fixtures/ramos/495e52e6-4c2b-4574-9474-eadbdcc8b4ac.json) ×1 | V | P | F | U | — | NR | — | —/— | M2 | `unsupported complete search/move/shuffle sequence` |
| [Fellwar Stone](../packages/game-engine/tests/fixtures/ramos/95560508-7ac9-4be9-8a3f-3c7d5b52807b.json) ×1 | V | P | F | U | — | NR | — | —/— | M2 | `mana:could-produce` |
| [Forest](../packages/game-engine/tests/fixtures/ramos/b34bb2dc-c1af-4d77-b0b3-a0fb342a5fc6.json) ×6 | V | P | F | L | V | NR | — | —/— | M2 | `—` |
| [Golgari Signet](../packages/game-engine/tests/fixtures/ramos/1cf51f50-24e4-48d0-95b3-1dad3ffa4bf5.json) ×1 | V | P | F | L | V | NR | — | —/— | M2 | `—` |
| [Hallowed Fountain](../packages/game-engine/tests/fixtures/ramos/f1750962-a87c-49f6-b731-02ae971ac6ea.json) ×1 | V | P | F | U | — | NR | — | —/— | M2 | `static:entry-choice` |
| [Hardened Scales](../packages/game-engine/tests/fixtures/ramos/a1f3da21-af6d-450e-bf0b-985d158418e6.json) ×1 | V | P | I | L | V | R | — | —/— | M1b complete; admission separate | `—` |
| [Imoti, Celebrant of Bounty](../packages/game-engine/tests/fixtures/ramos/eca32dcd-6845-433e-a631-ed1f0ee78f25.json) ×1 | V | P | F | U | — | NR | — | —/— | M4 | `keyword:cascade` |
| [Island](../packages/game-engine/tests/fixtures/ramos/b2c6aa39-2d2a-459c-a555-fb48ba993373.json) ×2 | V | P | F | L | V | NR | — | —/— | M2 | `—` |
| [Ketria Triome](../packages/game-engine/tests/fixtures/ramos/6bae00e8-06cf-4ac4-a1cc-757e454109fe.json) ×1 | V | P | F | U | — | NR | — | —/— | M2 | `keyword:cycling` |
| [Kodama's Reach](../packages/game-engine/tests/fixtures/ramos/1593ea18-2f2f-4ab4-83fb-6ccc0bec8a90.json) ×1 | V | P | F | L | V | S | — | —/— | M2 | `—` |
| [Leyline of the Guildpact](../packages/game-engine/tests/fixtures/ramos/6b2e6a51-b9e6-4848-b7d7-98f02f2b897f.json) ×1 | V | P | F | U | — | NR | — | —/— | M5 | `static:opening-hand-action` |
| [Maelstrom Archangel](../packages/game-engine/tests/fixtures/ramos/ac50ef98-1791-4dd4-9c1f-8cea7db7ade5.json) ×1 | V | P | F | U | — | NR | — | —/— | M10 | `effect:play-card` |
| [Maelstrom Nexus](../packages/game-engine/tests/fixtures/ramos/b1c346a4-fc9b-474a-9d51-cac168e363fa.json) ×1 | V | P | F | U | — | NR | — | —/— | M4 | `static:cast-spell-abilities` |
| [Maelstrom Wanderer](../packages/game-engine/tests/fixtures/ramos/ad9b7fbc-61c8-43ee-a65c-99206fd1e4df.json) ×1 | V | P | F | U | — | NR | — | —/— | M4 | `keyword:cascade` |
| [Mosswort Bridge](../packages/game-engine/tests/fixtures/ramos/7cb9e29f-835f-4155-a2a5-4b778866c773.json) ×1 | V | P | I | U | — | NR | — | —/— | M9 | `keyword:hideaway` |
| [Mountain](../packages/game-engine/tests/fixtures/ramos/a3fb7228-e76b-4e96-a40e-20b5fed75685.json) ×4 | V | P | F | L | V | NR | — | —/— | M2 | `—` |
| [Nature's Lore](../packages/game-engine/tests/fixtures/ramos/78826359-fe63-44ad-adc4-a17ffcd710e4.json) ×1 | V | P | F | U | — | NR | — | —/— | M2 | `unsupported complete spell operator, destination, chooser or cardinality` |
| [Omnath, Locus of All](../packages/game-engine/tests/fixtures/ramos/26445dc7-8363-4205-aee8-1cafeb4ba4c0.json) ×1 | V | U | I | U | — | NR | — | —/— | M5 | `complete-card parsing is required` |
| [Overgrown Tomb](../packages/game-engine/tests/fixtures/ramos/975ec9a3-6f20-4177-8211-82526e092538.json) ×1 | V | P | F | U | — | NR | — | —/— | M2 | `static:entry-choice` |
| [Plains](../packages/game-engine/tests/fixtures/ramos/bc71ebf6-2056-41f7-be35-b2e5c34afa99.json) ×4 | V | P | F | L | V | NR | — | —/— | M2 | `—` |
| [Polluted Delta](../packages/game-engine/tests/fixtures/ramos/ef86989d-ce80-4e55-aece-7d11710eeffa.json) ×1 | V | P | F | U | — | NR | — | —/— | M2 | `cost:pay-life` |
| [Prismatic Omen](../packages/game-engine/tests/fixtures/ramos/d90882ae-6572-4214-8d9c-0bdfb75f9be2.json) ×1 | V | P | F | U | — | NR | — | —/— | M5 | `unsupported modifier scope` |
| [Progenitus](../packages/game-engine/tests/fixtures/ramos/2d7e00b6-12f0-4b03-82a6-50e3d1b5395e.json) ×1 | V | P | I | U | — | NR | — | —/— | M10 | `ref:self:unbound_source` |
| [Quandrix Command](../packages/game-engine/tests/fixtures/ramos/9c0f7cdc-9095-40ae-bf83-75a5ffda72b9.json) ×1 | V | P | F | U | — | NR | — | —/— | M6 | `effect:shuffle-into-library` |
| [Rhystic Study](../packages/game-engine/tests/fixtures/ramos/53236dd7-845a-444c-96d5-f41ed7325d8f.json) ×1 | V | P | F | U | — | NR | — | —/— | M6 | `abilities[*].effects[*].condition` |
| [Ruinous Ultimatum](../packages/game-engine/tests/fixtures/ramos/a6f38908-aa4f-4f99-a28e-85d11dab52e4.json) ×1 | V | P | F | U | — | NR | — | —/— | M7 | `unsupported complete filter` |
| [Sakashima's Protege](../packages/game-engine/tests/fixtures/ramos/cf127842-5fb1-44df-8d91-8cc0f48c559a.json) ×1 | V | P | I | U | — | NR | — | —/— | M8 | `keyword:cascade` |
| [Selvala, Heart of the Wilds](../packages/game-engine/tests/fixtures/ramos/1d725121-e50c-42f0-9128-56802f07c89e.json) ×1 | V | U | I | U | — | NR | — | —/— | M5 | `complete-card parsing is required` |
| [Shardless Agent](../packages/game-engine/tests/fixtures/ramos/2afbaa9a-c171-4a8b-90f3-5250d8498356.json) ×1 | V | P | F | U | — | NR | — | —/— | M3 | `keyword:cascade` |
| [Skyshroud Claim](../packages/game-engine/tests/fixtures/ramos/376c9d3f-21d3-4251-bb6a-026fa9e1b0e1.json) ×1 | V | P | F | U | — | NR | — | —/— | M2 | `unsupported complete spell operator, destination, chooser or cardinality` |
| [Spara's Headquarters](../packages/game-engine/tests/fixtures/ramos/3123ec89-8e95-4761-ba17-747ec667509f.json) ×1 | V | P | F | U | — | NR | — | —/— | M2 | `keyword:cycling` |
| [Stomping Ground](../packages/game-engine/tests/fixtures/ramos/16052b52-ade1-406f-a06b-ce7ea607fb63.json) ×1 | V | P | F | U | — | NR | — | —/— | M2 | `static:entry-choice` |
| [Sulfurous Springs](../packages/game-engine/tests/fixtures/ramos/f5c38c01-4a40-469f-91a0-7479daf4e8e7.json) ×1 | V | P | F | U | — | NR | — | —/— | M2 | `mana:choice` |
| [Swamp](../packages/game-engine/tests/fixtures/ramos/56719f6a-1a6c-4c0a-8d21-18f7d7350b68.json) ×4 | V | P | F | L | V | NR | — | —/— | M2 | `—` |
| [Talisman of Creativity](../packages/game-engine/tests/fixtures/ramos/14d2979d-5728-42d7-a027-0eb1f754655d.json) ×1 | V | P | F | U | — | NR | — | —/— | M2 | `mana:choice` |
| [Talisman of Curiosity](../packages/game-engine/tests/fixtures/ramos/8c34b089-aad1-476e-958a-3077bf1bbb51.json) ×1 | V | P | F | U | — | NR | — | —/— | M2 | `mana:choice` |
| [Talisman of Unity](../packages/game-engine/tests/fixtures/ramos/e5fcc5d7-6a60-4a5b-9d02-6c30041a95b9.json) ×1 | V | P | F | U | — | NR | — | —/— | M2 | `mana:choice` |
| [Teferi's Protection](../packages/game-engine/tests/fixtures/ramos/0d4ecdb1-ec90-497f-a7a4-1c68092b8757.json) ×1 | V | P | F | U | — | NR | — | —/— | M10 | `effect:simultaneous` |
| [Temple Garden](../packages/game-engine/tests/fixtures/ramos/f413a83d-a40d-434c-b20a-4c707c0527fa.json) ×1 | V | P | F | U | — | NR | — | —/— | M2 | `static:entry-choice` |
| [Temur Ascendancy](../packages/game-engine/tests/fixtures/ramos/e68dc47c-692f-4420-9799-eee104017273.json) ×1 | V | P | F | U | — | NR | — | —/— | M6 | `abilities[*].trigger.what.filter.properties` |
| [Thunder of Unity](../packages/game-engine/tests/fixtures/ramos/8419d1d5-bb0e-4a2d-bb6f-67957d035dde.json) ×1 | V | P | F | U | — | NR | — | —/— | M9 | `only complete, single-face normal cards in the initial subset are supported` |
| [Unite the Coalition](../packages/game-engine/tests/fixtures/ramos/164651c5-b831-4070-8491-16d30ac8395b.json) ×1 | V | P | I | U | — | NR | — | —/— | M10 | `effect:phase-out` |
| [Urban Evolution](../packages/game-engine/tests/fixtures/ramos/c715e525-fc3a-4cab-94a0-04e0a22a5013.json) ×1 | V | P | F | U | — | NR | — | —/— | M6 | `effect:additional-land-plays` |
| [Vega, the Watcher](../packages/game-engine/tests/fixtures/ramos/95e66851-7aff-4559-91b4-6b8a95c6e1f8.json) ×1 | V | P | F | U | — | NR | — | —/— | M4 | `abilities[*].trigger.notFrom` |
| [Void Rend](../packages/game-engine/tests/fixtures/ramos/713f16db-95ec-479e-a48c-7a69f7668d7f.json) ×1 | V | P | F | L | V | NR | — | —/— | M7 | `—` |
| [Whirlwind of Thought](../packages/game-engine/tests/fixtures/ramos/6467cbb7-1e4e-482d-a20f-6cb9fc0f1ad1.json) ×1 | V | P | F | L | V | NR | — | —/— | M6 | `—` |
| [Wild-Magic Sorcerer](../packages/game-engine/tests/fixtures/ramos/a450ef56-8511-43e0-bdca-0c25f54ba338.json) ×1 | V | P | F | U | — | NR | — | —/— | M4 | `static:cast-spell-abilities` |
| [Windswept Heath](../packages/game-engine/tests/fixtures/ramos/29737a60-3ebd-40d9-b935-c4f54b90d45d.json) ×1 | V | P | F | U | — | NR | — | —/— | M2 | `cost:pay-life` |
| [Yavimaya Coast](../packages/game-engine/tests/fixtures/ramos/40b36bc6-c185-4bda-99e7-0118953c2c97.json) ×1 | V | P | F | U | — | NR | — | —/— | M2 | `mana:choice` |
| [Yidris, Maelstrom Wielder](../packages/game-engine/tests/fixtures/ramos/9efe8aff-9a7d-4397-b5fd-c1a0fad7c15f.json) ×1 | V | U | I | U | — | NR | — | —/— | M4 | `complete-card parsing is required` |
| [Zacama, Primal Calamity](../packages/game-engine/tests/fixtures/ramos/23270e4a-a222-4ffb-a946-d0e20d665187.json) ×1 | V | P | I | U | — | NR | — | —/— | M10 | `abilities[*].condition.was-cast` |
| [Ziatora's Proving Ground](../packages/game-engine/tests/fixtures/ramos/f7e7b78c-c769-4720-8585-1874773eb342.json) ×1 | V | P | F | U | — | NR | — | —/— | M2 | `keyword:cycling` |

## Dependency backlog and handoff boundaries

M0 adds no mechanics or pools. Keep the original milestone completion obligations,
including modes not named by the first compiler diagnostic. The measured blockers
refine the [plan's dependency order](game-engine-ramos-plan.md#dependencies-and-parallel-work):

| Owner | Measured source blockers and next completion obligations | Dependencies |
| --- | --- | --- |
| M1 | M1a and M1b complete frozen Ramos/Scales/Corpsejack through parameterized shared families: cast-color counters, counter-cost/use/undo, prospective placement replacements and affected-controller order/recovery. Full paid-source composition and direct/source/target producers are witnessed; diagnostic bindings remain unchanged. Entry-with-counters and simultaneous/distributed placement producers, exact target and consumer admission remain separate/excluded. | [M1 contract](game-engine-ramos-m1-contract.md#bounded-m1b-counter-placement-family); M0 pins retained |
| M2 | M0’s eight lowering definitions remain: the five basics, Command Tower, Arcane Signet and Golgari Signet. Cultivate and Kodama’s Reach now also lower and execute in the bounded private source witnesses (S). The other 22 definitions remain future M2 work: entry choice/conditions, cycling, pay-life costs, typed searches, opponent production and mana choice. Preserve all modes and land subtypes. | M0; coordinate payment/search/entry interfaces with M1 |
| M3 | Shardless Agent and Bloodbraid Elf reject `keyword:cascade`. Build generic casting during resolution and a continuation before cascade. | M1 payment foundation; M2 useful opening infrastructure |
| M4 | Cascade/affinity, cast grants/history and non-hand casts remain unsupported. Call Forth, Averna and Yidris lack complete archived parses. | M3; narrowly scoped parser intake before complete lowering |
| M5 | Opening-hand action, modifier scopes and dynamic mana reject. Selvala and Omnath lack complete parses; Faeburrow's binding is incomplete. Hybrid/Phyrexian payment, land types/color layers and unspent-mana replacement remain obligations. | M1/M2 payments; layers and private-choice contracts |
| M6 | Whirlwind of Thought already lowers; runtime/admission witnesses remain separate. Other rows reject color/property triggers, optional-payment conditions, distributed counters, reveal, discard, shuffle or extra lands. Abzan Charm and Conflux have incomplete bindings. | Stable trigger/choice/search contracts; M5 where relevant |
| M7 | Anguished Unmaking and Void Rend already lower; exact-deck scenarios/admission remain unrun. Other rows reject recipient scope, no-regeneration mass destruction, activated operator, ability removal, variable modal count or wipe filter. Complete every protection/wipe branch. | Keyword/target/choice primitives; coordinate common contracts |
| M8 | Creative Technique and Double Major lack complete parses; Protege rejects cascade and has an incomplete copy binding. Complete copy characteristics, actual-cast distinction and copied-token trigger capacity proof. | M3/M4 casting history; capacity review before admission |
| M9 | Hideaway and bestow reject; Thunder is a non-normal layout (Saga). Mosswort's linked permission binding is incomplete. | Linking/private exile, payment, layers and copy primitives |
| M10 | Simultaneous/phasing, repeated choices, unbound Progenitus replacement, play-card, token shape, was-cast condition and lose-game reject. Zacama has bound references but fullyBound false: event-history feature needs review. | Relevant earlier continuations/replacements/payments; never erase target alternatives |
| M11 | No exact Ramos catalog/profile, pod admission, policies, completion batch, host or search qualification exists. The M1a private paid source witness and M2 split-search witnesses do not establish those gates. Reuse L rows without calling them admitted. | Complete source/capability closure and resource proof across M1–M10; host/SDK separate; search follow-on |

The seven failed parses must be repaired through the parser's own manual and
archive/provenance process. Do not hand-edit these frozen ASTs or bindings to
force successful compilation. Incomplete bindings additionally affect Corpsejack,
Hardened Scales, Abzan Charm, Conflux, Faeburrow, Mosswort, Progenitus, Protege,
Unite and Zacama; exact diagnostics and engine features are in their source files.
Ramos's archived fullyBound status was not M0 semantic support; M1a/M1b separately check complete source lowering and private runtime witnesses. M1b independently consumes both replacement scopes without rewriting their incomplete archived binding diagnostics.

## Reproduce and verification limits

Use the [engine environment and shared lock](game-engine-development.md#cargo-coordination-and-cache-integration).
Run at repository root; choose a new ignored output directory for each audit:

```sh
python3 packages/game-engine/tests/fixtures/ramos/audit.py --verify
sh scripts/engine-env.sh python3 scripts/engine-cache.py check
sh scripts/engine-env.sh flock "$HOME/.local/share/mtg-ecosystem/engine-verification/cargo.lock" cargo build --locked -p mtg-sim
sh scripts/engine-env.sh flock "$HOME/.local/share/mtg-ecosystem/engine-verification/cargo.lock" sh -c '"$CARGO_TARGET_DIR/debug/mtg-sim" catalog verify /home/josh/.local/share/mtg-ecosystem/engine-evidence/archives/oracle-ast-d705222b280a71cb.jsonl packages/game-engine/tests/fixtures/ramos/provenance.json'
sh scripts/engine-env.sh flock "$HOME/.local/share/mtg-ecosystem/engine-verification/cargo.lock" sh -c 'python3 packages/game-engine/tests/fixtures/ramos/audit.py "$CARGO_TARGET_DIR/debug/mtg-sim" generated/ramos/NEW-AUDIT'
```

The small script verifies this fixed manifest and calls the existing production
catalog command; it implements no compiler semantics or admission. Singleton
manifests/IR/digests are ignored outputs, deliberately outside `catalogs/` so this
audit cannot broaden embedded pools. It continues after each card rejection and
preserves each result immediately. A normal audit exits zero after recording
expected Unsupported rows; inspect the statuses, not only the process exit.

To recover extraction, create a **new** scratch directory with this provenance's
metadata and empty `records`, the accepted-origin metadata with empty record pins,
and the unchanged archive header. Pass all 85 manifest names/IDs to production
`mtg-sim catalog extract ARCHIVE NEW_PROVENANCE NAME_OR_ID...`, then `catalog verify`.
The committed raw records are the byte-exact extraction result, about 1 MB total.
Neither full archive nor bulk generated IR is committed.

Executed checks on the identified base, with no production edits:

- Locked `cargo build --locked -p mtg-sim`: passed (dev); zero unit tests run.
- Production extraction and archive verification: passed, 85 records, 0 other
  origins; full accepted-archive hash checked. Singleton compiler audit: 85
  invocations, 11 lower/validate, 74 preserved Unsupported, 0 Invalid.
- Manifest check: 85 definitions, 100 quantities, 37 lands; export digest matches.
  Documentation/fixture consistency and `git diff --check` passed.
- One preliminary Ramos-only diagnostic probe preceded the 85-definition audit;
  it returned the same `factors` rejection. Its manifest and captured result are
  preserved separately and are not another successful audit gate.
- Worktree doctor failed on missing local Node dependencies. The documented
  bootstrap failed with npm EUSAGE because current workspace manifests and
  `package-lock.json` disagree (including game-client/SDK entries and dependency
  versions). No lockfile, package manifest or shared installation was repaired.
  The npm failure log, doctor JSON and nonexecuting check plan are retained.

Raw evidence is this worktree's `generated/ramos/m0-46ef08fa/`: `build.log`,
`cache-preflight.json`, `build-identity.json`, `core-build-output.txt`,
`extract.json`, `verify.json`, `compiler/results.json`, `audit.log`,
`bootstrap-npm-failure.log`, `doctor.json`, `check-plan.json`, and `probe*`.
Existing ignored evidence and caches were preserved. The native slot is released.

Omitted: Node source gate (dependency installation blocked), unit/runtime test
suites, full workspace/all-target checks, formatting/Clippy, full corpus coverage,
release/profile/completion matrices, schema export, replay/snapshot/privacy
scenarios, exact-deck/pod admission, host/SDK/client/search and retention. No new
mechanic or runtime claim requires those gates in this M0 fixture/doc checkpoint.
