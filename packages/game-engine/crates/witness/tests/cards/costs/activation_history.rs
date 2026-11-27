//! "Activate only if" conditions on turn history, opponents' boards, shared
//! names, graveyard card types and life from frozen R5 (CR 602.5, 500,
//! 601.2i, 111.2, 102.2-102.3, 201.2, 205.2a, 103.4). One measurement
//! decides whether an activation is offered and may begin; the counts it
//! read stay with the activation and survive a snapshot round trip.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_core::ManaAbilityIdentity;

macro_rules! record {
    ($id:literal) => {
        include_str!(concat!(
            "../../../../../tests/fixtures/phase1-r5/",
            $id,
            ".json"
        ))
    };
}

const IDOL: &str = record!("c232d974-9c65-4bce-b045-e5a7cc0c62e0");
const ALARM: &str = record!("5b2364d7-a811-4595-a1b4-224c70555ffa");
const PRIEST: &str = record!("c6064c08-e4e0-49f4-9f0b-55cdecf443af");
const ATLAS: &str = record!("cb923c4d-1d2e-4039-90bb-72d1981b9738");
const SCEPTRE: &str = record!("ce08aff3-efe6-4186-9557-274e00a7ca38");
const EDGE: &str = record!("4927150d-7ff6-4232-b20e-d2ea245ac710");
const WAYFARER: &str = record!("1093fdb6-bd4f-46a8-92e3-5aa46b52bb4f");
const SPEAKER: &str = record!("e4bacf1b-0a45-4c73-857d-dffadb7e6fa5");
const XEREX: &str = record!("2e150f97-5688-4559-94b8-86c33fda28a0");
const TROVE: &str = record!("d604c75e-9d81-49de-80bc-08f4ef3ee975");
const STALLION: &str = record!("0dc6a90e-c517-445e-9a93-29b3119fb4dc");

const CONDITIONED: &[Cell] = &[Cell::ActivationRestriction];

/// The top stack entry's recorded activation-time counts.
fn evidence(g: &Game) -> Option<Vec<u32>> {
    match &g.state.rules.stack.last()?.kind {
        StackKind::Activated { condition, .. } => condition.as_ref().map(|c| c.counts.clone()),
        _ => None,
    }
}

/// A fork whose top activation carries `counts` instead.
fn forge(g: &Game, counts: Vec<u32>) -> Game {
    let mut f = g.fork();
    let StackKind::Activated { condition, .. } = &mut f.state.rules.stack.last_mut().unwrap().kind
    else {
        panic!("expected an activated stack entry")
    };
    *condition = Some(stack_costs::ActivationConditionCheck { counts });
    f
}

fn may(g: &Game, seat: u8, object: ObjectId, ability: u32) -> bool {
    g.validate_activation(PlayerId(seat), object, ability)
        .is_ok()
}

/// Cast the hand card at board index `card` targeting `target` (if any),
/// paying with the board indices `pay`, then resolve it.
fn cast_and_resolve(g: &mut Game, card: u32, target: Option<ObjectId>, pay: &[u32]) {
    let spell = Driver::object(g, card);
    Driver::cast(g, spell.card);
    if let Some(object) = target {
        let d = Driver::request(g, u64::MAX);
        g.respond(
            d.player,
            d.id,
            d.pick_one(&Candidate::Object { object }).unwrap(),
        )
        .unwrap();
    }
    Driver::pay(g, pay);
    settle(g);
}

