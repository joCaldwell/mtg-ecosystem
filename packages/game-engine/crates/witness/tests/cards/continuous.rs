//! Frozen R4 witnesses for temporary combat prohibitions, printed fixed-color
//! protection and static keyword grants. CR 508.1c, 509.1b, 514.2, 611.2a/c,
//! 611.3a, 613.1f, 613.11, 702.3b, 702.11b, 702.12b, 702.16a/b/g/h.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_contracts::kir::{Cell, Tiers, definition_cells, definition_tiers};
use mtg_core::testkit::*;

macro_rules! record {
    ($id:literal) => {
        include_str!(concat!(
            "../../../../tests/fixtures/phase1-r4/",
            $id,
            ".json"
        ))
    };
}

const FALTER: &str = record!("d4b50749-a016-4aff-8d70-a1707cabf57b");
const CROSSWAY_VAMPIRE: &str = record!("543071da-fc89-4274-a2ae-d5d755d1f2a9");
const ROGUES_PASSAGE: &str = record!("f29dc596-2121-4421-8463-15f6c2e8b9b3");
const AKROMA: &str = record!("107f204c-9fd3-490d-8887-be63d14fc6a4");
const IRIDESCENT_ANGEL: &str = record!("28fa9211-96bb-4fef-81bf-8f58114a8df7");
const BANESLAYER: &str = record!("0e11792b-7fe5-4208-aa0b-e5d09b2b65fe");
const AVACYN: &str = record!("216cb26e-8da9-478b-bfbc-8030f7adee72");
const LEONIN_ABUNAS: &str = record!("10064324-34a0-47eb-a58e-01db10234ed9");
const ZEPHYR_NET: &str = record!("cd510cf2-df54-4024-919b-e4377cdb2be1");

fn lowered(raw: &str) -> Definition {
    let result = mtg_witness::compile(mtg_witness::R4_MANIFEST, raw);
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("frozen witness failed: {result:?}");
    };
    definition.validate().unwrap();
    assert_eq!(definition_tiers(&definition).unwrap(), Tiers::EXECUTES);
    *definition
}

fn board(raws: &[&str]) -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    catalog
        .definitions
        .extend(raws.iter().map(|raw| lowered(raw)));
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    Board::new(catalog, "r4-continuous-effects").step(TurnStep::PrecombatMain)
}

fn with(board: Board, cards: &[(&str, u8, bool)]) -> Game {
    let mut board = board;
    for &(name, seat, hand) in cards {
        board = board.permanent_or_hand(name, seat, seat, hand);
    }
    let mut game = Game::with_trace(board.libraries("Forest", 6).prepare(), TraceMode::Detailed);
    Driver::request(&mut game, u64::MAX);
    game
}

fn checked(g: &Game) {
    g.validate_state().unwrap();
    assert_recoverable(g.into());
}

/// Answer targets with `targets`, pass priority and stop on an empty stack.
fn settle(g: &mut Game, targets: &[TargetEntity]) {
    for _ in 0..40 {
        let r = Driver::request(g, u64::MAX);
        let answer = match &r.reason {
            Reason::Targets { .. } => r.entity_targets(targets.to_vec()),
            Reason::Priority if g.state.rules.stack.is_empty() => return checked(g),
            Reason::Priority => mtg_witness::temporary::pass(&r),
            x => panic!("settle {x:?}"),
        };
        g.respond(r.player, r.id, answer).unwrap();
    }
    panic!("stack did not settle");
}

fn pay(g: &mut Game) {
    mtg_witness::equipped::pay_to_priority(g, u64::MAX, vec![]);
}

