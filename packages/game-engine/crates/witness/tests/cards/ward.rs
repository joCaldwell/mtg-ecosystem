//! Ward from exact frozen R5 source on private boards (CR 702.21a):
//! "Whenever this permanent becomes the target of a spell or ability an
//! opponent controls, counter that spell or ability unless that player pays
//! [cost]." The trigger names the targeting stack object without targeting it
//! (CR 115.1, 603.2); "that player" is its controller (CR 118.12a). A spell
//! is countered to its owner's graveyard, an ability leaves the stack
//! (CR 701.6). Each board is checked for state validity, a snapshot round trip
//! and journal recovery at both quanta in every trace mode.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_contracts::kir::{
    Amount, EventPattern, Node, Payment, PlayerExpr, Pred, Ref, TargetingKind, Verb,
};
use mtg_core::testkit::*;

macro_rules! record {
    ($id:literal) => {
        include_str!(concat!(
            "../../../../tests/fixtures/phase1-r5/",
            $id,
            ".json"
        ))
    };
}

const TOMAKUL_HONOR_GUARD: &str = record!("b68cf4e6-205e-40b6-b28b-f4b7d8af7017");
const PUNK_FROGS: &str = record!("dfb13eca-55fd-4ac6-aee3-2bdbfe9e1c4e");
const OWLIN_SHIELDMAGE: &str = record!("50c477c1-8457-40f6-bba9-1a4ba0862178");
const DWARVEN_FORGE_CHANTER: &str = record!("e69a8970-3ead-479d-860a-26ed65f9c846");
const SPECTRAL_SNATCHER: &str = record!("371762f6-e07b-4c0e-a4ee-6c35a3809eb6");

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);
const P2: PlayerId = PlayerId(2);

fn lowered(raw: &str) -> Definition {
    let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, raw);
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("frozen ward witness must lower: {result:?}");
    };
    definition.validate().unwrap();
    *definition
}

fn rejected(raw: &str) -> String {
    format!("{:?}", mtg_witness::compile(mtg_witness::R5_MANIFEST, raw))
}

fn board(seats: u8) -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    catalog
        .definitions
        .extend([TOMAKUL_HONOR_GUARD, OWLIN_SHIELDMAGE].map(lowered));
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    Board::new(catalog, "r5-ward-witness")
        .step(TurnStep::PrecombatMain)
        .seats(seats)
}

fn game(b: Board) -> Game {
    Game::with_trace(
        b.libraries("Island", 6).seed([31; 32]).prepare(),
        TraceMode::Detailed,
    )
}

fn checked(g: &Game) {
    g.validate_state().unwrap();
    let snapshot = g.snapshot().unwrap();
    let restored = Game::restore(g.prepared.as_ref().clone(), &snapshot).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
}

fn pick(g: &mut Game, candidate: Candidate<Rules>) {
    let d = Driver::request(g, u64::MAX);
    let answer = d
        .pick_one(&candidate)
        .unwrap_or_else(|e| panic!("{e:?}: {d:?}"));
    g.respond(d.player, d.id, answer).unwrap();
}

/// Pass priority until a decision other than priority, or an empty stack.
fn next_choice(g: &mut Game) -> Option<Decision<Rules>> {
    for _ in 0..32 {
        let d = Driver::request(g, u64::MAX);
        if d.reason != Reason::Priority {
            return Some(d);
        }
        if g.state.rules.stack.is_empty() {
            return None;
        }
        Driver::pass(g);
    }
    panic!("no decision within 32 passes");
}

/// The ward payer's offer: accept and pay `lands`, or decline.
fn answer_ward(g: &mut Game, payer: PlayerId, pay: Option<&[u32]>) {
    let d = next_choice(g).expect("ward asks its payer");
    assert_eq!(
        (d.player, &d.reason),
        (payer, &Reason::OptionalEffect),
        "{d:?}"
    );
    checked(g);
    let flag = if pay.is_some() {
        Flag::Accept
    } else {
        Flag::Decline
    };
    g.respond(
        d.player,
        d.id,
        d.pick_one(&Candidate::Flag { flag }).unwrap(),
    )
    .unwrap();
    if let Some(lands) = pay.filter(|lands| !lands.is_empty()) {
        Driver::pay(g, lands);
    }
}

