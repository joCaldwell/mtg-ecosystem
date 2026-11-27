//! Frozen R5 witnesses for static ability loss and type replacement.
//! CR 113.10, 205.1a-b, 205.3d, 205.4b, 305.6-7, 604.3a, 613.1d, 613.1f,
//! 613.4a, 613.6, 613.7, 613.8, 614.12, 700.5.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_contracts::kir::{Change, Scope, StaticEffect, definition_tiers};
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

const KENRITHS_TRANSFORMATION: &str = record!("a492a323-df8a-40fd-bff0-50091baa7700");
const DARKSTEEL_MUTATION: &str = record!("05a4f8ff-49da-42af-add5-6248c4b0644b");
const COLOSSUS_HAMMER: &str = record!("8ec03b88-8d3a-4a32-8b7c-7da59b0c03d0");
const MYSTIC_SUBDUAL: &str = record!("fdd423d5-f19b-4bb2-98ca-75cc3c058d32");
const HUMILITY: &str = record!("ed7bdb3e-5c51-4547-9266-76a791e0b2b0");
const BLOOD_MOON: &str = record!("94fac5fe-97d5-4c12-a80c-8efff9d853ae");
const SONG_OF_THE_DRYADS: &str = record!("7c3944fa-7c86-4979-85a9-86196aa94594");
const ARCHETYPE_OF_IMAGINATION: &str = record!("a5458de0-0f61-49a3-a013-d90f92559809");
const HELIOD: &str = record!("4ea488b2-5ba0-4565-b928-570c8e03b926");
const TOXICRENE: &str = record!("fbfe98f0-110b-4d4b-9240-2767b08acf68");
const LIGHTFOOT_TECHNIQUE: &str = record!("601b7481-19fc-4779-950e-883a9a22aec3");
const LORD_OF_ATLANTIS: &str = record!("cc7f290f-ca00-4285-9bdb-4b4402444f30");
const KIORAS_FOLLOWER: &str = record!("22c044ad-77d7-4c93-953d-e2daa9686ff7");
const TARMOGOYF: &str = record!("45900b2f-f6a9-4c42-9642-008f3c1cf6dd");
const LLANOWAR_ELVES: &str = record!("68954295-54e3-4303-a6bc-fc4547a4e3a3");
const KEIGA: &str = record!("521b1598-58c6-47a0-a7b3-021d70bf2b05");
const COMMAND_TOWER: &str = record!("0895c9b7-ae7d-4bb3-af17-3b75deb50a25");
const BREEDING_POOL: &str = record!("20283c4a-f1f0-42f0-bc08-6da87474426b");
const CRUMBLING_NECROPOLIS: &str = record!("7190debf-708b-4f41-9714-0d0a5bd5a74e");
const HIVESTONE: &str = record!("72aeed80-653b-4c53-ba65-a339aad14b9a");
const NIBLIS_OF_DUSK: &str = record!("d14d41a5-6f38-43c7-b95c-b4a723002c20");
const DEEP_FREEZE: &str = record!("648aead4-dc0b-47bc-911c-8dcd2cfd32d3");
const RUSTED_RELIC: &str = record!("57b22bc3-23e8-4632-b10a-495fac5c77c7");
const LITHOFORM_BLIGHT: &str = record!("b256c311-e37d-4b3c-889c-85443e3d5e7e");
const TURN_TO_FROG: &str = record!("74c4c1e2-c50d-4c8f-889a-0d5674dc6d67");
const TRUE_CONVICTION: &str = record!("fc299c1c-50f3-492a-b6b8-a3664bb72ab7");
const REGRESS: &str = record!("2fa763ad-4d94-4c3a-a099-13ac22c09ce4");
const DARU_WARCHIEF: &str = record!("3fce53fb-6914-4f43-b46b-18ca32ab21b2");
const BLAZING_ARCHON: &str = record!("82a70e4c-01ed-4280-80e4-86cb2bf6e63d");

const ALL: &[&str] = &[
    KENRITHS_TRANSFORMATION,
    DARKSTEEL_MUTATION,
    COLOSSUS_HAMMER,
    MYSTIC_SUBDUAL,
    HUMILITY,
    BLOOD_MOON,
    SONG_OF_THE_DRYADS,
    ARCHETYPE_OF_IMAGINATION,
    HELIOD,
    TOXICRENE,
    LIGHTFOOT_TECHNIQUE,
    LORD_OF_ATLANTIS,
    KIORAS_FOLLOWER,
    TARMOGOYF,
    LLANOWAR_ELVES,
    KEIGA,
    COMMAND_TOWER,
    BREEDING_POOL,
    CRUMBLING_NECROPOLIS,
    HIVESTONE,
    NIBLIS_OF_DUSK,
    DARU_WARCHIEF,
    TRUE_CONVICTION,
    REGRESS,
    BLAZING_ARCHON,
];

fn lowered(raw: &str) -> Definition {
    let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, raw);
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("frozen witness failed: {result:?}");
    };
    definition.validate().unwrap();
    definition_tiers(&definition).unwrap();
    *definition
}

