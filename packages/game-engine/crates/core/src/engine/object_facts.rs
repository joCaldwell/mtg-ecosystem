//! The rules core's private `ObjectFacts` reader over derived state. It is not
//! exported: a policy can obtain these facts only from a permitted view.
use super::*;
use crate::query::{ObjectFacts, Tri};
use std::borrow::Cow;

pub struct DerivedObject<'a> {
    game: &'a RulesTxn,
    object: &'a Object,
}

impl RulesTxn {
    pub(crate) fn object_facts<'a>(&'a self, object: &'a Object) -> DerivedObject<'a> {
        DerivedObject { game: self, object }
    }
}

impl ObjectFacts for DerivedObject<'_> {
    fn counters(&self, kind: Option<&mtg_contracts::kir::CounterKind>) -> Option<Quantity> {
        (self.object.zone == Zone::Battlefield).then(|| {
            crate::query::counter_balance(
                kind,
                &self.object.plus_one_counters,
                &self.object.loyalty_counters,
                self.object
                    .counters
                    .iter()
                    .map(|stack| (&stack.kind, &stack.count)),
            )
        })
    }

    fn zone(&self) -> Zone {
        self.object.zone
    }
    fn owner(&self) -> Option<PlayerId> {
        let card = self.game.state.rules.cards.get(&self.object.id.card)?;
        Some(card.owner)
    }
    fn controller(&self) -> PlayerId {
        self.object.controller
    }
    fn phased_out(&self) -> bool {
        self.object.phased_out.is_some()
    }
    fn tapped(&self) -> bool {
        self.object.tapped
    }
    fn is_token(&self) -> bool {
        self.game
            .state
            .rules
            .tokens
            .contains_key(&self.object.id.card)
    }
    fn copiable(&self) -> Option<Cow<'_, Characteristics>> {
        self.game.copiable_characteristics(self.object).ok()
    }
    fn structural(&self) -> Option<Cow<'_, Characteristics>> {
        // The engine currently supports type and color layer effects only on the battlefield.
        // Keep this incarnation's zone when a split spell has departed.
        if self.object.zone != Zone::Battlefield {
            return self.copiable();
        }
        self.game.structural_characteristics(self.object.id).ok()
    }
    // The color layer keeps the structural card types (CR 613.1d-e).
    fn current(&self) -> Option<Cow<'_, Characteristics>> {
        if self.object.zone != Zone::Battlefield {
            return self.copiable();
        }
        self.game.color_layer(self.object.id).ok()
    }
    fn derived(&self) -> Option<Cow<'_, Characteristics>> {
        self.game
            .characteristics(self.object.id)
            .ok()
            .map(Cow::Owned)
    }
    fn has_keyword(&self, keyword: KeywordAbility) -> Tri {
        self.game
            .has_keyword(self.object.id, keyword)
            .map_or(Tri::Unknown, Tri::from)
    }
    fn controller_is_live(&self) -> Tri {
        self.game.is_live(self.object.controller).into()
    }
    fn attacking(&self) -> Tri {
        self.game
            .state
            .rules
            .combat
            .iter()
            .any(|attack| attack.attacker == self.object.id)
            .into()
    }
    fn blocking(&self) -> Tri {
        self.game
            .state
            .rules
            .blocking_creatures
            .contains(&self.object.id)
            .into()
    }
}
