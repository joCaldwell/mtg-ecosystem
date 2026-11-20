//! Finite public-information planner for the v16 mana pool. No native state,
//! hidden hands, library order, receipts or query recovery data are consulted.
use super::*;
use mtg_contracts::{
    KeywordAbility, ManaAbility, ManaProduction, SingleColorManaAmount, basic_land_color,
};
use mtg_core::{ManaAbilityIdentity, PermittedView};

#[derive(Clone)]
pub(super) struct PlannedMana {
    pub source: ObjectHandle,
    pub color: Option<Color>,
    pub ability: Option<ManaAbilityIdentity>,
    pub combination: Option<[Quantity; 5]>,
}
struct Source<'a> {
    object_index: usize,
    ability: ManaAbilityIdentity,
    handle: &'a ObjectHandle,
    land: bool,
    amount: Quantity,
    production: ManaProduction,
    colors: Vec<Color>,
    cost: Option<&'a ManaCost>,
}
struct Planner<'a> {
    sources: Vec<Source<'a>>,
    used: Vec<bool>,
    pool: BTreeMap<Color, Quantity>,
    plan: Vec<PlannedMana>,
}
impl Planner<'_> {
    fn fund(&mut self, cost: &ManaCost) -> bool {
        let mut required = BTreeMap::<Color, Quantity>::new();
        for c in &cost.colored {
            required.entry(*c).or_default().add(&1.into());
        }
        for _ in 0..=self.sources.len() {
            if spend(cost, &self.pool).is_some() {
                return true;
            }
            let deficit = required
                .iter()
                .find(|(c, n)| self.pool.get(c).is_none_or(|amount| amount < *n))
                .map(|(c, _)| *c);
            // Prefer rocks/dorks to lands. Fixed multi-symbol production is
            // evaluated at its full quantity, including the Signet's input cost.
            let chosen = (0..self.sources.len())
                .filter(|i| !self.used[self.sources[*i].object_index])
                .filter(|i| deficit.is_none_or(|c| self.sources[*i].colors.contains(&c)))
                .min_by_key(|i| {
                    (
                        self.sources[*i].colors.len(),
                        self.sources[*i].land,
                        usize::from(self.sources[*i].cost.is_some()),
                        *i,
                    )
                });
            let Some(i) = chosen else { return false };
            self.used[self.sources[i].object_index] = true;
            if let Some(cost) = self.sources[i].cost {
                if !self.fund(cost) {
                    return false;
                }
                let payment = spend(cost, &self.pool).expect("funded inner cost");
                for (c, n) in payment {
                    self.pool.get_mut(&c).unwrap().subtract(&n);
                }
            }
            let source = &self.sources[i];
            let color = match source.production {
                ManaProduction::Fixed { .. } => None,
                _ => deficit.or_else(|| source.colors.first().copied()),
            };
            let combination =
                if source.production == ManaProduction::GreatestControlledPowerCombination {
                    let mut amounts = std::array::from_fn(|_| Quantity::zero());
                    let mut left = source.amount.clone();
                    for (i, c) in mtg_contracts::MANA_COMBINATION_COLORS.iter().enumerate() {
                        if !source.colors.contains(c) {
                            continue;
                        }
                        let mut need = required.get(c).cloned().unwrap_or_default();
                        need.subtract(&self.pool.get(c).cloned().unwrap_or_default());
                        amounts[i] = need.max(Quantity::zero()).min(left.clone());
                        left.subtract(&amounts[i]);
                    }
                    let Some(surplus) = mtg_contracts::MANA_COMBINATION_COLORS
                        .iter()
                        .position(|c| source.colors.contains(c))
                    else {
                        return false;
                    };
                    amounts[surplus].add(&left);
                    for (c, n) in mtg_contracts::MANA_COMBINATION_COLORS
                        .into_iter()
                        .zip(&amounts)
                    {
                        if *n > Quantity::zero() {
                            self.pool.entry(c).or_default().add(n);
                        }
                    }
                    Some(amounts)
                } else {
                    let symbols = match &source.production {
                        ManaProduction::Fixed { symbols } => symbols.as_slice(),
                        _ => color.as_slice(),
                    };
                    for &c in symbols {
                        self.pool.entry(c).or_default().add(&source.amount);
                    }
                    None
                };
            self.plan.push(PlannedMana {
                source: source.handle.clone(),
                ability: Some(source.ability),
                color,
                combination,
            });
        }
        false
    }
}
fn commander_colors<'a>(catalog: &'a Catalog, view: &PermittedView) -> &'a [Color] {
    view.commanders
        .iter()
        .find(|c| c.history.owner == view.viewer)
        .and_then(|c| definition(catalog, &c.oracle_id).ok())
        .map_or(&[], |d| &d.characteristics.color_identity)
}

