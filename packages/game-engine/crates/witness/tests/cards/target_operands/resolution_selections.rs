//! CR 109.5, 115.3, 400.7, 601.2c, 608.2b-d, 701.26, 800.4a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::{digest, strict_read};

macro_rules! frozen {
    ($id:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/phase1-r4/",
            $id,
            ".json"
        ))
    };
}

const BOUNCE_LANDS: [(&str, &str, [Color; 2]); 10] = [
    (
        frozen!("046f5783-cc7b-416a-8cf6-2bcef9c2cc1a"),
        "Simic Growth Chamber",
        [Color::G, Color::U],
    ),
    (
        frozen!("657243dd-e479-4f4b-99d2-09b55d833a35"),
        "Gruul Turf",
        [Color::R, Color::G],
    ),
    (
        frozen!("00ef1c55-dea1-4564-bd57-66de86cba4df"),
        "Selesnya Sanctuary",
        [Color::G, Color::W],
    ),
    (
        frozen!("0a023964-2905-4928-9c3e-dc63e6ebd218"),
        "Rakdos Carnarium",
        [Color::B, Color::R],
    ),
    (
        frozen!("189fc8f4-17ac-4f1d-82c8-8401445bdaf4"),
        "Azorius Chancery",
        [Color::W, Color::U],
    ),
    (
        frozen!("1b301478-b14f-4ef8-94e6-9647d582eabe"),
        "Golgari Rot Farm",
        [Color::B, Color::G],
    ),
    (
        frozen!("1cb9d94a-3039-4f2e-8fcc-6996f9a45f74"),
        "Izzet Boilerworks",
        [Color::U, Color::R],
    ),
    (
        frozen!("378a1d57-e2f1-4b84-9692-1564602e9e99"),
        "Dimir Aqueduct",
        [Color::U, Color::B],
    ),
    (
        frozen!("8fa3ac81-3dfe-4565-be99-5554f7597b4b"),
        "Boros Garrison",
        [Color::R, Color::W],
    ),
    (
        frozen!("aa00ae0b-7c0f-427e-8102-ce0e2a6af5df"),
        "Orzhov Basilica",
        [Color::W, Color::B],
    ),
];

const VOYAGING_SATYR: &str = frozen!("bce18f34-848e-4dd2-9a38-6084c0695f78");
const ETERNAL_STUDENT: &str = frozen!("140cb5cf-8aa2-457c-9d65-5acefea00195");
const KOR_SKYFISHER: &str = frozen!("054a0193-861e-4644-ab0a-8aa33407abc8");
const HELPING_HAND: &str = frozen!("170a5dbc-6a43-4715-9501-178b1eb7b08c");
const NATURES_SPIRAL: &str = frozen!("565922d6-8152-411f-8d4d-dc7c8523a49b");
const ODUNOS: &str = frozen!("0282b5c0-e792-4099-a3fa-329eda11fb44");
const STAND_UP: &str = frozen!("00e94b75-ad31-4e1f-b3be-eed7cd3e4d76");
const REPEL: &str = frozen!("47f5ea79-2bf2-4e17-9c6d-e1cbcd9d30d1");
const VINES: &str = frozen!("5372445b-5f67-435e-a736-61c6cb4f9c6e");
const LIGHTFOOT: &str = frozen!("601b7481-19fc-4779-950e-883a9a22aec3");

fn play_land(g: &mut Game, id: u32) {
    Driver::action(
        g,
        Action::PlayLand {
            object: Driver::object(g, id),
        },
    );
}

fn land_choice(g: &mut Game) -> Decision<Rules> {
    let d = pass_until_choice(g);
    assert_eq!(d.player, PlayerId(0));
    assert_eq!(d.reason, Reason::ResolveObjectChoice);
    assert!(g.state.rules.stack.last().unwrap().targets.is_empty());
    let QueryResult::Complete(QueryValue::Decision(Some(permitted))) =
        g.player_decision_for(d.player, Budget::new(u64::MAX))
    else {
        panic!("the controller has no permitted object choice");
    };
    assert_eq!(permitted.reason, Reason::ResolveObjectChoice);
    assert!(
        permitted
            .candidates
            .iter()
            .all(|c| matches!(c, Candidate::Object { .. }))
    );
    assert!(matches!(
        g.player_decision_for(g.live_players()[1], Budget::new(u64::MAX)),
        QueryResult::Complete(QueryValue::Decision(None))
    ));
    checked(g);
    d
}

