//! Finite public-information planner for the v16 mana pool. No native state,
//! hidden hands, library order, receipts or query recovery data are consulted.
use super::*;
use mtg_contracts::{
    KeywordAbility, ManaAbility, ManaLandController, ManaProduction, basic_land_color,
};
use mtg_core::{ManaAbilityIdentity, PermittedView};
use std::collections::BTreeSet;

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
    /// Pays life or performs a rider (damage, life gain): used last.
    harm: bool,
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
                        self.sources[*i].harm,
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

/// CR 903.4: the combined color identity of `player`'s commanders, as the
/// engine reads it from their public commander histories.
fn commander_colors(view: &PermittedView, player: PlayerId) -> Vec<Color> {
    view.commanders
        .iter()
        .filter(|c| c.history.owner == player)
        .flat_map(|c| c.history.color_identity.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn controlled_permanent(view: &PermittedView, object: &mtg_core::VisibleObject) -> bool {
    object.zone == Zone::Battlefield && !object.phased_out && object.controller == view.viewer
}

pub(crate) fn available_tap(object: &mtg_core::VisibleObject) -> bool {
    !object.tapped
        && (!object
            .characteristics
            .as_ref()
            .is_some_and(|c| c.has_type(CardKind::Creature))
            || object.attack_ready
            || object.keywords.contains(&KeywordAbility::Haste))
}

fn usable_printed(
    ability: &ManaAbility,
    object: &mtg_core::VisibleObject,
    life: &Quantity,
) -> bool {
    !ability.costs.sacrifice
        && ability.costs.pay_life.as_ref().is_none_or(|n| life >= n)
        && (!ability.costs.tap || available_tap(object))
        && ability
            .costs
            .remove_source_plus_one_counters
            .as_ref()
            .is_none_or(|n| object.plus_one_counters >= *n)
        && !(ability.restriction.is_some() && object.mana_ability_used == Some(true))
        // CR 602.5: the view does not prove an "Activate only if" condition;
        // the plan never names a source the payment window may not offer.
        && ability.condition.is_none()
}

/// The colors one ability of a permanent `controller` controls produces.
fn production_colors(
    catalog: &Catalog,
    view: &PermittedView,
    mana: &ManaProduction,
    controller: PlayerId,
) -> Vec<Color> {
    match mana {
        ManaProduction::Fixed { symbols } => symbols.clone(),
        ManaProduction::AnyColor => vec![Color::W, Color::U, Color::B, Color::R, Color::G],
        ManaProduction::CommanderColor => commander_colors(view, controller),
        ManaProduction::Choose { colors, .. } => colors.clone(),
        ManaProduction::LandsCouldProduce {
            controller: lands,
            colors_only,
        } => lands_could_produce(
            &could_produce_table(catalog, view),
            controller,
            *lands,
            *colors_only,
        ),
    }
}

fn names_land(player: PlayerId, lands: ManaLandController, land_controller: PlayerId) -> bool {
    match lands {
        ManaLandController::You => land_controller == player,
        ManaLandController::Opponents => land_controller != player,
    }
}

/// CR 106.7 over the public battlefield, with the engine's semantics: each
/// land's types seed from its intrinsic and defined productions, relative to
/// its own controller; "could produce" lands then add the types of the lands
/// they name until nothing changes (a least fixed point, independent of
/// object order). Keyed by object index.
fn could_produce_table(
    catalog: &Catalog,
    view: &PermittedView,
) -> BTreeMap<usize, (PlayerId, BTreeSet<Color>)> {
    let mut table = BTreeMap::new();
    let mut reads = vec![];
    for (index, o) in view.objects.iter().enumerate() {
        let Ok(d) = definition(catalog, &o.oracle_id) else {
            continue;
        };
        let c = o.characteristics.as_ref().unwrap_or(&d.characteristics);
        if o.zone != Zone::Battlefield || o.phased_out || !c.has_type(CardKind::Land) {
            continue;
        }
        let mut types: BTreeSet<Color> = c
            .subtypes
            .iter()
            .filter_map(|s| basic_land_color(s))
            .collect();
        for ability in &d.mana_abilities {
            match &ability.production {
                ManaProduction::LandsCouldProduce {
                    controller,
                    colors_only,
                } => reads.push((index, *controller, *colors_only)),
                production => {
                    types.extend(production_colors(catalog, view, production, o.controller))
                }
            }
        }
        table.insert(index, (o.controller, types));
    }
    loop {
        let mut changed = false;
        for &(index, lands, colors_only) in &reads {
            let owner = table[&index].0;
            let mut found: BTreeSet<Color> = table
                .values()
                .filter(|(controller, _)| names_land(owner, lands, *controller))
                .flat_map(|(_, types)| types.iter().copied())
                .collect();
            if colors_only {
                found.remove(&Color::C);
            }
            let types = &mut table.get_mut(&index).expect("table land").1;
            let before = types.len();
            types.extend(found);
            changed |= types.len() != before;
        }
        if !changed {
            return table;
        }
    }
}

fn lands_could_produce(
    table: &BTreeMap<usize, (PlayerId, BTreeSet<Color>)>,
    player: PlayerId,
    lands: ManaLandController,
    colors_only: bool,
) -> Vec<Color> {
    let mut colors: BTreeSet<Color> = table
        .values()
        .filter(|(controller, _)| names_land(player, lands, *controller))
        .flat_map(|(_, types)| types.iter().copied())
        .collect();
    if colors_only {
        colors.remove(&Color::C);
    }
    colors.into_iter().collect()
}

pub(super) fn spendable_pool(view: &PermittedView) -> Option<BTreeMap<Color, Quantity>> {
    let player = view.players.iter().find(|p| p.player == view.viewer)?;
    Some(player.mana.clone())
}

pub(super) fn plan_for(
    catalog: &Catalog,
    view: &PermittedView,
    cost: &ManaCost,
    exclude: Option<&ObjectHandle>,
) -> Option<std::collections::VecDeque<PlannedMana>> {
    let pool = spendable_pool(view)?;
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
        let life = view
            .players
            .iter()
            .find(|p| p.player == view.viewer)
            .map(|p| p.life.clone())
            .unwrap_or_default();
        for (index, ability) in d.mana_abilities.iter().enumerate() {
            if !usable_printed(ability, o, &life) {
                continue;
            }
            let colors = production_colors(catalog, view, &ability.production, view.viewer);
            if colors.is_empty() {
                continue;
            }
            sources.push(Source {
                object_index,
                ability: ManaAbilityIdentity::Printed {
                    index: index as u32,
                },
                handle: &o.handle,
                amount: match &ability.production {
                    ManaProduction::Choose { amount, .. } => amount.clone(),
                    _ => 1.into(),
                },
                land,
                production: ability.production.clone(),
                colors,
                cost: ability.costs.mana.clone(),
                harm: !ability.riders.is_empty() || ability.costs.pay_life.is_some(),
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
                    harm: false,
                });
            }
        }
    }
    let mut planner = Planner {
        sources,
        used: vec![false; view.objects.len()],
        pool,
        spell_only: BTreeMap::new(),
        plan: vec![],
    };
    planner.fund(cost, false).then(|| planner.plan.into())
}

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
                let payable = plan_for(catalog, view, &cost, None).is_some();
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
    Some((choices, plan_for(catalog, view, &cost, None)?))
}
#[cfg(test)]
mod tests {
    use super::*;
    use mtg_core::{
        Budget, CommanderHistory, Game, MatchConfig, Prepared, QueryResult, QueryValue,
    };

