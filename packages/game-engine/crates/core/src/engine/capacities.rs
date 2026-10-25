//! Catalog-independent structural limits. See docs/game-engine-capacity-policy.md.
pub(super) const DAMAGE_LIMIT: usize = 2_048;
pub(super) const EFFECT_LIMIT: usize = 256;
pub(super) const STACK_LIMIT: usize = 262_144;
pub(super) const TOKEN_CAPACITY: u32 = 256;
pub(super) const TOKEN_HISTORY_LIMIT: usize = 65_536;
pub(super) const EVENT_GROUP_LIMIT: usize = 1_024;
// Snapshot/wipe validation only; Commander deck size is enforced by admission.
pub(super) const OWNED_LIMIT: usize = 100;
pub(super) const EVENT_GROUP_DEPTH: u32 = 4;
