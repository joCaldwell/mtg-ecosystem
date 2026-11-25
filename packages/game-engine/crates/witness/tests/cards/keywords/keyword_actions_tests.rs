//! CR 701.14, 701.16, 701.34, 701.36, 704.5c, 707.2.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_core::testkit::*;

macro_rules! record {
    ($id:literal) => {
        include_str!(concat!(
            "../../../../../tests/fixtures/phase1-r4/",
            $id,
            ".json"
        ))
    };
}

const PREY_UPON: &str = record!("b3c12848-528d-4005-878f-4f950a07240a");
const RUTHLESS_PREDATION: &str = record!("478e59b8-03ad-4f8e-8c18-6ab5ad77a6f8");
const THRABEN_INSPECTOR: &str = record!("caa02547-66e3-4e27-a2d3-5e94f3e7a069");
const EYES_IN_THE_SKIES: &str = record!("9697cd05-474e-46f3-8bcc-d4b6fb2059a1");
const WAKE_THE_REFLECTIONS: &str = record!("211868ca-662b-4054-bf50-c9e16bb58c49");
const PROLOGUE_TO_PHYRESIS: &str = record!("3f661b58-e8ca-412c-bdac-af151162de12");
const CONTENTIOUS_PLAN: &str = record!("2fc6de19-9d04-422b-90bc-eee2f9f8c84c");
const TUNE_THE_NARRATIVE: &str = record!("4262a726-d1da-4bdc-ab14-73212aa83f9c");
const TITANIAS_BOON: &str = record!("0bf4c01d-3cb0-47be-b84a-6a399bc2d3fc");
const KARN_S_BASTION: &str = record!("9fb8cd81-403a-4988-8f1c-b8eccf8abd9c");
const PIT_FIGHT: &str = record!("ef4b0ced-70a1-41d8-9aed-34e75f979c5f");
const CONSULATE_SURVEILLANCE: &str = record!("06d62e22-df9c-488e-8d2d-793eaf4cd75c");

fn compile(raw: &str) -> mtg_compiler::CompileResult {
    mtg_witness::compile(mtg_witness::R4_MANIFEST, raw)
}

fn board(records: &[&str]) -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for raw in records {
        let result = compile(raw);
        let mtg_compiler::CompileResult::Lowered { definition } = result else {
            panic!("frozen keyword action witness failed: {result:?}");
        };
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Board::new(catalog, "r4-keyword-actions")
        .step(TurnStep::PrecombatMain)
        .libraries("Forest", 20)
}

fn add(board: Board, name: &str, player: u8, hand: bool) -> Board {
    board.permanent_or_hand(name, player, player, hand)
}

fn card(game: &Game, name: &str, player: u8, hand: bool) -> ObjectId {
    let zone = if hand {
        Zone::Hand {
            owner: PlayerId(player),
        }
    } else {
        Zone::Battlefield
    };
    game.state
        .rules
        .objects
        .values()
        .find(|object| {
            object.zone == zone
                && object.controller == PlayerId(player)
                && game
                    .copiable_definition(object.id)
                    .is_ok_and(|d| d.name == name)
        })
        .unwrap_or_else(|| panic!("missing {name} for seat {player} in {zone:?}"))
        .id
}

fn cast(game: &mut Game, name: &str, player: u8) {
    Driver::cast(game, card(game, name, player, true).card);
}

fn untapped_lands(game: &Game, player: u8, names: &[&str]) -> Vec<u32> {
    let mut chosen = vec![];
    for name in names {
        let land = game
            .state
            .rules
            .objects
            .values()
            .find(|land| {
                land.zone == Zone::Battlefield
                    && land.controller == PlayerId(player)
                    && !land.tapped
                    && !chosen.contains(&land.id.card.0)
                    && game
                        .copiable_definition(land.id)
                        .is_ok_and(|d| d.name == *name)
            })
            .unwrap_or_else(|| panic!("missing untapped {name} for seat {player}"));
        chosen.push(land.id.card.0);
    }
    chosen
}

fn pay(game: &mut Game, player: u8, names: &[&str]) {
    let lands = untapped_lands(game, player, names);
    Driver::pay(game, &lands);
}

fn empty_stack(game: &mut Game) {
    for _ in 0..24 {
        let request = Driver::request(game, 1);
        assert_eq!(request.reason, Reason::Priority, "{request:?}");
        if game.state.rules.stack.is_empty() {
            return;
        }
        Driver::pass(game);
    }
    panic!("expected empty stack priority");
}

