//! Frozen R5 announcement, resolution and entry decisions (CR 115.3,
//! 303.4f, 400.7, 601.2c, 608.2b-c, 608.2h and 614.12).
use super::*;

const TITAN: &str = record!("b2e950fb-cb7e-40a0-a311-5bbdd0477b29");
const MUMMY: &str = record!("002a9cea-8cf7-48ba-83eb-e1c87a7024e5");
const OOZE: &str = record!("1ff25f67-36a7-4cfa-a2b1-2135b5b6fb67");
const VICTIMIZE: &str = record!("240e85d3-e495-4877-8609-4b4056c402f7");
const PACIFISM: &str = record!("5f5e0b10-c8cf-450c-bfd3-bcb0528ec330");
const TEMPLE_GARDEN: &str = record!("f413a83d-a40d-434c-b20a-4c707c0527fa");
const CREMATE: &str = record!("c6a2e410-b182-48d1-aeb2-bc8de27e9cd2");

fn grave(owner: u8) -> Zone {
    Zone::Graveyard {
        owner: PlayerId(owner),
    }
}

#[test]
fn sun_titan_rechecks_own_permanent_and_resumes_its_entry_choice() {
    for aura in [false, true] {
        let mut b = board(&[
            (
                TITAN,
                &[Cell::MoveOwnGraveyardCardToBattlefield],
                Tiers::EXECUTES,
            ),
            (PACIFISM, &[], Tiers::EXECUTES),
            (TEMPLE_GARDEN, &[], Tiers::EXECUTES),
        ])
        .permanent_or_hand("Sun Titan", 0, 0, true);
        for _ in 0..6 {
            b = b.permanent_or_hand("Plains", 0, 0, false);
        }
        let prepared = b
            .card("Pacifism", 0, 0, grave(0))
            .card("Temple Garden", 0, 0, grave(0))
            .card("Colossal Dreadmaw", 0, 0, grave(0))
            .card("Grizzly Bears", 1, 1, grave(1))
            .card("Giant Growth", 0, 0, grave(0))
            .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        Driver::cast(&mut g, CardId(0));
        Driver::pay(&mut g, &[1, 2, 3, 4, 5, 6]);
        let d = pass_until_choice(&mut g);
        let (pacifism, land) = (Driver::object(&g, 7), Driver::object(&g, 8));
        assert!(matches!(d.reason, Reason::Targets { .. }));
        assert_eq!(
            d.candidates,
            [
                Candidate::Object { object: pacifism },
                Candidate::Object { object: land }
            ]
        );
        let entrant = if aura { pacifism } else { land };
        answer_targets(&mut g, &d, &[entrant]);
        let may = pass_until_choice(&mut g);
        assert_eq!(may.reason, Reason::OptionalEffect);
        g.respond(
            may.player,
            may.id,
            may.pick_one(&Candidate::Flag { flag: Flag::Accept })
                .unwrap(),
        )
        .unwrap();
        let choice = Driver::request(&mut g, 1);
        let titan = Driver::object(&g, 0);
        assert_recoverable((&g).into());
        if aura {
            assert_eq!(choice.reason, Reason::EntryAttachment);
            Driver::pick_object(&mut g, titan);
        } else {
            assert_eq!(choice.reason, Reason::OptionalEffect);
            g.respond(
                choice.player,
                choice.id,
                choice
                    .pick_one(&Candidate::Flag { flag: Flag::Accept })
                    .unwrap(),
            )
            .unwrap();
        }
        settle(&mut g);
        let object = g.current(Driver::object(&g, entrant.card.0)).unwrap();
        assert_ne!(object.id, entrant);
        assert_eq!(object.zone, Zone::Battlefield);
        assert_eq!(object.attached_to, aura.then_some(titan));
        assert!(!object.tapped);
        if !aura {
            assert_eq!(g.state.rules.players[0].life, Quantity::from(38));
        }
    }
}

