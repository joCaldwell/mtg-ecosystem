//! Characteristic-defining power and toughness from exact frozen R5 source.
//! Pinned CR 107.3, 208.2a, 604.3, 613.4a, 613.4c, 704.5f:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::planeswalker::JACE;
use mtg_contracts::kir::{Amount, DistinctCharacteristic, SetExpr, StaticEffect, ZoneExpr};
use mtg_core::testkit::*;

macro_rules! record {
    ($id:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/phase1-r5/",
            $id,
            ".json"
        ))
    };
}

const TARMOGOYF: &str = record!("45900b2f-f6a9-4c42-9642-008f3c1cf6dd");
const LHURGOYF: &str = record!("0fe76004-b2f4-4b11-abce-75fd76f68b4d");
const PSYCHOSIS_CRAWLER: &str = record!("2876e74f-a242-4995-9702-0b737a1ab67a");
const SOUL_OF_ETERNITY: &str = record!("4d211447-9f18-4d23-8f19-cdf6ad3c68ba");
const GODHEAD_OF_AWE: &str = record!("1ed5751a-4b45-4cd2-b19c-ac84508de4f8");
const SQUAWKROASTER: &str = record!("6b79404b-2fae-41fb-994c-d4cd895e8f09");
const NISHOBA_BRAWLER: &str = record!("c501e577-5137-4dd6-b923-95408b78258a");

fn lowered(raw: &str) -> Definition {
    let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, raw);
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("frozen CDA witness must lower: {result:?}");
    };
    definition.validate().unwrap();
    *definition
}

