# Engine structural capacity policy

D1b-2 replaces the historical profile lineage bounds with one catalog-independent
policy. These are generous executor resource ceilings, not claims that every
admitted game terminates within them. A ceiling never determines a Magic winner.

| Resource | Limit | Accounting |
| --- | ---: | --- |
| Owned physical cards per player | 100 | Admission/configuration validation |
| Live allocated tokens | 65,536 | Existing token objects; ceased tokens retain identity history |
| Damage occurrences per transaction | 262,144 | Entire simultaneous prepared damage group |
| Stack plus pending triggers | 4,194,304 | Combined entries, before any new entry |
| Event-group nesting depth | 4 | Checked before recursive collection |
| Event-group occurrences | 131,072 | Total leaves across ordered and simultaneous groups |
| Temporary effects | 65,536 | Current retained effects, before insertion |

`engine/capacities.rs` is the only runtime authority for these values. Catalog
names, schema versions, card identities, and historical publication order never
select a capacity. Checked identity/cursor arithmetic remains an independent invariant. Event-group
depth exhaustion also has its own typed fault.

Exhaustion produces a tagged, serializable `RuleFault` with capacity and requested
usage. Mutation runs on staged game state; an overflowing rule step retains only
the fault and its fault frame, with no partial game mutation. Event groups check
the total before application, and an error from a staged group carries its typed
fault back to the enclosing step. Snapshot validation checks both the resource
bound and the fault metadata; restoration still proves accepted-input replay.

Historical per-profile finite-pool arithmetic in slice documents and test-only
constants describes the old releases. It is preserved as historical evidence,
and no longer sets runtime limits or establishes current admission guarantees.