fn choice(game: &mut Game, expected: Reason<Rules>) -> Decision<Rules> {
    for _ in 0..24 {
        let request = Driver::request(game, 1);
        if request.reason == expected {
            return request;
        }
        assert_eq!(request.reason, Reason::Priority, "{request:?}");
        Driver::pass(game);
    }
    panic!("expected {expected:?}");
}

fn pick(game: &mut Game, decision: &Decision<Rules>, candidates: &[Candidate<Rules>]) {
    let picks = vec![
        candidates
            .iter()
            .map(|c| decision.index(c).unwrap())
            .collect(),
    ];
    game.respond(decision.player, decision.id, Answer::Pick { picks })
        .unwrap();
}

fn checked(game: &Game) {
    game.validate_state().unwrap();
    let raw = game.snapshot().unwrap();
    let restored = Game::restore(game.prepared.as_ref().clone(), &raw).unwrap();
    assert_eq!(restored.snapshot().unwrap(), raw);
}

fn reject(game: &Game, tamper: impl FnOnce(&mut serde_json::Value), expected: &str) {
    let mut forged: serde_json::Value = serde_json::from_str(&game.snapshot().unwrap()).unwrap();
    tamper(&mut forged["state"]);
    forged["state_sha256"] = digest(&forged["state"]).unwrap().into();
    let err = Game::restore(game.prepared.as_ref().clone(), &forged.to_string())
        .err()
        .expect("forged state restored");
    assert!(err.contains(expected), "{err}");
}

fn fight_game(spell: &str, record: &str, escape: bool) -> Game {
    let mut scenario = board(&[record]);
    for (name, seat, hand) in [
        (spell, 0, true),
        ("Forest", 0, false),
        ("Forest", 0, false),
        ("Forest", 0, false),
        ("Grizzly Bears", 0, false),
        ("Grizzly Bears", 1, false),
    ] {
        scenario = add(scenario, name, seat, hand);
    }
    if escape {
        scenario = add(scenario, "Unsummon", 0, true);
        scenario = add(scenario, "Island", 0, false);
    }
    Game::new(scenario.prepare())
}

fn announce_fight(game: &mut Game, spell: &str) -> (ObjectId, ObjectId) {
    let mine = card(game, "Grizzly Bears", 0, false);
    let theirs = card(game, "Grizzly Bears", 1, false);
    cast(game, spell, 0);
    let decision = Driver::request(game, 1);
    assert!(matches!(decision.reason, Reason::Targets { .. }));
    let entry = game.top().unwrap();
    assert!(game.object_target_legal(entry, 0, mine));
    assert!(game.object_target_legal(entry, 1, theirs));
    assert!(!game.object_target_legal(entry, 0, theirs));
    assert!(!game.object_target_legal(entry, 1, mine));
    let own = decision.index(&Candidate::Object { object: mine }).unwrap();
    let opposing = decision
        .index(&Candidate::Object { object: theirs })
        .unwrap();
    assert!(
        decision
            .shape
            .check(&Answer::Pick {
                picks: vec![vec![opposing], vec![own]]
            })
            .is_err()
    );
    game.respond(
        decision.player,
        decision.id,
        Answer::Pick {
            picks: vec![vec![own], vec![opposing]],
        },
    )
    .unwrap();
    (mine, theirs)
}

#[test]
fn fight_both_creatures_deal_damage_and_both_die() {
    let mut game = fight_game("Prey Upon", PREY_UPON, false);
    let (mine, theirs) = announce_fight(&mut game, "Prey Upon");
    pay(&mut game, 0, &["Forest"]);
    empty_stack(&mut game);
    for object in [mine, theirs] {
        assert!(matches!(
            game.state.rules.objects[&object.card].zone,
            Zone::Graveyard { .. }
        ));
    }
    checked(&game);
}

#[test]
fn fight_resolves_on_the_front_of_a_modal_double_faced_card() {
    let mut game = fight_game(
        "Khalni Ambush",
        record!("37a55560-6e32-4f54-b9a8-fd157aea6eb5"),
        false,
    );
    let (mine, theirs) = announce_fight(&mut game, "Khalni Ambush");
    pay(&mut game, 0, &["Forest", "Forest", "Forest"]);
    empty_stack(&mut game);
    for object in [mine, theirs] {
        assert!(matches!(
            game.state.rules.objects[&object.card].zone,
            Zone::Graveyard { .. }
        ));
    }
    checked(&game);
}