/// Pass priority to the attack declaration, declare `attackers` at seat 1,
/// then return seat 1's block declaration.
fn blockers(g: &mut Game, attackers: &[ObjectId]) -> Decision<Rules> {
    for _ in 0..40 {
        let r = Driver::request(g, u64::MAX);
        let answer = match r.reason {
            Reason::Priority => mtg_witness::temporary::pass(&r),
            Reason::Attackers => {
                for a in attackers {
                    assert!(r.index(&Candidate::Object { object: *a }).is_some());
                }
                r.attack(attackers.iter().map(|a| (*a, PlayerId(1))).collect())
            }
            Reason::Blockers => return r,
            ref x => panic!("combat {x:?}"),
        };
        g.respond(r.player, r.id, answer).unwrap();
    }
    panic!("no block declaration");
}

fn attackers(g: &mut Game) -> Decision<Rules> {
    for _ in 0..40 {
        let r = Driver::request(g, u64::MAX);
        match r.reason {
            Reason::Priority => {
                let answer = mtg_witness::temporary::pass(&r);
                g.respond(r.player, r.id, answer).unwrap();
            }
            Reason::Attackers => return r,
            ref x => panic!("combat {x:?}"),
        }
    }
    panic!("no attack declaration");
}

/// Whether `blocker` may block `attacker` in this declaration.
fn may_block(r: &Decision<Rules>, blocker: ObjectId, attacker: ObjectId) -> bool {
    let Shape::Pick { slots, .. } = &r.shape else {
        panic!("block shape")
    };
    let (Some(subject), Some(row)) = (
        r.index(&Candidate::Object { object: blocker }),
        r.index(&Candidate::Object { object: attacker }),
    ) else {
        return false;
    };
    slots
        .iter()
        .any(|s| s.subject == Some(subject) && s.allowed.contains(&row))
}

fn object(g: &Game, id: u32) -> ObjectId {
    Driver::object(g, id)
}

fn keywords(g: &Game, id: ObjectId) -> Vec<KeywordAbility> {
    g.visible_object(PlayerId(0), g.current(id).unwrap())
        .unwrap()
        .unwrap()
        .keywords
}

fn target(object: ObjectId) -> TargetEntity {
    TargetEntity::Object { object }
}

#[test]
fn combat_prohibition_rows_lower_to_the_audited_cell() {
    for raw in [FALTER, CROSSWAY_VAMPIRE, ROGUES_PASSAGE] {
        let definition = lowered(raw);
        assert!(
            definition_cells(&definition)
                .unwrap()
                .contains(&Cell::CombatProhibitionUntilEndOfTurn),
            "{}",
            definition.name
        );
        assert!(
            definition
                .capabilities
                .contains(&Capability::TemporaryCombatProhibitionV1)
        );
    }
}

// CR 611.2c, 613.11: Falter modifies the rules, so the class is read at the
// block declaration; a flier is not restricted. CR 514.2 ends it at cleanup.
#[test]
fn falter_stops_only_nonflying_blockers_this_turn() {
    let mut g = with(
        board(&[FALTER]),
        &[
            ("Falter", 0, true),
            ("Grizzly Bears", 0, false),
            ("Grizzly Bears", 1, false),
            ("Vampire Nighthawk", 1, false),
            ("Mountain", 0, false),
            ("Mountain", 0, false),
        ],
    );
    let [attacker, ground, flier] = [1, 2, 3].map(|i| object(&g, i));
    let before = blockers(&mut g.fork(), &[attacker]);
    assert!(may_block(&before, ground, attacker));
    Driver::cast(&mut g, CardId(0));
    pay(&mut g);
    settle(&mut g, &[]);
    assert_eq!(g.state.rules.effects.len(), 1);
    let r = blockers(&mut g, &[attacker]);
    assert!(!may_block(&r, ground, attacker));
    assert!(may_block(&r, flier, attacker));
    g.respond(r.player, r.id, r.block(vec![(ground, attacker)]))
        .unwrap_err();
    let turn = g.state.rules.turn.number;
    for _ in 0..80 {
        if g.state.rules.turn.number != turn {
            break;
        }
        let r = Driver::request(&mut g, u64::MAX);
        let answer = mtg_witness::temporary::pass(&r);
        g.respond(r.player, r.id, answer).unwrap();
    }
    assert_ne!(g.state.rules.turn.number, turn);
    assert!(g.state.rules.effects.is_empty());
    checked(&g);
}

