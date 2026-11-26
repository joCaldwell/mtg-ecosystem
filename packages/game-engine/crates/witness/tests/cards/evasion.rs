//! Evasion keywords and block restrictions from exact frozen R5 source.
//! Pinned CR 509.1a-c, 702.13b, 702.14c, 702.28b, 702.31b, 702.36b, 702.111b,
//! 702.118b: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_core::testkit::*;
use std::collections::BTreeMap;

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

const WEI_STRIKE_FORCE: &str = record!("002ad5fa-a20c-4d30-beef-401d89868d67");
const WHISPERING_SHADE: &str = record!("0036d062-10dc-4267-98b4-1d2f6b190a61");
const KRENKOS_ENFORCER: &str = record!("0d9787e3-f482-4a98-95af-0438ed09c96c");
const DAUTHI_MERCENARY: &str = record!("1b57c480-e2c5-44cd-820f-e208041bfd18");
const UNDERCITY_SHADE: &str = record!("28f3d535-9503-4d35-aeed-d3e59259f760");
const FURTIVE_HOMUNCULUS: &str = record!("6b59ffa2-5c8b-4549-aa09-2f2a5cabc5af");
const DRYAD_SOPHISTICATE: &str = record!("7cf59255-0ad7-4ca8-bc95-f3a8cc2643d5");

const KEYWORD_SOURCES: &[&str] = &[
    WEI_STRIKE_FORCE,
    WHISPERING_SHADE,
    KRENKOS_ENFORCER,
    DAUTHI_MERCENARY,
    UNDERCITY_SHADE,
    FURTIVE_HOMUNCULUS,
];

pub(super) fn lowered(raw: &str) -> Definition {
    let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, raw);
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("frozen evasion witness must lower: {result:?}");
    };
    definition.validate().unwrap();
    *definition
}

pub(super) fn rejected(raw: &str) -> String {
    match mtg_witness::compile(mtg_witness::R5_MANIFEST, raw) {
        mtg_compiler::CompileResult::Unsupported { reason, .. } => reason,
        other => panic!("expected an explicit rejection: {other:?}"),
    }
}