fn controlled_permanent(view: &PermittedView, object: &mtg_core::VisibleObject) -> bool {
    object.zone == Zone::Battlefield && !object.phased_out && object.controller == view.viewer
}

fn available_tap(object: &mtg_core::VisibleObject) -> bool {
    !object.tapped
        && (!object
            .characteristics
            .as_ref()
            .is_some_and(|c| c.has_type(CardKind::Creature))
            || object.attack_ready
            || object.keywords.contains(&KeywordAbility::Haste))
}

fn usable_printed(ability: &ManaAbility, object: &mtg_core::VisibleObject) -> bool {
    ability.self_damage.is_none()
        && !ability.costs.sacrifice
        && (!ability.costs.tap || available_tap(object))
        && ability
            .costs
            .remove_source_plus_one_counters
            .as_ref()
            .is_none_or(|n| object.plus_one_counters >= *n)
        && !(ability.restriction.is_some() && object.mana_ability_used == Some(true))
}

fn production(
    catalog: &Catalog,
    view: &PermittedView,
    mana: &ManaProduction,
) -> (ManaProduction, Vec<Color>, Quantity) {
    let mut amount = Quantity::from(1);
    let colors = match mana {
        ManaProduction::Fixed { symbols } => symbols.clone(),
        ManaProduction::Choice { colors } => colors.clone(),
        ManaProduction::AnyColor => mtg_contracts::MANA_COMBINATION_COLORS.to_vec(),
        ManaProduction::AnyOneColor {
            amount: single_color,
        } => {
            amount = match single_color {
                SingleColorManaAmount::Fixed { quantity } => quantity.clone(),
                SingleColorManaAmount::ControllerEnchantments => {
                    let count = view
                        .objects
                        .iter()
                        .filter(|o| {
                            controlled_permanent(view, o)
                                && o.characteristics
                                    .as_ref()
                                    .is_some_and(|c| c.has_type(CardKind::Enchantment))
                        })
                        .count();
                    // An unrepresentable count is planned conservatively as no mana.
                    Quantity::from(i64::try_from(count).unwrap_or_default())
                }
            };
            mtg_contracts::MANA_COMBINATION_COLORS.to_vec()
        }
        ManaProduction::CommanderColor => commander_colors(catalog, view).to_vec(),
        ManaProduction::GreatestControlledPowerCombination => {
            amount = view
                .objects
                .iter()
                .filter(|o| controlled_permanent(view, o))
                .filter_map(|o| o.characteristics.as_ref())
                .filter(|c| c.has_type(CardKind::Creature))
                .filter_map(|c| c.power.as_ref())
                .max()
                .cloned()
                .unwrap_or_default();
            mtg_contracts::MANA_COMBINATION_COLORS.to_vec()
        }
        ManaProduction::EachPermanentColor {
            scope: mtg_contracts::PermanentColorScope::Controller,
        } => {
            let colors: Vec<_> = mtg_contracts::MANA_COMBINATION_COLORS
                .into_iter()
                .filter(|color| {
                    view.objects.iter().any(|o| {
                        controlled_permanent(view, o)
                            && o.characteristics
                                .as_ref()
                                .is_some_and(|c| c.colors.contains(color))
                    })
                })
                .collect();
            return (
                ManaProduction::Fixed {
                    symbols: colors.clone(),
                },
                colors,
                amount,
            );
        }
        ManaProduction::OpponentLandColors => {
            // Basic land subtypes provide a conservative lower bound on the
            // opponent colors. Never assume that an ungrounded cycle adds any.
            mtg_contracts::MANA_COMBINATION_COLORS
                .into_iter()
                .filter(|color| {
                    view.objects.iter().any(|o| {
                        o.zone == Zone::Battlefield
                            && !o.phased_out
                            && o.controller != view.viewer
                            && view.live_turn_order.contains(&o.controller)
                            && view.live_turn_order.contains(&o.owner)
                            && o.characteristics.as_ref().is_some_and(|c| {
                                c.has_type(CardKind::Land)
                                    && c.subtypes
                                        .iter()
                                        .any(|s| basic_land_color(s) == Some(*color))
                            })
                    })
                })
                .collect()
        }
    };
    (mana.clone(), colors, amount)
}