fn unstarted(cards: &[(&str, u8, Zone)]) -> Game {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for raw in ALL {
        let d = lowered(raw);
        if !catalog
            .definitions
            .iter()
            .any(|x| x.oracle_id == d.oracle_id)
        {
            catalog.definitions.push(d);
        }
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let mut board = Board::new(catalog, "ability-loss-witness")
        .step(TurnStep::PrecombatMain)
        .seats(2);
    for &(name, seat, zone) in cards {
        let zone = match zone {
            Zone::Hand { .. } => Zone::Hand {
                owner: PlayerId(seat),
            },
            Zone::Graveyard { .. } => Zone::Graveyard {
                owner: PlayerId(seat),
            },
            other => other,
        };
        board = board.card(name, seat, seat, zone);
    }
    Game::with_trace(board.libraries("Forest", 6).prepare(), TraceMode::Detailed)
}

fn game(cards: &[(&str, u8, Zone)]) -> Game {
    let mut game = unstarted(cards);
    Driver::request(&mut game, u64::MAX);
    game
}

const B: Zone = Zone::Battlefield;
const H: Zone = Zone::Hand { owner: PlayerId(0) };
const G: Zone = Zone::Graveyard { owner: PlayerId(0) };

fn checked(g: &Game) {
    g.validate_state().unwrap();
    let snapshot = g.snapshot().unwrap();
    let restored = Game::restore(g.prepared.as_ref().clone(), &snapshot).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
    assert_recoverable(g.into());
}

/// Answer targets with `targets`, pass priority and stop on an empty stack.
fn settle(g: &mut Game, targets: &[ObjectId]) {
    for _ in 0..64 {
        let r = Driver::request(g, u64::MAX);
        let answer = match &r.reason {
            Reason::Targets { .. } => r.entity_targets(
                targets
                    .iter()
                    .map(|object| TargetEntity::Object { object: *object })
                    .collect(),
            ),
            Reason::Priority
                if g.state.rules.stack.is_empty() && g.state.rules.pending_triggers.is_empty() =>
            {
                return checked(g);
            }
            Reason::Priority => mtg_witness::temporary::pass(&r),
            Reason::OrderTriggers => Answer::Order {
                order: (0..u32::try_from(r.candidates.len()).unwrap()).collect(),
            },
            x => panic!("settle {x:?}"),
        };
        g.respond(r.player, r.id, answer).unwrap();
    }
    panic!("stack did not settle");
}

/// Cast card `spell` on `targets`, paying with the lands `mana`, and
/// resolve it.
fn cast_on(g: &mut Game, spell: u32, targets: &[ObjectId], mana: &[u32]) {
    Driver::cast(g, CardId(spell));
    if !targets.is_empty() {
        let r = Driver::request(g, u64::MAX);
        g.respond(
            r.player,
            r.id,
            r.entity_targets(
                targets
                    .iter()
                    .map(|object| TargetEntity::Object { object: *object })
                    .collect(),
            ),
        )
        .unwrap();
    }
    Driver::pay(g, mana);
    settle(g, &[]);
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

fn pt(g: &Game, id: ObjectId) -> (Quantity, Quantity) {
    let c = g.characteristics(id).unwrap();
    (c.power.unwrap(), c.toughness.unwrap())
}

fn q(value: i64) -> Quantity {
    Quantity::from(value)
}

fn can(g: &mut Game, action: Action<Rules>) -> bool {
    let r = Driver::request(g, u64::MAX);
    r.index(&Candidate::Action { action }).is_some()
}

fn continuous(d: &Definition) -> Vec<&[Change]> {
    d.static_abilities
        .iter()
        .filter_map(|a| match &a.effect {
            StaticEffect::Continuous { changes, .. } => Some(changes.as_slice()),
            _ => None,
        })
        .collect()
}

/// The printed sentences lower to one effect each, card types before
/// subtypes (CR 205.1a, 613.6).
#[test]
fn ability_loss_and_type_replacement_lower_exactly() {
    let kenrith = lowered(KENRITHS_TRANSFORMATION);
    assert_eq!(
        continuous(&kenrith),
        [&[
            Change::RemoveAllAbilities {
                except_own_grants: false
            },
            Change::SetCardTypes {
                kinds: vec![CardKind::Creature]
            },
            Change::SetCreatureSubtypes {
                subtypes: vec!["Elk".into()]
            },
            Change::SetColors {
                colors: vec![Color::G]
            },
            Change::SetPowerToughness {
                power: mtg_contracts::kir::Amount::fixed(3),
                toughness: mtg_contracts::kir::Amount::fixed(3)
            },
        ][..]]
    );
    let mutation = lowered(DARKSTEEL_MUTATION);
    assert!(
        continuous(&mutation)[0].contains(&Change::RemoveAllAbilities {
            except_own_grants: true
        })
    );
    assert!(continuous(&mutation)[0].contains(&Change::SetCardTypes {
        kinds: vec![CardKind::Creature, CardKind::Artifact]
    }));
    assert!(
        continuous(&lowered(COLOSSUS_HAMMER))[0].contains(&Change::RemoveKeywords {
            keywords: vec![KeywordAbility::Flying]
        })
    );
    let moon = lowered(BLOOD_MOON);
    assert!(matches!(&moon.static_abilities[0].effect,
        StaticEffect::Continuous { affects: Scope::Matching { .. }, changes, condition: None }
            if *changes == [Change::SetLandSubtypes { subtypes: vec!["Mountain".into()] }]));
    let heliod = lowered(HELIOD);
    assert!(heliod.static_abilities.iter().any(|a| matches!(&a.effect,
        StaticEffect::Continuous { affects: Scope::Source, changes, condition: Some(_) }
            if *changes == [Change::RemoveCardTypes { kinds: vec![CardKind::Creature] }])));
    let archetype = lowered(ARCHETYPE_OF_IMAGINATION);
    assert!(continuous(&archetype)[1].contains(&Change::ForbidKeywords {
        keywords: vec![KeywordAbility::Flying]
    }));
    // Adding a color, a static animation that sets creature subtypes,
    // removing every land type and resolving loss stay explicit gaps.
    for (raw, reason) in [
        (DEEP_FREEZE, "static:modifier:color-operation"),
        (RUSTED_RELIC, "static:modifier:type-operation"),
        (LITHOFORM_BLIGHT, "static:modifier:type-operands"),
        (TURN_TO_FROG, "lose-abilities"),
    ] {
        let result = format!("{:?}", mtg_witness::compile(mtg_witness::R5_MANIFEST, raw));
        assert!(
            result.contains("Unsupported") && result.contains(reason),
            "{result}"
        );
    }
}

/// CR 613.7: a grant after "loses all abilities" survives it; a grant
/// before it does not. The printed flying is earlier than either, and the
/// Aura's own type, color and base P/T changes persist (CR 613.6).
#[test]
fn later_grant_beats_earlier_loss() {
    // Kenrith's Transformation, then Lightfoot Technique.
    let mut g = game(&[
        ("Keiga, the Tide Star", 0, B),
        ("Kenrith's Transformation", 0, H),
        ("Lightfoot Technique", 0, H),
        ("Forest", 0, B),
        ("Forest", 0, B),
        ("Plains", 0, B),
        ("Plains", 0, B),
    ]);
    let keiga = object(&g, 0);
    assert!(keywords(&g, keiga).contains(&KeywordAbility::Flying));
    cast_on(&mut g, 1, &[keiga], &[3, 4]);
    let c = g.characteristics(keiga).unwrap();
    assert_eq!(c.kind, CardKind::Creature);
    assert!(c.additional_types.is_empty());
    assert_eq!(c.subtypes, vec!["Elk".to_string()]);
    assert_eq!(c.colors, vec![Color::G]);
    assert!(c.legendary, "CR 205.4b: supertypes stay");
    assert_eq!(pt(&g, keiga), (q(3), q(3)));
    assert!(keywords(&g, keiga).is_empty());
    cast_on(&mut g, 2, &[keiga], &[5, 6]);
    let now = keywords(&g, keiga);
    assert!(now.contains(&KeywordAbility::Flying) && now.contains(&KeywordAbility::Indestructible));
    assert_eq!(pt(&g, keiga), (q(4), q(4)));

    // Lightfoot Technique, then Kenrith's Transformation.
    let mut g = game(&[
        ("Grizzly Bears", 0, B),
        ("Kenrith's Transformation", 0, H),
        ("Lightfoot Technique", 0, H),
        ("Forest", 0, B),
        ("Forest", 0, B),
        ("Plains", 0, B),
        ("Plains", 0, B),
    ]);
    let bears = object(&g, 0);
    cast_on(&mut g, 2, &[bears], &[5, 6]);
    assert!(keywords(&g, bears).contains(&KeywordAbility::Flying));
    cast_on(&mut g, 1, &[bears], &[3, 4]);
    assert!(keywords(&g, bears).is_empty());
    // The counter still applies in layer 7c over the Elk's base 3/3.
    assert_eq!(pt(&g, bears), (q(4), q(4)));
}

/// CR 613.1f, 613.8a: a lord that loses its abilities stops granting its
/// anthem and keyword; its triggered and activated abilities stop too.
#[test]
fn lord_loses_its_anthem_and_abilities_stop() {
    let mut g = game(&[
        ("Lord of Atlantis", 0, B),
        ("Kiora's Follower", 0, B),
        ("Mystic Subdual", 0, H),
        ("Island", 0, B),
        ("Island", 0, B),
    ]);
    let (lord, follower) = (object(&g, 0), object(&g, 1));
    assert_eq!(pt(&g, follower), (q(3), q(3)));
    assert!(keywords(&g, follower).contains(&KeywordAbility::Islandwalk));
    cast_on(&mut g, 2, &[lord], &[3, 4]);
    assert_eq!(pt(&g, follower), (q(2), q(2)));
    assert!(keywords(&g, follower).is_empty());
    assert_eq!(pt(&g, lord), (q(0), q(2)));
    // The follower keeps its own activated ability.
    assert!(can(
        &mut g,
        Action::ActivateAbility {
            object: follower,
            ability: 0
        }
    ));

    let mut g = game(&[
        ("Kiora's Follower", 0, B),
        ("Llanowar Elves", 0, B),
        ("Kenrith's Transformation", 0, H),
        ("Kenrith's Transformation", 0, H),
        ("Forest", 0, B),
        ("Forest", 0, B),
        ("Forest", 0, B),
        ("Forest", 0, B),
    ]);
    let (follower, elves) = (object(&g, 0), object(&g, 1));
    assert_eq!(g.mana_abilities(elves).unwrap(), vec![Color::G]);
    cast_on(&mut g, 2, &[follower], &[4, 5]);
    cast_on(&mut g, 3, &[elves], &[6, 7]);
    assert!(!can(
        &mut g,
        Action::ActivateAbility {
            object: follower,
            ability: 0
        }
    ));
    assert!(!can(&mut g, Action::ActivateMana { object: elves }));
    assert!(g.mana_abilities(elves).unwrap().is_empty());
}

/// CR 603.2, 113.10: prowess triggers on the Aura's own cast, then the
/// creature has no triggered ability left to trigger.
#[test]
fn lost_triggered_abilities_do_not_trigger() {
    let mut g = game(&[
        ("Niblis of Dusk", 0, B),
        ("Grizzly Bears", 0, B),
        ("Mystic Subdual", 0, H),
        ("Lightfoot Technique", 0, H),
        ("Island", 0, B),
        ("Island", 0, B),
        ("Plains", 0, B),
        ("Plains", 0, B),
    ]);
    let (niblis, bears) = (object(&g, 0), object(&g, 1));
    // Prowess (+1/+1) and Mystic Subdual (-2/-0) both apply this turn.
    cast_on(&mut g, 2, &[niblis], &[4, 5]);
    assert!(keywords(&g, niblis).is_empty());
    assert_eq!(pt(&g, niblis), (q(1), q(2)));
    Driver::cast(&mut g, CardId(3));
    let r = Driver::request(&mut g, u64::MAX);
    g.respond(
        r.player,
        r.id,
        r.entity_targets(vec![TargetEntity::Object { object: bears }]),
    )
    .unwrap();
    Driver::pay(&mut g, &[6, 7]);
    assert!(g.state.rules.pending_triggers.is_empty());
    assert_eq!(g.state.rules.stack.len(), 1);
    settle(&mut g, &[]);
    assert_eq!(pt(&g, niblis), (q(1), q(2)));
}

/// CR 604.3a, 613.4a, 208.2a: a removed characteristic-defining ability no
/// longer defines the stat, so Tarmogoyf keeps its printed 0/1.
#[test]
fn removed_cda_leaves_printed_stats() {
    let mut g = game(&[
        ("Tarmogoyf", 0, B),
        ("Mystic Subdual", 0, H),
        ("Island", 0, B),
        ("Island", 0, B),
        ("Grizzly Bears", 0, G),
        ("Forest", 0, G),
        ("Lightfoot Technique", 0, G),
    ]);
    let goyf = object(&g, 0);
    assert_eq!(pt(&g, goyf), (q(3), q(4)));
    cast_on(&mut g, 1, &[goyf], &[2, 3]);
    // Mystic Subdual itself adds nothing to the graveyard.
    assert_eq!(pt(&g, goyf), (q(-2), q(1)));
}

/// CR 305.7: Blood Moon makes nonbasic lands Mountains: they lose their
/// rules-text mana abilities and tap for {R}; a basic land is unchanged.
/// CR 614.12: an entering nonbasic land lacks its own entry replacements.
#[test]
fn blood_moon_replaces_land_types_and_mana() {
    let mut g = game(&[
        ("Command Tower", 0, B),
        ("Breeding Pool", 0, B),
        ("Forest", 0, B),
        ("Mountain", 0, B),
        ("Mountain", 0, B),
        ("Blood Moon", 0, H),
        ("Crumbling Necropolis", 0, H),
    ]);
    let (tower, pool, forest) = (object(&g, 0), object(&g, 1), object(&g, 2));
    assert_eq!(g.mana_abilities(pool).unwrap(), vec![Color::G, Color::U]);
    cast_on(&mut g, 5, &[], &[2, 3, 4]);
    for land in [tower, pool] {
        let c = g.characteristics(land).unwrap();
        assert_eq!(c.subtypes, vec!["Mountain".to_string()]);
        assert_eq!(g.mana_abilities(land).unwrap(), vec![Color::R]);
    }
    assert_eq!(g.mana_abilities(forest).unwrap(), vec![Color::G]);
    let necropolis = object(&g, 6);
    Driver::action(&mut g, Action::PlayLand { object: necropolis });
    let r = Driver::request(&mut g, u64::MAX);
    assert_eq!(r.reason, Reason::Priority, "no entry choice is offered");
    let necropolis = g.object_id(CardId(6)).unwrap();
    assert_eq!(g.current(necropolis).unwrap().zone, Zone::Battlefield);
    assert!(!g.current(necropolis).unwrap().tapped);
    assert_eq!(g.mana_abilities(necropolis).unwrap(), vec![Color::R]);
    settle(&mut g, &[]);
}

/// CR 613.8a: Song of the Dryads makes a creature a land, which changes
/// what Blood Moon applies to, so Blood Moon waits for it and the
/// permanent is a Mountain despite the later Aura. On a nonbasic land
/// the effects are independent and the later timestamp wins.
#[test]
fn blood_moon_depends_on_song_of_the_dryads() {
    let mut g = game(&[
        ("Keiga, the Tide Star", 1, B),
        ("Command Tower", 1, B),
        ("Blood Moon", 0, B),
        ("Song of the Dryads", 0, H),
        ("Song of the Dryads", 0, H),
        ("Forest", 0, B),
        ("Forest", 0, B),
        ("Forest", 0, B),
        ("Forest", 0, B),
        ("Forest", 0, B),
        ("Forest", 0, B),
    ]);
    let (keiga, tower) = (object(&g, 0), object(&g, 1));
    cast_on(&mut g, 3, &[keiga], &[5, 6, 7]);
    let c = g.characteristics(keiga).unwrap();
    assert_eq!(
        (c.kind, c.additional_types.clone()),
        (CardKind::Land, vec![])
    );
    assert_eq!(c.subtypes, vec!["Mountain".to_string()]);
    assert!(c.colors.is_empty() && c.power.is_none());
    assert!(keywords(&g, keiga).is_empty());
    assert_eq!(g.mana_abilities(keiga).unwrap(), vec![Color::R]);
    cast_on(&mut g, 4, &[tower], &[8, 9, 10]);
    assert_eq!(
        g.characteristics(tower).unwrap().subtypes,
        vec!["Forest".to_string()]
    );
    assert_eq!(g.mana_abilities(tower).unwrap(), vec![Color::G]);
}

/// CR 205.1b: "in addition to its other creature types" keeps the Elk;
/// a later replacement removes the earlier addition (CR 613.7).
#[test]
fn additive_and_replacing_creature_types_follow_timestamps() {
    let mut g = game(&[
        ("Grizzly Bears", 0, B),
        ("Hivestone", 0, B),
        ("Kenrith's Transformation", 0, H),
        ("Forest", 0, B),
        ("Forest", 0, B),
    ]);
    let bears = object(&g, 0);
    assert!(g.characteristics(bears).unwrap().has_subtype("Sliver"));
    cast_on(&mut g, 2, &[bears], &[3, 4]);
    assert_eq!(
        g.characteristics(bears).unwrap().subtypes,
        vec!["Elk".to_string()]
    );

    let mut g = game(&[
        ("Grizzly Bears", 0, B),
        ("Kenrith's Transformation", 0, H),
        ("Hivestone", 0, H),
        ("Forest", 0, B),
        ("Forest", 0, B),
        ("Forest", 0, B),
        ("Forest", 0, B),
    ]);
    let bears = object(&g, 0);
    cast_on(&mut g, 1, &[bears], &[3, 4]);
    cast_on(&mut g, 2, &[], &[5, 6]);
    let c = g.characteristics(bears).unwrap();
    assert!(c.has_subtype("Elk") && c.has_subtype("Sliver"));
}

/// CR 613.1f: Humility removes every creature's abilities, lords' grants
/// included, sets base 1/1, and a later resolved grant still applies.
#[test]
fn humility_and_later_grants() {
    let mut g = game(&[
        ("Lord of Atlantis", 0, B),
        ("Kiora's Follower", 0, B),
        ("Keiga, the Tide Star", 0, B),
        ("Tarmogoyf", 0, B),
        ("Humility", 0, H),
        ("Lightfoot Technique", 0, H),
        ("Plains", 0, B),
        ("Plains", 0, B),
        ("Plains", 0, B),
        ("Plains", 0, B),
        ("Plains", 0, B),
        ("Plains", 0, B),
    ]);
    let [lord, follower, keiga, goyf] = [0, 1, 2, 3].map(|i| object(&g, i));
    cast_on(&mut g, 4, &[], &[6, 7, 8, 9]);
    for creature in [lord, follower, keiga, goyf] {
        assert_eq!(pt(&g, creature), (q(1), q(1)));
        assert!(keywords(&g, creature).is_empty());
    }
    cast_on(&mut g, 5, &[keiga], &[10, 11]);
    assert!(keywords(&g, keiga).contains(&KeywordAbility::Flying));
    assert_eq!(pt(&g, keiga), (q(2), q(2)));
}

/// CR 613.1f: "can't have or gain flying" applies whatever the timestamp,
/// and the Archetype's own grant reaches only its controller's creatures.
#[test]
fn archetype_forbids_later_grants() {
    let mut g = game(&[
        ("Archetype of Imagination", 0, B),
        ("Grizzly Bears", 0, B),
        ("Keiga, the Tide Star", 1, B),
        ("Lightfoot Technique", 0, H),
        ("Plains", 0, B),
        ("Plains", 0, B),
    ]);
    let (bears, keiga) = (object(&g, 1), object(&g, 2));
    assert!(keywords(&g, bears).contains(&KeywordAbility::Flying));
    assert!(!keywords(&g, keiga).contains(&KeywordAbility::Flying));
    cast_on(&mut g, 3, &[keiga], &[4, 5]);
    let now = keywords(&g, keiga);
    assert!(now.contains(&KeywordAbility::Indestructible));
    assert!(!now.contains(&KeywordAbility::Flying));
}

/// CR 700.5, 205.1a: below five devotion Heliod is no creature (and so no
/// God), while its other static still grants vigilance.
#[test]
fn heliod_is_a_creature_only_with_devotion() {
    let g = game(&[("Heliod, God of the Sun", 0, B), ("Daru Warchief", 0, B)]);
    let (heliod, angel) = (object(&g, 0), object(&g, 1));
    let c = g.characteristics(heliod).unwrap();
    assert_eq!(
        (c.kind, c.additional_types.clone()),
        (CardKind::Enchantment, vec![])
    );
    assert!(c.legendary && c.subtypes.is_empty() && c.power.is_none());
    assert!(keywords(&g, angel).contains(&KeywordAbility::Vigilance));
    checked(&g);
    let g = game(&[
        ("Heliod, God of the Sun", 0, B),
        ("Daru Warchief", 0, B),
        ("Blazing Archon", 0, B),
    ]);
    let heliod = object(&g, 0);
    let c = g.characteristics(heliod).unwrap();
    assert!(c.has_type(CardKind::Creature) && c.has_subtype("God"));
    assert_eq!(pt(&g, heliod), (q(5), q(6)));
    checked(&g);
}

/// CR 305.6, 613.1f: Toxicrene's lands lose their intrinsic mana abilities
/// and keep the one it grants.
#[test]
fn toxicrene_replaces_land_mana() {
    let g = game(&[
        ("Toxicrene", 0, B),
        ("Forest", 0, B),
        ("Command Tower", 0, B),
    ]);
    for land in [object(&g, 1), object(&g, 2)] {
        assert_eq!(
            g.mana_abilities(land).unwrap(),
            vec![Color::W, Color::U, Color::B, Color::R, Color::G]
        );
    }
    checked(&g);
}

/// Darksteel Mutation keeps the indestructible it grants and removes the
/// rest; the object is an Insect artifact creature 0/1 (CR 205.1a).
#[test]
fn darksteel_mutation_keeps_its_own_grant() {
    let mut g = game(&[
        ("Keiga, the Tide Star", 1, B),
        ("Darksteel Mutation", 0, H),
        ("Plains", 0, B),
        ("Plains", 0, B),
    ]);
    let keiga = object(&g, 0);
    cast_on(&mut g, 1, &[keiga], &[2, 3]);
    let c = g.characteristics(keiga).unwrap();
    assert!(c.has_type(CardKind::Artifact) && c.has_type(CardKind::Creature));
    assert_eq!(c.subtypes, vec!["Insect".to_string()]);
    assert_eq!(pt(&g, keiga), (q(0), q(1)));
    assert_eq!(keywords(&g, keiga), vec![KeywordAbility::Indestructible]);
}

/// A snapshot keeps each grant's place in the timestamp order (CR 613.7b).
fn restored(g: &Game) -> Game {
    Game::restore(g.prepared.as_ref().clone(), &g.snapshot().unwrap()).unwrap()
}

#[test]
fn keyword_grant_order_survives_restore() {
    // Loss, then a later grant: the restored game still has flying.
    let mut g = game(&[
        ("Grizzly Bears", 0, B),
        ("Kenrith's Transformation", 0, H),
        ("Lightfoot Technique", 0, H),
        ("Forest", 0, B),
        ("Forest", 0, B),
        ("Plains", 0, B),
        ("Plains", 0, B),
    ]);
    let bears = object(&g, 0);
    cast_on(&mut g, 1, &[bears], &[3, 4]);
    cast_on(&mut g, 2, &[bears], &[5, 6]);
    let r = restored(&g);
    assert!(keywords(&r, bears).contains(&KeywordAbility::Flying));
    assert_eq!(keywords(&r, bears), keywords(&g, bears));
    // An active grant saved before the loss is still removed by it.
    let mut g = game(&[
        ("Grizzly Bears", 0, B),
        ("Kenrith's Transformation", 0, H),
        ("Lightfoot Technique", 0, H),
        ("Forest", 0, B),
        ("Forest", 0, B),
        ("Plains", 0, B),
        ("Plains", 0, B),
    ]);
    let bears = object(&g, 0);
    cast_on(&mut g, 2, &[bears], &[5, 6]);
    let mut r = restored(&g);
    assert!(keywords(&r, bears).contains(&KeywordAbility::Flying));
    cast_on(&mut r, 1, &[bears], &[3, 4]);
    assert!(keywords(&r, bears).is_empty());
}

/// CR 613.8a-c: dependency is decided across the battlefield. The first
/// Song applies before the second by timestamp; Blood Moon depends on the
/// second (it makes a creature a land), so it applies last and both
/// enchanted permanents are Mountains.
#[test]
fn blood_moon_waits_for_every_song() {
    let mut g = game(&[
        ("Blood Moon", 0, B),
        ("Command Tower", 1, B),
        ("Grizzly Bears", 1, B),
        ("Song of the Dryads", 0, H),
        ("Song of the Dryads", 0, H),
        ("Forest", 0, B),
        ("Forest", 0, B),
        ("Forest", 0, B),
        ("Forest", 0, B),
        ("Forest", 0, B),
        ("Forest", 0, B),
    ]);
    let (tower, bears) = (object(&g, 1), object(&g, 2));
    cast_on(&mut g, 3, &[tower], &[5, 6, 7]);
    assert_eq!(g.mana_abilities(tower).unwrap(), vec![Color::G]);
    cast_on(&mut g, 4, &[bears], &[8, 9, 10]);
    for permanent in [tower, bears] {
        let c = g.characteristics(permanent).unwrap();
        assert_eq!(c.subtypes, vec!["Mountain".to_string()]);
        assert_eq!(g.mana_abilities(permanent).unwrap(), vec![Color::R]);
    }
}

/// CR 205.1a, 704.5p: an Equipment that becomes a land is no longer an
/// Equipment and becomes unattached; it grants nothing again until it is
/// re-equipped after the Song leaves.
#[test]
fn former_equipment_becomes_unattached() {
    let mut cards = vec![
        ("Colossus Hammer", 0, B),
        ("Grizzly Bears", 0, B),
        ("Song of the Dryads", 0, H),
        ("Regress", 0, H),
    ];
    cards.extend([("Forest", 0, B); 11]);
    cards.extend([("Island", 0, B); 3]);
    let mut g = game(&cards);
    let (hammer, bears) = (object(&g, 0), object(&g, 1));
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: hammer,
            ability: 0,
        },
    );
    let r = Driver::request(&mut g, u64::MAX);
    g.respond(r.player, r.id, r.targets(vec![bears])).unwrap();
    Driver::pay(&mut g, &[4, 5, 6, 7, 8, 9, 10, 11]);
    settle(&mut g, &[]);
    assert_eq!(g.current(hammer).unwrap().attached_to, Some(bears));
    assert_eq!(pt(&g, bears), (q(12), q(12)));
    cast_on(&mut g, 2, &[hammer], &[12, 13, 14]);
    let c = g.characteristics(hammer).unwrap();
    assert_eq!(
        (c.kind, c.subtypes.clone()),
        (CardKind::Land, vec!["Forest".to_string()])
    );
    assert_eq!(g.current(hammer).unwrap().attached_to, None);
    assert_eq!(pt(&g, bears), (q(2), q(2)));
    let song = object(&g, 2);
    cast_on(&mut g, 3, &[song], &[15, 16, 17]);
    assert!(g.characteristics(hammer).unwrap().has_subtype("Equipment"));
    assert_eq!(g.current(hammer).unwrap().attached_to, None);
    assert_eq!(pt(&g, bears), (q(2), q(2)));
}