#[test]
fn fight_uses_power_after_the_preceding_effect_and_rejects_forged_power() {
    let mut game = fight_game("Ruthless Predation", RUTHLESS_PREDATION, false);
    let (mine, theirs) = announce_fight(&mut game, "Ruthless Predation");
    pay(&mut game, 0, &["Forest", "Forest"]);
    for _ in 0..game.live_players().len() {
        Driver::pass(&mut game);
    }
    for _ in 0..100 {
        if matches!(game.state.rules.frame, Frame::DamageEffects { .. }) {
            break;
        }
        assert!(matches!(
            game.advance(Budget::new(1)),
            Advance::Suspended { .. }
        ));
    }
    assert!(matches!(
        game.state.rules.frame,
        Frame::DamageEffects { .. }
    ));
    checked(&game);
    reject(
        &game,
        |state| {
            state["frame"]["transaction"]["entries"][0]["original_amount"] = "9".into();
        },
        "unbound fight damage",
    );
    empty_stack(&mut game);
    assert!(matches!(
        game.state.rules.objects[&theirs.card].zone,
        Zone::Graveyard { .. }
    ));
    assert_eq!(
        game.state.rules.objects[&mine.card].marked_damage,
        Quantity::from(2)
    );
    assert_eq!(
        game.characteristics(mine).unwrap().toughness,
        Some(4.into())
    );
    checked(&game);
}

#[test]
fn fight_does_nothing_if_one_participant_changes_zone() {
    let mut game = fight_game("Prey Upon", PREY_UPON, true);
    let (mine, theirs) = announce_fight(&mut game, "Prey Upon");
    pay(&mut game, 0, &["Forest"]);
    cast(&mut game, "Unsummon", 0);
    Driver::pick_object(&mut game, theirs);
    pay(&mut game, 0, &["Island"]);
    empty_stack(&mut game);
    assert_eq!(
        game.state.rules.objects[&mine.card].marked_damage,
        Quantity::zero()
    );
    assert_eq!(
        game.state.rules.objects[&theirs.card].zone,
        Zone::Hand { owner: PlayerId(1) }
    );
    checked(&game);
}

#[test]
fn investigate_clue_activation_sacrifices_the_artifact_and_draws() {
    let mut scenario = board(&[THRABEN_INSPECTOR]);
    scenario = add(scenario, "Thraben Inspector", 0, true);
    for _ in 0..3 {
        scenario = add(scenario, "Plains", 0, false);
    }
    let mut game = Game::new(scenario.prepare());
    cast(&mut game, "Thraben Inspector", 0);
    pay(&mut game, 0, &["Plains"]);
    empty_stack(&mut game);
    let clue = card(&game, "Clue Token", 0, false);
    let library_before = game.state.rules.libraries[&PlayerId(0)].len();
    Driver::action(
        &mut game,
        Action::ActivateAbility {
            object: clue,
            ability: 0,
        },
    );
    pay(&mut game, 0, &["Plains", "Plains"]);
    empty_stack(&mut game);
    assert!(game.object_id(clue.card).is_none());
    assert_eq!(
        game.state.rules.libraries[&PlayerId(0)].len(),
        library_before - 1
    );
    checked(&game);
}