// CR 603.3d, 608.2b: the entry trigger's target cannot block this turn.
#[test]
fn crossway_vampire_trigger_restricts_its_target() {
    let mut g = with(
        board(&[CROSSWAY_VAMPIRE]),
        &[
            ("Crossway Vampire", 0, true),
            ("Grizzly Bears", 0, false),
            ("Grizzly Bears", 1, false),
            ("Colossal Dreadmaw", 1, false),
            ("Mountain", 0, false),
            ("Mountain", 0, false),
            ("Mountain", 0, false),
        ],
    );
    let [attacker, bears, big] = [1, 2, 3].map(|i| object(&g, i));
    Driver::cast(&mut g, CardId(0));
    pay(&mut g);
    settle(&mut g, &[target(big)]);
    let r = blockers(&mut g, &[attacker]);
    assert!(!may_block(&r, big, attacker));
    assert!(may_block(&r, bears, attacker));
}

// CR 509.1b: Rogue's Passage makes its target unblockable this turn.
#[test]
fn rogues_passage_makes_the_target_unblockable() {
    let mut g = with(
        board(&[ROGUES_PASSAGE]),
        &[
            ("Rogue's Passage", 0, false),
            ("Grizzly Bears", 0, false),
            ("Grizzly Bears", 0, false),
            ("Colossal Dreadmaw", 1, false),
            ("Forest", 0, false),
            ("Forest", 0, false),
            ("Forest", 0, false),
            ("Forest", 0, false),
        ],
    );
    let [passage, sneaky, plain, big] = [0, 1, 2, 3].map(|i| object(&g, i));
    assert_eq!(g.definition(passage.card).unwrap().activated.len(), 1);
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: passage,
            ability: 0,
        },
    );
    let r = Driver::request(&mut g, u64::MAX);
    g.respond(r.player, r.id, r.entity_targets(vec![target(sneaky)]))
        .unwrap();
    Driver::pay(&mut g, &[4, 5, 6, 7]);
    settle(&mut g, &[]);
    let r = blockers(&mut g, &[sneaky, plain]);
    assert!(!may_block(&r, big, sneaky));
    assert!(may_block(&r, big, plain));
}

// CR 702.16a,b,g: two separate fixed-color protections; red Lightning Bolt
// cannot target Akroma. CR 702.16h: each color is five separate abilities.
#[test]
fn printed_fixed_color_protection_blocks_bolt_targeting() {
    let akroma = lowered(AKROMA);
    assert!(
        akroma
            .keywords
            .contains(&KeywordAbility::ProtectionFromBlack)
    );
    assert!(akroma.keywords.contains(&KeywordAbility::ProtectionFromRed));
    assert!(
        !akroma
            .keywords
            .contains(&KeywordAbility::ProtectionFromWhite)
    );
    let angel = lowered(IRIDESCENT_ANGEL);
    for k in [
        KeywordAbility::ProtectionFromWhite,
        KeywordAbility::ProtectionFromBlue,
        KeywordAbility::ProtectionFromBlack,
        KeywordAbility::ProtectionFromRed,
        KeywordAbility::ProtectionFromGreen,
    ] {
        assert!(angel.keywords.contains(&k));
    }
    let mut g = with(
        board(&[AKROMA, IRIDESCENT_ANGEL]),
        &[
            ("Lightning Bolt", 0, true),
            ("Akroma, Angel of Wrath", 1, false),
            ("Iridescent Angel", 1, false),
            ("Grizzly Bears", 1, false),
            ("Mountain", 0, false),
        ],
    );
    let [akroma, angel, bears] = [1, 2, 3].map(|i| object(&g, i));
    Driver::cast(&mut g, CardId(0));
    let r = Driver::request(&mut g, u64::MAX);
    assert!(matches!(r.reason, Reason::Targets { .. }));
    assert!(r.index(&Candidate::Object { object: akroma }).is_none());
    assert!(r.index(&Candidate::Object { object: angel }).is_none());
    assert!(r.index(&Candidate::Object { object: bears }).is_some());
}

