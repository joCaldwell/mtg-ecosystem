//! A finite public-API rules laboratory, not an arbitrary-game fuzz generator.
//!
//! Enumerate one cast attempt, allowing basic mana before/during announcement,
//! every offered target, both failed-payment policies, and passes to resolution.
//! Stop after that attempt; do not enumerate empty-stack passes or cast retries.
//! The independent outcomes below follow the retained first/payment scenarios
//! and CR 601.2, 608.2 and 733.1 in the project's pinned rules edition.
use mtg_contracts::{CardKind, Catalog, Color, Quantity, basic_land_color};
use mtg_core::*;
use std::collections::{BTreeMap, BTreeSet};

const MAX_DEPTH: usize = 12;
const MAX_NODES: usize = 256;
const MAX_RESUMES: usize = 512;

fn complete(game: &Game, mut result: QueryResult, quantum: u64) -> QueryValue {
    for _ in 0..MAX_RESUMES {
        match result {
            QueryResult::Complete(value) => return value,
            QueryResult::Incomplete { work, .. } => {
                result = game.resume_work(work, Budget::new(quantum));
            }
            QueryResult::Rejected(reason) => panic!("legal laboratory query rejected: {reason:?}"),
        }
    }
    panic!("laboratory query exceeded its explicit resume bound");
}

fn observe(game: &Game, viewer: PlayerId, quantum: u64) -> Observation {
    match complete(game, game.observe(viewer, Budget::new(quantum)), quantum) {
        QueryValue::Observation(observation) => observation,
        _ => panic!("observation query returned a different value"),
    }
}

fn settle(game: &mut Game, quantum: u64) -> PlayerId {
    for _ in 0..MAX_RESUMES {
        match game.advance(Budget::new(quantum)) {
            Advance::AwaitingDecision(request) => return request.player(),
            Advance::Suspended { .. } => (),
            other => panic!("one-cast laboratory reached an unexpected boundary: {other:?}"),
        }
    }
    panic!("laboratory execution exceeded its explicit resume bound");
}

#[derive(Clone)]
struct CastStart {
    mana: BTreeMap<Color, Quantity>,
    lands: Vec<(ObjectHandle, bool, Color)>,
    activations: Vec<ObjectHandle>,
}

struct Laboratory {
    catalog: Catalog,
    prepared: Prepared,
    spell: String,
    target: String,
    nodes: usize,
    resolved: usize,
    reversed: usize,
    retained_casting_mana: usize,
    undone_casting_mana: usize,
    checkpoints: BTreeSet<&'static str>,
}

impl Laboratory {
    fn new(config: ScenarioConfig) -> Self {
        let catalog =
            Catalog::read(include_str!("../../../tests/fixtures/expected-ir.json")).unwrap();
        let identity = |kind| {
            config
                .cards
                .iter()
                .find(|card| {
                    catalog
                        .definition_by_oracle_id(&card.oracle_id)
                        .unwrap()
                        .characteristics
                        .kind
                        == kind
                })
                .unwrap()
                .oracle_id
                .clone()
        };
        Self {
            spell: identity(CardKind::Instant),
            target: identity(CardKind::Creature),
            prepared: Prepared::admit(catalog.clone(), config).unwrap(),
            catalog,
            nodes: 0,
            resolved: 0,
            reversed: 0,
            retained_casting_mana: 0,
            undone_casting_mana: 0,
            checkpoints: BTreeSet::new(),
        }
    }

    fn lands(&self, observation: &Observation) -> Vec<(ObjectHandle, bool, Color)> {
        observation
            .view
            .objects
            .iter()
            .filter(|object| {
                object.zone == Zone::Battlefield && object.controller == observation.view.viewer
            })
            .filter_map(|object| {
                let definition = self.catalog.definition_by_oracle_id(&object.oracle_id)?;
                (definition.characteristics.kind == CardKind::Land).then(|| {
                    (
                        object.handle.clone(),
                        object.tapped,
                        basic_land_color(&definition.characteristics.subtypes[0]).unwrap(),
                    )
                })
            })
            .collect()
    }