/// Idol of Oblivion: "{T}: Draw a card. Activate only if you created a token
/// this turn." Raise the Alarm creates two tokens; the draw then reads both
/// (CR 111.2) and keeps that count. Counts the turn never reached, or that
/// fail the condition, are rejected.
#[test]
fn idol_of_oblivion_draws_after_its_controller_creates_a_token() {
    let prepared = board(&[(IDOL, CONDITIONED), (ALARM, &[])])
        .permanent_or_hand("Idol of Oblivion", 0, 0, false)
        .permanent_or_hand("Raise the Alarm", 0, 0, true)
        .permanent_or_hand("Plains", 0, 0, false)
        .permanent_or_hand("Sol Ring", 0, 0, false)
        .permanent_or_hand("Raise the Alarm", 1, 1, true)
        .libraries("Forest", 3)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let idol = Driver::object(&g, 0);
    assert!(!actions(&mut g).contains(&activate(idol, 0)));
    assert!(!may(&g, 0, idol, 0));
    cast_and_resolve(&mut g, 1, None, &[2, 3]);
    assert_eq!(g.state.rules.cast_history[0].tokens_created, 2);
    assert_eq!(g.state.rules.cast_history[1].tokens_created, 0);
    let hand = hand_size(&g, 0);
    assert!(actions(&mut g).contains(&activate(idol, 0)));
    Driver::action(&mut g, activate(idol, 0));
    Driver::request(&mut g, u64::MAX);
    assert_eq!(evidence(&g), Some(vec![2]));
    checked(&g);
    forge(&g, vec![2]).validate_state().unwrap();
    for counts in [vec![0], vec![3], vec![2, 2]] {
        assert!(
            forge(&g, counts.clone()).validate_state().is_err(),
            "{counts:?}"
        );
    }
    settle(&mut g);
    assert_eq!(hand_size(&g, 0), hand + 1);
}

