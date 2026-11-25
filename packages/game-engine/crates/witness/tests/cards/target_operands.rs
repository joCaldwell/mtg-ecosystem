//! Wave C lane A: target and selection operands from exact frozen R4 source
//! on private boards. Controller-qualified targets, damage to a target
//! creature, +1/+1 counters on each creature you control, the source
//! permanent as an operand, and an up-to-one target that may be omitted.
//! Every decision is checked for state validity and an exact snapshot round
//! trip; every finished resolution replays from its journal.
//! Pinned CR 109.5, 115.1d, 122.6, 400.7, 601.2c, 608.2b/h, 701.26, 903.9a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_contracts::kir::{Cell, definition_cells};
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

const BLOSSOMING_DEFENSE: &str = record!("5a851367-1c4a-4cc9-a9c3-3d2775986b4c");
const EXCLUSION_MAGE: &str = record!("fd28fe55-36cd-4242-8dd4-edb58fbb9895");
const BREATH_OF_FIRE: &str = record!("046923d6-2d58-42ce-afb8-886f4932e76c");
const TITANIAS_BOON: &str = record!("0bf4c01d-3cb0-47be-b84a-6a399bc2d3fc");
const BLINKING_SPIRIT: &str = record!("a26f6958-9bc8-46f8-8ca7-7f55098e1291");
const HORSESHOE_CRAB: &str = record!("793f29ac-8a5d-49bf-91e8-a32f771268f3");
const THE_ART_OF_TEA: &str = record!("9ed4e48f-91bd-45f9-9f31-a2705b330713");
const RESCUE: &str = record!("f0ae687a-7222-4521-a514-ba10373fed7e");
const FENCER_CLIQUE: &str = record!("58374ee4-0497-4d12-847c-0f67555f8399");
const SELENIA: &str = record!("3db1ac09-e35a-4f50-97c4-f65dbc28daef");

/// A two-seat main-phase board whose catalog adds `cards`, each compiled
/// from frozen source and occupying the named cells.
fn board(cards: &[(&str, &[Cell])]) -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for &(raw, cells) in cards {
        let result = mtg_witness::compile(mtg_witness::R4_MANIFEST, raw);
        let mtg_compiler::CompileResult::Lowered { definition } = result else {
            panic!("frozen target witness failed: {result:?}");
        };
        let occupied = definition_cells(&definition).unwrap();
        for cell in cells {
            assert!(
                occupied.contains(cell),
                "{} lacks {cell:?}: {occupied:?}",
                definition.name
            );
        }
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Board::new(catalog, "r4-target-operands")
        .step(TurnStep::PrecombatMain)
        .seats(2)
}

/// State validation and an exact snapshot round trip.
fn checked(g: &Game) {
    g.validate_state().unwrap();
    let snapshot = g.snapshot().unwrap();
    let restored = Game::restore(g.prepared.as_ref().clone(), &snapshot).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
}

/// The pending target decision for seat 0, after a check.
fn targets(g: &mut Game) -> Decision<Rules> {
    let d = Driver::request(g, u64::MAX);
    assert_eq!(d.player, PlayerId(0), "{d:?}");
    assert!(matches!(d.reason, Reason::Targets { .. }), "{d:?}");
    checked(g);
    d
}

fn offers(d: &Decision<Rules>, object: ObjectId) -> bool {
    d.candidates.contains(&Candidate::Object { object })
}

fn answer_targets(g: &mut Game, d: &Decision<Rules>, objects: &[ObjectId]) {
    let picks = objects
        .iter()
        .map(|object| d.index(&Candidate::Object { object: *object }).unwrap())
        .collect();
    g.respond(d.player, d.id, Answer::Pick { picks: vec![picks] })
        .unwrap();
}

/// Resolve to an empty stack, then prove replay from the journal.
fn settle(g: &mut Game) {
    for _ in 0..24 {
        let d = Driver::request(g, 1);
        assert_eq!(d.reason, Reason::Priority, "{d:?}");
        if g.state.rules.stack.is_empty() {
            checked(g);
            assert_recoverable((&*g).into());
            return;
        }
        Driver::pass(g);
    }
    panic!("no empty-stack priority");
}