#[test]
fn ooze_reads_the_exiled_incarnation_and_cremate_fizzles() {
    for (name, creature) in [("Forest", false), ("Grizzly Bears", true)] {
        let prepared = board(&[
            (
                OOZE,
                &[Cell::ExileGraveyardTarget, Cell::GraveyardTargetWasCreature],
                Tiers::EXECUTES,
            ),
            (CREMATE, &[Cell::ExileGraveyardTarget], Tiers::EXECUTES),
        ])
        .permanent_or_hand("Scavenging Ooze", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Cremate", 0, 0, true)
        .permanent_or_hand("Swamp", 0, 0, false)
        .card(name, 1, 1, grave(1))
        .library_card("Island", 0)
        .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        let card = Driver::object(&g, 4);
        Driver::cast(&mut g, CardId(2));
        Driver::pick_object(&mut g, card);
        Driver::pay(&mut g, &[3]);
        activate(&mut g, 0, 0);
        Driver::pick_object(&mut g, card);
        Driver::pay(&mut g, &[1]);
        settle(&mut g);
        assert_eq!(zone(&g, 4), Zone::Exile);
        assert_eq!(zone(&g, 5), Zone::Library { owner: PlayerId(0) });
        assert_eq!(
            g.current(Driver::object(&g, 0)).unwrap().plus_one_counters,
            Quantity::from(i64::from(creature))
        );
        assert_eq!(
            g.state.rules.players[0].life,
            Quantity::from(40 + i64::from(creature))
        );
    }
}

#[test]
fn disposal_mummy_announces_only_opponents_graveyard_cards() {
    let prepared = board(&[(MUMMY, &[Cell::ExileGraveyardTarget], Tiers::EXECUTES)])
        .permanent_or_hand("Disposal Mummy", 0, 0, true)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .card("Forest", 0, 0, grave(0))
        .card("Forest", 1, 1, grave(1))
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2, 3, 4]);
    let d = pass_until_choice(&mut g);
    let target = Driver::object(&g, 6);
    assert_eq!(d.candidates, [Candidate::Object { object: target }]);
    answer_targets(&mut g, &d, &[target]);
    settle(&mut g);
    assert_eq!(zone(&g, 6), Zone::Exile);
}

#[test]
fn victimize_announces_both_cards_before_the_mandatory_sacrifice() {
    for (can_sacrifice, exile_one) in [(false, false), (true, false), (true, true)] {
        let mut b = board(&[
            (
                VICTIMIZE,
                &[Cell::SacrificeForTappedReturn],
                Tiers::EXECUTES,
            ),
            (CREMATE, &[Cell::ExileGraveyardTarget], Tiers::EXECUTES),
        ])
        .permanent_or_hand("Victimize", 0, 0, true);
        for _ in 0..4 {
            b = b.permanent_or_hand("Swamp", 0, 0, false);
        }
        b = b
            .card("Grizzly Bears", 0, 0, grave(0))
            .card("Storm Crow", 0, 0, grave(0))
            .permanent_or_hand("Cremate", 0, 0, true)
            .library_card("Forest", 0);
        if can_sacrifice {
            b = b.permanent_or_hand("Llanowar Elves", 0, 0, false);
        }
        let mut g = Game::with_trace(b.prepare(), TraceMode::Detailed);
        let cards = [Driver::object(&g, 5), Driver::object(&g, 6)];
        Driver::cast(&mut g, CardId(0));
        let d = targets(&mut g);
        let Shape::Pick { slots, .. } = &d.shape else {
            panic!("expected object choices")
        };
        assert_eq!((slots[0].min, slots[0].max), (2, 2));
        answer_targets(&mut g, &d, &cards);
        Driver::pay(&mut g, &[1, 2, 3]);
        if exile_one {
            Driver::cast(&mut g, CardId(7));
            Driver::pick_object(&mut g, cards[0]);
            Driver::pay(&mut g, &[4]);
        }
        if can_sacrifice {
            let d = pass_until_choice(&mut g);
            assert_eq!(d.reason, Reason::Sacrifice);
            assert_recoverable((&g).into());
            let elf = Driver::object(&g, 9);
            Driver::pick_object(&mut g, elf);
        }
        settle(&mut g);
        for (index, id) in cards.iter().enumerate() {
            let object = g.current(Driver::object(&g, id.card.0)).unwrap();
            let exiled = exile_one && index == 0;
            assert_eq!(
                object.zone,
                if exiled {
                    Zone::Exile
                } else if can_sacrifice {
                    Zone::Battlefield
                } else {
                    grave(0)
                }
            );
            assert_eq!(object.tapped, can_sacrifice && !exiled);
        }
    }
}