/// CR 613.7m: simultaneous timestamps go in APNAP order. Setup permanents
/// share one timestamp; True Conviction's source is untouched by Humility,
/// so only the order decides whether its grant survives.
#[test]
fn simultaneous_timestamps_follow_apnap_or_reject() {
    // The active player's True Conviction is earlier; Humility removes it.
    let g = game(&[
        ("True Conviction", 0, B),
        ("Grizzly Bears", 0, B),
        ("Humility", 1, B),
    ]);
    assert!(keywords(&g, object(&g, 1)).is_empty());
    checked(&g);
    // The active player's Humility is earlier; the later grant survives.
    let g = game(&[
        ("Humility", 0, B),
        ("True Conviction", 1, B),
        ("Grizzly Bears", 1, B),
    ]);
    let bears = object(&g, 2);
    let now = keywords(&g, bears);
    assert!(now.contains(&KeywordAbility::DoubleStrike) && now.contains(&KeywordAbility::Lifelink));
    assert_eq!(pt(&g, bears), (q(1), q(1)));
    checked(&g);
    // One controller's simultaneous objects need that player's order, which
    // no decision asks: the next stabilization rejects the state.
    let mut g = game(&[
        ("Humility", 0, B),
        ("True Conviction", 0, B),
        ("Grizzly Bears", 0, B),
        ("Lightfoot Technique", 0, H),
        ("Plains", 0, B),
        ("Plains", 0, B),
    ]);
    let bears = object(&g, 2);
    Driver::cast(&mut g, CardId(3));
    let r = Driver::request(&mut g, u64::MAX);
    g.respond(
        r.player,
        r.id,
        r.entity_targets(vec![TargetEntity::Object { object: bears }]),
    )
    .unwrap();
    Driver::pay(&mut g, &[4, 5]);
    let mut result = Driver::settle(&mut g, u64::MAX);
    for _ in 0..8 {
        let Advance::AwaitingDecision(r) = &result else {
            break;
        };
        let answer = mtg_witness::temporary::pass(r);
        g.respond(r.player, r.id, answer).unwrap();
        result = Driver::settle(&mut g, u64::MAX);
    }
    assert!(
        matches!(&result, Advance::Fault(reason) if reason.contains("613.7m")),
        "{result:?}"
    );
}