/// Pass priority until a decision other than priority is pending.
fn pass_until_choice(g: &mut Game) -> Decision<Rules> {
    for _ in 0..24 {
        let d = Driver::request(g, u64::MAX);
        if d.reason != Reason::Priority {
            return d;
        }
        Driver::pass(g);
    }
    panic!("no pending choice");
}

fn zone(g: &Game, card: u32) -> Zone {
    g.state.rules.objects[&CardId(card)].zone
}

fn activate(g: &mut Game, card: u32, ability: u32) {
    let object = Driver::object(g, card);
    Driver::action(g, Action::ActivateAbility { object, ability });
}

/// Blossoming Defense: "Target creature you control gets +2/+2 and gains
/// hexproof until end of turn." Only the caster's creature is offered
/// (CR 109.5, 601.2c).
#[test]
fn blossoming_defense_targets_only_a_creature_you_control() {
    let prepared = board(&[(
        BLOSSOMING_DEFENSE,
        &[Cell::PumpTarget, Cell::TargetKeywordsUntilEndOfTurn],
    )])
    .permanent_or_hand("Blossoming Defense", 0, 0, true)
    .permanent_or_hand("Forest", 0, 0, false)
    .permanent_or_hand("Grizzly Bears", 0, 0, false)
    .permanent_or_hand("Grizzly Bears", 1, 1, false)
    .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let (mine, theirs) = (Driver::object(&g, 2), Driver::object(&g, 3));
    Driver::cast(&mut g, CardId(0));
    let d = targets(&mut g);
    assert!(offers(&d, mine));
    assert!(!offers(&d, theirs));
    answer_targets(&mut g, &d, &[mine]);
    Driver::pay(&mut g, &[1]);
    settle(&mut g);
    let pumped = g.characteristics(mine).unwrap();
    assert_eq!(
        (pumped.power, pumped.toughness),
        (Some(4.into()), Some(4.into()))
    );
    assert!(g.current_keywords(mine).contains(&KeywordAbility::Hexproof));
    assert!(
        !g.current_keywords(theirs)
            .contains(&KeywordAbility::Hexproof)
    );
}

/// Exclusion Mage: "When this creature enters, return target creature an
/// opponent controls to its owner's hand." The trigger offers only the
/// opponent's creature (CR 109.5, 603.3d).
#[test]
fn exclusion_mage_returns_only_an_opponents_creature() {
    let prepared = board(&[(EXCLUSION_MAGE, &[Cell::MovePermanentOwnerHand])])
        .permanent_or_hand("Exclusion Mage", 0, 0, true)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let (mine, theirs) = (Driver::object(&g, 4), Driver::object(&g, 5));
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2, 3]);
    pass_until_choice(&mut g);
    let d = targets(&mut g);
    assert!(offers(&d, theirs));
    assert!(!offers(&d, mine));
    assert!(!offers(&d, Driver::object(&g, 0)));
    answer_targets(&mut g, &d, &[theirs]);
    settle(&mut g);
    assert_eq!(zone(&g, 5), Zone::Hand { owner: PlayerId(1) });
    assert_eq!(zone(&g, 4), Zone::Battlefield);
}

/// Breath of Fire: "Breath of Fire deals 2 damage to target creature."
/// Players are not offered; lethal damage destroys the creature (CR 120.3,
/// 704.5g).
#[test]
fn breath_of_fire_deals_two_damage_to_a_target_creature() {
    let prepared = board(&[(BREATH_OF_FIRE, &[Cell::DamageTarget])])
        .permanent_or_hand("Breath of Fire", 0, 0, true)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let bears = Driver::object(&g, 3);
    Driver::cast(&mut g, CardId(0));
    let d = targets(&mut g);
    assert!(
        !d.candidates
            .iter()
            .any(|c| matches!(c, Candidate::Player { .. }))
    );
    answer_targets(&mut g, &d, &[bears]);
    Driver::pay(&mut g, &[1, 2]);
    settle(&mut g);
    assert_eq!(zone(&g, 3), Zone::Graveyard { owner: PlayerId(1) });
}