    fn choices(&self, observation: &Observation) -> Vec<PlayerResponse> {
        let mut choices = vec![];
        match observation.decision.as_ref().unwrap() {
            PlayerRequest::Priority { .. } | PlayerRequest::Mana { .. } => {
                choices.extend(self.lands(observation).into_iter().filter_map(
                    |(object, tapped, _)| {
                        (!tapped).then_some(PlayerResponse::ActivateMana { object })
                    },
                ));
                if matches!(observation.decision, Some(PlayerRequest::Mana { .. })) {
                    choices.extend(
                        [false, true].map(|undo_mana| PlayerResponse::FinishMana { undo_mana }),
                    );
                } else if !observation.view.stack.is_empty() {
                    choices.push(PlayerResponse::Pass);
                } else {
                    choices.extend(observation.view.objects.iter().filter_map(|object| {
                        (object.oracle_id == self.spell
                            && object.zone
                                == (Zone::Hand {
                                    owner: observation.view.viewer,
                                }))
                        .then(|| PlayerResponse::Cast {
                            card: object.handle.clone(),
                        })
                    }));
                }
            }
            PlayerRequest::Targets {
                occurrences,
                candidates,
                ..
            } => {
                assert_eq!(occurrences.len(), 1, "this laboratory has one target slot");
                choices.extend(candidates.iter().map(|object| PlayerResponse::Targets {
                    objects: vec![object.clone()],
                }));
            }
            PlayerRequest::SpendMana {
                cost, available, ..
            } => {
                assert_eq!(cost.generic, Quantity::zero());
                let mut spending = BTreeMap::<Color, Quantity>::new();
                for color in &cost.colored {
                    spending.entry(*color).or_default().add(&Quantity::from(1));
                }
                for (color, amount) in &spending {
                    assert!(available.get(color).is_some_and(|pool| pool >= amount));
                }
                choices.push(PlayerResponse::SpendMana { spending });
            }
            other => panic!("request outside the declared laboratory grammar: {other:?}"),
        }
        assert!(
            !choices.is_empty(),
            "laboratory must not silently drop a branch"
        );
        choices
    }

    fn checkpoint(&mut self, game: &Game, label: &'static str) {
        if self.checkpoints.insert(label) {
            let restored = Game::restore(self.prepared.clone(), &game.snapshot().unwrap()).unwrap();
            let replayed = Game::replay(self.prepared.clone(), &game.replay_journal()).unwrap();
            assert_eq!(restored.snapshot().unwrap(), game.snapshot().unwrap());
            assert_eq!(
                replayed.state_digest().unwrap(),
                game.state_digest().unwrap()
            );
        }
    }

