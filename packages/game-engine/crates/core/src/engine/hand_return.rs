//! CR121.2/401.4/400.7/608.2c/903.9b: counted draws, then ordered hand return.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
impl RulesTxn {
    pub(super) fn bounded_hand_count(&self, player: PlayerId, count: &Quantity) -> usize {
        let len = self.action_hand(player).len();
        (0..len)
            .take_while(|n| Quantity::from(*n as i64) < *count)
            .count()
    }
}
