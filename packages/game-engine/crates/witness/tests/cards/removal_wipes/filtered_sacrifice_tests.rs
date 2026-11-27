//! CR 101.4, 603.10a, 701.21a and 903.9a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const TRIBUTE: &str = include_str!(
    "../../../../../tests/fixtures/phase1-r5/ba0fc2ca-7084-454a-9d4e-5cbca945a7ca.json"
);
const MARAUDER: &str = include_str!(
    "../../../../../tests/fixtures/phase1-r5/d8ad23a1-0b43-48ea-9fbe-d89b29194509.json"
);
const ARCHON: &str = include_str!(
    "../../../../../tests/fixtures/phase1-r5/aa1a6646-c1e6-4bff-9092-43ee3e137914.json"
);
const ENTRAPMENT: &str = include_str!(
    "../../../../../tests/fixtures/phase1-r5/2244bef9-1e4e-44ed-9b27-810f04f9e611.json"
);

fn compile(raw: &str) -> mtg_compiler::CompileResult {
    mtg_witness::compile(
        include_str!("../../../../../tests/fixtures/archive-manifest-phase1-r5.json"),
        raw,
    )
}

fn add(catalog: &mut Catalog, raw: &str) {
    let result = compile(raw);
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("complete sacrifice source must lower: {result:?}");
    };
    if catalog
        .definition_by_oracle_id(&definition.oracle_id)
        .is_none()
    {
        catalog.definitions.push(*definition);
    }
}

#[test]
fn filtered_sacrifice_sources_lower_completely() {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    add(&mut catalog, TRIBUTE);
    add(&mut catalog, MARAUDER);
    for raw in [
        include_str!(
            "../../../../../tests/fixtures/phase1-r5/5462cb38-7cd7-46e8-a273-35d0d1af45b4.json"
        ),
        include_str!(
            "../../../../../tests/fixtures/phase1-r5/efdaa3eb-e5d1-4438-b21d-2ec0fff44721.json"
        ),
    ] {
        add(&mut catalog, raw);
    }
    assert!(
        matches!(compile(ARCHON), mtg_compiler::CompileResult::Unsupported { reason, .. } if reason == "operand:filter:type:planeswalker")
    );
    assert!(!matches!(
        compile(ENTRAPMENT),
        mtg_compiler::CompileResult::Lowered { .. }
    ));
}

#[test]
fn filtered_sacrifice_apnap_suspends_before_any_move() {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    add(&mut catalog, TRIBUTE);
    add(
        &mut catalog,
        include_str!(
            "../../../../../tests/fixtures/phase1-r5/f90fe86c-5cca-483b-93c2-6e856fa01c88.json"
        ),
    );
    add(
        &mut catalog,
        include_str!(
            "../../../../../tests/fixtures/phase1-r5/037e7fc9-3aa6-484c-a2c8-43009e45f1d8.json"
        ),
    );
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    let prepared = Board::new(catalog, "filtered-sacrifice-apnap")
        .step(TurnStep::PrecombatMain)
        .permanent_or_hand("Tribute to the Wild", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Darksteel Myr", 1, 1, false)
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .permanent_or_hand("Sharuum the Hegemon", 2, 3, false)
        .commander(CardId(5))
        .libraries("Forest", 4)
        .prepare();
    let mut game = Game::new(prepared.clone());
    let cast = Driver::request(&mut game, u64::MAX).cast(CardId(0));
    Driver::answer(&mut game, u64::MAX, cast);
    for land in [1, 2] {
        let object = Driver::object(&game, land);
        let activate = Driver::request(&mut game, u64::MAX).activate_mana(object);
        Driver::answer(&mut game, u64::MAX, activate);
    }
    Driver::answer(
        &mut game,
        u64::MAX,
        Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: false,
            },
        },
    );
    Driver::answer(
        &mut game,
        u64::MAX,
        Answer::Pay {
            pay: PayAnswer::Spend {
                unrestricted: BTreeMap::from([(Color::G, 2.into())]),
            },
        },
    );
    for _ in 0..4 {
        Driver::answer(
            &mut game,
            u64::MAX,
            Answer::Pick {
                picks: vec![vec![0]],
            },
        );
    }
    let first = Driver::request(&mut game, u64::MAX);
    let first_object = Driver::object(&game, 3);
    assert_eq!(first.player, PlayerId(1));
    assert!(matches!(&first.reason, Reason::ForcedSacrifice { previous } if previous.is_empty()));
    assert_eq!(first.objects_offered(), vec![first_object]);
    Driver::answer(&mut game, u64::MAX, first.pick_object(first_object));
    let second = Driver::request(&mut game, u64::MAX);
    let second_object = Driver::object(&game, 5);
    assert_eq!(second.player, PlayerId(3));
    assert!(
        matches!(&second.reason, Reason::ForcedSacrifice { previous } if previous == &vec![(PlayerId(1), first_object)])
    );
    assert_eq!(second.objects_offered(), vec![second_object]);
    assert_eq!(game.current(first_object).unwrap().zone, Zone::Battlefield);
    let mut restored = Game::restore(prepared, &game.snapshot().unwrap()).unwrap();
    assert_eq!(Driver::request(&mut restored, u64::MAX), second);
    Driver::answer(&mut restored, u64::MAX, second.pick_object(second_object));
    let command = Driver::request(&mut restored, u64::MAX);
    assert_eq!(command.player, PlayerId(2));
    assert!(matches!(command.reason, Reason::CommanderReturn { .. }));
    for (card, owner) in [(3, 1), (5, 2)] {
        let current = Driver::object(&restored, card);
        assert_eq!(
            restored.current(current).unwrap().zone,
            Zone::Graveyard {
                owner: PlayerId(owner)
            }
        );
    }
    Driver::answer(
        &mut restored,
        u64::MAX,
        command.select(Candidate::Flag {
            flag: Flag::CommandZone,
        }),
    );
    Driver::request(&mut restored, u64::MAX);
    assert!(matches!(
        restored.current(Driver::object(&restored, 5)).unwrap().zone,
        Zone::Command { .. }
    ));
}
