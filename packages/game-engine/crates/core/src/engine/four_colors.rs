//! Reviewed P6.6 bounds; see docs/game-engine-blue-red.md.
//! Pinned CR 302.6, 605.3b, 903.8: mana taps do not stack; each source taps
//! at most once per step; commander recasts need an empty stack.
use super::*;
pub(super) const CREATURE_CARDS: usize = 4 * 31;
pub(super) const CREATURES: usize = CREATURE_CARDS + staples::LIVE_TOKEN_CAPACITY as usize;
pub(super) const DAMAGE_BOUND: usize = 2 * CREATURES;
// At most 400 mana sources, each producing <=2 mana per tap per step.
// Azure Mage costs four; all produced/floated mana together fund <=200 activations.
pub(super) const MAGE_ACTIVATIONS: usize = 400 * 2 / 4;
// 4 each Healer/Looter/Prodigal/Hapless, 8 sacrifice-draw artifacts.
// Bounce/tuck and recursion return only to hand/library, never battlefield;
// each creature still enters/dies once per nonempty-stack episode.
pub(super) const STACK_BOUND: usize =
    400 + 24 + MAGE_ACTIVATIONS + 8 * (CREATURES + 36) + CREATURE_CARDS + 4;
impl Game {
    // Exact slice: selects its resource bounds; later slices have separate bounds.
    pub(super) fn four_colors_enabled(&self) -> bool {
        self.prepared.config.profile == FOUR_COLORS_PROFILE
            && self.prepared.catalog.schema_version == 16
    }
}
