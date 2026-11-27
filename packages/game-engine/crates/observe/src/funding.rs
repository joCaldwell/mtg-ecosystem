//! Funding codes certify what the choosing player can establish from the
//! published pool and fixed-color intrinsic lands (CR 106, 302.6, 305.6).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_contracts::{
    ActivationCost, CardKind, Catalog, Color, KeywordAbility, Quantity, basic_land_color,
};
use mtg_view::*;
use std::collections::BTreeMap;

/// The pool is a lower bound. Ambiguous sources keep a failed payment unknown:
/// restricted units depend on the payment, printed mana can have costs or
/// riders, and tokens need not have catalog rows.
pub(super) fn intrinsic_supply(
    catalog: &Catalog,
    view: &PermittedView,
) -> (BTreeMap<Color, Quantity>, bool) {
    let Some(payer) = view.players.iter().find(|p| p.player == view.viewer) else {
        return (BTreeMap::new(), false);
    };
    let mut pool = payer.mana.clone();
    let mut complete = payer.restricted_mana.is_empty();
    for object in view
        .objects
        .iter()
        .filter(|o| o.controller == view.viewer && o.zone == Zone::Battlefield && !o.phased_out)
    {
        if object.token.is_some()
            || !object.granted_mana.is_empty()
            || catalog
                .definition_by_oracle_id(&object.oracle_id)
                .is_none_or(|d| !d.mana_abilities.is_empty())
        {
            complete = false;
        }
        let Some(ch) = object.characteristics.as_ref() else {
            complete = false;
            continue;
        };
        if object.tapped
            || !ch.has_type(CardKind::Land)
            || ch.has_type(CardKind::Creature)
                && !object.attack_ready
                && !object.keywords.contains(&KeywordAbility::Haste)
        {
            continue;
        }
        let mut colors = ch.subtypes.iter().filter_map(|s| basic_land_color(s));
        if let Some(color) = colors.next() {
            if colors.next().is_some() {
                complete = false;
            } else {
                pool.entry(color).or_default().add(&1.into());
            }
        }
    }
    (pool, complete)
}
pub(super) fn funding(
    c: &crate::catalog::ObservationCatalog,
    o: &Observation,
    d: &Decision<Viewer>,
) -> Vec<u8> {
    let v = &o.view;
    let battlefield_reducer = std::cell::OnceCell::new();
    let supply = std::cell::OnceCell::new();
    d.candidates
        .iter()
        .map(|candidate| {
            let Candidate::Action { action } = candidate else {
                return 2;
            };
            let h = match action {
                Action::Cast { card } | Action::CastBackFace { card } => card,
                Action::ActivateAbility { object, .. } => object,
                // The exact rules decision prices a face-down cast and the
                // turn-face-up special action (CR 702.37a, e).
                Action::CastFaceDown { .. } | Action::TurnFaceUp { .. } => return 2,
                _ => return 1,
            };
            let Some(obj) = v.objects.iter().find(|obj| &obj.handle == h) else {
                return 2;
            };
            let Some(def) = c.definition_by_oracle_id(&obj.oracle_id) else {
                return 2;
            };
            // Alternate-face costs are handled by the exact rules decision.
            // This advisory filter has no review for modal double-faced cards.
            if def.back_face.is_some() {
                return 2;
            }
            // CR 118.9, 702.74a: a hand or command-zone cast may announce
            // the printed alternative cost instead of the mana cost.
            let mut alternative = None;
            let (cost, hybrid) = match action {
                Action::Cast { .. } => {
                    if c.reduces_cost(&obj.oracle_id)
                        || *battlefield_reducer.get_or_init(|| c.has_visible_reducer(v))
                    {
                        return 2;
                    }
                    // CR 702.34a: a graveyard cast pays the flashback cost.
                    let mut cost = match (&obj.zone, &def.flashback) {
                        (Zone::Graveyard { .. }, Some(flashback)) => flashback.clone(),
                        // CR 702.138a: the escape cost replaces the mana cost.
                        (Zone::Graveyard { .. }, None) if def.escape.is_some() => {
                            def.escape.as_ref().map_or_else(
                                || def.characteristics.mana_cost.clone(),
                                |e| e.mana.clone(),
                            )
                        }
                        _ => def.characteristics.mana_cost.clone(),
                    };
                    if matches!(obj.zone, Zone::Command { .. })
                        && let Some(n) = v
                            .commanders
                            .iter()
                            .find(|r| r.history.owner == v.viewer && r.oracle_id == obj.oracle_id)
                            .and_then(|r| r.history.command_casts.get(&v.viewer))
                    {
                        cost.generic.add(n);
                        cost.generic.add(n);
                    }
                    if let Some(printed) = &def.alternative_cost
                        && matches!(obj.zone, Zone::Hand { .. } | Zone::Command { .. })
                    {
                        let mut alt = printed.mana.clone();
                        alt.generic.add(&cost.generic);
                        alt.generic.subtract(&def.characteristics.mana_cost.generic);
                        alternative = Some(alt);
                    }
                    (cost, def.characteristics.hybrid_mana.as_slice())
                }
                Action::ActivateAbility { ability, .. } => {
                    let Some(a) = def.activated.get(*ability as usize) else {
                        return 2;
                    };
                    let mana = match &a.cost {
                        ActivationCost::Mana { cost }
                        | ActivationCost::DiscardSource { mana: cost } => Some(cost),
                        ActivationCost::SacrificePermanent { mana, .. } => mana.as_ref(),
                        ActivationCost::ChooseObjects(cost) => cost.mana.as_ref(),
                        ActivationCost::Composite { costs } if costs.tap || costs.sacrifice => {
                            return 2;
                        }
                        ActivationCost::Composite { costs } => costs.mana.as_ref(),
                        _ => None,
                    };
                    let Some(cost) = mana else { return 1 };
                    // CR 107.4e-f: a hybrid announcement is not a fixed cost;
                    // an X of 0 is the cheapest announcement.
                    (cost.clone(), a.hybrid_mana.as_slice())
                }
                _ => unreachable!(),
            };
            let (pool, complete) = supply.get_or_init(|| intrinsic_supply(c, v));
            let pays_fixed = mtg_contracts::mana_spend::spend(&cost, pool).is_some()
                || alternative
                    .as_ref()
                    .is_some_and(|alt| mtg_contracts::mana_spend::spend(alt, pool).is_some());
            match (pays_fixed, hybrid.is_empty(), *complete) {
                (true, true, _) => 1,
                (false, _, true) => 0,
                _ => 2,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{catalog::ObservationCatalog, number_bound};
    use mtg_contracts::{
        CURRENT_IR_VERSION, ManaSpendRestriction, RULES_EDITION, RULES_SHA256, RestrictedMana,
    };
    use std::sync::Arc;

    fn announce_x() -> (ObservationCatalog, Observation) {
        let catalog = ObservationCatalog::new(Arc::new(Catalog {
            schema_version: CURRENT_IR_VERSION,
            rules_edition: RULES_EDITION.into(),
            rules_sha256: RULES_SHA256.into(),
            definitions: vec![],
        }));
        let player = PlayerId(0);
        let view = PermittedView {
            known_searched_hand: vec![],
            known_library_positions: vec![],
            revealed_library_cards: vec![],
            revealed_face_down: vec![],
            sacrifice_choices: vec![],
            setup: None,
            effects: vec![],
            combat: vec![],
            viewer: player,
            active: Some(player),
            turn_owner: player,
            turn: TurnState {
                number: 1,
                step: TurnStep::PrecombatMain,
                lands_played: 0,
            },
            live_turn_order: vec![player],
            commanders: vec![],
            priority: Some(player),
            players: vec![VisiblePlayer {
                life_frozen_until_next_turn: false,
                exile: vec![],
                library_shuffles: 0,
                poison_counters: 0.into(),
                energy_counters: 0.into(),
                player,
                status: PlayerStatus::Live,
                life: 40.into(),
                mana: BTreeMap::from([(Color::C, 2.into())]),
                restricted_mana: vec![],
                hand_count: 0,
                library_count: 0,
            }],
            objects: vec![],
            stack: vec![],
        };
        let observation = Observation {
            decision_context: None,
            observation_version: 1,
            view,
            decision: Some(Decision {
                id: DecisionToken("x".into()),
                player,
                reason: Reason::AnnounceX,
                candidates: vec![],
                shape: Shape::Number {
                    min: 0.into(),
                    max: None,
                },
            }),
        };
        (catalog, observation)
    }

    fn restricted_unit() -> RestrictedMana {
        RestrictedMana {
            restriction: ManaSpendRestriction::ActivatedAbility,
            color: Color::U,
            amount: 1.into(),
        }
    }

    #[test]
    fn restricted_mana_leaves_intrinsic_supply_incomplete() {
        let (catalog, mut observation) = announce_x();
        let pool = BTreeMap::from([(Color::C, 2.into())]);
        assert_eq!(
            intrinsic_supply(&catalog, &observation.view),
            (pool.clone(), true)
        );
        observation.view.players[0]
            .restricted_mana
            .push(restricted_unit());
        assert_eq!(intrinsic_supply(&catalog, &observation.view), (pool, false));
    }

    #[test]
    fn restricted_mana_does_not_cap_announced_x_at_the_unrestricted_pool() {
        let (catalog, mut observation) = announce_x();
        assert_eq!(number_bound(&catalog, &observation), Some(2.into()));
        observation.view.players[0]
            .restricted_mana
            .push(restricted_unit());
        assert_eq!(number_bound(&catalog, &observation), None);
    }
}