#[test]
fn naya_charm_uses_the_selected_modes_target() {
    let raw = record!("20cc0f3e-de69-4e5f-88b3-ae7e6c6b5996");
    for mask in [2, 4] {
        let prepared = board(&[(
            raw,
            &[
                Cell::MoveGraveyardTargetToOwnerHand,
                Cell::TapTargetPlayersCreatures,
            ],
            Tiers::EXECUTES,
        )])
        .permanent_or_hand("Naya Charm", 0, 0, true)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Plains", 0, 0, false)
        .card("Forest", 1, 1, grave(1))
        .permanent_or_hand("Storm Crow", 1, 1, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        Driver::cast(&mut g, CardId(0));
        let d = Driver::request(&mut g, 1);
        let mode = d
            .candidates
            .iter()
            .find(|c| matches!(c,Candidate::Modes{mask:m,..} if *m==mask))
            .unwrap();
        g.respond(d.player, d.id, d.pick_one(mode).unwrap())
            .unwrap();
        let d = targets(&mut g);
        let choice = if mask == 2 {
            Candidate::Object {
                object: Driver::object(&g, 4),
            }
        } else {
            Candidate::Player {
                player: PlayerId(1),
            }
        };
        g.respond(d.player, d.id, d.pick_one(&choice).unwrap())
            .unwrap();
        Driver::pay(&mut g, &[1, 2, 3]);
        settle(&mut g);
        assert_eq!(
            zone(&g, 4),
            if mask == 2 {
                Zone::Hand { owner: PlayerId(1) }
            } else {
                grave(1)
            }
        );
        assert_eq!(g.current(Driver::object(&g, 5)).unwrap().tapped, mask == 4);
        assert!(!g.current(Driver::object(&g, 6)).unwrap().tapped);
    }
}

#[test]
fn bombardment_announces_damage_before_paying_its_sacrifice() {
    let raw = record!("edad60c6-80de-4033-af1b-a703ac332983");
    let prepared = board(&[(raw, &[Cell::DamageTarget], Tiers::EXECUTES)])
        .permanent_or_hand("Goblin Bombardment", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    activate(&mut g, 0, 0);
    let d = targets(&mut g);
    g.respond(
        d.player,
        d.id,
        d.pick_one(&Candidate::Player {
            player: PlayerId(1),
        })
        .unwrap(),
    )
    .unwrap();
    let d = Driver::request(&mut g, 1);
    assert_eq!(d.reason, Reason::SacrificeCost);
    let bears = Driver::object(&g, 1);
    Driver::pick_object(&mut g, bears);
    settle(&mut g);
    assert_eq!(zone(&g, 1), grave(0));
    assert_eq!(g.state.rules.players[1].life, Quantity::from(39));
}

const ASHEN_POWDER: &str = record!("d906965a-0f33-4d75-8abe-66f2cf4817b8");

#[test]
fn ashen_powder_returns_an_opponents_creature_under_the_spell_controllers_control() {
    let mut b = board(&[(
        ASHEN_POWDER,
        &[Cell::MoveGraveyardCreatureToBattlefield],
        Tiers::EXECUTES,
    )])
    .permanent_or_hand("Ashen Powder", 0, 0, true);
    for _ in 0..4 {
        b = b.permanent_or_hand("Swamp", 0, 0, false);
    }
    let prepared = b
        .card("Grizzly Bears", 0, 0, grave(0))
        .card("Grizzly Bears", 1, 1, grave(1))
        .card("Forest", 1, 1, grave(1))
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    let target = Driver::object(&g, 6);
    let decision = targets(&mut g);
    assert_eq!(decision.candidates, [Candidate::Object { object: target }]);
    Driver::pick_object(&mut g, target);
    Driver::pay(&mut g, &[1, 2, 3, 4]);
    assert_recoverable((&g).into());
    settle(&mut g);

    let successor = Driver::object(&g, 6);
    assert_ne!(successor, target);
    let permanent = g.current(successor).unwrap();
    assert_eq!(permanent.zone, Zone::Battlefield);
    assert_eq!(permanent.controller, PlayerId(0));
    assert_eq!(g.state.rules.cards[&successor.card].owner, PlayerId(1));
    assert_eq!(zone(&g, 5), grave(0));
    assert_eq!(zone(&g, 7), grave(1));
}

const NOXIOUS_REVIVAL: &str = record!("97cabeda-9fe3-490d-99b4-4c8d87c17157");
const DREAM_TWIST: &str = record!("d569d969-ce39-4314-b5f0-76b45f5c4c7f");
const MORTUARY_MIRE: &str = record!("1b3fb20a-e090-4286-9c03-6b71c27c45be");

#[test]
fn moving_a_graveyard_target_to_the_library_and_back_breaks_the_earlier_target() {
    let prepared = board(&[
        (
            NOXIOUS_REVIVAL,
            &[Cell::MoveGraveyardTargetToOwnerLibraryTop],
            Tiers::EXECUTES,
        ),
        (DREAM_TWIST, &[], Tiers::EXECUTES),
        (CREMATE, &[Cell::ExileGraveyardTarget], Tiers::EXECUTES),
    ])
    .permanent_or_hand("Cremate", 0, 0, true)
    .permanent_or_hand("Noxious Revival", 0, 0, true)
    .permanent_or_hand("Dream Twist", 0, 0, true)
    .permanent_or_hand("Swamp", 0, 0, false)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Island", 0, 0, false)
    .card("Grizzly Bears", 1, 1, grave(1))
    .library_card("Forest", 1)
    .library_card("Island", 0)
    .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let target = Driver::object(&g, 6);

    Driver::cast(&mut g, CardId(0));
    Driver::pick_object(&mut g, target);
    Driver::pay(&mut g, &[3]);
    Driver::cast(&mut g, CardId(1));
    let payment = Driver::request(&mut g, 1);
    g.respond(
        payment.player,
        payment.id,
        payment
            .pick_one(&Candidate::HybridPayment {
                payment: HybridPayment::Mana(Color::G),
            })
            .unwrap(),
    )
    .unwrap();
    Driver::pick_object(&mut g, target);
    Driver::pay(&mut g, &[4]);
    for _ in 0..4 {
        Driver::request(&mut g, 1);
        if g.state.rules.stack.len() == 1 {
            break;
        }
        Driver::pass(&mut g);
    }
    Driver::request(&mut g, 1);
    assert_eq!(g.state.rules.libraries[&PlayerId(1)][0], target.card);
    assert_eq!(zone(&g, 6), Zone::Library { owner: PlayerId(1) });
    checked(&g);

    Driver::cast(&mut g, CardId(2));
    let decision = Driver::request(&mut g, 1);
    g.respond(
        decision.player,
        decision.id,
        decision
            .pick_one(&Candidate::Player {
                player: PlayerId(1),
            })
            .unwrap(),
    )
    .unwrap();
    Driver::pay(&mut g, &[5]);
    settle(&mut g);

    assert_eq!(zone(&g, 6), grave(1));
    assert_eq!(Driver::object(&g, 6).incarnation, target.incarnation + 2);
    assert_eq!(zone(&g, 8), Zone::Library { owner: PlayerId(0) });
}

#[test]
fn mortuary_mire_puts_its_announced_creature_on_top_only_if_accepted() {
    for flag in [Flag::Accept, Flag::Decline] {
        let prepared = board(&[(
            MORTUARY_MIRE,
            &[Cell::MoveGraveyardTargetToOwnerLibraryTop],
            Tiers::EXECUTES,
        )])
        .permanent_or_hand("Mortuary Mire", 0, 0, true)
        .card("Grizzly Bears", 0, 0, grave(0))
        .card("Grizzly Bears", 1, 1, grave(1))
        .library_card("Forest", 0)
        .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        let land = Driver::object(&g, 0);
        Driver::action(&mut g, Action::PlayLand { object: land });
        let target = Driver::object(&g, 1);
        let d = targets(&mut g);
        assert_eq!(d.candidates, [Candidate::Object { object: target }]);
        Driver::pick_object(&mut g, target);
        let may = pass_until_choice(&mut g);
        assert_eq!(may.reason, Reason::OptionalEffect);
        checked(&g);
        g.respond(
            may.player,
            may.id,
            may.pick_one(&Candidate::Flag { flag }).unwrap(),
        )
        .unwrap();
        settle(&mut g);
        assert_eq!(
            zone(&g, 1),
            if flag == Flag::Accept {
                Zone::Library { owner: PlayerId(0) }
            } else {
                grave(0)
            }
        );
    }
}

const REANIMATE: &str = record!("a044474a-cd72-4e9d-bd8d-a08f2de9cdc0");

#[test]
fn reanimate_uses_the_selected_graveyard_cards_mana_value() {
    let prepared = board(&[(
        REANIMATE,
        &[
            Cell::MoveGraveyardCreatureToBattlefield,
            Cell::LifeGraveyardCreatureManaValue,
        ],
        Tiers::EXECUTES,
    )])
    .permanent_or_hand("Reanimate", 0, 0, true)
    .permanent_or_hand("Swamp", 0, 0, false)
    .card("Grizzly Bears", 0, 0, grave(0))
    .card("Colossal Dreadmaw", 1, 1, grave(1))
    .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    let d = targets(&mut g);
    let target = Driver::object(&g, 3);
    assert_eq!(
        d.candidates,
        [
            Candidate::Object {
                object: Driver::object(&g, 2)
            },
            Candidate::Object { object: target },
        ]
    );
    Driver::pick_object(&mut g, target);
    Driver::pay(&mut g, &[1]);
    settle(&mut g);
    assert_eq!(zone(&g, 3), Zone::Battlefield);
    assert_eq!(
        g.current(Driver::object(&g, 3)).unwrap().controller,
        PlayerId(0)
    );
    assert_eq!(g.state.rules.players[0].life, Quantity::from(34));
}