/// Pass priority (declaring no attackers) until `player` is active.
fn until_active(g: &mut Game, player: PlayerId) {
    for _ in 0..200 {
        let r = Driver::request(g, u64::MAX);
        if g.state.rules.active == Some(player) && r.reason == Reason::Priority {
            return checked(g);
        }
        let answer = match r.reason {
            Reason::Priority => mtg_witness::temporary::pass(&r),
            Reason::Attackers => r.attack(vec![]),
            ref reason => panic!("unexpected turn boundary {reason:?}"),
        };
        g.respond(r.player, r.id, answer).unwrap();
    }
    panic!("turn did not pass");
}

/// CR 613.7m: the APNAP order of simultaneous timestamps is fixed when they
/// are assigned, so it does not change when another player becomes active.
#[test]
fn simultaneous_timestamp_order_survives_the_turn() {
    let mut g = game(&[
        ("Humility", 0, B),
        ("True Conviction", 1, B),
        ("Grizzly Bears", 1, B),
    ]);
    let bears = object(&g, 2);
    let granted = |g: &Game| {
        let now = keywords(g, bears);
        now.contains(&KeywordAbility::DoubleStrike) && now.contains(&KeywordAbility::Lifelink)
    };
    assert!(granted(&g));
    until_active(&mut g, PlayerId(1));
    assert!(granted(&g));
    assert!(granted(&restored(&g)));
}

