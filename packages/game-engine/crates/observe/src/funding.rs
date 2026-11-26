//! Funding codes certify what the choosing player can establish from the
//! published pool and fixed-color intrinsic lands (CR 106, 302.6, 305.6).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_contracts::{
    ActivationCost, CardKind, Catalog, Color, KeywordAbility, Quantity, basic_land_color,
};
use mtg_view::*;
use std::collections::BTreeMap;

/// The pool is a lower bound. Ambiguous sources keep a failed payment unknown:
/// printed mana can have costs/riders and tokens need not have catalog rows.
pub(super) fn intrinsic_supply(
    catalog: &Catalog,
    view: &PermittedView,
) -> (BTreeMap<Color, Quantity>, bool) {
    let Some(payer) = view.players.iter().find(|p| p.player == view.viewer) else {
        return (BTreeMap::new(), false);
    };
    let mut pool = payer.mana.clone();
    let mut complete = true;
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
                    (cost.clone(), &[][..])
                }
                _ => unreachable!(),
            };
            let (pool, complete) = supply.get_or_init(|| intrinsic_supply(c, v));
            let pays_fixed = mtg_contracts::mana_spend::spend(&cost, pool).is_some();
            match (pays_fixed, hybrid.is_empty(), *complete) {
                (true, true, _) => 1,
                (false, _, true) => 0,
                _ => 2,
            }
        })
        .collect()
}
