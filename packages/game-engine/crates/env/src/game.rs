//! Trusted match driver. Only an acting player's Observation crosses into the encoder.
use mtg_contracts::{ActivationCost, Catalog, bulk_catalog::BulkArtifact};
use mtg_core::*;
use mtg_observe::{
    Row,
    action::{Composer, Step},
};
use mtg_sim::{
    play,
    pods::{QUANTUM, bulk_input},
    policy::{Policy, PolicyMemory},
};
use std::path::Path;

pub enum Source {
    Profile(&'static MatchProfile, Vec<[Decklist; 4]>),
    Bulk(Box<BulkArtifact>, Vec<[Vec<DynamicDeckEntry>; 4]>),
}
impl Source {
    pub fn load(name: &str, pods: &[String], pin: Option<&str>) -> Result<Self, String> {
        if let Some(p) = match_profile(name) {
            let decks = pods
                .iter()
                .map(|path| {
                    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
                    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
                })
                .collect::<Result<_, _>>()?;
            return Ok(Self::Profile(p, decks));
        }
        let artifact = bulk_input::load(
            *build_identity(),
            Path::new(name),
            pin.ok_or("bulk catalog requires manifest_sha256")?,
        )?;
        if pods.is_empty() {
            return Err("bulk catalog requires pod deck files".into());
        }
        let decks = pods
            .iter()
            .map(|p| bulk_input::read_pod_decks(Path::new(p)))
            .collect::<Result<_, _>>()?;
        Ok(Self::Bulk(Box::new(artifact), decks))
    }
    pub fn catalog(&self) -> &Catalog {
        match self {
            Self::Profile(p, _) => p.catalog(),
            Self::Bulk(a, _) => a.catalog().catalog(),
        }
    }
    fn prepare(&self, seed: u64, index: usize, episode: u64) -> Result<Prepared, String> {
        let seed = play::seed_for(&format!("mtg-env-v1/{seed}/{index}/{episode}"), 0);
        let start = PlayerId(seed[0] % 4);
        match self {
            Self::Profile(p, pods) => {
                let mut config =
                    MatchConfig::for_profile(&p.name, seed, start).map_err(|e| e.to_string())?;
                if !pods.is_empty() {
                    config.decks = pods[index % pods.len()].clone();
                }
                Prepared::admit_pod(p.catalog().clone(), config).map_err(|e| e.to_string())
            }
            Self::Bulk(a, pods) => admit_dynamic_pod(
                a,
                &DynamicMatchConfig {
                    decks: pods[index % pods.len()].clone(),
                    starting_player: start,
                    seed,
                },
            )
            .map(|p| p.prepared)
            .map_err(|e| e.to_string()),
        }
    }
}
#[derive(Clone, Copy)]
pub struct Config {
    pub auto_pay: bool,
    pub baseline: bool,
    pub mask_unfundable: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            auto_pay: true,
            baseline: false,
            mask_unfundable: true,
        }
    }
}

