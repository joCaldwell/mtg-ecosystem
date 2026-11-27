//! Frozen R5 witnesses for cycle and filtered discard events (CR 603.10,
//! 701.9a, 702.29a, 702.29d).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const STOIC_CHAMPION: &str = record!("1203a80f-11ac-46ed-aa2a-5a6782b6303d");
const FLOURISHING_FOX: &str = record!("44add927-4e6b-408e-907f-6a665add8b16");
const HORROR: &str = record!("6de20c2c-b8a8-4f12-897b-a2f55d7eed87");
const GREEN_GOBLIN: &str = record!("d1f1e48f-f1f4-497f-9112-79159d2f443b");
const VIZIER: &str = record!("699fc7cf-1ad3-4c32-b3e4-49f9a2da4b26");
const TITANBONES: &str = record!("510bccb1-61cd-49f1-a54a-80352653d1e3");

fn collected(g: &Game, source: ObjectId) -> usize {
    g.events()
        .iter()
        .filter(|e| matches!(e, Event::TriggerCollected { source: s, .. } if *s == source))
        .count()
}

/// Flourishing Fox: "Whenever you cycle another card, put a +1/+1 counter
/// on this creature." Horror of the Broken Lands: "Whenever you cycle or
/// discard another card, this creature gets +2/+1 until end of turn."
/// Stoic Champion: "Whenever a player cycles a card, this creature gets
/// +2/+2 until end of turn." Cycling a second Fox triggers each once, the
/// opponent's Champion included; the cycled Fox's own ability does not
/// function from the graveyard (CR 702.29a, 702.29d).
#[test]
fn discard_events_cycling_triggers_cycle_and_discard_observers_once() {
    let mut g = Game::with_trace(
        prepare(
            board(&[FLOURISHING_FOX, HORROR, STOIC_CHAMPION], TurnStep::PrecombatMain)
                .permanent_or_hand("Flourishing Fox", 0, 0, false)
                .permanent_or_hand("Horror of the Broken Lands", 0, 0, false)
                .permanent_or_hand("Stoic Champion", 1, 1, false)
                .permanent_or_hand("Flourishing Fox", 0, 0, true)
                .permanent_or_hand("Plains", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let (fox, horror, champion, cycled) = (
        Driver::object(&g, 0),
        Driver::object(&g, 1),
        Driver::object(&g, 2),
        Driver::object(&g, 3),
    );
    assert!(matches!(
        g.definition(CardId(0)).unwrap().triggered[0].condition.pattern(),
        EventPattern::Discards { cycled: true, .. }
    ));
    assert!(matches!(
        g.definition(CardId(1)).unwrap().triggered[0].condition.pattern(),
        EventPattern::Discards { cycled: false, .. }
    ));
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: cycled,
            ability: 0,
        },
    );
    Driver::pay(&mut g, &[4]);
    drive(&mut g, &[], |g| g.state.rules.stack.len() == 4);
    checked(&g);
    settle(&mut g);
    assert_eq!(g.current(fox).unwrap().plus_one_counters, 1.into());
    assert_eq!(
        [fox, horror, champion].map(|o| collected(&g, o)),
        [1, 1, 1]
    );
    assert_recoverable((&g).into());
}

/// A cleanup discard of an Island is a discard but not a cycle: Horror
/// triggers and the Fox does not. Green Goblin, Nemesis: "Whenever you
/// discard a nonland card, put a +1/+1 counter on target Goblin you
/// control. Whenever you discard a land card, create a tapped Treasure
/// token." Only its land row matches the discarded card, read in the
/// graveyard (CR 603.10, 701.9a).
#[test]
fn discard_events_filtered_discard_reads_the_discarded_card() {
    let mut setup = board(&[FLOURISHING_FOX, HORROR, GREEN_GOBLIN], TurnStep::End)
        .permanent_or_hand("Flourishing Fox", 0, 0, false)
        .permanent_or_hand("Horror of the Broken Lands", 0, 0, false)
        .permanent_or_hand("Green Goblin, Nemesis", 0, 0, false);
    for _ in 0..8 {
        setup = setup.permanent_or_hand("Island", 0, 0, true);
    }
    let mut g = Game::with_trace(prepare(setup), TraceMode::Detailed);
    let (fox, horror, goblin) = (
        Driver::object(&g, 0),
        Driver::object(&g, 1),
        Driver::object(&g, 2),
    );
    assert!(matches!(
        g.definition(CardId(2)).unwrap().triggered[1].condition.pattern(),
        EventPattern::Discards { card, .. } if card.reads_characteristics()
    ));
    for _ in g.live_players() {
        Driver::pass(&mut g);
    }
    let discard = Driver::request(&mut g, u64::MAX);
    assert_eq!(discard.reason, Reason::Discard);
    let island = Driver::object(&g, 3);
    Driver::pick_object(&mut g, island);
    drive(&mut g, &[], |g| g.state.rules.stack.len() == 2);
    // The pending land trigger retains the discarded card's facts.
    checked(&g);
    settle(&mut g);
    assert_eq!(
        [fox, horror, goblin].map(|o| collected(&g, o)),
        [0, 1, 1]
    );
    let treasures = g
        .state
        .rules
        .objects
        .values()
        .filter(|o| {
            o.zone == Zone::Battlefield && o.tapped && g.state.rules.tokens.contains_key(&o.id.card)
        })
        .count();
    assert_eq!(treasures, 1);
    assert_recoverable((&g).into());
}

/// Vizier of Tumbling Sands: "Cycling {1}{U}" and "When you cycle this card,
/// untap target permanent." The ability triggers from the graveyard the
/// cycled card was put into, under its owner's control, and untaps an
/// Island tapped for the cost before the cycling draw (CR 702.29c).
#[test]
fn discard_events_cycled_card_triggers_from_its_graveyard() {
    let mut g = Game::with_trace(
        prepare(
            board(&[VIZIER], TurnStep::PrecombatMain)
                .permanent_or_hand("Vizier of Tumbling Sands", 0, 0, true)
                .permanent_or_hand("Island", 0, 0, false)
                .permanent_or_hand("Island", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let (vizier, island) = (Driver::object(&g, 0), Driver::object(&g, 1));
    let definition = g.definition(CardId(0)).unwrap();
    let trigger = definition.triggered.last().unwrap();
    assert_eq!(trigger.functioning_zone, FunctioningZone::Graveyard);
    assert!(trigger.condition.pattern().discards_source());
    let ability = u32::try_from(
        definition
            .activated
            .iter()
            .position(|a| a.functioning_zone == FunctioningZone::Hand)
            .unwrap(),
    )
    .unwrap();
    let before = hand(&g, 0);
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: vizier,
            ability,
        },
    );
    Driver::pay(&mut g, &[1, 2]);
    assert!(g.current(island).unwrap().tapped);
    // The pending trigger's source is the card in its owner's graveyard.
    checked(&g);
    Driver::pick_object(&mut g, island);
    assert_eq!(g.state.rules.stack.len(), 2);
    checked(&g);
    settle(&mut g);
    assert!(!g.current(island).unwrap().tapped);
    assert_eq!(hand(&g, 0), before);
    assert_eq!(
        g.state
            .rules
            .objects
            .values()
            .filter(|o| o.id.card == CardId(0))
            .map(|o| o.zone)
            .collect::<Vec<_>>(),
        [Zone::Graveyard { owner: PlayerId(0) }]
    );
    assert_recoverable((&g).into());
}

/// Titanbones, Towering Heart: "When you discard this card, you gain 3
/// life." Discarding it and a Vizier of Tumbling Sands to hand size
/// triggers Titanbones from the graveyard; the Vizier was discarded but not
/// cycled, so its cycle trigger does not (CR 702.29a, 702.29c).
#[test]
fn discard_events_discarded_card_triggers_and_a_plain_discard_is_no_cycle() {
    let mut setup = board(&[VIZIER, TITANBONES], TurnStep::End)
        .permanent_or_hand("Titanbones, Towering Heart", 0, 0, true)
        .permanent_or_hand("Vizier of Tumbling Sands", 0, 0, true);
    for _ in 0..7 {
        setup = setup.permanent_or_hand("Island", 0, 0, true);
    }
    let mut g = Game::with_trace(prepare(setup), TraceMode::Detailed);
    let cards = [Driver::object(&g, 0), Driver::object(&g, 1)];
    let before = life(&g, 0);
    for _ in g.live_players() {
        Driver::pass(&mut g);
    }
    let discard = Driver::request(&mut g, u64::MAX);
    assert_eq!(discard.reason, Reason::Discard);
    let picks = cards
        .iter()
        .map(|object| discard.index(&Candidate::Object { object: *object }).unwrap())
        .collect();
    g.respond(discard.player, discard.id, Answer::Pick { picks: vec![picks] })
        .unwrap();
    drive(&mut g, &[], triggered);
    assert_eq!(g.state.rules.stack.len(), 1);
    checked(&g);
    settle(&mut g);
    assert_eq!(life(&g, 0), plus(&before, 3));
    assert_recoverable((&g).into());
}

const VALIANT_RESCUER: &str = record!("c9de9f0b-bf07-4171-b54d-69033424a212");

/// Valiant Rescuer: "Whenever you cycle another card for the first time
/// each turn, create a 1/1 white Human Soldier creature token." Cycling two
/// Flourishing Foxes creates one token; the turn ledger counts both cycles
/// (CR 603.2, 702.29a).
#[test]
fn discard_events_first_cycle_each_turn_triggers_once() {
    let mut g = Game::with_trace(
        prepare(
            board(&[VALIANT_RESCUER, FLOURISHING_FOX], TurnStep::PrecombatMain)
                .permanent_or_hand("Valiant Rescuer", 0, 0, false)
                .permanent_or_hand("Flourishing Fox", 0, 0, true)
                .permanent_or_hand("Flourishing Fox", 0, 0, true)
                .permanent_or_hand("Plains", 0, 0, false)
                .permanent_or_hand("Plains", 0, 0, false),
        ),
        TraceMode::Detailed,
    );
    let rescuer = Driver::object(&g, 0);
    assert!(matches!(
        g.definition(CardId(0)).unwrap().triggered[0].condition.pattern(),
        EventPattern::Discards { cycled: true, nth: Some(ordinal), .. } if ordinal.nth == 1
    ));
    for (card, land) in [(1, 3), (2, 4)] {
        let object = Driver::object(&g, card);
        Driver::action(&mut g, Action::ActivateAbility { object, ability: 0 });
        Driver::pay(&mut g, &[land]);
        checked(&g);
        settle(&mut g);
    }
    assert_eq!(g.state.rules.cast_history[0].cycles, 2);
    assert_eq!(collected(&g, rescuer), 1);
    let tokens = g
        .state
        .rules
        .objects
        .values()
        .filter(|o| o.zone == Zone::Battlefield && g.state.rules.tokens.contains_key(&o.id.card))
        .count();
    assert_eq!(tokens, 1);
    assert_recoverable((&g).into());
}
