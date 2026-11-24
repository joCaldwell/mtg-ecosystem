//! Bounded strategy candidates from the real root's permitted observation only.
//! This is an explicitly pruned adapter, never an exhaustive legality oracle.
use mtg_contracts::{
    ActivationCost, CardKind, Catalog, Color, Definition, FunctioningZone, ManaCost, Quantity,
    SpellProgram, TargetDomain, canonical, fixed, kernel, sha256, target_object, target_player,
};
use mtg_core::{
    ObjectHandle, Observation, PlayerId, PlayerRequest, PlayerTargetEntity, RootHandleMap,
    VisibleObject, Zone,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const CANDIDATE_POLICY: &str = "foundations-public-plans-v1";
pub const MAX_CANDIDATES: usize = 32;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "targets", rename_all = "snake_case", deny_unknown_fields)]
pub enum TargetPlan {
    None,
    Objects { objects: Vec<ObjectHandle> },
    Players { players: Vec<PlayerId> },
    Entities { entities: Vec<PlayerTargetEntity> },
}
impl TargetPlan {
    pub fn mapped(&self, map: &RootHandleMap) -> Result<Self, String> {
        Ok(match self {
            Self::None => Self::None,
            Self::Objects { objects } => Self::Objects {
                objects: objects
                    .iter()
                    .map(|h| map.object(h))
                    .collect::<Result<_, _>>()?,
            },
            Self::Players { players } => Self::Players {
                players: players.clone(),
            },
            Self::Entities { entities } => Self::Entities {
                entities: entities
                    .iter()
                    .map(|entity| match entity {
                        PlayerTargetEntity::Player { player } => {
                            Ok(PlayerTargetEntity::Player { player: *player })
                        }
                        PlayerTargetEntity::Object { object } => Ok(PlayerTargetEntity::Object {
                            object: map.object(object)?,
                        }),
                    })
                    .collect::<Result<_, String>>()?,
            },
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActionPlan {
    Pass,
    PlayLand {
        object: ObjectHandle,
    },
    Cast {
        card: ObjectHandle,
        targets: TargetPlan,
        mana_sources: Vec<ObjectHandle>,
        spending: BTreeMap<Color, Quantity>,
    },
    Activate {
        object: ObjectHandle,
        ability: u32,
        targets: TargetPlan,
        mana_sources: Vec<ObjectHandle>,
        spending: BTreeMap<Color, Quantity>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub candidate_id: String,
    pub plan: ActionPlan,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateSet {
    pub policy: String,
    pub candidates: Vec<Candidate>,
    pub truncated_at_limit: bool,
    pub coalesced_basic_hand_copies: usize,
    pub standalone_mana_actions_omitted: bool,
}
pub fn definition<'a>(catalog: &'a Catalog, oracle: &str) -> Result<&'a Definition, String> {
    catalog
        .definitions
        .iter()
        .find(|d| d.oracle_id == oracle)
        .ok_or_else(|| format!("missing public definition {oracle}"))
}
pub fn land_color(catalog: &Catalog, oracle: &str) -> Option<Color> {
    let d = definition(catalog, oracle).ok()?;
    if d.characteristics.kind != CardKind::Land
        || !d.characteristics.basic
        || d.intrinsic_zone != Some(FunctioningZone::Battlefield)
    {
        return None;
    }
    // CR305.6, pinned2026-09-25: intrinsic mana follows basic land subtypes,
    // not printed colors/identity or a CardName dispatch.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    let colors = d
        .characteristics
        .subtypes
        .iter()
        .filter_map(|kind| match kind.as_str() {
            "Plains" => Some(Color::W),
            "Island" => Some(Color::U),
            "Swamp" => Some(Color::B),
            "Mountain" => Some(Color::R),
            "Forest" => Some(Color::G),
            _ => None,
        })
        .collect::<Vec<_>>();
    (colors.len() == 1).then(|| colors[0])
}
pub fn spend(
    cost: &ManaCost,
    available: &BTreeMap<Color, Quantity>,
) -> Option<BTreeMap<Color, Quantity>> {
    let mut free = available.clone();
    let mut spending = BTreeMap::<Color, Quantity>::new();
    for color in &cost.colored {
        let amount = free.get_mut(color)?;
        if *amount < Quantity::from(1) {
            return None;
        }
        amount.subtract(&1.into());
        spending.entry(*color).or_default().add(&1.into());
    }
    let mut generic = cost.generic.clone();
    for color in [Color::B, Color::G, Color::W, Color::U, Color::R, Color::C] {
        let take = std::cmp::min(
            free.get(&color).cloned().unwrap_or_default(),
            generic.clone(),
        );
        if take > Quantity::zero() {
            generic.subtract(&take);
            spending.entry(color).or_default().add(&take);
        }
    }
    (generic == Quantity::zero()).then_some(spending)
}
pub fn funding(
    catalog: &Catalog,
    observation: &Observation,
    cost: &ManaCost,
) -> Option<(Vec<ObjectHandle>, BTreeMap<Color, Quantity>)> {
    let view = &observation.view;
    let mut funds = view
        .players
        .iter()
        .find(|p| p.player == view.viewer)?
        .mana
        .clone();
    let mut lands = view
        .objects
        .iter()
        .filter(|o| {
            o.controller == view.viewer
                && o.zone == Zone::Battlefield
                && !o.tapped
                && land_color(catalog, &o.oracle_id).is_some()
        })
        .collect::<Vec<_>>();
    lands.sort_by(|a, b| a.handle.0.cmp(&b.handle.0));
    let mut sources = vec![];
    loop {
        if let Some(spending) = spend(cost, &funds) {
            return Some((sources, spending));
        }
        let mut needed = BTreeMap::<Color, Quantity>::new();
        for color in &cost.colored {
            needed.entry(*color).or_default().add(&1.into());
        }
        let deficit = needed
            .iter()
            .find(|(color, quantity)| funds.get(color).cloned().unwrap_or_default() < **quantity)
            .map(|(color, _)| *color);
        let index = lands.iter().position(|land| {
            deficit.is_none_or(|color| land_color(catalog, &land.oracle_id) == Some(color))
        })?;
        let land = lands.remove(index);
        funds
            .entry(land_color(catalog, &land.oracle_id)?)
            .or_default()
            .add(&1.into());
        sources.push(land.handle.clone());
    }
}
pub fn cast_cost(
    d: &Definition,
    object: &VisibleObject,
    observation: &Observation,
) -> Result<ManaCost, String> {
    let mut cost = d.characteristics.mana_cost.clone();
    if matches!(object.zone, Zone::Command { .. }) {
        let commander = observation
            .view
            .commanders
            .iter()
            .find(|c| c.oracle_id == object.oracle_id && c.history.owner == object.owner)
            .ok_or("command source has no permitted designation history")?;
        let casts = commander
            .history
            .command_casts
            .get(&observation.view.viewer)
            .cloned()
            .unwrap_or_default();
        cost.generic.add(&casts);
        cost.generic.add(&casts);
    }
    Ok(cost)
}
pub fn beneficial(program: &SpellProgram) -> bool {
    program
        .instructions
        .iter()
        .any(|instruction| match instruction {
            kernel!(GainLife {
                player: target_player!(_),
                ..
            }) => true,
            kernel!(ModifyUntil {
                what: target_object!(_),
                change: mtg_contracts::kir::Change::PowerToughness {
                    power: fixed!(power),
                    toughness: fixed!(toughness)
                },
                until: mtg_contracts::kir::Until::EndOfTurn,
                ..
            }) => power > &Quantity::zero() || toughness > &Quantity::zero(),
            _ => false,
        })
}
fn target_plans(
    program: &SpellProgram,
    observation: &Observation,
    check: &mut dyn FnMut() -> Result<(), String>,
) -> Result<Vec<TargetPlan>, String> {
    if program.targets.is_empty() {
        return Ok(vec![TargetPlan::None]);
    }
    let domain = program.targets[0].domain;
    if program.targets.iter().any(|s| s.domain != domain) {
        return Err("candidate adapter does not admit mixed target domains".into());
    }
    let view = &observation.view;
    let own = beneficial(program);
    let mut objects = view
        .objects
        .iter()
        .filter(|o| {
            mtg_core::query::matches_target_domain(
                &mtg_core::query::in_view(view, o),
                view.viewer,
                TargetDomain::BattlefieldCreature,
            )
            .is_yes()
        })
        .collect::<Vec<_>>();
    objects.sort_by(|a, b| {
        ((a.controller == view.viewer) != own, &a.handle.0)
            .cmp(&((b.controller == view.viewer) != own, &b.handle.0))
    });
    let mut players = view
        .players
        .iter()
        .filter(|p| {
            p.status == mtg_core::PlayerStatus::Live
                && (domain != TargetDomain::OpponentPlayer || p.player != view.viewer)
        })
        .map(|p| p.player)
        .collect::<Vec<_>>();
    players.sort_by_key(|p| ((*p == view.viewer) != own, p.0));
    let variants = match domain {
        TargetDomain::ControlledBattlefieldCreature
        | TargetDomain::ControlledBattlefieldPermanent
        | TargetDomain::BlockingBattlefieldCreature
        | TargetDomain::OwnGraveyardCreatureCard
        | TargetDomain::CreatureOrPlaneswalker
        | TargetDomain::PlayerGraveyardCards { .. }
        | TargetDomain::OwnGraveyardCard => {
            return Err("staples target outside foundations search".into());
        }
        TargetDomain::BattlefieldCreature => objects
            .iter()
            .map(|o| TargetPlan::Objects {
                objects: vec![o.handle.clone()],
            })
            .collect::<Vec<_>>(),
        TargetDomain::OpponentPlayer
        | TargetDomain::AnyPlayer
        | TargetDomain::PlayerOrPlaneswalker => players
            .iter()
            .map(|p| TargetPlan::Players { players: vec![*p] })
            .collect(),
        TargetDomain::AnyDamageTarget => players
            .iter()
            .map(|p| TargetPlan::Entities {
                entities: vec![PlayerTargetEntity::Player { player: *p }],
            })
            .chain(objects.iter().map(|o| TargetPlan::Entities {
                entities: vec![PlayerTargetEntity::Object {
                    object: o.handle.clone(),
                }],
            }))
            .collect(),
        TargetDomain::BattlefieldPermanent
        | TargetDomain::StackSpell
        | TargetDomain::ControlledCreatureSpell
        | TargetDomain::StackActivatedOrTriggeredAbility => vec![],
    };
    let mut plans = vec![TargetPlan::None];
    for _ in &program.targets {
        let mut next = vec![];
        'outer: for prefix in &plans {
            for choice in &variants {
                check()?;
                let plan = match (prefix, choice) {
                    (TargetPlan::None, choice) => choice.clone(),
                    (TargetPlan::Objects { objects: a }, TargetPlan::Objects { objects: b }) => {
                        TargetPlan::Objects {
                            objects: a.iter().chain(b).cloned().collect(),
                        }
                    }
                    (TargetPlan::Players { players: a }, TargetPlan::Players { players: b }) => {
                        TargetPlan::Players {
                            players: a.iter().chain(b).copied().collect(),
                        }
                    }
                    (
                        TargetPlan::Entities { entities: a },
                        TargetPlan::Entities { entities: b },
                    ) => TargetPlan::Entities {
                        entities: a.iter().chain(b).cloned().collect(),
                    },
                    _ => return Err("candidate target shape mismatch".into()),
                };
                next.push(plan);
                if next.len() > MAX_CANDIDATES {
                    break 'outer;
                }
            }
        }
        plans = next;
    }
    Ok(plans)
}
fn push(set: &mut CandidateSet, plan: ActionPlan) -> Result<bool, String> {
    if set.candidates.len() == MAX_CANDIDATES {
        set.truncated_at_limit = true;
        return Ok(false);
    }
    let candidate_id = sha256(canonical(&(CANDIDATE_POLICY, &plan))?.as_bytes());
    set.candidates.push(Candidate { candidate_id, plan });
    Ok(true)
}
/// Selects Pass, one representative per identical basic hand definition, funded
/// spells (all selected modes/target tuples up to the cap), and funded nonmana
/// activations. Standalone mana-floating actions are an explicit policy exclusion.
pub fn generate(
    catalog: &Catalog,
    observation: &Observation,
    check: &mut dyn FnMut() -> Result<(), String>,
) -> Result<CandidateSet, String> {
    let view = &observation.view;
    if !matches!(observation.decision, Some(PlayerRequest::Priority { player, .. }) if player == view.viewer)
        || view.active != Some(view.viewer)
        || !view.stack.is_empty()
        || !matches!(
            view.turn.step,
            mtg_core::TurnStep::PrecombatMain | mtg_core::TurnStep::PostcombatMain
        )
    {
        return Err("candidate generation requires acting active empty-stack priority".into());
    }
    let mut set = CandidateSet {
        policy: CANDIDATE_POLICY.into(),
        candidates: vec![],
        truncated_at_limit: false,
        coalesced_basic_hand_copies: 0,
        standalone_mana_actions_omitted: view.objects.iter().any(|o| {
            o.controller == view.viewer
                && o.zone == Zone::Battlefield
                && !o.tapped
                && land_color(catalog, &o.oracle_id).is_some()
        }),
    };
    push(&mut set, ActionPlan::Pass)?;
    let mut seen_basics = BTreeSet::new();
    if view.turn.lands_played == 0 {
        for object in &view.objects {
            check()?;
            if object.zone != (Zone::Hand { owner: view.viewer }) {
                continue;
            }
            let d = definition(catalog, &object.oracle_id)?;
            if d.characteristics.kind != CardKind::Land {
                continue;
            }
            if d.characteristics.basic && !seen_basics.insert(object.oracle_id.clone()) {
                set.coalesced_basic_hand_copies += 1;
                continue;
            }
            if !push(
                &mut set,
                ActionPlan::PlayLand {
                    object: object.handle.clone(),
                },
            )? {
                return Ok(set);
            }
        }
    }
    let mut sources = view
        .objects
        .iter()
        .filter(|o| {
            o.owner == view.viewer
                && (o.zone == (Zone::Hand { owner: view.viewer })
                    || o.zone == (Zone::Command { owner: view.viewer }))
        })
        .collect::<Vec<_>>();
    sources.sort_by_key(|o| {
        (
            o.zone != (Zone::Command { owner: view.viewer }),
            definition(catalog, &o.oracle_id)
                .map(|d| d.characteristics.kind != CardKind::Creature)
                .unwrap_or(true),
            o.handle.0.clone(),
        )
    });
    for object in sources {
        check()?;
        let d = definition(catalog, &object.oracle_id)?;
        let Some(program) = &d.spell else {
            continue;
        };
        let Some((mana_sources, spending)) =
            funding(catalog, observation, &cast_cost(d, object, observation)?)
        else {
            continue;
        };
        for targets in target_plans(program, observation, check)? {
            if !push(
                &mut set,
                ActionPlan::Cast {
                    card: object.handle.clone(),
                    targets,
                    mana_sources: mana_sources.clone(),
                    spending: spending.clone(),
                },
            )? {
                return Ok(set);
            }
        }
    }
    for object in view
        .objects
        .iter()
        .filter(|o| o.controller == view.viewer && o.zone == Zone::Battlefield)
    {
        let d = definition(catalog, &object.oracle_id)?;
        for (ability, activated) in d.activated.iter().enumerate() {
            check()?;
            let (mana_sources, spending) = match &activated.cost {
                ActivationCost::TapSource
                    if object.tapped
                        || (d.characteristics.kind == CardKind::Creature
                            && !object.attack_ready) =>
                {
                    continue;
                }
                ActivationCost::Mana { cost } => {
                    let Some(payment) = funding(catalog, observation, cost) else {
                        continue;
                    };
                    payment
                }
                _ => (vec![], BTreeMap::new()),
            };
            for targets in target_plans(&activated.program, observation, check)? {
                if !push(
                    &mut set,
                    ActionPlan::Activate {
                        object: object.handle.clone(),
                        ability: u32::try_from(ability).map_err(|_| "ability index overflow")?,
                        targets,
                        mana_sources: mana_sources.clone(),
                        spending: spending.clone(),
                    },
                )? {
                    return Ok(set);
                }
            }
        }
    }
    Ok(set)
}