#[test]
fn populate_copies_the_chosen_token_including_flying_without_temporary_stats() {
    let mut scenario = board(&[EYES_IN_THE_SKIES, WAKE_THE_REFLECTIONS]);
    for (name, hand) in [
        ("Eyes in the Skies", true),
        ("Wake the Reflections", true),
        ("Wake the Reflections", true),
        ("Giant Growth", true),
        ("Grizzly Bears", false),
        ("Forest", false),
    ] {
        scenario = add(scenario, name, 0, hand);
    }
    for _ in 0..6 {
        scenario = add(scenario, "Plains", 0, false);
    }
    let mut game = Game::new(scenario.prepare());
    cast(&mut game, "Wake the Reflections", 0);
    pay(&mut game, 0, &["Plains"]);
    empty_stack(&mut game);
    assert!(game.state.rules.tokens.is_empty());

    cast(&mut game, "Eyes in the Skies", 0);
    pay(&mut game, 0, &["Plains", "Plains", "Plains", "Plains"]);
    let decision = choice(&mut game, Reason::Populate);
    let [Candidate::Object { object: first }] = decision.candidates.as_slice() else {
        panic!("expected only the newly created Bird: {decision:?}");
    };
    let first = *first;
    checked(&game);
    pick(&mut game, &decision, &[Candidate::Object { object: first }]);
    empty_stack(&mut game);
    assert_eq!(game.state.rules.tokens.len(), 2);

    cast(&mut game, "Giant Growth", 0);
    Driver::pick_object(&mut game, first);
    pay(&mut game, 0, &["Forest"]);
    empty_stack(&mut game);
    assert_eq!(game.characteristics(first).unwrap().power, Some(4.into()));

    cast(&mut game, "Wake the Reflections", 0);
    pay(&mut game, 0, &["Plains"]);
    let decision = choice(&mut game, Reason::Populate);
    assert_eq!(decision.candidates.len(), 2);
    pick(&mut game, &decision, &[Candidate::Object { object: first }]);
    empty_stack(&mut game);
    assert_eq!(game.state.rules.tokens.len(), 3);
    let copied = game.state.rules.objects[game.state.rules.tokens.last_key_value().unwrap().0].id;
    assert_eq!(game.characteristics(copied).unwrap().power, Some(1.into()));
    assert!(
        game.current_keywords(copied)
            .contains(&KeywordAbility::Flying)
    );
    checked(&game);

    reject(
        &game,
        |state| {
            state["tokens"][copied.card.0.to_string()]["definition"]["name"] = "Forged Bird".into();
        },
        "token characteristic identity",
    );
    reject(
        &game,
        |state| {
            state["tokens"][copied.card.0.to_string()]["spec"]["original"]["card"] =
                copied.card.0.into();
        },
        "token copy provenance",
    );
    reject(
        &game,
        |state| {
            state["tokens"][copied.card.0.to_string()]["spec"]["original"]["face"] = "back".into();
        },
        "token copy provenance",
    );
}

#[test]
fn proliferate_selects_permanents_and_players_and_adds_every_existing_kind() {
    let mut scenario = board(&[
        PROLOGUE_TO_PHYRESIS,
        CONTENTIOUS_PLAN,
        TUNE_THE_NARRATIVE,
        TITANIAS_BOON,
    ]);
    for (name, seat, hand) in [
        ("Prologue to Phyresis", 0, true),
        ("Tune the Narrative", 1, true),
        ("Titania's Boon", 0, true),
        ("Contentious Plan", 0, true),
        ("Grizzly Bears", 0, false),
        ("Grizzly Bears", 1, false),
        ("Island", 1, false),
    ] {
        scenario = add(scenario, name, seat, hand);
    }
    for _ in 0..4 {
        scenario = add(scenario, "Island", 0, false);
        scenario = add(scenario, "Forest", 0, false);
    }
    let mut game = Game::new(scenario.prepare());
    cast(&mut game, "Prologue to Phyresis", 0);
    pay(&mut game, 0, &["Island", "Island"]);
    Driver::pass(&mut game);
    cast(&mut game, "Tune the Narrative", 1);
    pay(&mut game, 1, &["Island"]);
    empty_stack(&mut game);

    cast(&mut game, "Titania's Boon", 0);
    pay(&mut game, 0, &["Forest", "Forest", "Forest", "Forest"]);
    empty_stack(&mut game);
    let permanent = card(&game, "Grizzly Bears", 0, false);
    let no_counters = card(&game, "Grizzly Bears", 1, false);

    cast(&mut game, "Contentious Plan", 0);
    pay(&mut game, 0, &["Island", "Island"]);
    let decision = choice(&mut game, Reason::Proliferate);
    assert_eq!(
        decision.candidates,
        vec![
            Candidate::Object { object: permanent },
            Candidate::Player {
                player: PlayerId(1)
            },
            Candidate::Player {
                player: PlayerId(2)
            },
            Candidate::Player {
                player: PlayerId(3)
            },
        ]
    );
    decision
        .shape
        .check(&Answer::Pick {
            picks: vec![vec![]],
        })
        .unwrap();
    checked(&game);
    let draw_before = game.state.rules.libraries[&PlayerId(0)].len();
    pick(
        &mut game,
        &decision,
        &[
            Candidate::Object { object: permanent },
            Candidate::Player {
                player: PlayerId(1),
            },
        ],
    );
    checked(&game);
    reject(
        &game,
        |state| {
            state["pending"]["decision"]["candidates"][0]["object"] =
                serde_json::to_value(no_counters).unwrap();
        },
        "selected candidate no longer offered",
    );
    empty_stack(&mut game);
    assert_eq!(
        game.state.rules.objects[&permanent.card].plus_one_counters,
        2.into()
    );
    assert_eq!(
        game.characteristics(permanent).unwrap().power,
        Some(4.into())
    );
    assert_eq!(game.state.rules.players[1].poison_counters, 2.into());
    assert_eq!(game.state.rules.players[1].energy_counters, 3.into());
    assert_eq!(game.state.rules.players[2].poison_counters, 1.into());
    assert_eq!(
        game.state.rules.libraries[&PlayerId(0)].len(),
        draw_before - 1
    );
    checked(&game);
    reject(
        &game,
        |state| {
            state["players"][1]["energy_counters"] = "-1".into();
        },
        "player counter quantity",
    );
}