// Protection from creature types is not a fixed color; reject by name.
#[test]
fn protection_from_subtypes_rejects() {
    let result = mtg_witness::compile(mtg_witness::R4_MANIFEST, BANESLAYER);
    let mtg_compiler::CompileResult::Unsupported { reason, .. } = result else {
        panic!("{result:?}")
    };
    assert!(reason.contains("keyword:protection"), "{reason}");
}

// CR 611.3a, 613.1f, 702.12b: Avacyn grants indestructible to other
// permanents its controller controls, including lands; Murder fails.
#[test]
fn avacyn_grants_indestructible_to_other_permanents() {
    let mut g = with(
        board(&[AVACYN]),
        &[
            ("Avacyn, Angel of Hope", 0, false),
            ("Murder", 0, true),
            ("Grizzly Bears", 0, false),
            ("Grizzly Bears", 1, false),
            ("Swamp", 0, false),
            ("Swamp", 0, false),
            ("Swamp", 0, false),
        ],
    );
    let [avacyn, bears, theirs, swamp] = [0, 2, 3, 4].map(|i| object(&g, i));
    assert!(keywords(&g, bears).contains(&KeywordAbility::Indestructible));
    assert_eq!(keywords(&g, swamp), [KeywordAbility::Indestructible]);
    assert!(!keywords(&g, theirs).contains(&KeywordAbility::Indestructible));
    assert_eq!(
        keywords(&g, avacyn)
            .iter()
            .filter(|k| **k == KeywordAbility::Indestructible)
            .count(),
        1
    );
    Driver::cast(&mut g, CardId(1));
    let r = Driver::request(&mut g, u64::MAX);
    g.respond(r.player, r.id, r.entity_targets(vec![target(bears)]))
        .unwrap();
    pay(&mut g);
    settle(&mut g, &[]);
    assert_eq!(g.current(bears).unwrap().zone, Zone::Battlefield);
}

// CR 702.11b: Leonin Abunas gives its controller's artifacts hexproof.
#[test]
fn leonin_abunas_grants_artifacts_hexproof() {
    let g = with(
        board(&[LEONIN_ABUNAS]),
        &[
            ("Leonin Abunas", 0, false),
            ("Sol Ring", 0, false),
            ("Sol Ring", 1, false),
        ],
    );
    let [abunas, ring, theirs] = [0, 1, 2].map(|i| object(&g, i));
    assert_eq!(keywords(&g, ring), [KeywordAbility::Hexproof]);
    assert!(!g.targetable_by(ring, PlayerId(1)));
    assert!(g.targetable_by(ring, PlayerId(0)));
    assert!(g.targetable_by(theirs, PlayerId(0)));
    assert!(g.targetable_by(abunas, PlayerId(1)));
    checked(&g);
}

// CR 702.3b: an Aura-granted defender stops the enchanted creature attacking.
#[test]
fn zephyr_net_grants_defender_and_flying() {
    let mut g = with(
        board(&[ZEPHYR_NET]),
        &[
            ("Zephyr Net", 0, true),
            ("Grizzly Bears", 0, false),
            ("Grizzly Bears", 0, false),
            ("Island", 0, false),
            ("Island", 0, false),
        ],
    );
    let [netted, free] = [1, 2].map(|i| object(&g, i));
    Driver::cast(&mut g, CardId(0));
    let r = Driver::request(&mut g, u64::MAX);
    g.respond(r.player, r.id, r.entity_targets(vec![target(netted)]))
        .unwrap();
    pay(&mut g);
    settle(&mut g, &[]);
    let granted = keywords(&g, netted);
    assert!(granted.contains(&KeywordAbility::Defender));
    assert!(granted.contains(&KeywordAbility::Flying));
    let r = attackers(&mut g);
    assert!(r.index(&Candidate::Object { object: netted }).is_none());
    assert!(r.index(&Candidate::Object { object: free }).is_some());
}
