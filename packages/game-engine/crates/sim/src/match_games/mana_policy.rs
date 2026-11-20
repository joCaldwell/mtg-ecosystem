//! Finite public-information planner for the v16 mana pool. No native state,
//! hidden hands, library order, receipts or query recovery data are consulted.
use super::*;
use mtg_contracts::{KeywordAbility, ManaProduction, SingleColorManaAmount};

#[derive(Clone)]
pub(super) struct PlannedMana {
    pub source: ObjectHandle,
    pub color: Option<Color>,
    pub combination: Option<[Quantity; 5]>,
}
#[derive(Clone)]
struct Source {
    handle: ObjectHandle,
    land: bool,
    amount: Quantity,
    production: ManaProduction,
    colors: Vec<Color>,
    cost: Option<ManaCost>,
}
struct Planner {
    sources: Vec<Source>,
    used: Vec<usize>,
    pool: BTreeMap<Color, Quantity>,
    plan: Vec<PlannedMana>,
}
impl Planner {
    fn fund(&mut self, cost: &ManaCost) -> bool {
        for _ in 0..=self.sources.len() {
            if spend(cost, &self.pool).is_some() {
                return true;
            }
            let mut required = BTreeMap::<Color, Quantity>::new();
            for c in &cost.colored {
                required.entry(*c).or_default().add(&1.into());
            }
            let deficit = required
                .iter()
                .find(|(c, n)| self.pool.get(c).cloned().unwrap_or_default() < **n)
                .map(|(c, _)| *c);
            // Prefer rocks/dorks to lands. Fixed multi-symbol production is
            // evaluated at its full quantity, including the Signet's input cost.
            let chosen = (0..self.sources.len())
                .filter(|i| !self.used.contains(i))
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
            self.used.push(i);
            let source = self.sources[i].clone();
            if let Some(cost) = &source.cost {
                if !self.fund(cost) {
                    return false;
                }
                let payment = spend(cost, &self.pool).expect("funded inner cost");
                for (c, n) in payment {
                    self.pool.get_mut(&c).unwrap().subtract(&n);
                }
            }
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
                    let symbols = match source.production {
                        ManaProduction::Fixed { symbols } => symbols,
                        _ => color.into_iter().collect(),
                    };
                    for c in symbols {
                        self.pool.entry(c).or_default().add(&source.amount);
                    }
                    None
                };
            self.plan.push(PlannedMana {
                source: source.handle,
                color,
                combination,
            });
        }
        false
    }
}
pub(super) fn plan(
    catalog: &Catalog,
    v: &mtg_core::PermittedView,
    cost: &ManaCost,
    exclude: Option<&ObjectHandle>,
) -> Option<std::collections::VecDeque<PlannedMana>> {
    let identity = v
        .commanders
        .iter()
        .find(|c| c.history.owner == v.viewer)
        .and_then(|c| definition(catalog, &c.oracle_id).ok())
        .map(|d| d.characteristics.color_identity.clone())
        .unwrap_or_default();
    let mut sources = vec![];
    for o in &v.objects {
        if o.zone != Zone::Battlefield
            || o.controller != v.viewer
            || o.phased_out
            || o.token.is_some()
            || exclude == Some(&o.handle)
        {
            continue;
        }
        let d = definition(catalog, &o.oracle_id).ok()?;
        let (production, cost, tap) = match d.mana_abilities.as_slice() {
            [ability]
                if ability.self_damage.is_none()
                    && !ability.costs.sacrifice
                    && !ability
                        .costs
                        .remove_source_plus_one_counters
                        .as_ref()
                        .is_some_and(|n| o.plus_one_counters < *n)
                    && !(ability.restriction.is_some() && o.mana_ability_used == Some(true)) =>
            {
                (
                    ability.production.clone(),
                    ability.costs.mana.clone(),
                    ability.costs.tap,
                )
            }
            [] if d.characteristics.kind == CardKind::Land && d.characteristics.basic => (
                ManaProduction::Fixed {
                    symbols: vec![*d.characteristics.color_identity.first()?],
                },
                None,
                true,
            ),
            _ => continue,
        };
        if tap
            && (o.tapped
                || (o
                    .characteristics
                    .as_ref()
                    .is_some_and(|c| c.has_type(CardKind::Creature))
                    && !o.attack_ready
                    && !o.keywords.contains(&KeywordAbility::Haste)))
        {
            continue;
        }
        let amount = match &production {
            ManaProduction::AnyOneColor {
                amount: SingleColorManaAmount::Fixed { quantity },
            } => quantity.clone(),
            ManaProduction::AnyOneColor {
                amount: SingleColorManaAmount::ControllerEnchantments,
            } => {
                let count = v
                    .objects
                    .iter()
                    .filter(|object| {
                        object.zone == Zone::Battlefield
                            && !object.phased_out
                            && object.controller == v.viewer
                            && object
                                .characteristics
                                .as_ref()
                                .is_some_and(|c| c.has_type(CardKind::Enchantment))
                    })
                    .count();
                Quantity::from(i64::try_from(count).ok()?)
            }
            ManaProduction::GreatestControlledPowerCombination => v
                .objects
                .iter()
                .filter(|object| {
                    object.zone == Zone::Battlefield
                        && !object.phased_out
                        && object.controller == v.viewer
                })
                .filter_map(|object| object.characteristics.as_ref())
                .filter(|c| c.has_type(CardKind::Creature))
                .filter_map(|c| c.power.as_ref())
                .max()
                .cloned()
                .unwrap_or_default(),
            _ => 1.into(),
        };
        if amount <= Quantity::zero() {
            continue;
        }
        let colors = match &production {
            ManaProduction::Fixed { symbols } => symbols.clone(),
            ManaProduction::GreatestControlledPowerCombination
            | ManaProduction::AnyColor
            | ManaProduction::AnyOneColor { .. } => {
                vec![Color::W, Color::U, Color::B, Color::R, Color::G]
            }
            ManaProduction::CommanderColor => identity.clone(),
            ManaProduction::Choice { colors } => colors.clone(),
            ManaProduction::OpponentLandColors | ManaProduction::EachPermanentColor { .. } => {
                continue;
            }
        };
        if colors.is_empty() {
            continue;
        }
        sources.push(Source {
            handle: o.handle.clone(),
            amount,
            land: d.characteristics.kind == CardKind::Land,
            production,
            colors,
            cost,
        });
    }
    let mut p = Planner {
        sources,
        used: vec![],
        pool: v
            .players
            .iter()
            .find(|p| p.player == v.viewer)?
            .mana
            .clone(),
        plan: vec![],
    };
    p.fund(cost).then(|| p.plan.into())
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
        let mut planner = Planner {
            sources: vec![Source {
                handle: ObjectHandle("two-mana-provider".into()),
                land: false,
                amount: 2.into(),
                production: ManaProduction::GreatestControlledPowerCombination,
                colors: mtg_contracts::MANA_COMBINATION_COLORS.to_vec(),
                cost: None,
            }],
            used: vec![],
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
        let mut strategy = Strategy {
            catalog: &catalog,
            policy: Policy::Develop,
            mulligans: [0; 4],
            mana_plan: Default::default(),
            mana_color: None,
            mana_combination: None,
            hybrid_choices: vec![],
        };
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