/// Attach the setup Aura `aura` to `host` as if it had entered attached
/// with every other setup permanent (one shared timestamp).
fn forge_attached(g: &mut Game, aura: ObjectId, host: ObjectId) {
    let rules = &mut g.state.rules;
    let o = rules.objects.get_mut(&aura.card).unwrap();
    o.attached_to = Some(host);
    o.attachment_timestamp = Some(1);
}

/// CR 613.4b, 613.7m: simultaneous base power and toughness setters of
/// different controllers apply in APNAP order; one controller's conflict
/// needs that player's order and rejects.
#[test]
fn simultaneous_base_pt_setters_follow_apnap() {
    for (humility, aura, expected) in [(0, 1, 3), (1, 0, 1)] {
        let mut g = unstarted(&[
            ("Humility", humility, B),
            ("Kenrith's Transformation", aura, B),
            ("Grizzly Bears", aura, B),
        ]);
        let (kenrith, bears) = (object(&g, 1), object(&g, 2));
        forge_attached(&mut g, kenrith, bears);
        Driver::request(&mut g, u64::MAX);
        assert_eq!(
            pt(&g, bears),
            (q(expected), q(expected)),
            "Humility seat {humility}"
        );
        assert!(keywords(&g, bears).is_empty());
        // A forged board has no replay journal to restore from.
        g.validate_state().unwrap();
    }
    let mut g = unstarted(&[
        ("Humility", 0, B),
        ("Kenrith's Transformation", 0, B),
        ("Grizzly Bears", 0, B),
    ]);
    let (kenrith, bears) = (object(&g, 1), object(&g, 2));
    forge_attached(&mut g, kenrith, bears);
    let err = g.characteristics(bears).unwrap_err();
    assert!(err.contains("simultaneous"), "{err}");
}