#[test]
fn bounce_lands_resolve_then_produce_both_printed_mana() {
    for (raw, name, colors) in BOUNCE_LANDS {
        let prepared = board(&[
            (raw, &[Cell::ChooseObjectOwnerHand], Tiers::EXECUTES),
            (VOYAGING_SATYR, &[Cell::UntapTarget], Tiers::EXECUTES),
        ])
        .permanent_or_hand(name, 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Island", 1, 1, false)
        .permanent_or_hand("Voyaging Satyr", 0, 0, false)
        .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        play_land(&mut g, 0);
        let d = land_choice(&mut g);
        let land = Driver::object(&g, 0);
        let forest = Driver::object(&g, 1);
        assert_eq!(
            d.candidates,
            [
                Candidate::Object { object: land },
                Candidate::Object { object: forest },
            ]
        );
        assert!(g.state.rules.objects[&CardId(0)].tapped);
        answer_targets(&mut g, &d, &[forest]);
        settle(&mut g);
        assert_eq!(zone(&g, 1), Zone::Hand { owner: PlayerId(0) });
        assert_ne!(Driver::object(&g, 1), forest);
        activate(&mut g, 3, 0);
        Driver::pick_object(&mut g, land);
        settle(&mut g);
        assert!(!g.state.rules.objects[&CardId(0)].tapped, "{name}");
        Driver::action(&mut g, Action::ActivateMana { object: land });
        Driver::request(&mut g, 1);
        let mana = &g.state.rules.players[0].mana;
        assert_eq!(mana.len(), 2, "{name}");
        for color in colors {
            assert_eq!(mana[&color], 1.into(), "{name}");
        }
        assert_recoverable((&g).into());
    }
}

#[test]
fn bounce_land_can_return_itself_or_a_borrowed_land_to_its_owner() {
    let (raw, name, _) = BOUNCE_LANDS[1];
    let prepared = board(&[(raw, &[Cell::ChooseObjectOwnerHand], Tiers::EXECUTES)])
        .permanent_or_hand(name, 0, 0, true)
        .permanent_or_hand("Forest", 1, 0, false)
        .prepare();
    for (choice, owner) in [(0, 0), (1, 1)] {
        let mut g = Game::with_trace(prepared.clone(), TraceMode::Detailed);
        play_land(&mut g, 0);
        let d = land_choice(&mut g);
        let object = Driver::object(&g, choice);
        answer_targets(&mut g, &d, &[object]);
        settle(&mut g);
        assert_eq!(
            zone(&g, choice),
            Zone::Hand {
                owner: PlayerId(owner)
            }
        );
    }
}

#[test]
fn chosen_commander_uses_its_owners_replacement_choice() {
    let prepared = board(&[(
        KOR_SKYFISHER,
        &[Cell::ChooseObjectOwnerHand],
        Tiers::EXECUTES,
    )])
    .permanent_or_hand("Kor Skyfisher", 0, 0, true)
    .permanent_or_hand("Plains", 0, 0, false)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Isamaru, Hound of Konda", 1, 0, false)
    .commander(CardId(3))
    .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2]);
    let d = land_choice(&mut g);
    let commander = Driver::object(&g, 3);
    answer_targets(&mut g, &d, &[commander]);
    let replacement = Driver::request(&mut g, 1);
    assert_eq!(replacement.player, PlayerId(1));
    assert!(matches!(replacement.reason, Reason::CommanderReturn { .. }));
    checked(&g);
    g.respond(
        replacement.player,
        replacement.id,
        replacement
            .pick_one(&Candidate::Flag {
                flag: Flag::CommandZone,
            })
            .unwrap(),
    )
    .unwrap();
    settle(&mut g);
    assert_eq!(zone(&g, 3), Zone::Command { owner: PlayerId(1) });
}