pub(super) fn plan(
    catalog: &Catalog,
    view: &PermittedView,
    cost: &ManaCost,
    exclude: Option<&ObjectHandle>,
) -> Option<std::collections::VecDeque<PlannedMana>> {
    let pool = &view.players.iter().find(|p| p.player == view.viewer)?.mana;
    if spend(cost, pool).is_some() {
        return Some(Default::default());
    }
    let mut sources = vec![];
    for (object_index, o) in view.objects.iter().enumerate() {
        if !controlled_permanent(view, o) || o.token.is_some() || exclude == Some(&o.handle) {
            continue;
        }
        let Ok(d) = definition(catalog, &o.oracle_id) else {
            continue;
        };
        let land = d.characteristics.has_type(CardKind::Land);
        for (index, ability) in d.mana_abilities.iter().enumerate() {
            if !usable_printed(ability, o) {
                continue;
            }
            let (production, colors, amount) = production(catalog, view, &ability.production);
            if colors.is_empty() || amount <= Quantity::zero() {
                continue;
            }
            sources.push(Source {
                object_index,
                ability: ManaAbilityIdentity::Printed {
                    index: index as u32,
                },
                handle: &o.handle,
                amount,
                land,
                production,
                colors,
                cost: ability.costs.mana.as_ref(),
            });
        }
        if land && available_tap(o) {
            let c = o.characteristics.as_ref().unwrap_or(&d.characteristics);
            for color in c
                .subtypes
                .iter()
                .filter_map(|subtype| basic_land_color(subtype))
            {
                sources.push(Source {
                    object_index,
                    ability: ManaAbilityIdentity::Intrinsic { color },
                    handle: &o.handle,
                    amount: 1.into(),
                    land,
                    production: ManaProduction::Fixed {
                        symbols: vec![color],
                    },
                    colors: vec![color],
                    cost: None,
                });
            }
        }
    }
    let mut planner = Planner {
        sources,
        used: vec![false; view.objects.len()],
        pool: pool.clone(),
        plan: vec![],
    };
    planner.fund(cost).then(|| planner.plan.into())
}