/// Skirsdag High Priest: "Morbid — {T}, Tap two untapped creatures you
/// control: Create a 5/5 black Demon creature token with flying. Activate
/// only if a creature died this turn." An opponent's creature dying counts.
#[test]
fn skirsdag_high_priest_reads_any_creature_dying() {
    let prepared = board(&[(PRIEST, CONDITIONED)])
        .permanent_or_hand("Skirsdag High Priest", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .permanent_or_hand("Lightning Bolt", 0, 0, true)
        .permanent_or_hand("Mountain", 0, 0, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let priest = Driver::object(&g, 0);
    let theirs = Driver::object(&g, 3);
    assert!(!may(&g, 0, priest, 0));
    cast_and_resolve(&mut g, 4, Some(theirs), &[5]);
    assert!(may(&g, 0, priest, 0));
    assert!(actions(&mut g).contains(&activate(priest, 0)));
    checked(&g);
}

/// Endless Atlas: "{2}, {T}: Draw a card. Activate only if you control three
/// or more lands with the same name." Sceptre of Eternal Glory's second mana
/// ability has the same condition (CR 201.2, 605.3a). Two Forests and an
/// Island are not three of one name.
#[test]
fn shared_name_conditions_count_one_name() {
    let printed = |index| Candidate::ManaAbility {
        ability: ManaAbilityIdentity::Printed { index },
    };
    for (third, holds) in [("Island", false), ("Forest", true)] {
        let prepared = board(&[(ATLAS, CONDITIONED), (SCEPTRE, CONDITIONED)])
            .permanent_or_hand("Endless Atlas", 0, 0, false)
            .permanent_or_hand("Sceptre of Eternal Glory", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand(third, 0, 0, false)
            .permanent_or_hand("Sol Ring", 0, 0, false)
            .libraries("Forest", 3)
            .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        let [atlas, sceptre] = [0, 1].map(|i| Driver::object(&g, i));
        assert_eq!(actions(&mut g).contains(&activate(atlas, 0)), holds);
        let mut f = g.fork();
        let answer = f.next_test_decision().activate_mana(sceptre);
        Driver::answer(&mut f, u64::MAX, answer);
        let d = Driver::request(&mut f, u64::MAX);
        let expected: Vec<_> = if holds {
            vec![printed(0), printed(1)]
        } else {
            vec![printed(0)]
        };
        assert_eq!(d.candidates, expected, "{d:?}");
        if holds {
            Driver::action(&mut g, activate(atlas, 0));
            Driver::pay(&mut g, &[5]);
            assert_eq!(evidence(&g), Some(vec![3]));
            assert!(forge(&g, vec![2]).validate_state().is_err());
            checked(&g);
            settle(&mut g);
        }
    }
}

/// Tectonic Edge: "Activate only if an opponent controls four or more
/// lands." Weathered Wayfarer: "Activate only if an opponent controls more
/// lands than you." Each reads one opponent's lands, never the sum over
/// opponents (CR 102.2-102.3).
#[test]
fn opponent_land_conditions_read_one_opponent() {
    // (lands of seat 1, lands of seat 2, Edge, Wayfarer)
    for (one, two, edge_ok, wayfarer_ok) in [
        (2, 2, false, false),
        (3, 0, false, true),
        (4, 1, true, true),
    ] {
        let mut b = board(&[(EDGE, CONDITIONED), (WAYFARER, CONDITIONED)])
            .seats(3)
            .permanent_or_hand("Tectonic Edge", 0, 0, false)
            .permanent_or_hand("Weathered Wayfarer", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Command Tower", 1, 1, false);
        for _ in 1..one {
            b = b.permanent_or_hand("Forest", 1, 1, false);
        }
        for _ in 0..two {
            b = b.permanent_or_hand("Forest", 2, 2, false);
        }
        let g = Game::with_trace(b.libraries("Forest", 3).prepare(), TraceMode::Detailed);
        let [edge, wayfarer] = [0, 1].map(|i| Driver::object(&g, i));
        assert_eq!(may(&g, 0, edge, 0), edge_ok, "{one} {two}");
        assert_eq!(may(&g, 0, wayfarer, 0), wayfarer_ok, "{one} {two}");
        checked(&g);
    }
}

/// Speaker of the Heavens: "Activate only if you have at least 7 life more
/// than your starting life total and only as a sorcery." (CR 103.4, 307.5)
#[test]
fn speaker_of_the_heavens_reads_life_above_the_starting_total() {
    for (step, above, holds) in [
        (TurnStep::PrecombatMain, 6, false),
        (TurnStep::PrecombatMain, 7, true),
        (TurnStep::Upkeep, 9, false),
    ] {
        let prepared = board(&[(SPEAKER, CONDITIONED)])
            .step(step)
            .permanent_or_hand("Speaker of the Heavens", 0, 0, false)
            .prepare();
        let mut g = Game::with_trace(prepared, TraceMode::Detailed);
        let speaker = Driver::object(&g, 0);
        let life = plus(&g.state.rules.players[0].life, above);
        g.state.rules.players[0].life = life;
        assert_eq!(may(&g, 0, speaker, 0), holds, "{step:?} {above}");
    }
}

/// Xerex Strobe-Knight: "Activate only if you've cast two or more spells
/// this turn." Potioner's Trove: "Activate only if you've cast an instant or
/// sorcery spell this turn." A creature spell counts only for the Knight
/// (CR 601.2i).
#[test]
fn spell_count_conditions_read_this_turns_casts() {
    let prepared = board(&[(XEREX, CONDITIONED), (TROVE, CONDITIONED)])
        .permanent_or_hand("Xerex Strobe-Knight", 0, 0, false)
        .permanent_or_hand("Potioner's Trove", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 0, 0, true)
        .permanent_or_hand("Lightning Bolt", 0, 0, true)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Forest", 0, 0, false)
        .permanent_or_hand("Mountain", 0, 0, false)
        .permanent_or_hand("Grizzly Bears", 1, 1, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let [xerex, trove] = [0, 1].map(|i| Driver::object(&g, i));
    let theirs = Driver::object(&g, 7);
    assert!(!may(&g, 0, xerex, 0) && !may(&g, 0, trove, 0));
    cast_and_resolve(&mut g, 2, None, &[4, 5]);
    assert!(!may(&g, 0, xerex, 0) && !may(&g, 0, trove, 0));
    cast_and_resolve(&mut g, 3, Some(theirs), &[6]);
    assert!(may(&g, 0, xerex, 0) && may(&g, 0, trove, 0));
    Driver::action(&mut g, activate(xerex, 0));
    Driver::request(&mut g, u64::MAX);
    assert_eq!(evidence(&g), Some(vec![2]));
    assert!(forge(&g, vec![1]).validate_state().is_err());
    checked(&g);
    settle(&mut g);
}

/// Stallion of Ashmouth: "Delirium — Activate only if there are four or
/// more card types among cards in your graveyard." (CR 205.2a)
#[test]
fn stallion_of_ashmouth_counts_card_types_in_its_graveyard() {
    for artifact in [false, true] {
        let grave = Zone::Graveyard { owner: PlayerId(0) };
        let mut b = board(&[(STALLION, CONDITIONED)])
            .permanent_or_hand("Stallion of Ashmouth", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .card("Forest", 0, 0, grave)
            .card("Lightning Bolt", 0, 0, grave)
            .card("Grizzly Bears", 0, 0, grave)
            // Another player's artifact is not in your graveyard.
            .card("Sol Ring", 1, 1, Zone::Graveyard { owner: PlayerId(1) });
        if artifact {
            b = b.card("Sol Ring", 0, 0, grave);
        }
        let mut g = Game::with_trace(b.prepare(), TraceMode::Detailed);
        let stallion = Driver::object(&g, 0);
        assert_eq!(actions(&mut g).contains(&activate(stallion, 0)), artifact);
        if artifact {
            Driver::action(&mut g, activate(stallion, 0));
            Driver::pay(&mut g, &[1, 2]);
            assert_eq!(evidence(&g), Some(vec![4]));
            for counts in [vec![3], vec![10]] {
                assert!(forge(&g, counts).validate_state().is_err());
            }
            checked(&g);
            settle(&mut g);
        }
    }
}

const PIT_IMP: &str = record!("3decbc45-3e79-455f-9d62-878ee31b458d");

/// Pit Imp: "{B}: This creature gets +1/+0 until end of turn. Activate no
/// more than twice each turn." Two completed activations use the limit
/// (CR 602.5b); the uses survive a round trip, forged or gapped uses are
/// rejected, and the Imp returned to hand and recast is a new object with
/// a fresh limit (CR 400.7).
#[test]
fn pit_imp_activates_at_most_twice_per_object_each_turn() {
    let prepared = board(&[(PIT_IMP, CONDITIONED)])
        .permanent_or_hand("Pit Imp", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Swamp", 0, 0, false)
        .permanent_or_hand("Unsummon", 0, 0, true)
        .permanent_or_hand("Island", 0, 0, false)
        .prepare();
    let mut g = Game::with_trace(prepared, TraceMode::Detailed);
    let imp = Driver::object(&g, 0);
    for swamp in [1, 2] {
        assert!(actions(&mut g).contains(&activate(imp, 0)));
        Driver::action(&mut g, activate(imp, 0));
        Driver::pay(&mut g, &[swamp]);
        settle(&mut g);
    }
    assert_eq!(g.state.rules.activation_uses.len(), 2);
    assert!(!actions(&mut g).contains(&activate(imp, 0)));
    assert!(!may(&g, 0, imp, 0));
    // Pit Imp is 0/1: two resolved pumps.
    assert_eq!(g.characteristics(imp).unwrap().power, Some(2.into()));
    // A third use, or a second use without its first, is rejected.
    let used = *g.state.rules.activation_uses.iter().next().unwrap();
    let mut f = g.fork();
    f.state
        .rules
        .activation_uses
        .insert(stack_costs::ActivationUse { nth: 2, ..used });
    assert!(f.validate_state().is_err());
    let mut f = g.fork();
    f.state.rules.activation_uses.retain(|k| k.nth != 0);
    assert!(f.validate_state().is_err());
    // CR 400.7: bounced and recast, the Imp is a new object.
    cast_and_resolve(&mut g, 5, Some(imp), &[6]);
    assert!(g.state.rules.activation_uses.is_empty());
    let card = imp.card;
    Driver::cast(&mut g, card);
    Driver::pay(&mut g, &[3]);
    settle(&mut g);
    let again = g.object_id(card).unwrap();
    assert_ne!(again, imp);
    assert!(actions(&mut g).contains(&activate(again, 0)));
}

const BEAR: &str = record!("2f6f49a4-f2ea-4147-a6d3-69cf875c302e");
const COMPOUND: &str = record!("99c70f4e-de8a-426d-99aa-17b2f87625ba");
const KEEPER: &str = record!("434e82b5-abd4-41ac-a5b1-2dd01c667374");

/// Dragon-Scarred Bear: "Formidable — {1}{G}: Regenerate this creature.
/// Activate only if creatures you control have total power 8 or greater."
/// Only your creatures count (CR 208.1); 3 + 2 + 2 is seven, and a
/// Colossal Dreadmaw makes it thirteen.
#[test]
fn formidable_reads_the_total_power_you_control() {
    for dreadmaw in [false, true] {
        let mut b = board(&[(BEAR, CONDITIONED)])
            .permanent_or_hand("Dragon-Scarred Bear", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .permanent_or_hand("Colossal Dreadmaw", 1, 1, false);
        if dreadmaw {
            b = b.permanent_or_hand("Colossal Dreadmaw", 0, 0, false);
        }
        let mut g = Game::with_trace(b.prepare(), TraceMode::Detailed);
        let bear = Driver::object(&g, 0);
        assert_eq!(actions(&mut g).contains(&activate(bear, 0)), dreadmaw);
        if dreadmaw {
            Driver::action(&mut g, activate(bear, 0));
            Driver::pay(&mut g, &[1, 2]);
            assert_eq!(evidence(&g), Some(vec![13]));
            assert!(forge(&g, vec![7]).validate_state().is_err());
            checked(&g);
            settle(&mut g);
        }
    }
}

/// Training Compound: "{T}: Add {R} or {G}. Activate only if this land
/// entered this turn or if you control a basic land." The land played this
/// turn qualifies on its own; one on the battlefield since before needs a
/// basic land (CR 400.7, 605.3a).
#[test]
fn training_compound_reads_its_own_entry_or_a_basic_land() {
    let colored = Candidate::ManaAbility {
        ability: ManaAbilityIdentity::Printed { index: 1 },
    };
    let offered = |g: &Game, land: ObjectId| {
        let mut f = g.fork();
        let answer = f.next_test_decision().activate_mana(land);
        Driver::answer(&mut f, u64::MAX, answer);
        Driver::request(&mut f, u64::MAX)
            .candidates
            .contains(&colored)
    };
    for (in_hand, basic, holds) in [
        (false, false, false),
        (false, true, true),
        (true, false, true),
    ] {
        let mut b = board(&[(COMPOUND, CONDITIONED)])
            .permanent_or_hand("Training Compound", 0, 0, in_hand)
            .permanent_or_hand("Sol Ring", 0, 0, false);
        if basic {
            b = b.permanent_or_hand("Mountain", 0, 0, false);
        }
        let mut g = Game::with_trace(b.prepare(), TraceMode::Detailed);
        let mut land = Driver::object(&g, 0);
        if in_hand {
            Driver::action(&mut g, Action::PlayLand { object: land });
            Driver::request(&mut g, u64::MAX);
            land = g.object_id(land.card).unwrap();
        }
        assert_eq!(offered(&g, land), holds, "{in_hand} {basic}");
        checked(&g);
    }
}

/// Bloodline Keeper: "{B}: Transform this creature. Activate only if you
/// control five or more Vampires." The Keeper counts itself.
#[test]
fn bloodline_keeper_counts_five_vampires() {
    for nighthawks in [3usize, 4] {
        let mut b = board(&[(KEEPER, CONDITIONED)])
            .permanent_or_hand("Bloodline Keeper", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Vampire Nighthawk", 1, 1, false);
        for _ in 0..nighthawks {
            b = b.permanent_or_hand("Vampire Nighthawk", 0, 0, false);
        }
        let g = Game::with_trace(b.prepare(), TraceMode::Detailed);
        let keeper = Driver::object(&g, 0);
        // Ability 0 makes a Vampire; ability 1 transforms.
        assert_eq!(may(&g, 0, keeper, 1), nighthawks == 4);
    }
}
