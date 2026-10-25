//! Catalog-independent structural limits. See docs/game-engine-capacity-policy.md.
pub(super) const DAMAGE_LIMIT: usize = 262_144;
pub(super) const EFFECT_LIMIT: usize = 65_536;
pub(super) const STACK_LIMIT: usize = 4_194_304;
pub(super) const TOKEN_CAPACITY: u32 = 65_536;
pub(super) const EVENT_GROUP_LIMIT: usize = 131_072;
pub(super) const OWNED_LIMIT: usize = 100;
pub(super) const EVENT_GROUP_DEPTH: u32 = 4;
