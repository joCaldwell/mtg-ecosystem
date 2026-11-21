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
    cost: Option<ManaCost>,
    filter_input: Option<Color>,
}
struct Planner<'a> {
    sources: Vec<Source<'a>>,
    used: Vec<bool>,
    pool: BTreeMap<Color, Quantity>,
    spell_only: BTreeMap<Color, Quantity>,
    plan: Vec<PlannedMana>,
    filter_retries: usize,
}
impl Planner<'_> {
    fn balance(&self, color: &Color, nested: bool) -> Quantity {
        let mut amount = self.pool.get(color).cloned().unwrap_or_default();
        if !nested && let Some(spell) = self.spell_only.get(color) {
            amount.add(spell);
        }
        amount
    }

    fn can_fund(&self, cost: &ManaCost, nested: bool) -> bool {
        let mut pool = self.pool.clone();
        if !nested {
            for (color, amount) in &self.spell_only {
                pool.entry(*color).or_default().add(amount);
            }
        }
        spend(cost, &pool).is_some()
    }

    fn fund(&mut self, cost: &ManaCost, nested: bool) -> bool {
        let mut required = BTreeMap::<Color, Quantity>::new();
        for c in &cost.colored {
            required.entry(*c).or_default().add(&1.into());
        }
        for _ in 0..=self.sources.len() {
            if self.can_fund(cost, nested) {
                return true;
            }
            let deficit = required
                .iter()
                .find(|(c, n)| self.balance(c, nested) < **n)
                .map(|(c, _)| *c);
            // Prefer rocks/dorks to lands. Fixed multi-symbol production is
            // evaluated at its full quantity, including the Signet's input cost.
            let chosen = (0..self.sources.len())
                .filter(|i| !self.used[self.sources[*i].object_index])
                .filter(|i| !nested || !spell_only_source(&self.sources[*i].production))
                .filter(|i| deficit.is_none_or(|c| self.sources[*i].colors.contains(&c)))
                .min_by_key(|i| {
                    (
                        self.sources[*i].colors.len(),
                        self.sources[*i].land,
                        usize::from(self.sources[*i].cost.is_some()),
                        self.sources[*i]
                            .cost
                            .as_ref()
                            .is_some_and(|cost| spend(cost, &self.pool).is_none()),
                        self.sources[*i]
                            .filter_input
                            .is_some_and(|color| Some(color) == deficit),
                        *i,
                    )
                });
            let Some(i) = chosen else { return false };
            self.used[self.sources[i].object_index] = true;
            let Some(i) = self.fund_source_cost(i) else {
                return false;
            };
            let source = &self.sources[i];
            let color = match source.production {
                ManaProduction::Fixed { .. }
                | ManaProduction::FixedWithLifeGain { .. }
                | ManaProduction::RestrictedFixed { .. } => None,
                ManaProduction::HybridFilter { .. } => source.filter_input,
                _ => deficit.or_else(|| source.colors.first().copied()),
            };
            let combination = if matches!(
                source.production,
                ManaProduction::GreatestControlledPowerCombination
                    | ManaProduction::HybridFilter { .. }
            ) {
                let mut amounts = std::array::from_fn(|_| Quantity::zero());
                let mut left = source.amount.clone();
                for (i, c) in mtg_contracts::MANA_COMBINATION_COLORS.iter().enumerate() {
                    if !source.colors.contains(c) {
                        continue;
                    }
                    let mut need = required.get(c).cloned().unwrap_or_default();
                    need.subtract(&self.balance(c, nested));
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
                    ManaProduction::Fixed { symbols }
                    | ManaProduction::FixedWithLifeGain { symbols, .. }
                    | ManaProduction::RestrictedFixed { symbols, .. } => symbols.as_slice(),
                    _ => color.as_slice(),
                };
                let pool = if spell_only_source(&source.production) {
                    &mut self.spell_only
                } else {
                    &mut self.pool
                };
                for &c in symbols {
                    pool.entry(c).or_default().add(&source.amount);
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

    fn fund_source_cost(&mut self, index: usize) -> Option<usize> {
        let Some(cost) = self.sources[index].cost.clone() else {
            return Some(index);
        };
        let mut funded = index;
        let mut cost = cost;
        if spend(&cost, &self.pool).is_none() {
            let alternate = self.sources[index].filter_input.and_then(|input| {
                let source = &self.sources[index];
                self.sources.iter().position(|other| {
                    other.object_index == source.object_index
                        && other.ability == source.ability
                        && other.filter_input.is_some_and(|color| color != input)
                })
            });
            let checkpoint =
                alternate.map(|_| (self.used.clone(), self.pool.clone(), self.plan.len()));
            if !self.fund(&cost, true) {
                // Charge retries across the entire search, including any
                // recursively attempted branch that is rolled back.
                let (used, pool, length) = checkpoint?;
                self.filter_retries = self.filter_retries.checked_sub(1)?;
                self.used = used;
                self.pool = pool;
                self.plan.truncate(length);
                funded = alternate?;
                cost = self.sources[funded].cost.clone()?;
                if !self.fund(&cost, true) {
                    return None;
                }
            }
        }
        for (color, quantity) in spend(&cost, &self.pool)? {
            self.pool.get_mut(&color)?.subtract(&quantity);
        }
        Some(funded)
    }
}
fn spell_only_source(mana: &ManaProduction) -> bool {
    matches!(mana, ManaProduction::RestrictedFixed { spend, .. } if !spend.permits_activation(false))
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
        ManaProduction::Fixed { symbols } | ManaProduction::FixedWithLifeGain { symbols, .. } => {
            symbols.clone()
        }
        ManaProduction::Choice { colors } => colors.clone(),
        ManaProduction::RestrictedFixed { symbols, .. } => symbols.clone(),
        ManaProduction::HybridFilter { colors } => {
            amount = Quantity::from(2);
            colors.to_vec()
        }
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

#[derive(Clone, Copy)]
pub(super) enum ManaPurpose {
    Activate { battlefield_creature: bool },
    Cast(mtg_contracts::ManaSpellTraits),
}

impl ManaPurpose {
    pub(super) fn activation(view: &PermittedView, source: &ObjectHandle) -> Self {
        Self::Activate {
            battlefield_creature: view.objects.iter().any(|o| {
                &o.handle == source
                    && controlled_permanent(view, o)
                    && o.characteristics
                        .as_ref()
                        .is_some_and(|c| c.has_type(CardKind::Creature))
            }),
        }
    }

    fn permits(self, spend: mtg_contracts::ManaSpendRestriction) -> bool {
        match self {
            Self::Activate {
                battlefield_creature,
            } => spend.permits_activation(battlefield_creature),
            Self::Cast(spell) => spend.permits_spell(spell),
        }
    }
}

pub(super) fn spendable_pool(
    view: &PermittedView,
    purpose: Option<ManaPurpose>,
) -> Option<BTreeMap<Color, Quantity>> {
    let player = view.players.iter().find(|p| p.player == view.viewer)?;
    let mut pool = player.mana.clone();
    if let Some(purpose) = purpose {
        for unit in &player.restricted_mana {
            if purpose.permits(unit.restriction) {
                pool.entry(unit.color).or_default().add(&unit.amount);
            }
        }
    }
    Some(pool)
}

pub(super) fn plan_for(
    catalog: &Catalog,
    view: &PermittedView,
    cost: &ManaCost,
    exclude: Option<&ObjectHandle>,
    purpose: Option<ManaPurpose>,
) -> Option<std::collections::VecDeque<PlannedMana>> {
    let pool = spendable_pool(view, purpose)?;
    if spend(cost, &pool).is_some() {
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
            if !usable_printed(ability, o)
                || matches!(ability.production, ManaProduction::RestrictedFixed { spend, .. }
                    if purpose.is_none_or(|purpose| !purpose.permits(spend)))
            {
                continue;
            }
            let (production, colors, amount) = production(catalog, view, &ability.production);
            if colors.is_empty() || amount <= Quantity::zero() {
                continue;
            }
            let mut push_source = |cost, filter_input| {
                sources.push(Source {
                    object_index,
                    ability: ManaAbilityIdentity::Printed {
                        index: index as u32,
                    },
                    handle: &o.handle,
                    amount: amount.clone(),
                    land,
                    production: production.clone(),
                    colors: colors.clone(),
                    cost,
                    filter_input,
                })
            };
            if let ManaProduction::HybridFilter { colors } = production {
                for input in colors {
                    push_source(
                        Some(ManaCost {
                            generic: Quantity::zero(),
                            colored: vec![input],
                        }),
                        Some(input),
                    );
                }
            } else {
                push_source(ability.costs.mana.clone(), None);
            }
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
                    filter_input: None,
                });
            }
        }
    }
    let player = view.players.iter().find(|p| p.player == view.viewer)?;
    let mut nested_pool = pool;
    let mut spell_only = BTreeMap::<Color, Quantity>::new();
    for unit in &player.restricted_mana {
        if !unit.restriction.permits_activation(false)
            && purpose.is_some_and(|purpose| purpose.permits(unit.restriction))
        {
            nested_pool.get_mut(&unit.color)?.subtract(&unit.amount);
            spell_only.entry(unit.color).or_default().add(&unit.amount);
        }
    }
    let mut planner = Planner {
        filter_retries: sources.len(),
        sources,
        used: vec![false; view.objects.len()],
        pool: nested_pool,
        spell_only,
        plan: vec![],
    };
    planner.fund(cost, false).then(|| planner.plan.into())
}

