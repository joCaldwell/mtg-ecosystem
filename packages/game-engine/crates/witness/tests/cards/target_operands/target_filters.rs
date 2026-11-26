//! Target criteria use current facts at announcement and resolution.
//! CR 115.1, 400.7, 601.2c, 608.2b, 702.73a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const ASPHYXIATE: &str = record!("1c2ce0d0-8c3e-48e8-b1fd-54658bdb9af9");
const FOLLOWER: &str = record!("22c044ad-77d7-4c93-953d-e2daa9686ff7");
const LODGE: &str = record!("1275653f-de4e-4fe9-aad8-88555fa11680");
const AUTOMATON: &str = record!("d50b1d2e-1825-432a-ab73-068fdc356f50");
const UNEARTH: &str = record!("4e0b647a-6e1a-421b-bca9-8d5e346bda80");

#[test]
fn untapped_target_becomes_illegal_after_tapping_for_mana() {
    let prepared = board(&[(ASPHYXIATE, &[], Tiers::EXECUTES)])
        .permanent_or_hand("Asphyxiate", 0, 0, true)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Llanowar Elves", 0, 0, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let elves = Driver::object(&g, 4);
    Driver::cast(&mut g, CardId(0));
    let d = targets(&mut g);
    answer_targets(&mut g, &d, &[elves]);
    Driver::pay(&mut g, &[1, 2, 3]);
    Driver::action(&mut g, Action::ActivateMana { object: elves });
    checked(&g);
    settle(&mut g);
    assert_eq!(zone(&g, 4), Zone::Battlefield);
    assert_eq!(zone(&g, 0), Zone::Graveyard { owner: PlayerId(0) });
}

#[test]
fn another_excludes_source_from_castability_and_choice() {
    for with_target in [false, true] {
        let mut setup = board(&[(FOLLOWER, &[Cell::UntapTarget], Tiers::EXECUTES)])
            .permanent_or_hand("Kiora's Follower", 0, 0, false);
        if with_target {
            setup = setup.permanent_or_hand("Forest", 0, 0, false);
        }
        let mut g = Game::with_trace(setup.prepare(), TraceMode::Detailed);
        let source = Driver::object(&g, 0);
        let priority = Driver::request(&mut g, u64::MAX);
        assert_eq!(
            priority.candidates.contains(&Candidate::Action {
                action: Action::ActivateAbility {
                    object: source,
                    ability: 0
                },
            }),
            with_target
        );
        if with_target {
            activate(&mut g, 0, 0);
            let d = targets(&mut g);
            let forest = Driver::object(&g, 1);
            assert!(!offers(&d, source));
            answer_targets(&mut g, &d, &[forest]);
            settle(&mut g);
        }
    }
}

#[test]
fn elf_filter_offers_changeling_and_excludes_other_creatures() {
    let prepared = board(&[(LODGE, &[Cell::UntapTarget], Tiers::EXECUTES)])
        .permanent_or_hand("Wirewood Lodge", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Universal Automaton", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    activate(&mut g, 0, 0);
    let d = targets(&mut g);
    let changeling = Driver::object(&g, 2);
    assert!(!offers(&d, Driver::object(&g, 3)));
    answer_targets(&mut g, &d, &[changeling]);
    Driver::pay(&mut g, &[1]);
    settle(&mut g);
}

#[test]
fn graveyard_filter_checks_owner_zone_and_printed_mana_value() {
    let own = Zone::Graveyard { owner: PlayerId(0) };
    let opposing = Zone::Graveyard { owner: PlayerId(1) };
    let prepared = board(&[(
        UNEARTH,
        &[Cell::MoveOwnGraveyardCreatureToBattlefield],
        Tiers::EXECUTES,
    )])
    .permanent_or_hand("Unearth", 0, 0, true)
    .permanent_or_hand("Swamp", 0, 0, false)
    .card("Grizzly Bears", 0, 0, own)
    .card("Grizzly Bears", 1, 1, opposing)
    .permanent_or_hand("Grizzly Bears", 0, 0, false)
    .card("Swamp", 0, 0, own)
    .card("Colossal Dreadmaw", 0, 0, own)
    .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let bears = Driver::object(&g, 2);
    Driver::cast(&mut g, CardId(0));
    let d = targets(&mut g);
    for id in [3, 4, 5, 6] {
        assert!(!offers(&d, Driver::object(&g, id)));
    }
    answer_targets(&mut g, &d, &[bears]);
    Driver::pay(&mut g, &[1]);
    settle(&mut g);
    assert_eq!(zone(&g, 2), Zone::Battlefield);
    assert!(
        g.current(bears).is_err(),
        "CR 400.7 gives the card a new identity"
    );
}
