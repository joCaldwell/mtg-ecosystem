//! Equipped-indestructible witness drivers shared by keyword, aura and
//! permanent binaries: source preparation, paid casts and recovery checks.
use mtg_core::testkit::*;
use std::{collections::BTreeMap, sync::Arc};

pub fn prepared(rows: &[(&str, bool)], synthetic: bool) -> Prepared {
    let mut c = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    for raw in [
        include_str!("../../../tests/fixtures/phase1-r4/9d8fe3f8-5027-4f9e-999e-e277caa96478.json"),
        include_str!("../../../tests/fixtures/phase1-r4/f90fe86c-5cca-483b-93c2-6e856fa01c88.json"),
        include_str!("../../../tests/fixtures/phase1-r4/2bda1fb1-528c-4e66-89f1-bb1a85e9181e.json"),
    ] {
        let mtg_compiler::CompileResult::Lowered { definition } =
            crate::compile(crate::R4_MANIFEST, raw)
        else {
            panic!("source")
        };
        c.definitions.push(*definition);
    }
    // Darksteel Plate's source predates Archive R4: its frozen reviewed
    // definition, bound to the same retained record.
    let plate: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/b5b4cf54-ed5e-42d0-9d98-5fec76b0b0b8.json"
    ))
    .unwrap();
    let reviewed = mtg_compiler::catalog::embedded_catalog("sacrifice").unwrap();
    let definition = reviewed
        .definition_by_oracle_id(plate["oracleIdentity"].as_str().unwrap())
        .unwrap()
        .clone();
    assert_eq!(definition.provenance.source_sha256, plate["sourceSha256"]);
    assert_eq!(definition.provenance.binding_sha256, plate["bindingSha256"]);
    c.definitions.push(definition);
    if synthetic {
        // Typed nonintrinsic equipment control, NOT a real card source claim.
        let d = c
            .definitions
            .iter_mut()
            .find(|d| d.name == "Darksteel Plate")
            .unwrap();
        d.name = "Typed nonintrinsic equipment grant control".into();
        d.keywords.clear();
        d.capabilities = d.requirements();
    }
    c.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.profile = "equipped-indestructible-private".into();
    config.commanders.clear();
    config.cards = rows
        .iter()
        .enumerate()
        .map(|(i, (name, hand))| CardSetup {
            id: CardId(i as u32),
            oracle_id: c
                .definitions
                .iter()
                .find(|d| d.name == *name)
                .unwrap()
                .oracle_id
                .clone(),
            owner: PlayerId(0),
            controller: PlayerId(0),
            zone: if *hand {
                Zone::Hand { owner: PlayerId(0) }
            } else {
                Zone::Battlefield
            },
        })
        .collect();
    for seat in 0..4 {
        for _ in 0..6 {
            config.cards.push(CardSetup {
                id: CardId(config.cards.len() as u32),
                oracle_id: FOREST.into(),
                owner: PlayerId(seat),
                controller: PlayerId(seat),
                zone: Zone::Library {
                    owner: PlayerId(seat),
                },
            });
        }
    }
    Prepared {
        catalog_digest: digest(&c).unwrap(),
        profile_digest: digest(&config).unwrap(),
        catalog: Arc::new(c),
        config,
        match_config: None,
    }
}
pub fn object(g: &Game, id: u32) -> ObjectId {
    g.object_id(CardId(id)).unwrap()
}
pub fn request(g: &mut Game, q: u64) -> Request {
    for _ in 0..1000 {
        match g.advance(Budget::new(q)) {
            Advance::Suspended { .. } => (),
            Advance::AwaitingDecision(r) => return r,
            x => panic!("{x:?}"),
        }
    }
    panic!("bounded stabilization")
}
pub fn answer(g: &mut Game, q: u64, response: Response) {
    let r = request(g, q);
    g.respond(r.player(), r.decision_id(), response).unwrap();
}
pub fn recover(g: &Game) {
    let p = g.prepared.as_ref().clone();
    let restored = Game::restore(p.clone(), &g.snapshot().unwrap()).unwrap();
    assert_eq!(g.state, restored.state);
    assert_eq!(g.trace(), restored.trace());
    let replay = Game::replay_to(
        p,
        &g.replay_journal(),
        Some(g.rule_boundary()),
        g.trace.mode,
    )
    .unwrap();
    assert_eq!(g.state, replay.state);
    assert_eq!(g.trace(), replay.trace());
}
pub fn both(p: Prepared, run: impl Fn(&mut Game, u64)) {
    let mut prior = None;
    for (q, mode) in [(1, TraceMode::Detailed), (4096, TraceMode::Off)] {
        let mut g = Game::with_trace(p.clone(), mode);
        request(&mut g, q);
        run(&mut g, q);
        recover(&g);
        let hash = g.state_digest().unwrap();
        if let Some(old) = &prior {
            assert_eq!(&hash, old)
        }
        prior = Some(hash);
    }
}
pub fn cast(g: &mut Game, q: u64, id: u32, targets: Vec<TargetEntity>) {
    let obj = object(g, id);
    answer(g, q, Response::Cast { card: obj.card });
    pay_and_resolve(g, q, targets);
}
pub fn pay_to_priority(g: &mut Game, q: u64, targets: Vec<TargetEntity>) {
    loop {
        let r = request(g, q);
        let response = match &r {
            Request::EntityTargets { .. } => Response::EntityTargets {
                entities: targets.clone(),
            },
            Request::Targets { .. } => Response::Targets {
                objects: targets
                    .iter()
                    .map(|t| match t {
                        TargetEntity::Object { object } => *object,
                        _ => panic!("object target"),
                    })
                    .collect(),
            },
            Request::Mana { cost, .. } => {
                let enough = g.payment_pool(r.player(), cost).is_ok();
                if enough {
                    Response::FinishMana { undo_mana: false }
                } else {
                    let land = g
                        .state
                        .rules
                        .objects
                        .values()
                        .find(|o| {
                            o.controller == r.player()
                                && o.zone == Zone::Battlefield
                                && !o.tapped
                                && g.definition(o.id.card).unwrap().characteristics.kind
                                    == CardKind::Land
                        })
                        .unwrap();
                    Response::ActivateMana { object: land.id }
                }
            }
            Request::SpendMana {
                cost, available, ..
            } => {
                let mut rest = available.clone();
                let mut spending = BTreeMap::<Color, Quantity>::new();
                for c in &cost.colored {
                    rest.get_mut(c).unwrap().subtract(&1.into());
                    spending.entry(*c).or_default().add(&1.into());
                }
                let mut generic = cost.generic.clone();
                for (c, n) in rest {
                    let used = n.min(generic.clone());
                    generic.subtract(&used);
                    if used > Quantity::zero() {
                        spending.entry(c).or_default().add(&used);
                    }
                }
                assert_eq!(generic, Quantity::zero());
                Response::SpendMana { spending }
            }
            Request::Priority { .. } => break,
            x => panic!("cast request {x:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
}
pub fn pay_and_resolve(g: &mut Game, q: u64, targets: Vec<TargetEntity>) {
    pay_to_priority(g, q, targets);
    while !g.state.rules.stack.is_empty() {
        answer(g, q, Response::Pass);
        request(g, q);
    }
}
pub fn equip(g: &mut Game, q: u64, source: u32, target: u32) {
    let source = object(g, source);
    let target = object(g, target);
    answer(
        g,
        q,
        Response::ActivateAbility {
            object: source,
            ability: 0,
        },
    );
    pay_and_resolve(g, q, vec![TargetEntity::Object { object: target }]);
    assert_eq!(g.current(source).unwrap().attached_to, Some(target));
}