/// Plan against the complete fixed price as each printed hybrid pip is
/// selected. A life payment is available only on its printed symbol.
pub(crate) fn spend_response(
    cost: &ManaCost,
    available: &BTreeMap<Color, Quantity>,
    restricted_available: &[mtg_contracts::RestrictedMana],
) -> Option<mtg_core::PlayerResponse> {
    let mut combined = available.clone();
    for unit in restricted_available {
        combined.entry(unit.color).or_default().add(&unit.amount);
    }
    let mut spending = spend(cost, &combined)?;
    let mut restricted = vec![];
    for unit in restricted_available {
        if let Some(amount) = spending.get_mut(&unit.color) {
            let used = unit.amount.clone().min(amount.clone());
            if used > Quantity::zero() {
                amount.subtract(&used);
                restricted.push(mtg_contracts::RestrictedMana {
                    amount: used,
                    ..unit.clone()
                });
            }
        }
    }
    if restricted.is_empty() {
        return Some(mtg_core::PlayerResponse::SpendMana { spending });
    }
    spending.retain(|_, amount| *amount > Quantity::zero());
    Some(mtg_core::PlayerResponse::SpendManaAllocated {
        spending,
        restricted,
    })
}

pub(super) fn hybrid_plan(
    catalog: &Catalog,
    view: &mtg_core::PermittedView,
    cost: ManaCost,
    symbols: &[mtg_contracts::ColorHybridSymbol],
) -> Option<(
    Vec<mtg_contracts::HybridPayment>,
    std::collections::VecDeque<PlannedMana>,
)> {
    hybrid_plan_for(catalog, view, cost, symbols, None)
}

