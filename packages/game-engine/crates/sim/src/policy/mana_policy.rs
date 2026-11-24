//! Finite public-information planner for the v16 mana pool. No native state,
//! hidden hands, library order, receipts or query recovery data are consulted.
use super::*;
use mtg_contracts::{KeywordAbility, ManaAbility, ManaProduction, basic_land_color};
use mtg_core::{ManaAbilityIdentity, PermittedView};

#[derive(Clone)]
pub(super) struct PlannedMana {
    pub source: ObjectHandle,
    pub color: Option<Color>,
    pub ability: Option<ManaAbilityIdentity>,
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
}
struct Planner<'a> {
    sources: Vec<Source<'a>>,
    used: Vec<bool>,
    pool: BTreeMap<Color, Quantity>,
    spell_only: BTreeMap<Color, Quantity>,
    plan: Vec<PlannedMana>,
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
                ManaProduction::Fixed { .. } => None,
                _ => deficit.or_else(|| source.colors.first().copied()),
            };
            let symbols = match &source.production {
                ManaProduction::Fixed { symbols } => symbols.as_slice(),
                _ => color.as_slice(),
            };
            for &c in symbols {
                self.pool.entry(c).or_default().add(&source.amount);
            }
            self.plan.push(PlannedMana {
                source: source.handle.clone(),
                ability: Some(source.ability),
                color,
            });
        }
        false
    }

    fn fund_source_cost(&mut self, index: usize) -> Option<usize> {
        let Some(cost) = self.sources[index].cost.clone() else {
            return Some(index);
        };
        if spend(&cost, &self.pool).is_none() && !self.fund(&cost, true) {
            return None;
        }
        for (color, quantity) in spend(&cost, &self.pool)? {
            self.pool.get_mut(&color)?.subtract(&quantity);
        }
        Some(index)
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
    !ability.costs.sacrifice
        && (!ability.costs.tap || available_tap(object))
        && ability
            .costs
            .remove_source_plus_one_counters
            .as_ref()
            .is_none_or(|n| object.plus_one_counters >= *n)
        && !(ability.restriction.is_some() && object.mana_ability_used == Some(true))
}

fn production_colors(catalog: &Catalog, view: &PermittedView, mana: &ManaProduction) -> Vec<Color> {
    match mana {
        ManaProduction::Fixed { symbols } => symbols.clone(),
        ManaProduction::AnyColor => vec![Color::W, Color::U, Color::B, Color::R, Color::G],
        ManaProduction::CommanderColor => commander_colors(catalog, view).to_vec(),
    }
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
            if !usable_printed(ability, o) {
                continue;
            }
            let colors = production_colors(catalog, view, &ability.production);
            if colors.is_empty() {
                continue;
            }
            sources.push(Source {
                object_index,
                ability: ManaAbilityIdentity::Printed {
                    index: index as u32,
                },
                handle: &o.handle,
                amount: 1.into(),
                land,
                production: ability.production.clone(),
                colors,
                cost: ability.costs.mana.clone(),
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
        sources,
        used: vec![false; view.objects.len()],
        pool: nested_pool,
        spell_only,
        plan: vec![],
    };
    planner.fund(cost, false).then(|| planner.plan.into())
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
    fn any_color_source_is_planned_in_the_missing_color() {
        let source = ObjectHandle("any-color-provider".into());
        let mut planner = Planner {
            sources: vec![Source {
                object_index: 0,
                ability: ManaAbilityIdentity::Printed { index: 0 },
                handle: &source,
                land: false,
                amount: 1.into(),
                production: ManaProduction::AnyColor,
                colors: vec![Color::W, Color::U, Color::B, Color::R, Color::G],
                cost: None,
            }],
            used: vec![false],
            pool: BTreeMap::new(),
            spell_only: BTreeMap::new(),
            plan: vec![],
        };
        let cost = ManaCost {
            generic: 0.into(),
            colored: vec![Color::B],
        };
        assert!(planner.fund(&cost, false));
        assert_eq!(planner.plan[0].color, Some(Color::B));
        assert!(spend(&cost, &planner.pool).is_some());
    }
}
