//! Printed triggered keywords lowered to their defining triggers, from exact
//! frozen R5 source on private boards: CR 702.108 (prowess), 702.45
//! (bushido), 702.135 (afterlife) and 702.46 (soulshift).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::kir::Atom;

const THOR_ODINSON: &str = record!("ec9b74d0-e4fe-4764-b628-992a62f2a1cf");
const JADE_AVENGER: &str = record!("177248b7-f3fd-42f8-b325-2625a2bb28ff");
const DEVOTED_RETAINER: &str = record!("54970935-b3d3-4f65-a262-c4975f2787e5");
const MINISTRANT_OF_OBLIGATION: &str = record!("0cee5539-d10b-4de2-aee8-e02f0e24ac26");
const HUNDRED_TALON_KAMI: &str = record!("100b4440-c588-423e-93d2-23c9b595614d");
const NIBLIS_OF_DUSK: &str = record!("d14d41a5-6f38-43c7-b95c-b4a723002c20");
const SCUTTLING_DEATH: &str = record!("f9927216-edf2-4ef7-90eb-447b9e08f7b3");

fn power_toughness(g: &Game, object: ObjectId) -> (Option<Quantity>, Option<Quantity>) {
    let c = g.characteristics(object).unwrap();
    (c.power, c.toughness)
}

fn pt(power: i64, toughness: i64) -> (Option<Quantity>, Option<Quantity>) {
    (Some(power.into()), Some(toughness.into()))
}

/// Cast Lightning Bolt (`bolt`) at the opponent, paying with `mountain`.
fn bolt_opponent(g: &mut Game, bolt: u32, mountain: u32) {
    Driver::cast(g, CardId(bolt));
    let d = Driver::request(g, u64::MAX);
    let answer = d
        .pick_one(&Candidate::Player {
            player: PlayerId(1),
        })
        .unwrap();
    g.respond(d.player, d.id, answer).unwrap();
    Driver::pay(g, &[mountain]);
}