/// CR 613.7m: one simultaneous battlefield move orders interacting layer-6
/// effects by APNAP, even when the nonactive player's event is applied first.
#[test]
fn pre_e2_entering_statics_share_time_and_use_apnap() {
    for (humility, conviction, granted) in [(0, 1, true), (1, 0, false)] {
        let initial = unstarted(&[
            ("Humility", humility, G),
            ("True Conviction", conviction, G),
            ("Grizzly Bears", conviction, B),
        ]);
        let mut expected = None;
        for mode in [
            TraceMode::Off,
            TraceMode::Compact,
            TraceMode::Stream,
            TraceMode::Detailed,
        ] {
            for quantum in [1, u64::MAX] {
                let mut g = Game::with_trace(initial.prepared.as_ref().clone(), mode);
                Driver::request(&mut g, quantum);
                assert_recoverable((&g).into());
                let mut ids = [0, 1];
                ids.sort_by_key(|id| if *id == 0 { humility } else { conviction });
                ids.reverse();
                let occurrences = ids.map(|id| ProposedEvent::Move {
                    object: object(&g, id),
                    destination: Zone::Battlefield,
                });
                g.commit_batch(
                    BatchCause::Fixture,
                    EventGroup::Simultaneous {
                        occurrences: occurrences.into(),
                    },
                )
                .unwrap();
                let hold = |g: &Game| {
                    let rows = [0, 1].map(|id| g.object_before(object(g, id)).unwrap());
                    assert_eq!(rows[0].keyword_timestamp, rows[1].keyword_timestamp);
                    assert_eq!(
                        [rows[0].keyword_rank, rows[1].keyword_rank],
                        [humility, conviction]
                    );
                    let bears = object(g, 2);
                    assert_eq!(
                        keywords(g, bears).contains(&KeywordAbility::DoubleStrike),
                        granted
                    );
                };
                hold(&g);
                let (restored, _) =
                    Game::read_snapshot(initial.prepared.as_ref().clone(), &g.snapshot().unwrap())
                        .unwrap();
                hold(&restored.fork());
                let digest = g.state_digest().unwrap();
                assert_eq!(expected.get_or_insert_with(|| digest.clone()), &digest);
            }
        }
    }
}