fn game(cards: &[&str], setup: impl FnOnce(Board) -> Board) -> Game {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    catalog
        .definitions
        .extend(cards.iter().map(|raw| lowered(raw)));
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let board = Board::new(catalog, "characteristic-pt-witness")
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

/// Pass priority until the stack and pending triggers are empty.
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

fn graveyard(owner: u8) -> Zone {
    Zone::Graveyard {
        owner: PlayerId(owner),
    }
}

/// CR 208.2a, 604.3: a printed star is the value one self CDA defines in
/// every zone; `N+*` prints the fixed term of its sum. A formula outside
/// the admitted values rejects by name.
#[test]
fn cda_lowers_as_a_static_in_every_zone() {
    let goyf = lowered(TARMOGOYF);
    assert_eq!(
        (goyf.characteristics.power, goyf.characteristics.toughness),
        (Some(q(0)), Some(q(1)))
    );
    let [ability] = goyf.static_abilities.as_slice() else {
        panic!("one CDA");
    };
    assert_eq!(ability.functioning_zone, FunctioningZone::Any);
    let types = Amount::Distinct {
        set: SetExpr::Matching {
            zone: ZoneExpr::Graveyards,
            filter: Default::default(),
        },
        characteristic: DistinctCharacteristic::CardType,
    };
    assert_eq!(
        ability.effect,
        StaticEffect::DefinePowerToughness {
            power: Some(types.clone()),
            toughness: Some(Amount::Sum {
                terms: vec![types, Amount::fixed(1)],
            }),
        }
    );
    let crawler = lowered(PSYCHOSIS_CRAWLER);
    assert!(crawler.static_abilities.iter().any(|a| matches!(&a.effect,
        StaticEffect::DefinePowerToughness {
            power: Some(Amount::Count { set: SetExpr::Matching { zone: ZoneExpr::Hand { .. }, filter } }),
            toughness: Some(_),
        } if filter.0.is_empty())));
    let soul = format!(
        "{:?}",
        mtg_witness::compile(mtg_witness::R5_MANIFEST, SOUL_OF_ETERNITY)
    );
    assert!(soul.contains("static:cda-pt:amount:life_total"), "{soul}");
}

/// CR 604.3, 613.4a, 613.4c: Tarmogoyf's value is read whenever it is
/// looked at. A Bolt on the stack is in no graveyard; once it resolves the
/// dead creature and the instant are, and the 7a value takes Glorious
/// Anthem's +1/+1 after it.
#[test]
fn tarmogoyf_follows_the_graveyards_before_an_anthem() {
    let mut g = game(&[TARMOGOYF], |b| {
        b.permanent_or_hand("Tarmogoyf", 0, 0, false)
            .permanent_or_hand("Glorious Anthem", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 1, 1, false)
            .permanent_or_hand("Lightning Bolt", 0, 0, true)
            .permanent_or_hand("Mountain", 0, 0, false)
    });
    // 0/1 printed star values plus the anthem.
    assert_eq!(pt(&g, 0), (q(1), q(2)));
    Driver::cast(&mut g, CardId(3));
    let r = Driver::request(&mut g, u64::MAX);
    g.respond(r.player, r.id, r.targets(vec![Driver::object(&g, 2)]))
        .unwrap();
    Driver::pay(&mut g, &[4]);
    assert_eq!(pt(&g, 0), (q(1), q(2)));
    settle(&mut g);
    assert_eq!(g.zone(Driver::object(&g, 2)), Some(graveyard(1)));
    assert_eq!(g.zone(Driver::object(&g, 3)), Some(graveyard(0)));
    // Creature and instant: 2/3, then +1/+1.
    assert_eq!(pt(&g, 0), (q(3), q(4)));
    assert_recoverable((&g).into());
}

/// CR 604.3, 208.2a: the CDA functions in hand and graveyard. A crawler in
/// hand counts its owner's hand including itself; a Lhurgoyf in a
/// graveyard counts itself, and the opponent's permitted view reads that
/// public value.
#[test]
fn cda_values_hold_in_hand_and_graveyard() {
    let g = game(&[PSYCHOSIS_CRAWLER, LHURGOYF], |b| {
        b.permanent_or_hand("Psychosis Crawler", 0, 0, true)
            .permanent_or_hand("Grizzly Bears", 0, 0, true)
            .permanent_or_hand("Mountain", 0, 0, true)
            .card("Lhurgoyf", 0, 0, graveyard(0))
            .card("Grizzly Bears", 1, 1, graveyard(1))
            .card("Lightning Bolt", 1, 1, graveyard(1))
    });
    assert_eq!(pt(&g, 0), (q(3), q(3)));
    assert_eq!(pt(&g, 3), (q(2), q(3)));
    let QueryResult::Complete(QueryValue::Observation(observation)) =
        g.player_session(PlayerId(1)).observe(Budget::unlimited())
    else {
        panic!("expected permitted observation");
    };
    let lhurgoyf = observation
        .view
        .objects
        .iter()
        .find(|o| o.zone == graveyard(0))
        .and_then(|o| o.characteristics.as_ref())
        .expect("public graveyard card");
    assert_eq!(
        (lhurgoyf.power.clone(), lhurgoyf.toughness.clone()),
        (Some(q(2)), Some(q(3)))
    );
    checked(&g);
}

/// CR 704.5f, 613.4a: casting the last card from hand leaves Psychosis
/// Crawler 0/0, and the state-based check puts it into the graveyard,
/// where its hand count is still zero.
#[test]
fn zero_toughness_cda_creature_dies() {
    let mut g = game(&[PSYCHOSIS_CRAWLER], |b| {
        b.permanent_or_hand("Psychosis Crawler", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, true)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
    });
    assert_eq!(pt(&g, 0), (q(1), q(1)));
    Driver::cast(&mut g, CardId(1));
    Driver::pay(&mut g, &[2, 3]);
    settle(&mut g);
    assert_eq!(g.zone(Driver::object(&g, 0)), Some(graveyard(0)));
    assert_eq!(g.zone(Driver::object(&g, 1)), Some(Zone::Battlefield));
    assert_eq!(pt(&g, 0), (q(0), q(0)));
    assert_recoverable((&g).into());
}

/// CR 205.2a: Tarmogoyf counts every represented card type once, so one
/// graveyard card of each type makes it `CARD_TYPES.len()` / that plus 1;
/// an artifact creature adds no new type.
#[test]
fn tarmogoyf_counts_every_represented_card_type() {
    let g = game(&[TARMOGOYF, JACE], |b| {
        let b = b.permanent_or_hand("Tarmogoyf", 0, 0, false);
        [
            "Forest",
            "Grizzly Bears",
            "Lightning Bolt",
            "Sign in Blood",
            "Glorious Anthem",
            "Sol Ring",
            "Darksteel Myr",
            "Jace Beleren",
        ]
        .into_iter()
        .fold(b, |b, name| b.card(name, 1, 1, graveyard(1)))
    });
    let all = i64::try_from(CardKind::CARD_TYPES.len()).unwrap();
    assert_eq!(pt(&g, 0), (q(all), q(all + 1)));
    checked(&g);
}

/// CR 205.3i, 305.6: domain counts the basic land types among lands its
/// controller controls; a nonbasic land without a basic land type and an
/// opponent's Mountain add nothing.
#[test]
fn domain_counts_basic_land_types_you_control() {
    let g = game(&[NISHOBA_BRAWLER], |b| {
        b.permanent_or_hand("Nishoba Brawler", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Yavimaya Coast", 0, 0, false)
            .permanent_or_hand("Mountain", 1, 1, false)
    });
    assert_eq!(pt(&g, 0).0, q(2));
    checked(&g);
}

/// CR 105.1, 105.2: colors among permanents you control, the source
/// included; colorless permanents and an opponent's colors add nothing.
#[test]
fn vivid_counts_colors_among_your_permanents() {
    let g = game(&[SQUAWKROASTER], |b| {
        b.permanent_or_hand("Squawkroaster", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .permanent_or_hand("Storm Crow", 0, 0, false)
            .permanent_or_hand("Sol Ring", 0, 0, false)
            .permanent_or_hand("Typhoid Rats", 1, 1, false)
    });
    assert_eq!(pt(&g, 0).0, q(3));
    checked(&g);
}

/// CR 613.4a-c: Godhead of Awe's base 1/1 (layer 7b) overrides the CDA's
/// 7a value, and Glorious Anthem's +1/+1 (7c) still applies after it.
#[test]
fn a_later_base_setter_overrides_the_cda() {
    let g = game(&[TARMOGOYF, GODHEAD_OF_AWE], |b| {
        b.permanent_or_hand("Tarmogoyf", 0, 0, false)
            .permanent_or_hand("Godhead of Awe", 1, 1, false)
            .permanent_or_hand("Glorious Anthem", 0, 0, false)
            .card("Grizzly Bears", 1, 1, graveyard(1))
            .card("Lightning Bolt", 1, 1, graveyard(1))
    });
    assert_eq!(pt(&g, 0), (q(2), q(2)));
    // Without the setter the 7a value is 2/3, plus the anthem.
    let g = game(&[TARMOGOYF], |b| {
        b.permanent_or_hand("Tarmogoyf", 0, 0, false)
            .permanent_or_hand("Glorious Anthem", 0, 0, false)
            .card("Grizzly Bears", 1, 1, graveyard(1))
            .card("Lightning Bolt", 1, 1, graveyard(1))
    });
    assert_eq!(pt(&g, 0), (q(3), q(4)));
    checked(&g);
}