fn settle(g: &mut Game) {
    assert!(next_choice(g).is_none(), "an unexpected decision remains");
    checked(g);
    assert_recoverable((&*g).into());
}

fn zone(g: &Game, id: u32) -> Zone {
    g.state.rules.objects[&CardId(id)].zone
}

fn cast_on(g: &mut Game, card: u32, target: u32, lands: &[u32]) {
    Driver::cast(g, CardId(card));
    let object = Driver::object(g, target);
    pick(g, Candidate::Object { object });
    Driver::pay(g, lands);
}

/// Tomakul Honor Guard's "Ward {2}" lowers to one targeting trigger: an
/// opponent's spell or ability that targets this permanent is countered
/// unless its controller pays the generic {2}. Punk Frogs pays {3}; Owlin
/// Shieldmage's and Dwarven Forge-Chanter's "Ward—Pay N life" pay life.
/// Spectral Snatcher's "Ward—Discard a card" has no payment the kernel
/// executes and rejects the whole card by name.
#[test]
fn ward_lowers_to_a_targeting_trigger_with_its_printed_payment() {
    let unless = |payment: Payment| Node::UnlessPays {
        payer: PlayerExpr::ControllerOf {
            object: Ref::EventStackObject,
        },
        payment,
        node: Box::new(Node::Do(Verb::Counter {
            what: Ref::EventStackObject,
        })),
    };
    for (raw, payment) in [
        (
            TOMAKUL_HONOR_GUARD,
            Payment::Generic {
                amount: Amount::fixed(2),
            },
        ),
        (
            PUNK_FROGS,
            Payment::Generic {
                amount: Amount::fixed(3),
            },
        ),
        (
            OWLIN_SHIELDMAGE,
            Payment::Life {
                amount: Amount::fixed(3),
            },
        ),
        (
            DWARVEN_FORGE_CHANTER,
            Payment::Life {
                amount: Amount::fixed(2),
            },
        ),
    ] {
        let d = lowered(raw);
        let [ward] = d
            .triggered
            .iter()
            .filter(|t| matches!(t.condition.pattern(), EventPattern::BecomesTarget { .. }))
            .collect::<Vec<_>>()[..]
        else {
            panic!("one ward trigger: {:?}", d.triggered);
        };
        assert_eq!(
            ward.condition.pattern(),
            &EventPattern::BecomesTarget {
                object: Pred::source(),
                by: PlayerExpr::EachOpponent,
                kind: TargetingKind::SpellOrAbility,
            }
        );
        assert!(ward.program.targets.is_empty());
        let [Instruction::Kernel { node, .. }] = ward.program.instructions.as_slice() else {
            panic!("one kernel instruction: {ward:?}");
        };
        assert_eq!(node, &unless(payment), "{}", d.name);
    }
    let owlin = lowered(OWLIN_SHIELDMAGE);
    assert!(owlin.keywords.contains(&KeywordAbility::Flying));
    let snatcher = rejected(SPECTRAL_SNATCHER);
    assert!(snatcher.contains("keyword:ward:cost"), "{snatcher}");
}

/// An opponent's Unsummon targets Tomakul Honor Guard. Declining the {2}
/// counters Unsummon to its owner's graveyard and the Honor Guard stays;
/// paying lets Unsummon return it to its owner's hand.
#[test]
fn ward_counters_an_opponents_spell_unless_its_controller_pays() {
    for pay in [false, true] {
        let mut g = game(
            board(2)
                .permanent_or_hand("Unsummon", 0, 0, true)
                .permanent_or_hand("Island", 0, 0, false)
                .permanent_or_hand("Island", 0, 0, false)
                .permanent_or_hand("Island", 0, 0, false)
                .permanent_or_hand("Tomakul Honor Guard", 1, 1, false),
        );
        cast_on(&mut g, 0, 4, &[1]);
        // The ward trigger is above Unsummon, controlled by its permanent's
        // controller.
        assert_eq!(g.state.rules.stack.len(), 2);
        assert_eq!(g.state.rules.stack[1].controller, P1);
        answer_ward(&mut g, P0, pay.then_some(&[2, 3][..]));
        settle(&mut g);
        if pay {
            assert_eq!(zone(&g, 4), Zone::Hand { owner: P1 });
        } else {
            assert_eq!(zone(&g, 4), Zone::Battlefield);
        }
        assert_eq!(zone(&g, 0), Zone::Graveyard { owner: P0 });
    }
}