/// A two-seat board at seat 0's first main phase. `cards` are
/// (name, controller, zone) and every object is owned by its controller.
pub(super) fn game(sources: &[&str], cards: &[(&str, u8, Zone)]) -> Game {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for raw in sources {
        let definition = lowered(raw);
        if catalog
            .definition_by_oracle_id(&definition.oracle_id)
            .is_none()
        {
            catalog.definitions.push(definition);
        }
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let mut board = Board::new(catalog, "evasion-witness")
        .step(TurnStep::PrecombatMain)
        .seats(2);
    for &(name, controller, zone) in cards {
        board = board.card(name, controller, controller, zone);
    }
    Game::with_trace(board.libraries("Island", 8).prepare(), TraceMode::Detailed)
}

pub(super) fn checked(g: &Game) {
    g.validate_state().unwrap();
    let snapshot = g.snapshot().unwrap();
    let restored = Game::restore(g.prepared.as_ref().clone(), &snapshot).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
}

/// Pass priority until a decision with `reason` is requested.
pub(super) fn until(g: &mut Game, reason: &Reason<Rules>) -> Decision<Rules> {
    for _ in 0..200 {
        let d = Driver::request(g, u64::MAX);
        if &d.reason == reason {
            return d;
        }
        let answer = match &d.reason {
            Reason::Priority => mtg_witness::temporary::pass(&d),
            Reason::Attackers => d.attack(vec![]),
            Reason::Blockers => d.block(vec![]),
            other => panic!("unexpected decision {other:?}"),
        };
        g.respond(d.player, d.id, answer).unwrap();
    }
    panic!("never reached {reason:?}");
}

/// Seat 0 attacks seat 1 with the listed board ids; returns seat 1's
/// blocker decision.
pub(super) fn attack(g: &mut Game, attackers: &[u32]) -> Decision<Rules> {
    let d = until(g, &Reason::Attackers);
    let attacks = attackers
        .iter()
        .map(|id| (Driver::object(g, *id), PlayerId(1)))
        .collect();
    g.respond(d.player, d.id, d.attack(attacks)).unwrap();
    let blockers = until(g, &Reason::Blockers);
    checked(g);
    blockers
}

/// The attackers each offered blocker may block, by board id.
pub(super) fn offered(g: &Game, d: &Decision<Rules>) -> BTreeMap<u32, Vec<u32>> {
    let Shape::Pick { slots, .. } = &d.shape else {
        panic!("blocks are picks")
    };
    let id = |c: &Candidate<Rules>| {
        let Candidate::Object { object } = c else {
            panic!("blocks name objects")
        };
        (0..64)
            .find(|i| Driver::object(g, *i) == *object)
            .expect("board object")
    };
    slots
        .iter()
        .map(|s| {
            (
                id(&d.candidates[s.subject.unwrap() as usize]),
                s.allowed
                    .iter()
                    .map(|a| id(&d.candidates[*a as usize]))
                    .collect(),
            )
        })
        .collect()
}

/// The declaration by board ids is rejected without changing the game.
pub(super) fn refuses(g: &mut Game, d: &Decision<Rules>, blocks: &[(u32, u32)]) {
    let before = g.snapshot().unwrap();
    let blocks = blocks
        .iter()
        .map(|(b, a)| (Driver::object(g, *b), Driver::object(g, *a)))
        .collect();
    let answer = forged_block(d, blocks);
    assert!(g.respond(d.player, d.id, answer).is_err());
    assert_eq!(g.snapshot().unwrap(), before);
}

/// The declaration by board ids is accepted.
pub(super) fn accepts(g: &mut Game, d: &Decision<Rules>, blocks: &[(u32, u32)]) {
    let blocks = blocks
        .iter()
        .map(|(b, a)| (Driver::object(g, *b), Driver::object(g, *a)))
        .collect();
    g.respond(d.player, d.id, d.block(blocks)).unwrap();
    checked(g);
}

/// A complete pick answer naming candidates even where the shape omits them.
fn forged_block(d: &Decision<Rules>, blocks: Vec<(ObjectId, ObjectId)>) -> Answer {
    let Shape::Pick { slots, .. } = &d.shape else {
        panic!("blocks are picks")
    };
    let index = |o: ObjectId| {
        d.candidates
            .iter()
            .position(|c| *c == Candidate::Object { object: o })
            .map(|i| i as u32)
    };
    Answer::Pick {
        picks: slots
            .iter()
            .map(|s| {
                blocks
                    .iter()
                    .filter(|(b, _)| index(*b) == s.subject)
                    .filter_map(|(_, a)| index(*a).or(Some(u32::MAX)))
                    .collect()
            })
            .collect(),
    }
}

use Zone::Battlefield as B;

#[test]
fn evasion_keywords_lower_from_printed_source() {
    let expect = [
        (WEI_STRIKE_FORCE, KeywordAbility::Horsemanship),
        (WHISPERING_SHADE, KeywordAbility::Swampwalk),
        (KRENKOS_ENFORCER, KeywordAbility::Intimidate),
        (DAUTHI_MERCENARY, KeywordAbility::Shadow),
        (UNDERCITY_SHADE, KeywordAbility::Fear),
        (FURTIVE_HOMUNCULUS, KeywordAbility::Skulk),
    ];
    for (raw, keyword) in expect {
        let d = lowered(raw);
        assert_eq!(d.keywords, vec![keyword], "{}", d.name);
    }
    // CR 702.14c: nonbasic landwalk is not a basic land type.
    assert_eq!(rejected(DRYAD_SOPHISTICATE), "keyword:landwalk:quality");
}

/// CR 702.14c: swampwalk is unblockable only while the defending player
/// controls a Swamp.
#[test]
fn swampwalk_reads_the_defending_players_lands() {
    for swamp in [false, true] {
        let land = if swamp { "Swamp" } else { "Island" };
        let mut g = game(
            KEYWORD_SOURCES,
            &[
                ("Whispering Shade", 0, B),
                ("Grizzly Bears", 1, B),
                (land, 1, B),
            ],
        );
        let d = attack(&mut g, &[0]);
        let offered = offered(&g, &d);
        if swamp {
            assert_eq!(offered[&1], Vec::<u32>::new());
            refuses(&mut g, &d, &[(1, 0)]);
        } else {
            assert_eq!(offered[&1], vec![0]);
            accepts(&mut g, &d, &[(1, 0)]);
        }
    }
}

/// CR 702.31b, 702.28b: horsemanship restricts only its blockers; shadow
/// restricts both directions.
#[test]
fn horsemanship_and_shadow_restrict_blockers() {
    let mut g = game(
        KEYWORD_SOURCES,
        &[
            ("Wei Strike Force", 0, B),
            ("Dauthi Mercenary", 0, B),
            ("Grizzly Bears", 0, B),
            ("Grizzly Bears", 1, B),
            ("Wei Strike Force", 1, B),
            ("Dauthi Mercenary", 1, B),
        ],
    );
    let d = attack(&mut g, &[0, 1, 2]);
    let offered = offered(&g, &d);
    assert_eq!(offered[&3], vec![2]);
    assert_eq!(offered[&4], vec![0, 2]);
    assert_eq!(offered[&5], vec![1]);
    refuses(&mut g, &d, &[(3, 0)]);
    refuses(&mut g, &d, &[(3, 1)]);
    refuses(&mut g, &d, &[(5, 2)]);
    accepts(&mut g, &d, &[(4, 0), (5, 1), (3, 2)]);
}

/// CR 702.36b, 702.13b, 702.118b: fear admits artifact or black blockers,
/// intimidate artifact or color-sharing blockers, skulk no greater power.
#[test]
fn fear_intimidate_and_skulk_filter_blockers() {
    let mut g = game(
        KEYWORD_SOURCES,
        &[
            ("Undercity Shade", 0, B),
            ("Krenko's Enforcer", 0, B),
            ("Furtive Homunculus", 0, B),
            ("Grizzly Bears", 1, B),
            ("Royal Assassin", 1, B),
            ("Darksteel Myr", 1, B),
            ("Krenko's Enforcer", 1, B),
            ("Colossal Dreadmaw", 1, B),
        ],
    );
    let d = attack(&mut g, &[0, 1, 2]);
    let offered = offered(&g, &d);
    assert_eq!(offered[&3], vec![2]);
    assert_eq!(offered[&4], vec![0, 2]);
    assert_eq!(offered[&5], vec![0, 1, 2]);
    assert_eq!(offered[&6], vec![1, 2]);
    assert_eq!(offered[&7], Vec::<u32>::new());
    refuses(&mut g, &d, &[(3, 0)]);
    refuses(&mut g, &d, &[(4, 1)]);
    refuses(&mut g, &d, &[(7, 2)]);
    accepts(&mut g, &d, &[(4, 0), (6, 1), (3, 2)]);
}