#[test]
fn proliferating_the_tenth_poison_counter_causes_a_state_based_loss() {
    let mut scenario = add(
        board(&[PROLOGUE_TO_PHYRESIS, CONTENTIOUS_PLAN]),
        "Contentious Plan",
        0,
        true,
    );
    for _ in 0..9 {
        scenario = add(scenario, "Prologue to Phyresis", 0, true);
    }
    for _ in 0..20 {
        scenario = add(scenario, "Island", 0, false);
    }
    let mut game = Game::new(scenario.prepare());
    for _ in 0..9 {
        cast(&mut game, "Prologue to Phyresis", 0);
        pay(&mut game, 0, &["Island", "Island"]);
        empty_stack(&mut game);
    }
    assert_eq!(game.state.rules.players[1].poison_counters, 9.into());
    assert_eq!(game.state.rules.players[2].poison_counters, 9.into());
    cast(&mut game, "Contentious Plan", 0);
    pay(&mut game, 0, &["Island", "Island"]);
    let decision = choice(&mut game, Reason::Proliferate);
    pick(
        &mut game,
        &decision,
        &[Candidate::Player {
            player: PlayerId(1),
        }],
    );
    empty_stack(&mut game);
    assert_eq!(
        game.live_players(),
        vec![PlayerId(0), PlayerId(2), PlayerId(3)]
    );
    assert_eq!(game.state.rules.players[2].poison_counters, 9.into());
    checked(&game);
}

#[test]
fn keyword_action_allowlist_preserves_residual_clauses() {
    assert!(matches!(
        compile(KARN_S_BASTION),
        mtg_compiler::CompileResult::Lowered { .. }
    ));
    for raw in [PIT_FIGHT, CONSULATE_SURVEILLANCE] {
        assert!(matches!(
            compile(raw),
            mtg_compiler::CompileResult::Unsupported { .. }
        ));
    }
}

