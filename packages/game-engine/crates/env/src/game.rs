//! Trusted match driver. Only an acting player's Observation crosses into the encoder.
use mtg_contracts::{Catalog, bulk_catalog::BulkArtifact};
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

/// Episode outcome classification published in info column 2.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CensorClass {
    /// Natural win/draw, or a still-live episode.
    None = 0,
    /// Pod host work, turn or decision limit.
    WorkLimit = 1,
    /// A typed resource fault from the rules engine.
    EngineCapacity = 2,
    /// A permitted frame cannot fit every row in the tensor layout.
    TensorCapacity = 3,
}
enum Boundary {
    Decision(Box<Row>),
    Terminal {
        reward: [i32; 4],
        censor: CensorClass,
    },
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
        if let Some(a) = action {
            self.prefix.push(a);
        }
        match self.until_decision(source.catalog(), config)? {
            Boundary::Decision(row) => Ok(*row),
            Boundary::Terminal { reward, censor } => {
                let mut info = [0; 8];
                info[..4].copy_from_slice(&[
                    self.illegal,
                    self.reversals,
                    censor as i32,
                    self.decisions,
                ]);
                *self = Self::new(
                    source,
                    self.seed,
                    self.index,
                    self.episode.checked_add(1).ok_or("episode overflow")?,
                )?;
                let Boundary::Decision(mut row) = self.until_decision(source.catalog(), config)?
                else {
                    return Err("reset terminated before yielding a decision".into());
                };
                row.done = 1;
                row.reward = reward;
                row.info = info;
                Ok(*row)
            }
        }
    }
    fn until_decision(&mut self, c: &Catalog, config: Config) -> Result<Boundary, String> {
        loop {
            if let Some(o) = self.observation.take() {
                let d = o.decision.as_ref().ok_or("missing decision")?;
                if !mtg_observe::fits_layout(&o.view, d) {
                    return Ok(Boundary::Terminal {
                        reward: [0; 4],
                        censor: CensorClass::TensorCapacity,
                    });
                }
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
                                config.mask_unfundable,
                            )?;
                            row.info[..4].copy_from_slice(&[
                                self.illegal,
                                self.reversals,
                                0,
                                self.decisions,
                            ]);
                            self.observation = Some(o);
                            return Ok(Boundary::Decision(Box::new(row)));
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
            self.game.drain_trace(&mut |record| {
                if let EvidenceRecord::Event { event } = &record.record
                    && matches!(event.as_ref(), Event::CastReversed { .. })
                {
                    self.reversals += 1;
                }
            });
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
                Advance::AwaitingDecision(_) => CensorClass::WorkLimit,
                Advance::Suspended { .. } if self.work < play::WORK_LIMIT => continue,
                Advance::Suspended { .. } => CensorClass::WorkLimit,
                Advance::Finished { winner } => {
                    reward.fill(-1);
                    reward[winner.0 as usize] = 1;
                    CensorClass::None
                }
                Advance::Draw => CensorClass::None,
                Advance::Fault(_) if self.game.typed_fault().is_some() => {
                    CensorClass::EngineCapacity
                }
                Advance::Fault(e) => return Err(e),
            };
            return Ok(Boundary::Terminal { reward, censor });
        }
    }
}