/// CR 614.12 and 700.5: the same layer evaluator sees Heliod's prospective
/// type-changing static and the prior devotion. Actual entry adds Heliod's pip.
#[test]
fn pre_e2_prospective_heliod_reads_prior_devotion_without_disclosing_his_hand() {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    catalog
        .definitions
        .extend([lowered(HELIOD), lowered(HUMILITY)]);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    let prepared = Board::new(catalog, "prospective-devotion-privacy")
        .seats(4)
        .step(TurnStep::PrecombatMain)
        .card("Heliod, God of the Sun", 0, 0, H)
        .card("Humility", 0, 0, B)
        .card("Humility", 0, 0, B)
        .card("Plains", 0, 0, B)
        .card("Plains", 0, 0, B)
        .card("Plains", 0, 0, B)
        .card("Plains", 0, 0, B)
        .libraries("Forest", 6)
        .prepare();
    let mut g = Game::new(prepared);
    Driver::request(&mut g, u64::MAX);
    let heliod = object(&g, 0);
    let digest = g.state_digest().unwrap();
    assert!(
        g.characteristics(heliod)
            .unwrap()
            .has_type(CardKind::Creature)
    );
    let prospect = g.prospective_characteristics(heliod, PlayerId(0)).unwrap();
    assert!(prospect.has_type(CardKind::Enchantment));
    assert!(!prospect.has_type(CardKind::Creature));
    assert_eq!(g.state_digest().unwrap(), digest);
    for seat in 0..4 {
        let QueryResult::Complete(QueryValue::Observation(view)) =
            g.observe(PlayerId(seat), Budget::unlimited())
        else {
            panic!("missing seat observation");
        };
        assert_eq!(
            view.view
                .objects
                .iter()
                .any(|row| row.oracle_id == lowered(HELIOD).oracle_id),
            seat == 0,
        );
    }
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[3, 4, 5, 6]);
    settle(&mut g, &[]);
    let heliod = object(&g, 0);
    assert!(
        g.characteristics(heliod)
            .unwrap()
            .has_type(CardKind::Creature)
    );
    assert_eq!(pt(&g, heliod), (q(1), q(1)));
    assert_recoverable((&g).into());
}