    /// A public view of a Foundations pod with its board cleared; the viewer
    /// is seat 3.
    fn view() -> PermittedView {
        let p = Prepared::admit_pod(
            mtg_compiler::catalog::embedded_catalog("match").unwrap(),
            MatchConfig::for_profile("commander-foundations-v1", [3; 32], PlayerId(3)).unwrap(),
        )
        .unwrap();
        let mut g = Game::new(p);
        g.advance(Budget::unlimited());
        let QueryResult::Complete(QueryValue::Observation(o)) =
            g.observe(PlayerId(3), Budget::unlimited())
        else {
            panic!("view")
        };
        let mut v = o.view;
        v.objects.clear();
        v.stack.clear();
        v.commanders.clear();
        v
    }

    fn land(c: &Catalog, name: &str, controller: u8) -> mtg_core::VisibleObject {
        let d = c.definitions.iter().find(|d| d.name == name).unwrap();
        mtg_core::VisibleObject {
            modal_faces: None,
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
            plus_one_counters: 0.into(),
            characteristics: Some(d.characteristics.clone()),
            marked_damage: 0.into(),
            attack_ready: true,
            handle: ObjectHandle(format!("{name}-{controller}")),
            oracle_id: d.oracle_id.clone(),
            owner: PlayerId(controller),
            controller: PlayerId(controller),
            zone: Zone::Battlefield,
            tapped: false,
        }
    }

    fn commander(owner: u8, color: Color) -> mtg_core::VisibleCommander {
        mtg_core::VisibleCommander {
            designation: format!("commander-{owner}"),
            oracle_id: String::new(),
            history: CommanderHistory {
                owner: PlayerId(owner),
                color_identity: vec![color],
                command_casts: BTreeMap::new(),
                combat_damage: BTreeMap::new(),
            },
        }
    }

    /// CR 106.7, 903.4: an opponent's Command Tower could produce its own
    /// controller's commander colors, and an opponent's Exotic Orchard
    /// could produce what its opponents' lands could, which chains back to
    /// the viewer's own Forest. The planner's set matches the engine's
    /// least fixed point.
    #[test]
    fn could_produce_reads_each_land_relative_to_its_controller() {
        let c = mtg_compiler::catalog::embedded_catalog("witness-r4").unwrap();
        let orchard = ManaProduction::LandsCouldProduce {
            controller: ManaLandController::Opponents,
            colors_only: true,
        };
        let mut v = view();
        v.commanders = vec![commander(0, Color::B), commander(3, Color::G)];
        v.objects = vec![
            land(&c, "Exotic Orchard", 3),
            land(&c, "Command Tower", 0),
            land(&c, "Exotic Orchard", 1),
        ];
        assert_eq!(
            production_colors(&c, &v, &orchard, PlayerId(3)),
            vec![Color::B]
        );
        v.objects.push(land(&c, "Forest", 3));
        assert_eq!(
            production_colors(&c, &v, &orchard, PlayerId(3)),
            vec![Color::B, Color::G]
        );
        assert_eq!(
            production_colors(&c, &v, &ManaProduction::CommanderColor, PlayerId(0)),
            vec![Color::B]
        );
    }

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
                harm: false,
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