/// CR 122.1a, 613.4c, 701.34a: the counter stays on a land after its
/// animation wears off, and proliferate can still choose the land.
#[test]
fn proliferate_can_choose_an_unanimated_permanent_with_a_counter() {
    let mut scenario = board(&[
        record!("09a1ab1b-d9f0-4a2a-a448-3190f85006e4"), // Natural Affinity
        record!("1ba59349-a162-4444-b3e0-cddfc01ef03b"), // Rubblebelt Maverick
        KARN_S_BASTION,
    ])
    .card(
        "Rubblebelt Maverick",
        0,
        0,
        Zone::Graveyard { owner: PlayerId(0) },
    );
    scenario = add(scenario, "Natural Affinity", 0, true);
    scenario = add(scenario, "Karn's Bastion", 0, false);
    for _ in 0..8 {
        scenario = add(scenario, "Forest", 0, false);
    }
    let mut game = Game::new(scenario.prepare());
    let land = card(&game, "Forest", 0, false);
    let maverick = game
        .state
        .rules
        .objects
        .values()
        .find(|o| o.zone == (Zone::Graveyard { owner: PlayerId(0) }))
        .unwrap()
        .id;
    cast(&mut game, "Natural Affinity", 0);
    pay(&mut game, 0, &["Forest", "Forest", "Forest"]);
    empty_stack(&mut game);
    Driver::action(
        &mut game,
        Action::ActivateAbility {
            object: maverick,
            ability: 0,
        },
    );
    Driver::pick_object(&mut game, land);
    pay(&mut game, 0, &["Forest"]);
    empty_stack(&mut game);
    assert_eq!(game.characteristics(land).unwrap().power, Some(3.into()));

    let first_turn = game.state.rules.turn.number;
    for _ in 0..60 {
        let decision = Driver::request(&mut game, 1);
        match decision.reason {
            Reason::Priority
                if game.state.rules.turn.number > first_turn && decision.player == PlayerId(0) =>
            {
                break;
            }
            Reason::Priority => Driver::pass(&mut game),
            Reason::Attackers => {
                let answer = decision.attack(vec![]);
                game.respond(decision.player, decision.id, answer).unwrap();
            }
            reason => panic!("unexpected turn boundary {reason:?}"),
        }
    }
    assert!(game.state.rules.turn.number > first_turn);
    assert_eq!(game.characteristics(land).unwrap().power, None);
    let bastion = card(&game, "Karn's Bastion", 0, false);
    Driver::action(
        &mut game,
        Action::ActivateAbility {
            object: bastion,
            ability: 0,
        },
    );
    pay(&mut game, 0, &["Forest", "Forest", "Forest", "Forest"]);
    let decision = choice(&mut game, Reason::Proliferate);
    assert_eq!(
        decision.candidates,
        vec![Candidate::Object { object: land }]
    );
    pick(&mut game, &decision, &[Candidate::Object { object: land }]);
    empty_stack(&mut game);
    assert_eq!(game.characteristics(land).unwrap().power, None);
    assert_eq!(
        game.state.rules.objects[&land.card].plus_one_counters,
        2.into()
    );
    checked(&game);
}

/// CR 701.36, 707.2: an animated Clue can be chosen. The resulting Clue
/// retains its printed activated ability and enters as a noncreature.
#[test]
fn populate_copies_an_animated_clues_artifact_values() {
    let mut scenario = board(&[
        THRABEN_INSPECTOR,
        WAKE_THE_REFLECTIONS,
        record!("4acacdd2-a736-43a2-ad67-891b9bd124f8"), // Alloy Animist
    ]);
    for (name, hand) in [
        ("Thraben Inspector", true),
        ("Wake the Reflections", true),
        ("Alloy Animist", false),
    ] {
        scenario = add(scenario, name, 0, hand);
    }
    for _ in 0..4 {
        scenario = add(scenario, "Plains", 0, false);
    }
    for _ in 0..3 {
        scenario = add(scenario, "Forest", 0, false);
    }
    let mut game = Game::new(scenario.prepare());
    cast(&mut game, "Thraben Inspector", 0);
    pay(&mut game, 0, &["Plains"]);
    empty_stack(&mut game);
    let original = card(&game, "Clue Token", 0, false);
    let animist = card(&game, "Alloy Animist", 0, false);
    Driver::action(
        &mut game,
        Action::ActivateAbility {
            object: animist,
            ability: 0,
        },
    );
    Driver::pick_object(&mut game, original);
    pay(&mut game, 0, &["Forest", "Forest", "Forest"]);
    empty_stack(&mut game);
    assert_eq!(
        game.characteristics(original).unwrap().power,
        Some(4.into())
    );

    cast(&mut game, "Wake the Reflections", 0);
    pay(&mut game, 0, &["Plains"]);
    let decision = choice(&mut game, Reason::Populate);
    assert_eq!(
        decision.candidates,
        vec![Candidate::Object { object: original }]
    );
    pick(
        &mut game,
        &decision,
        &[Candidate::Object { object: original }],
    );
    empty_stack(&mut game);
    let copied = game.state.rules.objects[game.state.rules.tokens.last_key_value().unwrap().0].id;
    let printed = game.characteristics(copied).unwrap();
    assert_eq!(printed.kind, CardKind::Artifact);
    assert_eq!(printed.power, None);
    checked(&game);

    let library_before = game.state.rules.libraries[&PlayerId(0)].len();
    Driver::action(
        &mut game,
        Action::ActivateAbility {
            object: copied,
            ability: 0,
        },
    );
    pay(&mut game, 0, &["Plains", "Plains"]);
    empty_stack(&mut game);
    assert!(game.object_id(copied.card).is_none());
    assert_eq!(
        game.state.rules.libraries[&PlayerId(0)].len(),
        library_before - 1
    );
    assert_eq!(
        game.characteristics(original).unwrap().power,
        Some(4.into())
    );
    checked(&game);
}
