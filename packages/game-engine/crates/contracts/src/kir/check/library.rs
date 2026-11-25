//! Library cells: search, shuffle, scry and surveil (CR 701.22-701.25).
//!
//! A library is a hidden zone (CR 401.2). Only the player who searches,
//! scries or surveils looks at its cards, and only these cells open a
//! decision over them. Each player names its own library: "you", or the
//! last-known controller of one filtered announced target (CR 608.2h), the
//! player Path to Exile-style text calls "its controller".
//!
//! Pinned Comprehensive Rules 2026-09-25:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl Walk<'_, '_> {
    /// The library a library cell acts on: the controller's own, or the
    /// last-known controller of a filtered target whose facts the frame
    /// captures (CR 608.2h). Returns true for the target controller.
    pub(super) fn library_player(&self, player: &PlayerExpr) -> Result<bool, KirError> {
        match player {
            PlayerExpr::Is {
                who: Ref::Controller { .. },
            } => Ok(false),
            PlayerExpr::ControllerOf {
                object: Ref::Target { slot },
            } if self.target_controller_facts(*slot) => Ok(true),
            _ => unaudited("library player"),
        }
    }

    /// A filtered battlefield-permanent or stack-spell target, whose current
    /// or last-known controller the resolving frame captures (CR 608.2h).
    pub(super) fn target_controller_facts(&self, slot: u32) -> bool {
        self.env.targets.get(slot as usize).is_some_and(|target| {
            target.filter.is_some()
                && matches!(
                    target.domain,
                    TargetDomain::BattlefieldPermanent | TargetDomain::StackSpell
                )
        })
    }

    /// CR 701.23a-e: a player searches their own library for up to
    /// `maximum` cards with the stated qualities, may reveal them, and puts
    /// them into one destination. CR 701.23b, d: with no stated quality
    /// ("a card") the player must find as many as possible; with one they
    /// may find fewer. The shuffle that follows is its own verb.
    pub(super) fn audit_search(
        &self,
        player: &PlayerExpr,
        search: &SearchSpec,
    ) -> Result<Cell, KirError> {
        let target_controller = self.library_player(player)?;
        Self::library_card_filter(&search.filter)?;
        Self::fixed_at_least(&search.maximum, 1, "search maximum")?;
        if search
            .maximum
            .as_fixed()
            .is_some_and(|max| max.to_u32().is_none())
        {
            return kind("search maximum exceeds the card bound");
        }
        match &search.to {
            // CR 110.2: the searching player puts the card onto the
            // battlefield, under their own control.
            Destination::Battlefield { controller, .. } if controller == player => {}
            Destination::OwnerHand | Destination::OwnerGraveyard => {}
            _ => return unaudited("search destination"),
        }
        Ok(if target_controller {
            Cell::SearchTargetController
        } else {
            Cell::SearchController
        })
    }

    /// CR 701.24a: a player shuffles their own library.
    pub(super) fn audit_shuffle(&self, player: &PlayerExpr) -> Result<Cell, KirError> {
        Ok(if self.library_player(player)? {
            Cell::ShuffleTargetController
        } else {
            Cell::ShuffleController
        })
    }

    /// CR 701.22a, 701.25a: the controller scries or surveils a fixed
    /// positive number. CR 701.22b: scry 0 is no event and is never printed.
    pub(super) fn audit_look(
        &self,
        player: &PlayerExpr,
        amount: &Amount,
        surveil: bool,
    ) -> Result<Cell, KirError> {
        if self.library_player(player)? {
            return unaudited("look player");
        }
        Self::fixed_at_least(amount, 1, "look amount")?;
        if amount.as_fixed().is_some_and(|n| n.to_u32().is_none()) {
            return kind("look amount exceeds the card bound");
        }
        Ok(if surveil {
            Cell::SurveilController
        } else {
            Cell::ScryController
        })
    }

    /// The printed characteristics of a library card the core evaluates:
    /// card types, supertypes, subtypes, colors, fixed power, toughness or
    /// mana value comparisons, and alternatives of these (CR 205, 208,
    /// 202.3). Status, control, ownership and keyword atoms reject.
    fn library_card_filter(filter: &Pred) -> Result<(), KirError> {
        for atom in &filter.0 {
            match atom {
                Atom::Type { .. }
                | Atom::NotType { .. }
                | Atom::AnyType { .. }
                | Atom::Subtype { .. }
                | Atom::NotSubtype { .. }
                | Atom::Legendary
                | Atom::NotLegendary
                | Atom::Basic
                | Atom::NotBasic
                | Atom::Color { .. }
                | Atom::NotColor { .. } => {}
                Atom::Power { value, .. }
                | Atom::Toughness { value, .. }
                | Atom::ManaValue { value, .. } => {
                    if value.as_fixed().is_none() {
                        return unaudited("computed library comparison");
                    }
                }
                Atom::AnyOf { alternatives } => {
                    alternatives
                        .iter()
                        .try_for_each(Self::library_card_filter)?;
                }
                Atom::Tapped
                | Atom::Untapped
                | Atom::Attacking
                | Atom::Blocking
                | Atom::Token
                | Atom::NotToken
                | Atom::Keyword { .. }
                | Atom::NotKeyword { .. }
                | Atom::ControlledBy { .. }
                | Atom::OwnedBy { .. }
                | Atom::IsNot { .. } => return unaudited("library card filter atom"),
            }
        }
        Ok(())
    }
}
