//! Finite public-information planner for the v16 mana pool. No native state,
//! hidden hands, library order, receipts or query recovery data are consulted.
use super::*;
use mtg_contracts::{KeywordAbility, ManaProduction};

#[derive(Clone)]
pub(super) struct PlannedMana {
    pub source: ObjectHandle,
    pub color: Option<Color>,
}
#[derive(Clone)]
struct Source {
    handle: ObjectHandle,
    land: bool,
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
            let symbols = match source.production {
                ManaProduction::Fixed { symbols } => symbols,
                _ => color.into_iter().collect(),
            };
            for c in symbols {
                self.pool.entry(c).or_default().add(&1.into());
            }
            self.plan.push(PlannedMana {
                source: source.handle,
                color,
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
            || o.tapped
            || o.token.is_some()
            || exclude == Some(&o.handle)
        {
            continue;
        }
        let d = definition(catalog, &o.oracle_id).ok()?;
        if d.characteristics.kind == CardKind::Creature
            && !o.attack_ready
            && !({ o.keywords.contains(&KeywordAbility::Haste) })
        {
            continue;
        }
        let (production, cost) = if let Some(a) = d.mana_abilities.first() {
            (a.production.clone(), a.costs.mana.clone())
        } else if d.characteristics.kind == CardKind::Land && d.characteristics.basic {
            (
                ManaProduction::Fixed {
                    symbols: vec![*d.characteristics.color_identity.first()?],
                },
                None,
            )
        } else {
            continue;
        };
        let colors = match &production {
            ManaProduction::Fixed { symbols } => symbols.clone(),
            ManaProduction::AnyColor => vec![Color::W, Color::U, Color::B, Color::R, Color::G],
            ManaProduction::CommanderColor => identity.clone(),
        };
        if colors.is_empty() {
            continue;
        }
        sources.push(Source {
            handle: o.handle.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tokens_without_oracle_ids_do_not_disable_casts_or_sacrifice_draws() {
        let catalog = mtg_compiler::catalog::embedded_catalog("mana").unwrap();
        let player = PlayerId(0);
        let card = |name: &str, zone: Zone| {
            let d = catalog.definitions.iter().find(|d| d.name == name).unwrap();
            mtg_core::VisibleObject {
                attached_to: None,
                attachments: vec![],
                keywords: vec![],
                restrictions: vec![],
                token: None,
                plus_one_counters: Quantity::zero(),
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
            observation_version: 16,
            decision: None,
            view: mtg_core::PermittedView {
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