pub struct Lane {
    pub game: Game,
    pub observation: Option<Observation>,
    prefix: Vec<usize>,
    memory: PolicyMemory,
    seed: u64,
    index: usize,
    episode: u64,
    work: usize,
    queries: usize,
    decisions: i32,
    reversals: i32,
    pub illegal: i32,
}
pub fn funding(c: &Catalog, o: &Observation) -> Vec<u8> {
    let v = &o.view;
    let d = o.decision.as_ref().expect("published decision");
    d.candidates
        .iter()
        .map(|candidate| {
            let Candidate::Action { action } = candidate else {
                return 2;
            };
            let h = match action {
                Action::Cast { card } => card,
                Action::ActivateAbility { object, .. } => object,
                _ => return 1,
            };
            let Some(obj) = v.objects.iter().find(|obj| &obj.handle == h) else {
                return 2;
            };
            let Some(def) = c.definition_by_oracle_id(&obj.oracle_id) else {
                return 2;
            };
            let (cost, hybrid) = match action {
                Action::Cast { .. } => {
                    let mut cost = def.characteristics.mana_cost.clone();
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
            u8::from(mtg_sim::policy::can_fund(c, v, cost, hybrid))
        })
        .collect()
}
impl Lane {
    pub fn new(source: &Source, seed: u64, index: usize, episode: u64) -> Result<Self, String> {
        let game = Game::with_trace(source.prepare(seed, index, episode)?, TraceMode::Stream);
        Ok(Self {
            game,
            seed,
            index,
            episode,
            prefix: vec![],
            observation: None,
            memory: Default::default(),
            work: 0,
            queries: 0,
            decisions: 0,
            reversals: 0,
            illegal: 0,
        })
    }
    fn submit(&mut self, o: &Observation, answer: Answer) -> Result<(), String> {
        let d = o.decision.as_ref().ok_or("missing decision")?;
        let query = play::complete(
            &self.game,
            || {
                self.game.check_player_response(
                    d.player,
                    d.id.clone(),
                    answer,
                    Budget::new(QUANTUM),
                )
            },
            QUANTUM,
            &mut self.work,
            &mut self.queries,
        )?;
        let QueryValue::Validated(receipt) = query else {
            return Err("wrong validation receipt".into());
        };
        self.game.submit(receipt)?;
        self.decisions += 1;
        Ok(())
    }
    pub fn run(
        &mut self,
        source: &Source,
        config: Config,
        action: Option<usize>,
    ) -> Result<Row, String> {
        let c = source.catalog();
        if let Some(a) = action {
            self.prefix.push(a);
        }
        let mut terminal = None;
        loop {
            if let Some(o) = self.observation.take() {
                let d = o.decision.as_ref().ok_or("missing decision")?;
                let auto = (config.baseline && d.player.0 != 0)
                    || (config.auto_pay && matches!(d.shape, Shape::Pay { .. }))
                    || (config.auto_pay
                        && matches!(
                            d.reason,
                            Reason::ManaAbility { .. } | Reason::ManaColor { .. }
                        )
                        && !o.view.stack.is_empty());
                let answer = if auto {
                    self.memory.choose(c, Policy::Develop, &o)?
                } else {
                    match Composer::compose(d, &self.prefix, mtg_observe::number_bound(c, &o.view))?
                    {
                        Step::Choose(choices) => {
                            let mut row = mtg_observe::encode(
                                c,
                                &o,
                                self.prefix.len(),
                                &choices,
                                &funding(c, &o),
                                config.mask_unfundable,
                            )?;
                            row.info[..4].copy_from_slice(&[
                                self.illegal,
                                self.reversals,
                                0,
                                self.decisions,
                            ]);
                            if let Some((reward, info)) = terminal {
                                row.done = 1;
                                row.reward = reward;
                                row.info = info;
                            }
                            self.observation = Some(o);
                            return Ok(row);
                        }
                        Step::Answer(a) => a,
                    }
                };
                if !auto && matches!(d.reason, Reason::Priority) {
                    self.memory.external_priority(&o.view);
                }
                self.submit(&o, answer)?;
                self.prefix.clear();
            }
            self.work += 1;
            let next = self.game.advance(Budget::new(QUANTUM));
            self.game.drain_trace(&mut |r|{if matches!(&r.record,EvidenceRecord::Event{event} if matches!(event.as_ref(),Event::CastReversed{..})){self.reversals+=1;}});
            let mut reward = [0; 4];
            let censor = match next {
                Advance::AwaitingDecision(ref d)
                    if self.work < play::WORK_LIMIT
                        && self.decisions < play::DECISION_LIMIT as i32
                        && self.game.turn().number <= play::TURN_LIMIT =>
                {
                    self.observation = Some(play::observe(
                        &self.game,
                        d.player,
                        QUANTUM,
                        &mut self.work,
                        &mut self.queries,
                    )?);
                    continue;
                }
                Advance::AwaitingDecision(_) => 1,
                Advance::Suspended { .. } if self.work < play::WORK_LIMIT => continue,
                Advance::Suspended { .. } => 1,
                Advance::Finished { winner } => {
                    reward.fill(-1);
                    reward[winner.0 as usize] = 1;
                    0
                }
                Advance::Draw => 0,
                Advance::Fault(_) if self.game.typed_fault().is_some() => 2,
                Advance::Fault(e) => return Err(e),
            };
            if terminal.is_some() {
                return Err("reset terminated before yielding a decision".into());
            }
            let mut info = [0; 8];
            info[..4].copy_from_slice(&[self.illegal, self.reversals, censor, self.decisions]);
            terminal = Some((reward, info));
            *self = Self::new(
                source,
                self.seed,
                self.index,
                self.episode.checked_add(1).ok_or("episode overflow")?,
            )?;
        }
    }
}
