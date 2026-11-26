//! Changeling and "is every creature type" from exact frozen R4 source.
//! Pinned CR 205.3d, 205.3m, 604.3, 613.1d, 613.3, 702.73a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_contracts::kir::{Change, Scope, StaticEffect};
use mtg_core::testkit::*;

macro_rules! record {
    ($id:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/phase1-r4/",
            $id,
            ".json"
        ))
    };
}

const MUSCLE_SLIVER: &str = record!("06e0459f-d691-4470-884a-27c09b6612d2");
const RUNED_STALACTITE: &str = record!("1c169307-71f5-4679-8b5a-d60c1109b9db");
const WOODLAND_CHANGELING: &str = record!("338f0ab6-f744-4e5a-a358-05e71b93758d");
const WORTHY_KNIGHT: &str = record!("37733e29-3ade-4bfd-89c9-efd13b548a53");
const DARU_WARCHIEF: &str = record!("3fce53fb-6914-4f43-b46b-18ca32ab21b2");
const ELVISH_VANGUARD: &str = record!("4163aa30-7e3d-424f-b003-f4a300f0071e");
const FORERUNNER: &str = record!("56e2bdcf-d1d8-4c56-af3a-388cd42353e6");
const MISTFORM_ULTIMUS: &str = record!("7bf0e628-06b5-4e12-a526-a2bce08844f2");
const MASKWOOD_NEXUS: &str = record!("9b2cdbed-c733-409b-b0e4-2c8960c25111");
const CHANGELING_OUTCAST: &str = record!("a61ef0cc-1da9-49f9-b0dc-01cf9f6205be");
const IRREGULAR_COHORT: &str = record!("c0636d16-671c-4e80-af8c-67d80d2cd979");

fn lowered(raw: &str) -> Definition {
    let result = mtg_witness::compile(mtg_witness::R4_MANIFEST, raw);
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("frozen changeling witness must lower: {result:?}");
    };
    definition.validate().unwrap();
    *definition
}

fn game(cards: &[&str], setup: impl FnOnce(Board) -> Board) -> Game {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    catalog.definitions.extend(cards.iter().map(|raw| lowered(raw)));
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let board = Board::new(catalog, "changeling-witness")
        .step(TurnStep::PrecombatMain)
        .seats(2);
    Game::with_trace(
        setup(board).libraries("Island", 4).prepare(),
        TraceMode::Detailed,
    )
}

fn checked(g: &Game) {
    g.validate_state().unwrap();
    let snapshot = g.snapshot().unwrap();
    let restored = Game::restore(g.prepared.as_ref().clone(), &snapshot).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
}

/// Pass priority, accept optional effects and order triggers as offered
/// until the stack and pending triggers are empty.
fn settle(g: &mut Game) {
    for _ in 0..256 {
        let d = Driver::request(g, u64::MAX);
        if g.state.rules.stack.is_empty()
            && g.state.rules.pending_triggers.is_empty()
            && matches!(g.state.rules.frame, Frame::Priority)
        {
            checked(g);
            return;
        }
        let answer = match &d.reason {
            Reason::Priority => mtg_witness::temporary::pass(&d),
            Reason::OptionalEffect => d.pick_one(&Candidate::Flag { flag: Flag::Accept }).unwrap(),
            Reason::OrderTriggers => Answer::Order {
                order: (0..u32::try_from(d.candidates.len()).unwrap()).collect(),
            },
            other => panic!("unexpected decision {other:?}"),
        };
        g.respond(d.player, d.id, answer).unwrap();
    }
    panic!("the board never settled");
}

fn pt(g: &Game, id: u32) -> (Quantity, Quantity) {
    let c = g.characteristics(Driver::object(g, id)).unwrap();
    (c.power.unwrap(), c.toughness.unwrap())
}

fn q(value: i64) -> Quantity {
    Quantity::from(value)
}

/// CR 702.73a, 604.3: changeling and the "is every creature type" CDA are
/// printed characteristics, not keywords, so they hold in every zone; an
/// Equipment's "is every creature type" is its own unfiltered layer-4
/// effect (CR 205.3d), and a scope reaching other zones rejects.
#[test]
fn changeling_lowers_as_a_characteristic_in_every_zone() {
    for raw in [CHANGELING_OUTCAST, WOODLAND_CHANGELING, MISTFORM_ULTIMUS, IRREGULAR_COHORT] {
        let d = lowered(raw);
        assert!(d.characteristics.all_creature_types, "{}", d.name);
        assert!(d.characteristics.has_subtype("Sliver"));
        assert!(!d.characteristics.has_subtype("Equipment"));
        assert!(d.static_abilities.is_empty() || d.name == "Changeling Outcast");
    }
    let stalactite = lowered(RUNED_STALACTITE);
    assert!(!stalactite.characteristics.all_creature_types);
    assert!(stalactite.static_abilities.iter().any(|a| matches!(&a.effect,
        StaticEffect::Continuous { affects: Scope::Attachment { filter, .. }, changes, condition: None }
            if filter.0.is_empty() && *changes == [Change::AllCreatureTypes])));
    let result = mtg_witness::compile(mtg_witness::R4_MANIFEST, MASKWOOD_NEXUS);
    assert!(
        format!("{result:?}").contains("static:modifier:all-creature-types:scope"),
        "{result:?}"
    );
}