/// CR 303.4f, 614.12: both Auras choose their host from the prior board.
/// Song changes its type only once both have entered, then CR 704.5m
/// removes Mystic Subdual, which can no longer enchant it.
#[test]
fn pre_e2_simultaneous_auras_read_the_common_attachment_host() {
    let mut g = game(&[
        ("Song of the Dryads", 0, G),
        ("Mystic Subdual", 1, G),
        ("Grizzly Bears", 1, B),
    ]);
    assert_recoverable((&g).into());
    let host = object(&g, 2);
    let occurrences = [0, 1].map(|id| ProposedEvent::EnterAttached {
        object: object(&g, id),
        target: host,
    });
    g.commit_batch(
        BatchCause::Fixture,
        EventGroup::Simultaneous {
            occurrences: occurrences.into(),
        },
    )
    .unwrap();
    let rows = [0, 1].map(|id| g.current(object(&g, id)).unwrap());
    assert!(rows.iter().all(|row| row.attached_to == Some(host)));
    assert_eq!(rows[0].attachment_timestamp, rows[1].attachment_timestamp);
    assert!(g.characteristics(host).unwrap().has_subtype("Forest"));
    g.stabilize().unwrap();
    Driver::request(&mut g, u64::MAX);
    assert_eq!(
        g.zone(object(&g, 1)),
        Some(Zone::Graveyard { owner: PlayerId(1) })
    );
}
