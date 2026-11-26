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
    pub bundle: Option<Vec<Color>>,
    pub ability: Option<ManaAbilityIdentity>,
}
struct Source<'a> {
    object_index: usize,
    ability: ManaAbilityIdentity,
    handle: &'a ObjectHandle,
    land: bool,
    taps: bool,
    amount: Quantity,
    production: ManaProduction,
    choose_bundle: bool,
    colors: Vec<Color>,
    cost: Option<ManaCost>,
    /// Pays life or performs a rider (damage, life gain): used last.
    harm: bool,
}
struct Planner<'a> {
    catalog: &'a Catalog,
    view: &'a PermittedView,
    sources: Vec<Source<'a>>,
    used: Vec<bool>,
    tapped: Vec<bool>,
    pool: BTreeMap<Color, Quantity>,
    scoped_mana: BTreeMap<Color, Quantity>,
    plan: Vec<PlannedMana>,
}
impl Planner<'_> {
    fn amount(&self, source: &Source<'_>) -> Quantity {
        let ManaProduction::Counted {
            filter,
            factor,
            zone,
            ..
        } = &source.production
        else {
            return source.amount.clone();
        };
        let count = self
            .view
            .objects
            .iter()
            .enumerate()
            .filter(|(index, object)| {
                let mut facts = mtg_core::query::in_view(self.view, object);
                if *index == source.object_index || self.tapped[*index] {
                    facts = facts.after_tap();
                }
                mtg_core::query::matches_zone_pred(&facts, self.view.viewer, zone, filter).is_yes()
            })
            .count();
        let mut amount = Quantity::from(i64::try_from(count).expect("bounded visible objects"));
        amount.multiply(factor);
        amount
    }

    fn balance(&self, color: &Color, nested: bool) -> Quantity {
        let mut amount = self.pool.get(color).cloned().unwrap_or_default();
        if !nested && let Some(spell) = self.scoped_mana.get(color) {
            amount.add(spell);
        }
        amount
    }

    fn can_fund(&self, cost: &ManaCost, nested: bool) -> bool {
        let mut pool = self.pool.clone();
        if !nested {
            for (color, amount) in &self.scoped_mana {
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
            let needed = required
                .iter()
                .filter_map(|(&color, amount)| {
                    let available = self.balance(&color, nested);
                    (available < *amount).then(|| {
                        let mut missing = amount.clone();
                        missing.subtract(&available);
                        (color, missing)
                    })
                })
                .collect::<BTreeMap<_, _>>();
            let deficit = needed.keys().next().copied();
            let chosen = self
                .sources
                .iter()
                .enumerate()
                .filter(|(_, source)| !self.used[source.object_index])
                .filter(|(_, source)| deficit.is_none_or(|color| source.colors.contains(&color)))
                .min_by_key(|(index, source)| {
                    let mut needed = needed.clone();
                    let mut useful = Quantity::zero();
                    let color = deficit.or_else(|| source.colors.first().copied());
                    let symbols = match &source.production {
                        ManaProduction::Fixed { symbols } => symbols.as_slice(),
                        _ => color.as_slice(),
                    };
                    let output = self.amount(source);
                    for symbol in symbols {
                        if let Some(missing) = needed.get_mut(symbol) {
                            let amount = missing.clone().min(output.clone());
                            missing.subtract(&amount);
                            useful.add(&amount);
                        }
                    }
                    (
                        source.cost.is_some(),
                        source
                            .cost
                            .as_ref()
                            .is_some_and(|cost| spend(cost, &self.pool).is_none()),
                        source.harm,
                        std::cmp::Reverse(useful),
                        source.colors.len(),
                        source.land,
                        *index,
                    )
                })
                .map(|(index, _)| index);
            let Some(i) = chosen else { return false };
            self.used[self.sources[i].object_index] = true;
            let Some(i) = self.fund_source_cost(i) else {
                return false;
            };
            let source = &self.sources[i];
            let colors = if matches!(source.production, ManaProduction::LandsCouldProduce { .. })
                && (source.taps || self.tapped.iter().any(|tapped| *tapped))
            {
                let mut view = self.view.clone();
                for (index, object) in view.objects.iter_mut().enumerate() {
                    object.tapped |=
                        self.tapped[index] || (index == source.object_index && source.taps);
                }
                production_colors(self.catalog, &view, &source.production, view.viewer)
            } else {
                source.colors.clone()
            };
            let color = match source.production {
                ManaProduction::Fixed { .. } | ManaProduction::Counted { .. } => None,
                _ => deficit
                    .filter(|color| colors.contains(color))
                    .or_else(|| colors.first().copied()),
            };
            let symbols = match &source.production {
                ManaProduction::Counted { color, .. } => std::slice::from_ref(color),
                ManaProduction::Fixed { symbols } => symbols.as_slice(),
                _ => color.as_slice(),
            };
            let amount = self.amount(source);
            self.tapped[source.object_index] = source.taps;
            for &c in symbols {
                self.pool.entry(c).or_default().add(&amount);
            }
            self.plan.push(PlannedMana {
                source: source.handle.clone(),
                ability: Some(source.ability),
                color,
                bundle: source.choose_bundle.then(|| symbols.to_vec()),
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
}

/// CR 602.5: whether an "Activate only if" condition provably holds in the
/// permitted view, read the way the rules core reads it. Each counted object
/// must be decided from public facts; an undecided object or an unaudited
/// condition leaves the source out, so a plan never names a source the
/// payment window may not offer.
fn condition_holds(
    view: &PermittedView,
    source: &mtg_core::VisibleObject,
    condition: Option<&mtg_contracts::kir::Cond>,
) -> bool {
    use mtg_contracts::kir::ActivationCondition as C;
    use mtg_core::query::{Tri, in_view, matches_battlefield_pred, matches_pred};
    let Some(condition) = condition else {
        return true;
    };
    let Ok(form) = C::of(condition) else {
        return false;
    };
    // The plan's own taps would change a count that reads tapped status.
    fn reads_tapped(atoms: &[mtg_contracts::kir::Atom]) -> bool {
        use mtg_contracts::kir::Atom;
        atoms.iter().any(|a| match a {
            Atom::Tapped | Atom::Untapped => true,
            Atom::AnyOf { alternatives } => alternatives.iter().any(|p| reads_tapped(&p.0)),
            _ => false,
        })
    }
    if form.leaves().into_iter().any(|leaf| match leaf {
        C::Controlled { filter, .. } | C::GraveyardCards { filter, .. } | C::Source { filter } => {
            reads_tapped(&filter.0)
        }
        _ => false,
    }) {
        return false;
    }
    let you = view.viewer;
    let decided = |answers: Vec<Tri>| -> Option<u32> {
        let mut count = 0u32;
        for answer in answers {
            match answer {
                Tri::Yes => count = count.checked_add(1)?,
                Tri::No => {}
                Tri::Unknown => return None,
            }
        }
        Some(count)
    };
    let count = |leaf: &C| -> Option<u32> {
        match leaf {
            C::Controlled { filter, .. } => decided(
                view.objects
                    .iter()
                    .filter(|o| o.zone == Zone::Battlefield)
                    .map(|o| matches_battlefield_pred(&in_view(view, o), you, filter))
                    .collect(),
            ),
            C::GraveyardCards { filter, .. } => decided(
                view.objects
                    .iter()
                    .filter(|o| o.zone == (Zone::Graveyard { owner: you }) && o.token.is_none())
                    .map(|o| matches_pred(&in_view(view, o), you, filter))
                    .collect(),
            ),
            C::HandCards { .. } => view
                .players
                .iter()
                .find(|p| p.player == you)
                .map(|p| p.hand_count),
            C::Source { filter } => {
                decided(vec![matches_pred(&in_view(view, source), you, filter)])
            }
            C::All(_) | C::Any(_) => None,
        }
    };
    form.leaves()
        .into_iter()
        .map(count)
        .collect::<Option<Vec<_>>>()
        .is_some_and(|counts| form.passes(&counts))
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
        ManaProduction::Counted { color, .. } => vec![*color],
        ManaProduction::Bundles { options } => options.iter().flatten().copied().collect(),
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
        // CR 106.7, 113.10a: granted abilities are abilities the land has.
        for ability in d.mana_abilities.iter().chain(&o.granted_mana) {
            match &ability.production {
                ManaProduction::LandsCouldProduce {
                    controller,
                    colors_only,
                } => reads.push((index, *controller, *colors_only)),
                ManaProduction::Counted {
                    color,
                    filter,
                    zone,
                    ..
                } => {
                    if view.objects.iter().any(|object| {
                        mtg_core::query::matches_zone_pred(
                            &mtg_core::query::in_view(view, object),
                            o.controller,
                            zone,
                            filter,
                        )
                        .is_yes()
                    }) {
                        types.insert(*color);
                    }
                }
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
    plan_for_payment(catalog, view, cost, exclude, &[])
}

pub(super) fn plan_for_payment(
    catalog: &Catalog,
    view: &PermittedView,
    cost: &ManaCost,
    exclude: Option<&ObjectHandle>,
    eligible: &[mtg_contracts::RestrictedMana],
) -> Option<std::collections::VecDeque<PlannedMana>> {
    let pool = spendable_pool(view)?;
    let mut scoped_mana = BTreeMap::<Color, Quantity>::new();
    for unit in eligible {
        scoped_mana.entry(unit.color).or_default().add(&unit.amount);
    }
    let mut sources = vec![];
    for (object_index, o) in view.objects.iter().enumerate() {
        if !controlled_permanent(view, o) || exclude == Some(&o.handle) {
            continue;
        }
        // A token plans only the abilities statics grant it (CR 113.10a);
        // its own printed and intrinsic mana are not planned.
        let d = match (&o.token, definition(catalog, &o.oracle_id)) {
            (None, Ok(d)) => Some(d),
            (None, Err(_)) => continue,
            (Some(_), _) if o.granted_mana.is_empty() => continue,
            (Some(_), _) => None,
        };
        let Some(c) = o.characteristics.as_ref().or(d.map(|d| &d.characteristics)) else {
            continue;
        };
        let land = c.has_type(CardKind::Land);
        let life = view
            .players
            .iter()
            .find(|p| p.player == view.viewer)
            .map(|p| p.life.clone())
            .unwrap_or_default();
        let printed = d
            .iter()
            .flat_map(|d| d.mana_abilities.iter().enumerate())
            .map(|(index, a)| {
                (
                    ManaAbilityIdentity::Printed {
                        index: index as u32,
                    },
                    a,
                )
            });
        let granted = o.granted_mana.iter().enumerate().map(|(index, a)| {
            (
                ManaAbilityIdentity::Granted {
                    index: index as u32,
                },
                a,
            )
        });
        for (identity, ability) in printed.chain(granted) {
            if ability
                .spend_restriction
                .is_some_and(|rule| !rule.permits_resolution_payment())
                || !usable_printed(ability, o, &life)
                || !condition_holds(view, o, ability.condition.as_ref())
            {
                continue;
            }
            let productions = match &ability.production {
                ManaProduction::Bundles { options } => options
                    .iter()
                    .cloned()
                    .map(|symbols| (ManaProduction::Fixed { symbols }, true))
                    .collect(),
                production => vec![(production.clone(), false)],
            };
            for (production, choose_bundle) in productions {
                let colors = production_colors(catalog, view, &production, view.viewer);
                if colors.is_empty() {
                    continue;
                }
                sources.push(Source {
                    object_index,
                    ability: identity,
                    handle: &o.handle,
                    amount: match &production {
                        ManaProduction::Choose { amount, .. } => amount.clone(),
                        _ => 1.into(),
                    },
                    land,
                    taps: ability.costs.tap,
                    production,
                    choose_bundle,
                    colors,
                    cost: ability.costs.mana.clone(),
                    harm: !ability.riders.is_empty() || ability.costs.pay_life.is_some(),
                });
            }
        }
        if land && d.is_some() && available_tap(o) {
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
                    taps: true,
                    production: ManaProduction::Fixed {
                        symbols: vec![color],
                    },
                    choose_bundle: false,
                    colors: vec![color],
                    cost: None,
                    harm: false,
                });
            }
        }
    }
    let mut planner = Planner {
        catalog,
        view,
        sources,
        used: vec![false; view.objects.len()],
        tapped: vec![false; view.objects.len()],
        pool,
        scoped_mana,
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
            faces: None,
            face_down: false,
            exiled_by: None,
            phased_out: false,
            entered_this_turn: false,
            mana_ability_used: None,
            attached_to: None,
            attachments: vec![],
            keywords: vec![],
            granted_mana: vec![],
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

    /// CR 113.10a, 106.7: a Forest that has a granted any-color ability is
    /// planned through that ability for blue, and its controller's other
    /// "could produce" reads see blue too.
    #[test]
    fn granted_any_color_land_is_planned_by_its_granted_identity() {
        let c = mtg_compiler::catalog::embedded_catalog("match").unwrap();
        let mut v = view();
        let mut forest = land(&c, "Forest", 3);
        forest.granted_mana = vec![ManaAbility {
            restriction: None,
            condition: None,
            costs: mtg_contracts::PermanentActivationCosts {
                remove_source_plus_one_counters: None,
                tap: true,
                mana: None,
                sacrifice: false,
                pay_life: None,
            },
            production: ManaProduction::AnyColor,
            spend_restriction: None,
            riders: vec![],
            source_block: 0,
        }];
        v.objects.push(forest);
        let cost = ManaCost {
            generic: 0.into(),
            colored: vec![Color::U],
        };
        let plan = plan_for(&c, &v, &cost, None).expect("granted blue");
        assert_eq!(plan.len(), 1);
        assert_eq!(
            plan[0].ability,
            Some(ManaAbilityIdentity::Granted { index: 0 })
        );
        let table = could_produce_table(&c, &v);
        assert!(table.values().any(|(_, colors)| colors.contains(&Color::U)));
        let granted = v.objects[0].granted_mana.clone();
        v.objects[0].granted_mana.clear();
        assert!(plan_for(&c, &v, &cost, None).is_none());

        // A token with no catalog definition (a creature token under
        // Cryptolith Rite) plans only its granted ability.
        let token = &mut v.objects[0];
        token.oracle_id.clear();
        token.token = Some(mtg_core::VisibleToken {
            name: "Saproling".into(),
            keywords: vec![],
        });
        let green = ManaCost {
            generic: 0.into(),
            colored: vec![Color::G],
        };
        assert!(plan_for(&c, &v, &green, None).is_none());
        v.objects[0].granted_mana = granted;
        let plan = plan_for(&c, &v, &cost, None).expect("granted token blue");
        assert_eq!(
            plan[0].ability,
            Some(ManaAbilityIdentity::Granted { index: 0 })
        );
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
        let c = mtg_compiler::catalog::embedded_catalog("witness-r5").unwrap();
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

    /// Temple of the False God: "{T}: Add {C}{C}. Activate only if you
    /// control five or more lands." The plan names it exactly when the public
    /// board proves the condition (CR 602.5).
    #[test]
    fn temple_of_the_false_god_is_planned_only_with_five_lands() {
        let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
            include_str!("../../../../tests/fixtures/archive-manifest-phase1-r5.json"),
            include_str!(
                "../../../../tests/fixtures/phase1-r5/cfdd5dc6-593e-495a-8cfe-3a56b3c4c7df.json"
            ),
        ) else {
            panic!("frozen Temple of the False God lowers");
        };
        let mut c = mtg_compiler::catalog::embedded_catalog("match").unwrap();
        c.definitions.push(*definition);
        let generic = |n: i64| ManaCost {
            generic: n.into(),
            colored: vec![],
        };
        for lands in [4usize, 5] {
            let mut v = view();
            v.objects.push(land(&c, "Temple of the False God", 3));
            for i in 1..lands {
                let mut forest = land(&c, "Forest", 3);
                forest.handle = ObjectHandle(format!("Forest-{i}"));
                v.objects.push(forest);
            }
            // Every land, with the Temple's two: lands + 1 mana.
            let all = plan_for(&c, &v, &generic(lands as i64 + 1), None);
            assert_eq!(all.is_some(), lands == 5, "{lands}");
            if let Some(plan) = all {
                assert!(plan.iter().any(|p| p.source == v.objects[0].handle));
            }
            // Without the condition the Forests alone still pay what they can.
            if lands == 4 {
                let forests = plan_for(&c, &v, &generic(3), None).unwrap();
                assert!(forests.iter().all(|p| p.source != v.objects[0].handle));
            }
        }
    }

    #[test]
    fn a_one_drop_uses_a_forest_before_an_unfundable_cataracts_bundle() {
        let mut catalog = mtg_compiler::catalog::embedded_catalog("match").unwrap();
        let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
            include_str!("../../../../tests/fixtures/archive-manifest-phase1-r5.json"),
            include_str!(
                "../../../../tests/fixtures/phase1-r5/d98b4250-3492-4864-9c4c-42db09b3ccd4.json"
            ),
        ) else {
            panic!("frozen Cascading Cataracts must lower")
        };
        catalog.definitions.push(*definition);
        let mut view = view();
        view.objects.push(land(&catalog, "Cascading Cataracts", 3));
        for index in 0..4 {
            let mut forest = land(&catalog, "Forest", 3);
            forest.handle = ObjectHandle(format!("Forest-{index}"));
            view.objects.push(forest);
        }
        let cost = ManaCost {
            generic: 0.into(),
            colored: vec![Color::G],
        };
        let plan =
            plan_for(&catalog, &view, &cost, None).expect("a Forest pays for Llanowar Elves");
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].source, view.objects[1].handle);
    }

    #[test]
    fn mana_with_only_a_spend_rider_can_fund_a_generic_cost() {
        let mut catalog = mtg_compiler::catalog::embedded_catalog("match").unwrap();
        let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(
            include_str!("../../../../tests/fixtures/archive-manifest-phase1-r5.json"),
            include_str!(
                "../../../../tests/fixtures/phase1-r5/36937483-30cb-449a-8028-75017a124922.json"
            ),
        ) else {
            panic!("frozen Boseiju must lower")
        };
        catalog.definitions.push(*definition);
        let mut view = view();
        view.objects
            .push(land(&catalog, "Boseiju, Who Shelters All", 3));
        let cost = ManaCost {
            generic: 1.into(),
            colored: vec![],
        };
        let plan = plan_for(&catalog, &view, &cost, None).expect("Boseiju can pay a generic cost");
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].source, view.objects[0].handle);
    }

    #[test]
    fn counted_mana_plan_reads_taps_in_payment_order() {
        let mut catalog = mtg_compiler::catalog::embedded_catalog("match").unwrap();
        let mtg_compiler::CompileResult::Lowered { mut definition } = mtg_compiler::compile(
            include_str!("../../../../tests/fixtures/archive-manifest-phase1-r5.json"),
            include_str!(
                "../../../../tests/fixtures/phase1-r5/7c427c3d-ecd8-45ef-bebd-8f10f4a311db.json"
            ),
        ) else {
            panic!("frozen Cradle must lower")
        };
        let ManaProduction::Counted { filter, .. } = &mut definition.mana_abilities[0].production
        else {
            panic!("expected counted production")
        };
        filter.0.push(mtg_contracts::kir::Atom::Untapped);
        catalog.definitions.push(*definition);

        let mut view = view();
        view.objects.push(land(&catalog, "Gaea's Cradle", 3));
        let mut island = land(&catalog, "Island", 3);
        island
            .characteristics
            .as_mut()
            .unwrap()
            .additional_types
            .push(CardKind::Creature);
        view.objects.push(island);
        let mut creature = land(&catalog, "Forest", 3);
        let facts = creature.characteristics.as_mut().unwrap();
        facts.kind = CardKind::Creature;
        facts.subtypes.clear();
        view.objects.push(creature);

        let mut cost = ManaCost {
            generic: 0.into(),
            colored: vec![Color::U, Color::G],
        };
        let plan =
            plan_for(&catalog, &view, &cost, None).expect("Island and one untapped creature");
        assert_eq!(plan[0].source, view.objects[1].handle);
        assert_eq!(plan[1].source, view.objects[0].handle);
        cost.colored.push(Color::G);
        assert!(plan_for(&catalog, &view, &cost, None).is_none());
    }

    #[test]
    fn any_color_source_is_planned_in_the_missing_color() {
        let source = ObjectHandle("any-color-provider".into());
        let view = view();
        let catalog = mtg_compiler::catalog::embedded_catalog("match").unwrap();
        let mut planner = Planner {
            catalog: &catalog,
            view: &view,
            sources: vec![Source {
                object_index: 0,
                ability: ManaAbilityIdentity::Printed { index: 0 },
                handle: &source,
                land: false,
                taps: true,
                amount: 1.into(),
                production: ManaProduction::AnyColor,
                choose_bundle: false,
                colors: vec![Color::W, Color::U, Color::B, Color::R, Color::G],
                cost: None,
                harm: false,
            }],
            used: vec![false],
            tapped: vec![false],
            pool: BTreeMap::new(),
            scoped_mana: BTreeMap::new(),
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
