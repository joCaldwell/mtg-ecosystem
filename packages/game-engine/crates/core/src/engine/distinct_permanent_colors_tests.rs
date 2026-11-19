//! CR 105.1–2, 109.5, 605.3b, 608.2h, 611.3 and 702.20.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const MANIFEST: &str =
    include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json");
const ELDER: &str =
    include_str!("../../../../tests/fixtures/ramos/70a6f08e-854d-4e2f-9d8c-c45ec3231157.json");
const AGENT: &str =
    include_str!("../../../../tests/fixtures/ramos/2afbaa9a-c171-4a8b-90f3-5250d8498356.json");
const ELF: &str =
    include_str!("../../../../tests/fixtures/ramos/3f0c9466-5ab9-4205-a84f-b4b27b5a678e.json");
const VEGA: &str =
    include_str!("../../../../tests/fixtures/ramos/95e66851-7aff-4559-91b4-6b8a95c6e1f8.json");
const ACTIVE: PlayerId = PlayerId(0);
const AGENT_OWNER: PlayerId = PlayerId(1);
const OPPONENT: PlayerId = PlayerId(2);
const ELDER_CARD: CardId = CardId(0);
const AGENT_CARD: CardId = CardId(1);
const VEGA_CARD: CardId = CardId(4);

fn prepared() -> Prepared {
    let archives: Vec<AcceptedArchive> = strict_read(include_str!(
        "../../../../tests/fixtures/ramos/accepted-archives.json"
    ))
    .unwrap();
    let manifest = accepted_manifest(MANIFEST).unwrap();
    let archive = archives
        .iter()
        .find(|archive| archive.manifest_sha256 == manifest.manifest_sha256)
        .unwrap();
    let mut catalog =
        Catalog::read(include_str!("../../../../tests/fixtures/expected-mana-ir.json")).unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.profile = "distinct-permanent-colors-private-witness".into();
    config.cards.clear();
    let mountain = catalog
        .definitions
        .iter()
        .find(|definition| definition.name == "Mountain")
        .unwrap()
        .oracle_id
        .clone();
    for (raw, owner, controller, zone) in [
        (Some(ELDER), ACTIVE, ACTIVE, Zone::Battlefield),
        (Some(AGENT), AGENT_OWNER, ACTIVE, Zone::Battlefield),
        (Some(ELF), ACTIVE, OPPONENT, Zone::Battlefield),
        (None, ACTIVE, ACTIVE, Zone::Battlefield),
        (Some(VEGA), ACTIVE, ACTIVE, Zone::Hand { owner: ACTIVE }),
    ] {
        let oracle_id = if let Some(raw) = raw {
            assert!(
                archive.record_sha256.contains(&sha256(raw.trim_end().as_bytes()))
            );
            let result = mtg_compiler::compile(MANIFEST, raw);
            let mtg_compiler::CompileResult::Lowered { definition } = result else {
                panic!("{result:?}");
            };
            if raw == ELDER {
                assert!(
                    definition.capabilities.contains(&Capability::DistinctPermanentColorsV1)
                );
                let labels = definition.source_labels.as_ref().unwrap();
                assert_eq!(labels.statics, [1]);
                assert_eq!(labels.mana, [2]);
            }
            let id = definition.oracle_id.clone();
            catalog.definitions.push(*definition);
            id
        } else {
            mountain.clone()
        };
        config.cards.push(CardSetup {
            id: CardId(config.cards.len() as u32),
            oracle_id,
            owner,
            controller,
            zone,
        });
    }
    catalog.definitions.sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    Prepared {
        catalog_digest: digest(&catalog).unwrap(),
        profile_digest: digest(&config).unwrap(),
        catalog: Arc::new(catalog),
        config,
        match_config: None,
    }
}

fn request(game: &mut Game) -> Request {
    match game.advance(Budget::unlimited()) {
        Advance::AwaitingDecision(request) => request,
        other => panic!("{other:?}"),
    }
}

fn answer(game: &mut Game, response: Response) -> Request {
    let next = request(game);
    game.respond(next.player(), next.decision_id(), response)
        .unwrap();
    request(game)
}

#[test]
fn faeburrow_elder_distinct_permanent_colors_fund_paid_spell() {
    let mut game = Game::with_trace(prepared(), TraceMode::Detailed);
    let elder = game.object_id(ELDER_CARD).unwrap();
    assert!(game.has_keyword(elder, KeywordAbility::Vigilance));
    let stats = game.characteristics(elder).unwrap();
    assert_eq!(
        (stats.power, stats.toughness),
        (Some(3.into()), Some(3.into()))
    );
    // Agent adds blue and shares green with Elder. The opponent's Elf and
    // the controlled Mountain supply no colors to this controller's set.
    let colors = [Color::W, Color::U, Color::G];
    assert_eq!(game.mana_abilities(elder).unwrap(), colors);
    assert!(matches!(
        answer(&mut game, Response::Cast { card: VEGA_CARD }),
        Request::Mana { .. }
    ));
    assert!(matches!(
        answer(&mut game, Response::ActivateMana { object: elder }),
        Request::Mana { .. }
    ));
    assert!(game.current(elder).unwrap().tapped);
    assert_eq!(game.state.rules.stack.len(), 1);
    let added = game
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::Committed {
                proposed: ProposedEvent::AddMana { source, .. },
                actual: EventOutcome::ManaAdded { player, color, amount },
            } if *source == elder && *player == ACTIVE && *amount == Quantity::from(1) => Some(*color),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(added, colors);
    for color in colors {
        assert_eq!(game.mana(ACTIVE, color), Some(1.into()));
    }
    for color in [Color::B, Color::R, Color::C] {
        assert_eq!(game.mana(ACTIVE, color), Some(0.into()));
    }

    // Agent's owner leaves while Vega is being cast. Elder's continuous
    // bonus shrinks immediately, while the mana from its completed activation
    // remains available for the spell.
    game.concede(AGENT_OWNER).unwrap();
    assert!(matches!(request(&mut game), Request::Mana { .. }));
    assert!(game.object_id(AGENT_CARD).is_none());
    assert_eq!(game.mana_abilities(elder).unwrap(), [Color::W, Color::G]);
    let stats = game.characteristics(elder).unwrap();
    assert_eq!(
        (stats.power, stats.toughness),
        (Some(2.into()), Some(2.into()))
    );
    assert!(matches!(
        answer(&mut game, Response::FinishMana { undo_mana: true }),
        Request::SpendMana { .. }
    ));
    let spending = BTreeMap::from(colors.map(|color| (color, Quantity::from(1))));
    let next = answer(&mut game, Response::SpendMana { spending });
    assert!(matches!(next, Request::Priority { .. }));
    let top = game.top().unwrap();
    assert_eq!(top.paid.as_ref(), Some(&top.total_cost));
    for color in colors {
        assert_eq!(game.mana(ACTIVE, color), Some(0.into()));
    }
    for _ in 0..3 {
        assert!(matches!(
            answer(&mut game, Response::Pass),
            Request::Priority { .. }
        ));
    }
    assert!(game.state.rules.stack.is_empty());
    assert_eq!(
        game.zone(game.object_id(VEGA_CARD).unwrap()),
        Some(Zone::Battlefield)
    );
}