    fn visit(
        &mut self,
        game: Game,
        viewer: PlayerId,
        depth: usize,
        cast: Option<CastStart>,
        last: Option<PlayerResponse>,
    ) {
        self.nodes += 1;
        assert!(self.nodes <= MAX_NODES && depth <= MAX_DEPTH);
        let before = game.snapshot().unwrap();
        let observation = observe(&game, viewer, 1);
        assert_eq!(observation, observe(&game, viewer, u64::MAX));
        for player in (0..4).map(PlayerId) {
            let view = observe(&game, player, 1).view;
            assert!(
                view.objects.iter().all(|object| {
                    !matches!(object.zone, Zone::Hand { owner } if owner != player)
                })
            );
            assert!(view.players.iter().all(|p| p.life == Quantity::from(40)));
        }
        assert_eq!(game.snapshot().unwrap(), before, "queries must be pure");

        if let Some(start) = &cast
            && observation.view.stack.is_empty()
        {
            let own = observe(&game, PlayerId(0), 1);
            let spell = own
                .view
                .objects
                .iter()
                .find(|o| o.oracle_id == self.spell)
                .unwrap();
            let owner = observe(&game, PlayerId(1), 1);
            let target = owner
                .view
                .objects
                .iter()
                .find(|o| o.oracle_id == self.target)
                .unwrap();
            if spell.zone == (Zone::Graveyard { owner: PlayerId(0) }) {
                self.resolved += 1;
                assert_eq!(target.zone, Zone::Hand { owner: PlayerId(1) });
                assert_eq!(target.owner, PlayerId(1));
                assert!(self.lands(&own).iter().all(|(_, tapped, _)| *tapped));
                assert!(
                    own.view.players[0]
                        .mana
                        .values()
                        .all(|n| *n == Quantity::zero())
                );
                self.checkpoint(&game, "resolved");
            } else {
                self.reversed += 1;
                assert_eq!(spell.zone, Zone::Hand { owner: PlayerId(0) });
                assert_eq!(target.zone, Zone::Battlefield);
                assert_eq!(target.controller, PlayerId(2));
                let Some(PlayerResponse::FinishMana { undo_mana }) = last else {
                    panic!("failed cast must finish through the explicit payment response");
                };
                let mut expected_mana = start.mana.clone();
                for (handle, tapped, color) in &start.lands {
                    let activated = start.activations.contains(handle);
                    let current = own
                        .view
                        .objects
                        .iter()
                        .find(|o| &o.handle == handle)
                        .unwrap();
                    assert_eq!(current.tapped, *tapped || (activated && !undo_mana));
                    if activated && !undo_mana {
                        expected_mana
                            .entry(*color)
                            .or_default()
                            .add(&Quantity::from(1));
                    }
                }
                expected_mana.retain(|_, amount| *amount != Quantity::zero());
                let mut actual_mana = own.view.players[0].mana.clone();
                actual_mana.retain(|_, amount| *amount != Quantity::zero());
                assert_eq!(actual_mana, expected_mana);
                if !start.activations.is_empty() {
                    if undo_mana {
                        self.undone_casting_mana += 1;
                    } else {
                        self.retained_casting_mana += 1;
                    }
                }
                self.checkpoint(&game, "reversed");
            }
            return;
        }

        match observation.decision.as_ref().unwrap() {
            PlayerRequest::Targets { .. } => self.checkpoint(&game, "targets"),
            PlayerRequest::Mana { .. } => self.checkpoint(&game, "mana"),
            PlayerRequest::SpendMana { .. } => self.checkpoint(&game, "spending"),
            _ => (),
        }
        for response in self.choices(&observation) {
            let mut next_cast = cast.clone();
            if matches!(response, PlayerResponse::Cast { .. }) {
                assert!(next_cast.is_none());
                next_cast = Some(CastStart {
                    mana: observation.view.players[0].mana.clone(),
                    lands: self.lands(&observation),
                    activations: vec![],
                });
            } else if let PlayerResponse::ActivateMana { object } = &response
                && let Some(start) = &mut next_cast
            {
                start.activations.push(object.clone());
            }
            let mut outcomes = vec![];
            for quantum in [1, u64::MAX] {
                let mut branch = game.fork();
                let branch_observation = observe(&branch, viewer, quantum);
                let checked = branch.check_player_response(
                    viewer,
                    branch_observation.decision.unwrap().token().clone(),
                    response.clone(),
                    Budget::new(quantum),
                );
                let QueryValue::Validated(receipt) = complete(&branch, checked, quantum) else {
                    panic!("validation returned a different query value");
                };
                branch.submit(receipt).unwrap();
                let next_viewer = settle(&mut branch, quantum);
                outcomes.push((branch, next_viewer));
            }
            let (branch, next_viewer) = outcomes.pop().unwrap();
            let (reference, reference_viewer) = outcomes.pop().unwrap();
            assert_eq!(next_viewer, reference_viewer);
            assert_eq!(branch.snapshot().unwrap(), reference.snapshot().unwrap());
            assert_eq!(
                game.snapshot().unwrap(),
                before,
                "fork must preserve parent"
            );
            self.visit(branch, next_viewer, depth + 1, next_cast, Some(response));
            assert_eq!(
                game.snapshot().unwrap(),
                before,
                "siblings must be independent"
            );
        }
    }

    fn run(&mut self) {
        let mut game = Game::with_trace(self.prepared.clone(), TraceMode::Off);
        let viewer = settle(&mut game, 1);
        self.visit(game, viewer, 0, None, None);
    }
}

#[test]
fn architecture_exploration_cast_choices_preserve_owner_and_execution_boundaries() {
    let mut laboratory = Laboratory::new(ScenarioConfig::land_return());
    laboratory.run();
    assert!(laboratory.nodes > 20);
    assert!(laboratory.resolved >= 2);
    assert!(laboratory.reversed >= 2);
    assert!(laboratory.checkpoints.contains("targets"));
    assert!(laboratory.checkpoints.contains("mana"));
    assert!(laboratory.checkpoints.contains("spending"));
    assert!(laboratory.checkpoints.contains("resolved"));
    assert!(laboratory.checkpoints.contains("reversed"));
}

#[test]
fn architecture_exploration_failed_payment_preserves_only_selected_mana() {
    let mut laboratory = Laboratory::new(ScenarioConfig::casting_reversal());
    laboratory.run();
    assert!(laboratory.nodes > 20);
    assert_eq!(laboratory.resolved, 0);
    assert!(laboratory.retained_casting_mana > 0);
    assert!(laboratory.undone_casting_mana > 0);
}
