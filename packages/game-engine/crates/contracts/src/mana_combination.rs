//! CR 106.1a, 106.3, 107.1. Quantities correspond to W, U, B, R and G.
use super::{Color, Quantity};

pub const MANA_COMBINATION_COLORS: [Color; 5] = [Color::W, Color::U, Color::B, Color::R, Color::G];

pub fn validate_mana_combination(
    allowed: &[Color],
    total: &Quantity,
    amounts: &[Quantity; 5],
) -> Result<(), String> {
    if total.is_negative()
        || allowed
            .iter()
            .enumerate()
            .any(|(i, c)| *c == Color::C || allowed[..i].contains(c))
    {
        return Err("invalid colored mana domain or total".into());
    }
    let mut sum = Quantity::zero();
    for (color, amount) in MANA_COMBINATION_COLORS.iter().zip(amounts) {
        if amount.is_negative() || (*amount > Quantity::zero() && !allowed.contains(color)) {
            return Err("mana allocated outside permitted colors".into());
        }
        sum.add(amount);
    }
    if sum != *total {
        return Err("mana combination must allocate the exact total".into());
    }
    Ok(())
}