#[test]
fn bounce_land_rejects_forged_saved_offers_and_reoffers_after_departure() {
    let (raw, name, _) = BOUNCE_LANDS[0];
    let prepared = board(&[(raw, &[Cell::ChooseObjectOwnerHand], Tiers::EXECUTES)])
        .seats(3)
        .permanent_or_hand(name, 0, 0, true)
        .permanent_or_hand("Forest", 1, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Island", 2, 2, false)
        .permanent_or_hand("Mountain", 0, 0, true)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    play_land(&mut g, 0);
    let d = land_choice(&mut g);
    let borrowed = Driver::object(&g, 1);
    let borrowed_index = d.index(&Candidate::Object { object: borrowed }).unwrap() as usize;
    let mut departed_chooser = g.fork();
    departed_chooser.concede(PlayerId(0)).unwrap();
    assert_eq!(
        Driver::request(&mut departed_chooser, 1).reason,
        Reason::Priority
    );
    assert!(departed_chooser.state.rules.stack.is_empty());
    assert_recoverable((&departed_chooser).into());

    answer_targets(&mut g, &d, &[borrowed]);
    checked(&g);
    let valid = g.snapshot().unwrap();
    let mut unexpected: serde_json::Value = strict_read(&valid).unwrap();
    unexpected["state"]["rules"]["frame"]["choice"]["candidates"] = serde_json::json!([borrowed]);
    assert!(strict_read::<Snapshot>(&serde_json::to_string(&unexpected).unwrap()).is_err());
    for (index, object) in [
        (borrowed_index, Driver::object(&g, 3)),
        (borrowed_index, Driver::object(&g, 4)),
        (0, Driver::object(&g, 2)),
    ] {
        let mut forged: Snapshot = strict_read(&valid).unwrap();
        forged
            .state
            .rules
            .pending
            .as_mut()
            .unwrap()
            .decision
            .candidates[index] = Candidate::Object { object };
        forged.state_sha256 = digest(&forged.state).unwrap();
        let mut structural = g.fork();
        structural.state = forged.state.clone();
        assert_eq!(
            structural.validate_state().unwrap_err(),
            "resolution object choice offer changed"
        );
        assert!(
            Game::restore(
                g.prepared.as_ref().clone(),
                &serde_json::to_string(&forged).unwrap()
            )
            .is_err()
        );
    }
    // Seat 1 owns the accepted land. Their departure removes it and invalidates
    // the pending answer, leaving the controller to choose from a fresh offer.
    g.concede(PlayerId(1)).unwrap();
    let revised = land_choice(&mut g);
    assert!(!offers(&revised, borrowed));
    assert_ne!(revised.id, d.id);
    let old: Snapshot = strict_read(&valid).unwrap();
    let mut stale: Snapshot = strict_read(&g.snapshot().unwrap()).unwrap();
    stale.state.rules.pending = old.state.rules.pending.clone();
    stale.state_sha256 = digest(&stale.state).unwrap();
    assert!(
        Game::restore(
            g.prepared.as_ref().clone(),
            &serde_json::to_string(&stale).unwrap()
        )
        .err()
        .expect("obsolete offer must not restore")
        .contains("resolution object choice offer changed")
    );
    let new_choice = Driver::object(&g, 2);
    answer_targets(&mut g, &revised, &[new_choice]);
    settle(&mut g);
    assert_eq!(zone(&g, 2), Zone::Hand { owner: PlayerId(0) });
}

#[test]
fn graveyard_target_uses_exact_characteristics_and_enters_tapped() {
    let prepared = board(&[
        (
            HELPING_HAND,
            &[Cell::MoveOwnGraveyardCreatureToBattlefield],
            Tiers::EXECUTES,
        ),
        (
            NATURES_SPIRAL,
            &[Cell::MoveOwnGraveyardTargetToHand],
            Tiers::EXECUTES,
        ),
        (
            ODUNOS,
            &[Cell::MoveOwnGraveyardTargetToHand],
            Tiers::EXECUTES,
        ),
    ])
    .permanent_or_hand("Helping Hand", 0, 0, true)
    .permanent_or_hand("Plains", 0, 0, false)
    .card(
        "Grizzly Bears",
        0,
        0,
        Zone::Graveyard { owner: PlayerId(0) },
    )
    .card(
        "Colossal Dreadmaw",
        0,
        0,
        Zone::Graveyard { owner: PlayerId(0) },
    )
    .card("Forest", 0, 0, Zone::Graveyard { owner: PlayerId(0) })
    .card("Storm Crow", 1, 1, Zone::Graveyard { owner: PlayerId(1) })
    .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let bear = Driver::object(&g, 2);
    Driver::cast(&mut g, CardId(0));
    let d = targets(&mut g);
    assert_eq!(d.candidates, [Candidate::Object { object: bear }]);
    answer_targets(&mut g, &d, &[bear]);
    Driver::pay(&mut g, &[1]);
    settle(&mut g);
    assert_eq!(zone(&g, 2), Zone::Battlefield);
    assert!(g.state.rules.objects[&CardId(2)].tapped);
    assert_ne!(Driver::object(&g, 2), bear);
}

#[test]
fn graveyard_target_fails_after_the_selected_card_pays_an_exile_cost() {
    let prepared = board(&[
        (
            NATURES_SPIRAL,
            &[Cell::MoveOwnGraveyardTargetToHand],
            Tiers::EXECUTES,
        ),
        (
            ETERNAL_STUDENT,
            &[Cell::GraveyardExileSource],
            Tiers::EXECUTES,
        ),
    ])
    .permanent_or_hand("Nature's Spiral", 0, 0, true)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Swamp", 0, 0, false)
    .permanent_or_hand("Swamp", 0, 0, false)
    .card(
        "Eternal Student",
        0,
        0,
        Zone::Graveyard { owner: PlayerId(0) },
    )
    .card("Giant Growth", 0, 0, Zone::Graveyard { owner: PlayerId(0) })
    .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let student = Driver::object(&g, 5);
    Driver::cast(&mut g, CardId(0));
    let d = targets(&mut g);
    assert_eq!(d.candidates, [Candidate::Object { object: student }]);
    answer_targets(&mut g, &d, &[student]);
    Driver::pay(&mut g, &[1, 2]);
    activate(&mut g, 5, 0);
    Driver::pay(&mut g, &[3, 4]);
    settle(&mut g);
    assert_eq!(zone(&g, 5), Zone::Exile);
}

#[test]
fn property_target_rechecks_derived_power_on_resolution() {
    let prepared = board(&[(STAND_UP, &[Cell::DestroyTargetPermanent], Tiers::EXECUTES)])
        .permanent_or_hand("Stand Up for Yourself", 0, 0, true)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Glorious Anthem", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .permanent_or_hand("Disfigure", 1, 1, true)
        .permanent_or_hand("Swamp", 1, 1, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let bear = Driver::object(&g, 5);
    Driver::cast(&mut g, CardId(0));
    let d = targets(&mut g);
    assert_eq!(d.candidates, [Candidate::Object { object: bear }]);
    answer_targets(&mut g, &d, &[bear]);
    Driver::pay(&mut g, &[1, 2, 3]);
    Driver::pass(&mut g);
    Driver::cast(&mut g, CardId(7));
    Driver::pick_object(&mut g, bear);
    Driver::pay(&mut g, &[8]);
    settle(&mut g);
    assert_eq!(zone(&g, 5), Zone::Battlefield);
}

#[test]
fn optional_announced_group_resolves_zero_or_surviving_targets() {
    let prepared = board(&[(REPEL, &[Cell::TapTargetGroup], Tiers::EXECUTES)])
        .permanent_or_hand("Repel the Darkness", 0, 0, true)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .permanent_or_hand("Storm Crow", 1, 1, false)
        .permanent_or_hand("Lightning Bolt", 1, 1, true)
        .permanent_or_hand("Mountain", 1, 1, false)
        .library_card("Swamp", 0)
        .prepare();
    for (count, draw) in [(0, true), (1, false), (2, true)] {
        let mut g = Game::with_trace(prepared.clone(), TraceMode::Detailed);
        let bear = Driver::object(&g, 4);
        let crow = Driver::object(&g, 5);
        Driver::cast(&mut g, CardId(0));
        let d = targets(&mut g);
        let Shape::Pick { slots, .. } = &d.shape else {
            panic!("{d:?}")
        };
        assert_eq!(slots.len(), 1);
        assert_eq!((slots[0].min, slots[0].max), (0, 2));
        let bear_index = d.index(&Candidate::Object { object: bear }).unwrap();
        assert!(
            g.respond(
                d.player,
                d.id,
                Answer::Pick {
                    picks: vec![vec![bear_index; 2]]
                }
            )
            .is_err()
        );
        answer_targets(&mut g, &d, &[bear, crow][..count]);
        Driver::pay(&mut g, &[1, 2, 3]);
        if count > 0 {
            Driver::pass(&mut g);
            Driver::cast(&mut g, CardId(6));
            Driver::pick_object(&mut g, bear);
            Driver::pay(&mut g, &[7]);
        }
        settle(&mut g);
        assert_eq!(g.state.rules.objects[&CardId(5)].tapped, count == 2);
        assert_eq!(zone(&g, 8) == (Zone::Hand { owner: PlayerId(0) }), draw);
    }
}

#[test]
fn it_reuses_the_certified_target_for_untap_and_keyword_grants() {
    let prepared = board(&[
        (
            VINES,
            &[Cell::PumpTarget, Cell::UntapTarget],
            Tiers::EXECUTES,
        ),
        (
            LIGHTFOOT,
            &[
                Cell::PlusOneCountersTarget,
                Cell::TargetKeywordsUntilEndOfTurn,
            ],
            Tiers::EXECUTES,
        ),
    ])
    .permanent_or_hand("Vines of the Recluse", 0, 0, true)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Llanowar Elves", 0, 0, false)
    .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let elf = Driver::object(&g, 2);
    Driver::action(&mut g, Action::ActivateMana { object: elf });
    Driver::cast(&mut g, CardId(0));
    let d = targets(&mut g);
    assert!(matches!(&d.reason, Reason::Targets { specs } if specs.len() == 1));
    answer_targets(&mut g, &d, &[elf]);
    Driver::pay(&mut g, &[]);
    settle(&mut g);
    assert!(!g.state.rules.objects[&CardId(2)].tapped);
    assert_eq!(g.characteristics(elf).unwrap().toughness, Some(3.into()));
    assert!(g.current_keywords(elf).contains(&KeywordAbility::Reach));
}

#[test]
fn growth_spiral_rejects_its_private_hand_choice() {
    let raw = frozen!("34bcc217-dd91-45a0-90d7-a94d02f1f317");
    assert!(matches!(
        mtg_witness::compile(mtg_witness::R4_MANIFEST, raw),
        mtg_compiler::CompileResult::Unsupported { reason, .. } if reason.contains("ref_filter")
    ));
}
