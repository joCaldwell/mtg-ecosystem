//! Keyword action cells (CR 701.14, 701.34, 701.36).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl Walk<'_, '_> {
    pub(super) fn audit_fight(&self, first: &Ref, second: &Ref) -> Result<Cell, KirError> {
        let fighter = |fighter: &Ref| match fighter {
            Ref::Target { slot } => self.creature_target(*slot),
            Ref::Source => Ok(self.env.context != Context::Spell),
            _ => Ok(false),
        };
        if fighter(first)? && fighter(second)? {
            Ok(Cell::Fight)
        } else {
            unaudited("fight participants")
        }
    }

    pub(super) fn audit_keyword_player(player: &PlayerExpr, cell: Cell) -> Result<Cell, KirError> {
        if player.is_you() {
            Ok(cell)
        } else {
            unaudited("keyword action player")
        }
    }
}