pub(super) fn hybrid_plan_for(
    catalog: &Catalog,
    view: &mtg_core::PermittedView,
    mut cost: ManaCost,
    symbols: &[mtg_contracts::ColorHybridSymbol],
    purpose: Option<ManaPurpose>,
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
                let payable = plan_for(catalog, view, &cost, None, purpose).is_some();
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
    Some((choices, plan_for(catalog, view, &cost, None, purpose)?))
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
                filter_input: None,
            }],
            used: vec![false],
            pool: BTreeMap::new(),
            spell_only: BTreeMap::new(),
            plan: vec![],
            filter_retries: 1,
        };
        let cost = ManaCost {
            generic: 0.into(),
            colored: vec![Color::W, Color::U],
        };
        assert!(planner.fund(&cost, false));
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
                    restricted_mana: vec![],
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

    #[test]
    fn hybrid_filter_mana_retries_the_unfundable_input_before_using_the_output() {
        let filter = ObjectHandle("filter".into());
        let land = ObjectHandle("colored-provider".into());
        let source = |color| Source {
            object_index: 0,
            ability: ManaAbilityIdentity::Printed { index: 1 },
            handle: &filter,
            land: true,
            amount: 2.into(),
            production: ManaProduction::HybridFilter {
                colors: [Color::U, Color::R],
            },
            colors: vec![Color::U, Color::R],
            cost: Some(ManaCost {
                generic: 0.into(),
                colored: vec![color],
            }),
            filter_input: Some(color),
        };
        let mut planner = Planner {
            sources: vec![
                source(Color::U),
                source(Color::R),
                Source {
                    object_index: 1,
                    ability: ManaAbilityIdentity::Printed { index: 0 },
                    handle: &land,
                    land: true,
                    amount: 1.into(),
                    production: ManaProduction::Choice {
                        colors: vec![Color::U, Color::W, Color::G],
                    },
                    colors: vec![Color::U, Color::W, Color::G],
                    cost: None,
                    filter_input: None,
                },
            ],
            used: vec![false; 2],
            pool: BTreeMap::new(),
            spell_only: BTreeMap::new(),
            plan: vec![],
            filter_retries: 3,
        };
        assert!(planner.fund(
            &ManaCost {
                generic: 0.into(),
                colored: vec![Color::U, Color::U]
            },
            false
        ));
        assert_eq!(planner.plan.len(), 2);
        assert_eq!(planner.plan[0].source, land);
        assert_eq!(planner.plan[1].source, filter);
        assert_eq!(planner.plan[1].color, Some(Color::U));
        assert_eq!(
            planner.plan[1].combination,
            Some([0_i64, 2, 0, 0, 0].map(Quantity::from))
        );
    }
}