/// Ward triggers only on an opponent's spell or ability: its controller's
/// own Unsummon returns the Honor Guard with no trigger.
#[test]
fn ward_ignores_its_controllers_own_spell() {
    let mut g = game(
        board(2)
            .permanent_or_hand("Unsummon", 0, 0, true)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Tomakul Honor Guard", 0, 0, false),
    );
    cast_on(&mut g, 0, 2, &[1]);
    assert_eq!(g.state.rules.stack.len(), 1);
    settle(&mut g);
    assert_eq!(zone(&g, 2), Zone::Hand { owner: P0 });
}

/// Icy Manipulator's activated ability targets Owlin Shieldmage. Declining
/// the 3 life counters the ability, which leaves the stack with no zone
/// change, and the Shieldmage stays untapped; paying costs 3 life and the
/// ability taps it (CR 119.4, 701.6).
#[test]
fn ward_counters_an_activated_ability_unless_life_is_paid() {
    for pay in [false, true] {
        let mut g = game(
            board(2)
                .permanent_or_hand("Icy Manipulator", 0, 0, false)
                .permanent_or_hand("Island", 0, 0, false)
                .permanent_or_hand("Owlin Shieldmage", 1, 1, false),
        );
        let (icy, shieldmage) = (Driver::object(&g, 0), Driver::object(&g, 2));
        Driver::action(
            &mut g,
            Action::ActivateAbility {
                object: icy,
                ability: 0,
            },
        );
        pick(&mut g, Candidate::Object { object: shieldmage });
        Driver::pay(&mut g, &[1]);
        assert_eq!(g.state.rules.stack.len(), 2);
        answer_ward(&mut g, P0, pay.then_some(&[][..]));
        settle(&mut g);
        let shieldmage = &g.state.rules.objects[&CardId(2)];
        assert_eq!(shieldmage.tapped, pay);
        assert_eq!(
            g.state.rules.players[0].life,
            Quantity::from(if pay { 37 } else { 40 })
        );
    }
}

/// In a four-seat game every opponent's targeting triggers ward: the
/// first player's Unsummon on the third player's Honor Guard is countered
/// when the first player declines.
#[test]
fn ward_triggers_for_each_opponent_in_a_four_seat_game() {
    let mut g = game(
        board(4)
            .permanent_or_hand("Unsummon", 0, 0, true)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Tomakul Honor Guard", 2, 2, false),
    );
    cast_on(&mut g, 0, 2, &[1]);
    assert_eq!(g.state.rules.stack.len(), 2);
    assert_eq!(g.state.rules.stack[1].controller, P2);
    answer_ward(&mut g, P0, None);
    settle(&mut g);
    assert_eq!(zone(&g, 2), Zone::Battlefield);
    assert_eq!(zone(&g, 0), Zone::Graveyard { owner: P0 });
}

/// CR 119.4, 118.12a: a player can't pay more life than they have. With 2
/// life the Icy Manipulator's controller is never offered Owlin
/// Shieldmage's "Ward—Pay 3 life"; the ability is countered and the
/// Shieldmage stays untapped.
#[test]
fn ward_life_payment_is_unpayable_with_too_little_life() {
    let mut g = game(
        board(2)
            .life(P0, Quantity::from(2))
            .permanent_or_hand("Icy Manipulator", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Owlin Shieldmage", 1, 1, false),
    );
    let (icy, shieldmage) = (Driver::object(&g, 0), Driver::object(&g, 2));
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: icy,
            ability: 0,
        },
    );
    pick(&mut g, Candidate::Object { object: shieldmage });
    Driver::pay(&mut g, &[1]);
    assert_eq!(g.state.rules.stack.len(), 2);
    checked(&g);
    settle(&mut g);
    assert!(!g.state.rules.objects[&CardId(2)].tapped);
    assert_eq!(g.state.rules.players[0].life, Quantity::from(2));
}