/// Titania's Boon: "Put a +1/+1 counter on each creature you control." One
/// simultaneous placement on the controller's creatures as it resolves
/// (CR 122.6, 608.2h); the opponent's creature is unaffected.
#[test]
fn titanias_boon_counters_each_creature_you_control() {
    let prepared = board(&[(TITANIAS_BOON, &[Cell::PlusOneCountersSet])])
        .permanent_or_hand("Titania's Boon", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .permanent_or_hand("Llanowar Elves", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2, 3, 4]);
    settle(&mut g);
    let counters = |g: &Game, card| {
        g.current(Driver::object(g, card))
            .unwrap()
            .plus_one_counters
            .clone()
    };
    assert_eq!(counters(&g, 5), Quantity::from(1));
    assert_eq!(counters(&g, 6), Quantity::from(1));
    assert_eq!(counters(&g, 7), Quantity::zero());
}

/// Blinking Spirit: "{0}: Return this creature to its owner's hand." Two
/// activations: the first to resolve returns it; the second finds a new
/// object in the hand and does nothing (CR 400.7).
#[test]
fn blinking_spirit_returns_its_own_battlefield_incarnation_once() {
    let prepared = board(&[(BLINKING_SPIRIT, &[Cell::MoveSourceOwnerZone])])
        .permanent_or_hand("Blinking Spirit", 0, 0, false)
        .libraries("Plains", 2)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    for _ in 0..2 {
        activate(&mut g, 0, 0);
        Driver::pay(&mut g, &[]);
    }
    assert_eq!(g.state.rules.stack.len(), 2);
    checked(&g);
    while g.state.rules.stack.len() == 2 {
        Driver::pass(&mut g);
    }
    let hand = Zone::Hand { owner: PlayerId(0) };
    assert_eq!(zone(&g, 0), hand);
    assert_eq!(g.state.rules.stack.len(), 1);
    checked(&g);
    settle(&mut g);
    assert_eq!(zone(&g, 0), hand);
}