/// Plan against the complete fixed price as each printed hybrid pip is
/// selected. A life payment is available only on its printed symbol.
pub(super) fn hybrid_plan(
    catalog: &Catalog,
    view: &mtg_core::PermittedView,
    mut cost: ManaCost,
    symbols: &[mtg_contracts::ColorHybridSymbol],
) -> Option<(
    Vec<mtg_contracts::HybridPayment>,
    std::collections::VecDeque<PlannedMana>,
)> {
    use mtg_contracts::HybridPayment;
    let payer = view.players.iter().find(|p| p.player == view.viewer)?;
    let mut life = payer.life.clone();
    let mut choices = Vec::with_capacity(symbols.len());
    for symbol in symbols {
        if let Some(color) = symbol
            .colors
            .iter()
            .filter_map(|choice| choice.color())
            .find(|color| {
                cost.colored.push(*color);
                let payable = plan(catalog, view, &cost, None).is_some();
                cost.colored.pop();
                payable
            })
        {
            choices.push(HybridPayment::Mana(color));
            cost.colored.push(color);
        } else if symbol.colors.contains(&HybridPayment::PayTwoLife)
            && !payer.life_frozen_until_next_turn
            && life >= 2.into()
        {
            choices.push(HybridPayment::PayTwoLife);
            life.subtract(&2.into());
        } else {
            return None;
        }
    }
    Some((choices, plan(catalog, view, &cost, None)?))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn combination_source_covers_two_distinct_colored_pips() {
        let source = ObjectHandle("two-mana-provider".into());
        let mut planner = Planner {
            sources: vec![Source {
                object_index: 0,
                ability: ManaAbilityIdentity::Printed { index: 0 },
                handle: &source,
                land: false,
                amount: 2.into(),
                production: ManaProduction::GreatestControlledPowerCombination,
                colors: mtg_contracts::MANA_COMBINATION_COLORS.to_vec(),
                cost: None,
            }],
            used: vec![false],
            pool: BTreeMap::new(),
            plan: vec![],
        };
        let cost = ManaCost {
            generic: 0.into(),
            colored: vec![Color::W, Color::U],
        };
        assert!(planner.fund(&cost));
        assert_eq!(
            planner.plan[0].combination,
            Some([1_i64, 1, 0, 0, 0].map(Quantity::from))
        );
        assert!(spend(&cost, &planner.pool).is_some());
    }
    #[test]
    fn tokens_without_oracle_ids_do_not_disable_casts_or_sacrifice_draws() {
        let catalog = mtg_compiler::catalog::embedded_catalog("mana").unwrap();
        let player = PlayerId(0);
        let card = |name: &str, zone: Zone| {
            let d = catalog.definitions.iter().find(|d| d.name == name).unwrap();
            mtg_core::VisibleObject {
                copied_values: None,
                face_down: false,
                exiled_by: None,
                phased_out: false,
                entered_this_turn: false,
                mana_ability_used: None,
                attached_to: None,
                attachments: vec![],
                keywords: vec![],
                restrictions: vec![],
                token: None,
                plus_one_counters: Quantity::zero(),
                lore_counters: 0.into(),
                characteristics: Some(d.characteristics.clone()),
                marked_damage: Quantity::zero(),
                attack_ready: true,
                handle: ObjectHandle(name.into()),
                oracle_id: d.oracle_id.clone(),
                owner: player,
                controller: player,
                zone,
                tapped: false,
            }
        };
        let mut token = card("Grizzly Bears", Zone::Battlefield);
        token.oracle_id.clear();
        token.handle = ObjectHandle("token".into());
        token.token = Some(mtg_core::VisibleToken {
            name: "Saproling Token".into(),
            keywords: vec![],
        });
        let mut observation = Observation {
            observation_version: 25,
            decision_context: None,
            decision: None,
            view: mtg_core::PermittedView {
                known_searched_hand: vec![],
                known_library_positions: vec![],
                sacrifice_choices: vec![],
                setup: None,
                effects: vec![],
                combat: vec![],
                viewer: player,
                active: Some(player),
                turn_owner: player,
                turn: mtg_core::TurnState {
                    number: 20,
                    step: TurnStep::PostcombatMain,
                    lands_played: 1,
                },
                live_turn_order: vec![player],
                commanders: vec![],
                priority: Some(player),
                players: vec![mtg_core::VisiblePlayer {
                    life_frozen_until_next_turn: false,
                    exile: vec![],
                    library_shuffles: 0,
                    player,
                    status: mtg_core::PlayerStatus::Live,
                    life: 40.into(),
                    mana: BTreeMap::new(),
                    hand_count: 1,
                    library_count: 4,
                }],
                objects: vec![
                    token.clone(),
                    card("Sol Ring", Zone::Battlefield),
                    card("Mind Stone", Zone::Hand { owner: player }),
                ],
                stack: vec![],
            },
        };
        let mut strategy = Strategy::new(&catalog, Policy::Develop, [0; 4]);
        assert_eq!(
            strategy.priority(&observation).unwrap(),
            PlayerResponse::Cast {
                card: ObjectHandle("Mind Stone".into())
            }
        );
        assert_eq!(strategy.mana_plan.len(), 1);
        assert_eq!(
            strategy.mana_plan[0].source,
            ObjectHandle("Sol Ring".into())
        );
        observation.view.objects = vec![token, card("Commander's Sphere", Zone::Battlefield)];
        assert_eq!(
            strategy.priority(&observation).unwrap(),
            PlayerResponse::ActivateAbility {
                object: ObjectHandle("Commander's Sphere".into()),
                ability: 0
            }
        );
    }
}
