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

    /// CR 608.2h: the last-known controller of a filtered target, the
    /// chooser Path to Exile-style text calls "its controller".
    pub(super) fn target_controller_chooser(&self, chooser: &PlayerExpr) -> bool {
        matches!(chooser, PlayerExpr::ControllerOf {
            object: Ref::Target { slot },
        } if self.target_controller_facts(*slot))
    }

    /// Every verb of `node` searches or shuffles `player`'s own library.
    pub(super) fn own_library(node: &Node, player: &PlayerExpr) -> bool {
        match node {
            Node::Seq(nodes) => nodes.iter().all(|node| Self::own_library(node, player)),
            Node::Do(Verb::Search { player: p, .. } | Verb::Shuffle { player: p }) => p == player,
            _ => false,
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
        if search.from != SelectionZone::Library {
            return self.audit_selection(player, search);
        }
        if search.optional || search.random {
            return unaudited("optional or random library search");
        }
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
        if let Some(group) = &search.group {
            if !matches!(
                search.to,
                Destination::Battlefield { .. }
                    | Destination::OwnerHand
                    | Destination::OwnerGraveyard
            ) {
                return unaudited("grouped search destination");
            }
            match group {
                SearchGroup::DifferentNames if !search.filter.0.is_empty() => {}
                SearchGroup::Separate { slots }
                    if (2..=8).contains(&slots.len())
                        && search.filter.0.is_empty()
                        && search.maximum.as_fixed()
                            == Some(&Quantity::from(slots.len() as i64))
                        && slots.iter().all(|slot| !slot.0.is_empty()) =>
                {
                    slots.iter().try_for_each(Self::library_card_filter)?;
                }
                _ => return unaudited("grouped search selection"),
            }
        }
        match &search.to {
            // CR 110.2: the searching player puts the card onto the
            // battlefield, under their own control.
            Destination::Battlefield { controller, .. } if controller == player => {}
            Destination::OwnerHand | Destination::OwnerGraveyard => {}
            Destination::OneTappedRestOwnerHand
                if !target_controller
                    && matches!(self.env.context, Context::Spell | Context::Activated)
                    && search.maximum.as_fixed() == Some(&Quantity::from(2))
                    && search.reveal
                    && matches!(
                        &search.filter.0[..],
                        [
                            Atom::Basic,
                            Atom::Type {
                                kind: CardKind::Land
                            }
                        ] | [
                            Atom::Type {
                                kind: CardKind::Land
                            },
                            Atom::Basic
                        ]
                    ) =>
            {
                return Ok(Cell::SearchSplitBasicLands);
            }
            // CR 701.24b: this search includes the shuffle of everything
            // except its one found card, followed by positioning that card.
            Destination::OwnerLibraryTop
                if !target_controller && search.maximum.as_fixed() == Some(&Quantity::from(1)) =>
            {
                return Ok(Cell::SearchLibraryTop);
            }
            _ => return unaudited("search destination"),
        }
        Ok(if target_controller {
            Cell::SearchTargetController
        } else {
            Cell::SearchController
        })
    }

    /// CR 608.2c-d, 400.7: the controller chooses up to `maximum` cards of
    /// their own hand or graveyard with printed qualities while resolving
    /// and moves them together. A hand card goes only onto the battlefield
    /// under its owner's control; a graveyard card onto the battlefield or
    /// into its owner's hand. Nothing is revealed, and only a "may" or "up
    /// to" choice may choose fewer than it can.
    fn audit_selection(&self, player: &PlayerExpr, search: &SearchSpec) -> Result<Cell, KirError> {
        if self.library_player(player)? || search.reveal || search.group.is_some() {
            return unaudited("resolution card choice player");
        }
        // CR 608.2d: a random selection takes as many as it can from a
        // public graveyard; nobody chooses.
        if search.random {
            if search.from != SelectionZone::Graveyard || search.optional {
                return unaudited("random card selection");
            }
            Self::library_card_filter(&search.filter)?;
            Self::fixed_at_least(&search.maximum, 1, "random selection maximum")?;
            return match &search.to {
                Destination::OwnerHand => Ok(Cell::RandomGraveyardCards),
                Destination::Battlefield { controller, .. } if controller == player => {
                    Ok(Cell::RandomGraveyardCards)
                }
                _ => unaudited("random card selection destination"),
            };
        }
        Self::library_card_filter(&search.filter)?;
        Self::fixed_at_least(&search.maximum, 1, "card choice maximum")?;
        if search
            .maximum
            .as_fixed()
            .is_none_or(|max| max.to_u32().is_none_or(|max| max > 7))
        {
            return kind("card choice maximum exceeds its bound");
        }
        match (search.from, &search.to) {
            (SelectionZone::Hand, Destination::Battlefield { controller, .. })
                if controller == player =>
            {
                Ok(Cell::ChooseHandCards)
            }
            (SelectionZone::Graveyard, Destination::Battlefield { controller, .. })
                if controller == player =>
            {
                Ok(Cell::ChooseGraveyardCards)
            }
            (SelectionZone::Graveyard, Destination::OwnerHand) => Ok(Cell::ChooseGraveyardCards),
            _ => unaudited("resolution card choice destination"),
        }
    }

    /// CR 701.24a: a player shuffles their own library.
    pub(super) fn audit_shuffle(&self, player: &PlayerExpr) -> Result<Cell, KirError> {
        Ok(if self.library_player(player)? {
            Cell::ShuffleTargetController
        } else {
            Cell::ShuffleController
        })
    }

    /// CR 701.20e, 701.22a, 701.25a: the controller privately looks at a
    /// fixed positive number for an audited library procedure.
    pub(super) fn audit_look(
        &self,
        player: &PlayerExpr,
        amount: &Amount,
        cell: Cell,
    ) -> Result<Cell, KirError> {
        if self.library_player(player)? {
            return unaudited("look player");
        }
        Self::fixed_at_least(amount, 1, "look amount")?;
        if amount.as_fixed().is_some_and(|n| n.to_u32().is_none()) {
            return kind("look amount exceeds the card bound");
        }
        Ok(cell)
    }

    /// The printed characteristics of a library card the core evaluates:
    /// card types, supertypes, subtypes, colors, fixed power, toughness or
    /// mana value comparisons, and alternatives of these (CR 205, 208,
    /// 202.3). Status, control, ownership and keyword atoms reject.
    pub(in crate::kir) fn library_card_filter(filter: &Pred) -> Result<(), KirError> {
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
                | Atom::IsNot { .. }
                | Atom::Is { .. }
                | Atom::Counters { .. }
                | Atom::SourceChosenColor {}
                | Atom::SourceChosenCreatureType {} => {
                    return unaudited("library card filter atom");
                }
            }
        }
        Ok(())
    }
}