/// Horseshoe Crab: "{U}: Untap this creature." after Icy Manipulator taps
/// it (CR 701.26b).
#[test]
fn horseshoe_crab_untaps_itself() {
    let prepared = board(&[(HORSESHOE_CRAB, &[Cell::TapStatusSource])])
        .permanent_or_hand("Horseshoe Crab", 0, 0, false)
        .permanent_or_hand("Icy Manipulator", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let crab = Driver::object(&g, 0);
    activate(&mut g, 1, 0);
    Driver::pick_object(&mut g, crab);
    Driver::pay(&mut g, &[2]);
    settle(&mut g);
    assert!(g.current(crab).unwrap().tapped);
    activate(&mut g, 0, 0);
    Driver::pay(&mut g, &[3]);
    settle(&mut g);
    assert!(!g.current(crab).unwrap().tapped);
}

/// Fencer Clique: "{U}: Put this creature on top of its owner's library."
#[test]
fn fencer_clique_puts_itself_on_top_of_its_library() {
    let prepared = board(&[(FENCER_CLIQUE, &[Cell::MoveSourceOwnerZone])])
        .permanent_or_hand("Fencer Clique", 0, 0, false)
        .permanent_or_hand("Island", 0, 0, false)
        .libraries("Plains", 2)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    activate(&mut g, 0, 0);
    Driver::pay(&mut g, &[1]);
    settle(&mut g);
    assert_eq!(zone(&g, 0), Zone::Library { owner: PlayerId(0) });
    assert_eq!(
        g.state.rules.libraries[&PlayerId(0)].first(),
        Some(&CardId(0)),
        "the source is the top card"
    );
}

/// Selenia, Dark Angel as a commander: "Pay 2 life: Return Selenia to its
/// owner's hand." The owner may put the commander into the command zone
/// instead (CR 903.9a); the pending choice validates and recovers.
#[test]
fn selenia_returning_itself_offers_the_command_zone() {
    for to_command in [true, false] {
        let prepared = board(&[(SELENIA, &[Cell::MoveSourceOwnerZone])])
            .permanent_or_hand("Selenia, Dark Angel", 0, 0, false)
            .commander(CardId(0))
            .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        let before = g.state.rules.players[0].life.clone();
        activate(&mut g, 0, 0);
        let d = pass_until_choice(&mut g);
        assert!(
            matches!(&d.reason, Reason::CommanderReturn { destination, .. }
                if *destination == Zone::Hand { owner: PlayerId(0) }),
            "{d:?}"
        );
        checked(&g);
        assert_recoverable((&g).into());
        let flag = if to_command {
            Flag::CommandZone
        } else {
            Flag::Destination
        };
        g.respond(
            d.player,
            d.id,
            d.pick_one(&Candidate::Flag { flag }).unwrap(),
        )
        .unwrap();
        settle(&mut g);
        let mut paid = before.clone();
        paid.add(&Quantity::from(-2));
        assert_eq!(g.state.rules.players[0].life, paid);
        assert_eq!(
            zone(&g, 0),
            if to_command {
                Zone::Command { owner: PlayerId(0) }
            } else {
                Zone::Hand { owner: PlayerId(0) }
            }
        );
    }
}

/// The Art of Tea: "Put a +1/+1 counter on up to one target creature you
/// control. Create a Food token." Zero targets is a legal announcement;
/// the untargeted instruction still happens (CR 115.1d, 601.2c, 608.2b).
#[test]
fn the_art_of_tea_may_announce_zero_targets() {
    for chosen in [false, true] {
        let prepared = board(&[(THE_ART_OF_TEA, &[Cell::PlusOneCountersTarget])])
            .permanent_or_hand("The Art of Tea", 0, 0, true)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 1, 1, false)
            .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        let (mine, theirs) = (Driver::object(&g, 3), Driver::object(&g, 4));
        Driver::cast(&mut g, CardId(0));
        let d = targets(&mut g);
        assert!(offers(&d, mine));
        assert!(!offers(&d, theirs));
        let picked: &[ObjectId] = if chosen { &[mine] } else { &[] };
        answer_targets(&mut g, &d, picked);
        Driver::pay(&mut g, &[1, 2]);
        settle(&mut g);
        assert_eq!(
            g.current(mine).unwrap().plus_one_counters,
            Quantity::from(i64::from(chosen))
        );
        assert_eq!(g.state.rules.tokens.len(), 1, "the Food token");
        assert_eq!(zone(&g, 0), Zone::Graveyard { owner: PlayerId(0) });
    }
}

/// Rescue: "Return target permanent you control to its owner's hand." Any
/// permanent type the caster controls is offered; the opponent's is not.
#[test]
fn rescue_returns_a_permanent_you_control() {
    let prepared = board(&[(RESCUE, &[Cell::MovePermanentOwnerHand])])
        .permanent_or_hand("Rescue", 0, 0, true)
        .permanent_or_hand("Island", 0, 0, false)
        .permanent_or_hand("Sol Ring", 0, 0, false)
        .permanent_or_hand("Sol Ring", 1, 1, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let (mine, theirs) = (Driver::object(&g, 2), Driver::object(&g, 3));
    Driver::cast(&mut g, CardId(0));
    let d = targets(&mut g);
    assert!(offers(&d, mine));
    assert!(offers(&d, Driver::object(&g, 1)));
    assert!(!offers(&d, theirs));
    answer_targets(&mut g, &d, &[mine]);
    Driver::pay(&mut g, &[1]);
    settle(&mut g);
    assert_eq!(zone(&g, 2), Zone::Hand { owner: PlayerId(0) });
    assert_eq!(zone(&g, 3), Zone::Battlefield);
}