/// CR 205.3m, 613.4c: a changeling is a Sliver and a Soldier, so both lords'
/// tribal statics apply; a non-changeling creature gets neither.
#[test]
fn changeling_counts_for_tribal_statics() {
    let g = game(&[MUSCLE_SLIVER, DARU_WARCHIEF, WOODLAND_CHANGELING], |b| {
        b.permanent_or_hand("Muscle Sliver", 0, 0, false)
            .permanent_or_hand("Daru Warchief", 0, 0, false)
            .permanent_or_hand("Woodland Changeling", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
    });
    // Woodland Changeling 2/2: +1/+1 (Sliver) and +1/+2 (Soldier).
    assert_eq!(pt(&g, 2), (q(4), q(5)));
    assert_eq!(pt(&g, 3), (q(2), q(2)));
    checked(&g);
}

/// CR 601.2f, 603.2, 604.3: a changeling spell is a Soldier spell and a
/// Knight spell, so Daru Warchief reduces its cost and Worthy Knight
/// triggers when it is cast.
#[test]
fn changeling_spell_is_every_creature_type_on_the_stack() {
    let mut g = game(&[WORTHY_KNIGHT, DARU_WARCHIEF, WOODLAND_CHANGELING], |b| {
        b.permanent_or_hand("Worthy Knight", 0, 0, false)
            .permanent_or_hand("Daru Warchief", 0, 0, false)
            .permanent_or_hand("Woodland Changeling", 0, 0, true)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
    });
    // {2}{G} less {1}: two Forests pay for it.
    Driver::cast(&mut g, CardId(2));
    Driver::pay(&mut g, &[3, 4]);
    settle(&mut g);
    assert_eq!(g.state.rules.tokens.len(), 1);
    let changeling = Driver::object(&g, 2);
    assert_eq!(g.current(changeling).unwrap().zone, Zone::Battlefield);
    assert_recoverable((&g).into());
}

/// CR 603.2, 111.1: Elvish Vanguard sees the changeling creature and the
/// changeling Shapeshifter token it creates each enter as another Elf.
#[test]
fn changeling_and_changeling_token_trigger_creature_type_entries() {
    let mut g = game(&[ELVISH_VANGUARD, IRREGULAR_COHORT], |b| {
        b.permanent_or_hand("Elvish Vanguard", 0, 0, false)
            .permanent_or_hand("Irregular Cohort", 0, 0, true)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
    });
    Driver::cast(&mut g, CardId(1));
    Driver::pay(&mut g, &[2, 3, 4, 5]);
    settle(&mut g);
    assert_eq!(g.state.rules.tokens.len(), 1);
    let token = g
        .state
        .rules
        .objects
        .values()
        .find(|o| g.state.rules.tokens.contains_key(&o.id.card))
        .unwrap()
        .id;
    assert!(g.characteristics(token).unwrap().has_subtype("Elf"));
    // Elvish Vanguard 1/1 with two +1/+1 counters.
    assert_eq!(pt(&g, 0), (q(3), q(3)));
    assert_recoverable((&g).into());
}

/// CR 604.3: Mistform Ultimus is every creature type in its owner's
/// library, so it is a Merfolk card there.
#[test]
fn every_creature_type_holds_in_the_library() {
    let g = game(&[MISTFORM_ULTIMUS, FORERUNNER], |b| {
        b.library_card("Mistform Ultimus", 0)
    });
    let card = g
        .state
        .rules
        .objects
        .values()
        .find(|o| {
            matches!(o.zone, Zone::Library { .. })
                && g.characteristics(o.id).is_ok_and(|c| c.has_subtype("Merfolk"))
        })
        .expect("the library changeling is a Merfolk card");
    assert!(g.characteristics(card.id).unwrap().all_creature_types);
    checked(&g);
}

/// CR 613.1d, 613.7e: an equipped creature becomes every creature type, so
/// Muscle Sliver pumps it; unequipped it is only a Bear.
#[test]
fn equipment_makes_the_equipped_creature_every_creature_type() {
    let mut g = game(&[RUNED_STALACTITE, MUSCLE_SLIVER], |b| {
        b.permanent_or_hand("Runed Stalactite", 0, 0, false)
            .permanent_or_hand("Muscle Sliver", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
    });
    let bears = Driver::object(&g, 2);
    assert!(!g.characteristics(bears).unwrap().has_subtype("Sliver"));
    assert_eq!(pt(&g, 2), (q(2), q(2)));
    let gear = Driver::object(&g, 0);
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: gear,
            ability: 0,
        },
    );
    let r = Driver::request(&mut g, u64::MAX);
    g.respond(r.player, r.id, r.targets(vec![bears])).unwrap();
    mtg_witness::equipped::pay_and_resolve(&mut g, u64::MAX, vec![]);
    checked(&g);
    let c = g.characteristics(bears).unwrap();
    assert!(c.all_creature_types && c.has_subtype("Sliver") && c.has_subtype("Bear"));
    // 2/2 +1/+1 (Stalactite) +1/+1 (Sliver).
    assert_eq!(pt(&g, 2), (q(4), q(4)));
    assert_recoverable((&g).into());
}
