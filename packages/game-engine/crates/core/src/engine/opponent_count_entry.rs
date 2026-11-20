//! CR 109.5, 614.12, 800.4a and 806.1 (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(super) fn count(
    game: &Game,
    before: Option<&PreEventFacts>,
    controller: PlayerId,
) -> Result<Quantity, String> {
    let current;
    let seats = if let Some(before) = before {
        &before.entry_live_seats
    } else {
        current = game.live_players();
        &current
    };
    if !seats.contains(&controller) {
        return Err("missing common-prior entering controller seat".into());
    }
    // Only free-for-all is admitted. Every other still-participating player
    // is an opponent of this prospective controller.
    Ok(Quantity::from((seats.len() - 1) as i64))
}