/// CR 702.21b: each instance of ward triggers separately. No frozen card
/// prints two instances, so Tomakul Honor Guard's exact lowered definition
/// carries Owlin Shieldmage's lowered "Ward—Pay 3 life" trigger beside its
/// own "Ward {2}". An opponent's Unsummon puts both triggers on the stack;
/// paying both lets Unsummon resolve, while declining either counters it.
#[test]
fn each_ward_instance_triggers_separately() {
    let ward_of = |d: &Definition| {
        d.triggered
            .iter()
            .find(|t| matches!(t.condition.pattern(), EventPattern::BecomesTarget { .. }))
            .cloned()
            .unwrap()
    };
    let mut guard = lowered(TOMAKUL_HONOR_GUARD);
    let [Instruction::Kernel { source_block, .. }] = ward_of(&guard).program.instructions[..]
    else {
        panic!("one ward kernel instruction");
    };
    // The added instance keeps its node and cites the Honor Guard's own
    // ward block and label, so its provenance names this definition's text.
    let mut life_ward = ward_of(&lowered(OWLIN_SHIELDMAGE));
    for Instruction::Kernel {
        source_block: block,
        ..
    } in &mut life_ward.program.instructions
    {
        *block = source_block;
    }
    let index = guard
        .triggered
        .iter()
        .position(|t| matches!(t.condition.pattern(), EventPattern::BecomesTarget { .. }))
        .unwrap();
    let labels = guard.source_labels.as_mut().unwrap();
    labels.triggered.push(labels.triggered[index]);
    guard.triggered.push(life_ward);
    guard.validate().unwrap();
    let mut catalog = FixtureCatalog::Witness.get().clone();
    catalog.definitions.push(guard);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    for pay_all in [true, false] {
        let mut g = game(
            Board::new(catalog.clone(), "r5-double-ward-witness")
                .step(TurnStep::PrecombatMain)
                .seats(2)
                .permanent_or_hand("Unsummon", 0, 0, true)
                .permanent_or_hand("Island", 0, 0, false)
                .permanent_or_hand("Island", 0, 0, false)
                .permanent_or_hand("Island", 0, 0, false)
                .permanent_or_hand("Tomakul Honor Guard", 1, 1, false),
        );
        Driver::cast(&mut g, CardId(0));
        let object = Driver::object(&g, 4);
        pick(&mut g, Candidate::Object { object });
        Driver::pay(&mut g, &[1]);
        // Both triggers wait for their controller to order them.
        let d = Driver::request(&mut g, u64::MAX);
        assert_eq!((d.player, &d.reason), (P1, &Reason::OrderTriggers), "{d:?}");
        assert_eq!(d.candidates.len(), 2);
        checked(&g);
        g.respond(d.player, d.id, Answer::Order { order: vec![0, 1] })
            .unwrap();
        assert_eq!(Driver::request(&mut g, u64::MAX).reason, Reason::Priority);
        assert_eq!(g.state.rules.stack.len(), 3);
        assert!(g.state.rules.stack[1..].iter().all(|e| e.controller == P1));
        // Each trigger asks the Unsummon's controller in turn: the accepted
        // {2} ward then asks for mana, the accepted life ward pays 3 life.
        // Declining the second-resolving ward counters Unsummon.
        let life = |g: &Game| g.state.rules.players[0].life.clone();
        let mut paid = vec![];
        for round in 0..2 {
            if !pay_all && round == 1 {
                answer_ward(&mut g, P0, None);
                break;
            }
            let before = life(&g);
            answer_ward(&mut g, P0, Some(&[]));
            if Driver::request(&mut g, u64::MAX).reason == (Reason::Mana { source: None }) {
                Driver::pay(&mut g, &[2, 3]);
                paid.push("mana");
            } else {
                let mut after = before;
                after.subtract(&Quantity::from(3));
                assert_eq!(life(&g), after);
                paid.push("life");
            }
        }
        if pay_all {
            paid.sort_unstable();
            assert_eq!(paid, ["life", "mana"]);
        }
        settle(&mut g);
        assert_eq!(zone(&g, 0), Zone::Graveyard { owner: P0 });
        if pay_all {
            assert_eq!(zone(&g, 4), Zone::Hand { owner: P1 });
            assert_eq!(life(&g), Quantity::from(37));
        } else {
            assert_eq!(zone(&g, 4), Zone::Battlefield);
        }
    }
}