/// Thor Odinson prints "prowess, prowess": each instance triggers
/// separately on a noncreature spell (CR 702.108b), so Lightning Bolt puts
/// two triggers on the stack and Thor gets +2/+2. Casting a creature spell
/// triggers neither.
#[test]
fn prowess_instances_trigger_separately_on_noncreature_spells_only() {
    let mut g = Game::with_trace(
        prepare(
            board(&[THOR_ODINSON], TurnStep::PrecombatMain)
                .permanent_or_hand("Thor Odinson", 0, 0, false)
                .permanent_or_hand("Lightning Bolt", 0, 0, true)
                .permanent_or_hand("Grizzly Bears", 0, 0, true)
                .permanent_or_hand("Mountain", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false)
                .permanent_or_hand("Forest", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let thor = Driver::object(&g, 0);
    let definition = g.definition(CardId(0)).unwrap();
    assert_eq!(definition.triggered.len(), 2);
    assert!(definition.triggered.iter().all(|t| matches!(
        t.condition.pattern(),
        EventPattern::Cast { spell, .. } if spell.0 == [Atom::NotType { kind: CardKind::Creature }]
    )));
    bolt_opponent(&mut g, 1, 3);
    drive(&mut g, &[], |g| g.state.rules.stack.len() == 3);
    assert!(
        g.state.rules.stack[1..]
            .iter()
            .all(|s| s.source == thor && matches!(s.kind, StackKind::Triggered { .. }))
    );
    checked(&g);
    settle(&mut g);
    assert_eq!(power_toughness(&g, thor), pt(6, 6));
    Driver::cast(&mut g, CardId(2));
    Driver::pay(&mut g, &[4, 5]);
    let d = Driver::request(&mut g, u64::MAX);
    assert_eq!(d.reason, Reason::Priority);
    assert_eq!(
        g.state.rules.stack.len(),
        1,
        "a creature spell triggers no prowess"
    );
    settle(&mut g);
    assert_eq!(power_toughness(&g, thor), pt(6, 6));
    assert_recoverable((&g).into());
}

/// Jade Avenger ("Bushido 2") attacks. Blocked by Grizzly Bears, it becomes
/// blocked and gets +2/+2 before damage, killing the Bears and surviving;
/// unblocked, nothing triggers (CR 702.45a, 509.3c).
#[test]
fn bushido_pumps_when_the_creature_becomes_blocked() {
    for blocked in [true, false] {
        let mut g = Game::with_trace(
            prepare(
                board(&[JADE_AVENGER], TurnStep::BeginningCombat)
                    .permanent_or_hand("Jade Avenger", 0, 0, false)
                    .permanent_or_hand("Grizzly Bears", 1, 1, false),
            ),
            TraceMode::Detailed,
        );
        let (avenger, bears) = (Driver::object(&g, 0), Driver::object(&g, 1));
        for _ in 0..64 {
            let d = Driver::request(&mut g, u64::MAX);
            if g.state.rules.turn.step == TurnStep::DeclareBlockers
                && d.reason == Reason::Priority
                && g.state.rules.stack.is_empty()
            {
                break;
            }
            let answer = match &d.reason {
                Reason::Priority => mtg_witness::temporary::pass(&d),
                Reason::Attackers => d.attack(vec![(avenger, PlayerId(1))]),
                Reason::Blockers => d.block(if blocked {
                    vec![(bears, avenger)]
                } else {
                    vec![]
                }),
                other => panic!("unexpected decision {other:?}"),
            };
            g.respond(d.player, d.id, answer).unwrap();
        }
        assert_eq!(g.state.rules.turn.step, TurnStep::DeclareBlockers);
        let expected = if blocked { pt(4, 4) } else { pt(2, 2) };
        assert_eq!(power_toughness(&g, avenger), expected);
        checked(&g);
        drive(&mut g, &[], |g| {
            g.state.rules.turn.step == TurnStep::EndCombat
        });
        assert_eq!(g.current(avenger).unwrap().zone, Zone::Battlefield);
        assert_eq!(g.current(bears).is_err(), blocked);
        assert_recoverable((&g).into());
    }
}

/// Devoted Retainer ("Bushido 1") blocks the opponent's attacking Grizzly
/// Bears on the opponent's turn: the blocks branch gives it +1/+1
/// (CR 702.45a, 509.3a).
#[test]
fn bushido_pumps_when_the_creature_blocks() {
    let mut g = Game::with_trace(
        prepare(
            board(&[DEVOTED_RETAINER], TurnStep::End)
                .permanent_or_hand("Devoted Retainer", 0, 0, false)
                .permanent_or_hand("Grizzly Bears", 1, 1, false),
        ),
        TraceMode::Detailed,
    );
    let (retainer, bears) = (Driver::object(&g, 0), Driver::object(&g, 1));
    let mut blocked = false;
    for _ in 0..256 {
        let d = Driver::request(&mut g, u64::MAX);
        if blocked && d.reason == Reason::Priority && g.state.rules.stack.is_empty() {
            break;
        }
        let answer = match &d.reason {
            Reason::Priority => mtg_witness::temporary::pass(&d),
            Reason::Attackers => {
                assert_eq!(d.player, PlayerId(1));
                d.attack(vec![(bears, PlayerId(0))])
            }
            Reason::Blockers => {
                blocked = true;
                d.block(vec![(retainer, bears)])
            }
            other => panic!("unexpected decision {other:?}"),
        };
        g.respond(d.player, d.id, answer).unwrap();
    }
    assert!(blocked);
    assert_eq!(g.state.rules.turn.number, 2, "the opponent's turn");
    assert_eq!(g.state.rules.turn.step, TurnStep::DeclareBlockers);
    assert_eq!(power_toughness(&g, retainer), pt(2, 2));
    checked(&g);
    assert_recoverable((&g).into());
}

/// Ministrant of Obligation ("Afterlife 2") dies to Lightning Bolt; its
/// controller creates two 1/1 white and black Spirit tokens with flying
/// (CR 702.135a).
#[test]
fn afterlife_creates_flying_spirits_when_the_permanent_dies() {
    let mut g = Game::with_trace(
        prepare(
            board(&[MINISTRANT_OF_OBLIGATION], TurnStep::PrecombatMain)
                .permanent_or_hand("Ministrant of Obligation", 0, 0, false)
                .permanent_or_hand("Lightning Bolt", 1, 1, true)
                .permanent_or_hand("Mountain", 1, 1, false),
        ),
        TraceMode::Detailed,
    );
    let ministrant = Driver::object(&g, 0);
    Driver::pass(&mut g);
    Driver::cast(&mut g, CardId(1));
    Driver::pick_object(&mut g, ministrant);
    Driver::pay(&mut g, &[2]);
    settle(&mut g);
    assert!(g.current(ministrant).is_err());
    let spirits: Vec<_> = g
        .state
        .rules
        .objects
        .values()
        .filter(|o| o.zone == Zone::Battlefield && g.state.rules.tokens.contains_key(&o.id.card))
        .cloned()
        .collect();
    assert_eq!(spirits.len(), 2);
    for spirit in &spirits {
        assert_eq!(spirit.controller, PlayerId(0));
        let c = g.characteristics(spirit.id).unwrap();
        assert_eq!((c.power, c.toughness), pt(1, 1));
        assert_eq!(c.colors, vec![Color::W, Color::B]);
        assert_eq!(c.subtypes, vec!["Spirit".to_string()]);
        let keywords = &g.copiable_definition(spirit.id).unwrap().keywords;
        assert_eq!(keywords, &vec![KeywordAbility::Flying]);
    }
    assert_recoverable((&g).into());
}

/// Hundred-Talon Kami ("Soulshift 4") dies with Niblis of Dusk (a Spirit,
/// mana value 3), Scuttling Death (a Spirit, mana value 5) and Grizzly
/// Bears in its owner's graveyard. Only Niblis is a legal target; the
/// controller may return it or decline (CR 702.46a, 608.2d).
#[test]
fn soulshift_returns_only_a_small_enough_spirit_card_by_choice() {
    for accept in [true, false] {
        let graveyard = Zone::Graveyard { owner: PlayerId(0) };
        let mut g = Game::with_trace(
            prepare(
                board(
                    &[HUNDRED_TALON_KAMI, NIBLIS_OF_DUSK, SCUTTLING_DEATH],
                    TurnStep::PrecombatMain,
                )
                .permanent_or_hand("Hundred-Talon Kami", 0, 0, false)
                .permanent_or_hand("Lightning Bolt", 0, 0, true)
                .permanent_or_hand("Mountain", 0, 0, false)
                .card("Niblis of Dusk", 0, 0, graveyard)
                .card("Scuttling Death", 0, 0, graveyard)
                .card("Grizzly Bears", 0, 0, graveyard),
            ),
            TraceMode::Detailed,
        );
        let kami = Driver::object(&g, 0);
        let (niblis, scuttling, bears) = (
            Driver::object(&g, 3),
            Driver::object(&g, 4),
            Driver::object(&g, 5),
        );
        Driver::cast(&mut g, CardId(1));
        Driver::pick_object(&mut g, kami);
        Driver::pay(&mut g, &[2]);
        let mut targeted = false;
        for _ in 0..64 {
            let d = Driver::request(&mut g, u64::MAX);
            if targeted && d.reason == Reason::Priority && g.state.rules.stack.is_empty() {
                break;
            }
            let answer = match &d.reason {
                Reason::Targets { .. } => {
                    let offered = |object| d.candidates.contains(&Candidate::Object { object });
                    assert!(offered(niblis));
                    assert!(!offered(scuttling) && !offered(bears));
                    assert!(g.current(kami).is_err(), "the Kami has died");
                    targeted = true;
                    d.targets(vec![niblis])
                }
                Reason::OptionalEffect => {
                    checked(&g);
                    d.pick_one(&Candidate::Flag {
                        flag: if accept { Flag::Accept } else { Flag::Decline },
                    })
                    .unwrap()
                }
                Reason::Priority => mtg_witness::temporary::pass(&d),
                other => panic!("unexpected decision {other:?}"),
            };
            g.respond(d.player, d.id, answer).unwrap();
        }
        assert!(targeted);
        let zone = |object: ObjectId| {
            g.state
                .rules
                .objects
                .values()
                .find(|o| o.id.card == object.card)
                .unwrap()
                .zone
        };
        assert_eq!(
            zone(niblis),
            if accept {
                Zone::Hand { owner: PlayerId(0) }
            } else {
                graveyard
            }
        );
        assert_eq!(zone(scuttling), graveyard);
        assert_eq!(zone(bears), graveyard);
        checked(&g);
        assert_recoverable((&g).into());
    }
}
