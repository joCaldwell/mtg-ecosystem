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
const ACT_OF_TREASON: &str = record!("9d08af23-9f4a-4097-9abc-3b17475ab744");
const KEIGA: &str = record!("521b1598-58c6-47a0-a7b3-021d70bf2b05");
const HARMLESS_OFFERING: &str = record!("f5d32e5f-066b-42cf-8dd1-072a96275313");
const INVOKE_THE_WINDS: &str = record!("721c3255-953c-49cd-a5dd-852c3d09eae4");
const SOWER: &str = record!("5881aac2-4f31-45cb-bdb5-64a29ec23316");
const DEATH_WIND: &str = record!("a636b62a-340b-444d-9161-f6d3367b745c");
const AURA_THIEF: &str = record!("cf0936ea-2741-43a4-ad6a-a1258b54945b");
const NATURAL_AFFINITY: &str = record!("09a1ab1b-d9f0-4a2a-a448-3190f85006e4");
const HYDROFORM: &str = record!("d8b6ea82-da4e-4763-84b1-4caa214ed945");
const DARKSTEEL_BRUTE: &str = record!("4f7d2774-2aec-4992-b35b-e03265e517af");
const CRIMSON_WISPS: &str = record!("e699d140-eb13-4f15-aecb-85c76e8b1b14");
const TURTLE_DUCK: &str = record!("e93782e3-1b93-459a-a7de-06c3b6591e6f");
const CELESTIAL_COLONNADE: &str = record!("876ac6f6-74de-4666-84c6-83d81f054723");
const DARKEST_HOUR: &str = record!("5667376e-e59c-4b17-b096-5d92cdfe3db1");
const THRAN_LENS: &str = record!("0ac723bf-ce86-4028-a693-0ef498e4c82d");
const LOSE_CALM: &str = record!("73d71731-f694-471c-b2ad-a36854bb06a1");
const RUBBLEBELT_MAVERICK: &str = record!("1ba59349-a162-4444-b3e0-cddfc01ef03b");

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
fn resolved_continuous_shapes_have_cells_and_reject_ability_removal() {
    for (raw, cell) in [
        (ACT_OF_TREASON, Cell::GainControl),
        (KEIGA, Cell::GainControl),
        (INVOKE_THE_WINDS, Cell::GainControl),
        (SOWER, Cell::GainControl),
        (AURA_THIEF, Cell::GainControl),
        (NATURAL_AFFINITY, Cell::SetCharacteristics),
        (HYDROFORM, Cell::SetCharacteristics),
        (DARKSTEEL_BRUTE, Cell::SetCharacteristics),
        (CRIMSON_WISPS, Cell::SetCharacteristics),
        (TURTLE_DUCK, Cell::SetCharacteristics),
        (CELESTIAL_COLONNADE, Cell::SetCharacteristics),
        (DEATH_WIND, Cell::PumpAnnouncedX),
    ] {
        assert!(definition_cells(&lowered(raw)).unwrap().contains(&cell));
    }
    let result = mtg_witness::compile(
        mtg_witness::R4_MANIFEST,
        record!("74c4c1e2-c50d-4c8f-889a-0d5674dc6d67"),
    );
    assert!(
        matches!(result, mtg_compiler::CompileResult::Unsupported { reason, .. }
        if reason.contains("lose-abilities"))
    );
    let result = mtg_witness::compile(mtg_witness::R4_MANIFEST, HARMLESS_OFFERING);
    assert!(
        matches!(result, mtg_compiler::CompileResult::Unsupported { reason, .. }
        if reason.contains("mixed creature/player target domains"))
    );
}

fn cast_on(g: &mut Game, spell: u32, targets: &[TargetEntity], mana: &[u32]) {
    Driver::cast(g, CardId(spell));
    if !targets.is_empty() {
        let r = Driver::request(g, u64::MAX);
        g.respond(r.player, r.id, r.entity_targets(targets.to_vec()))
            .unwrap();
    }
    Driver::pay(g, mana);
    settle(g, &[]);
}

fn next_turn(g: &mut Game) {
    let turn = g.state.rules.turn.number;
    for _ in 0..100 {
        let r = Driver::request(g, u64::MAX);
        if g.state.rules.turn.number > turn {
            return;
        }
        let answer = match r.reason {
            Reason::Priority => mtg_witness::temporary::pass(&r),
            Reason::Attackers => r.attack(vec![]),
            ref reason => panic!("unexpected turn boundary {reason:?}"),
        };
        g.respond(r.player, r.id, answer).unwrap();
    }
    panic!("cleanup did not finish");
}

// CR 302.6, 514.2, 613.7b, 800.4a: cleanup exposes the older grant, and the
// older effect ends when its recipient leaves.
#[test]
fn resolved_continuous_control_order_cleanup_and_departure() {
    let mut g = with(
        board(&[ACT_OF_TREASON, INVOKE_THE_WINDS]),
        &[
            ("Invoke the Winds", 0, true),
            ("Act of Treason", 1, true),
            ("Grizzly Bears", 2, false),
            ("Island", 0, false),
            ("Island", 0, false),
            ("Island", 0, false),
            ("Island", 0, false),
            ("Island", 0, false),
            ("Mountain", 1, false),
            ("Mountain", 1, false),
            ("Mountain", 1, false),
        ],
    );
    let bears = object(&g, 2);
    cast_on(&mut g, 0, &[target(bears)], &[3, 4, 5, 6, 7]);
    assert_eq!(g.current(bears).unwrap().controller, PlayerId(0));
    next_turn(&mut g);
    while g.state.rules.turn.step != TurnStep::PrecombatMain {
        Driver::pass(&mut g);
        Driver::request(&mut g, u64::MAX);
    }
    cast_on(&mut g, 1, &[target(bears)], &[8, 9, 10]);
    assert_eq!(g.current(bears).unwrap().controller, PlayerId(1));
    assert!(!g.current(bears).unwrap().attack_ready);
    assert!(keywords(&g, bears).contains(&KeywordAbility::Haste));
    next_turn(&mut g);
    assert_eq!(g.current(bears).unwrap().controller, PlayerId(0));
    assert!(!keywords(&g, bears).contains(&KeywordAbility::Haste));
    let mut forged = g.fork();
    let effect = forged.state.rules.effects.first_mut().unwrap();
    let ResolvedEffectKind::GainControl { targets, .. } = &mut effect.kind else {
        panic!("persistent grant");
    };
    targets.push(bears);
    assert!(forged.validate_state().is_err());
    g.concede(PlayerId(0)).unwrap();
    Driver::request(&mut g, u64::MAX);
    assert_eq!(g.current(bears).unwrap().controller, PlayerId(2));
    assert!(g.state.rules.effects.is_empty());
    checked(&g);
}

// CR 611.2c, 613.1d-f, 613.4b-c, 613.7b: a later set overwrites base P/T
// on one captured land, an existing pump applies after both, and a new
// land remains outside the mass animation.
#[test]
fn resolved_continuous_animation_captures_and_layers() {
    let mut g = with(
        board(&[NATURAL_AFFINITY, HYDROFORM]),
        &[
            ("Natural Affinity", 0, true),
            ("Hydroform", 0, true),
            ("Giant Growth", 0, true),
            ("Forest", 1, false),
            ("Forest", 0, true),
            ("Forest", 0, false),
            ("Forest", 0, false),
            ("Forest", 0, false),
            ("Forest", 0, false),
            ("Forest", 0, false),
            ("Island", 0, false),
        ],
    );
    let land = object(&g, 3);
    cast_on(&mut g, 0, &[], &[5, 6, 7]);
    assert_eq!(g.characteristics(land).unwrap().power, Some(2.into()));
    cast_on(&mut g, 2, &[target(land)], &[8]);
    cast_on(&mut g, 1, &[target(land)], &[9, 10]);
    let facts = g.characteristics(land).unwrap();
    assert!(facts.has_type(CardKind::Creature) && facts.has_type(CardKind::Land));
    assert_eq!(facts.power, Some(6.into()));
    assert_eq!(facts.toughness, Some(6.into()));
    assert_eq!(facts.subtypes, ["Forest", "Elemental"]);
    assert!(keywords(&g, land).contains(&KeywordAbility::Flying));
    let land_in_hand = object(&g, 4);
    Driver::action(
        &mut g,
        Action::PlayLand {
            object: land_in_hand,
        },
    );
    settle(&mut g, &[]);
    let entrant = object(&g, 4);
    assert!(
        !g.characteristics(entrant)
            .unwrap()
            .has_type(CardKind::Creature)
    );
    next_turn(&mut g);
    assert!(
        !g.characteristics(land)
            .unwrap()
            .has_type(CardKind::Creature)
    );
    assert_eq!(g.characteristics(land).unwrap().power, None);
    checked(&g);
}

// CR 611.2b: Sower must still be present when its trigger resolves, and
// the first departure ends its grant even when the target stays.
#[test]
fn resolved_continuous_source_duration_ends_when_the_source_leaves() {
    let mut g = with(
        board(&[SOWER]),
        &[
            ("Sower of Temptation", 0, true),
            ("Grizzly Bears", 1, false),
            ("Unsummon", 0, true),
            ("Island", 0, false),
            ("Island", 0, false),
            ("Island", 0, false),
            ("Island", 0, false),
            ("Island", 0, false),
        ],
    );
    let bears = object(&g, 1);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[3, 4, 5, 6]);
    settle(&mut g, &[target(bears)]);
    assert_eq!(g.current(bears).unwrap().controller, PlayerId(0));
    let source = object(&g, 0);
    cast_on(&mut g, 2, &[target(source)], &[7]);
    assert_eq!(g.current(bears).unwrap().controller, PlayerId(1));
    assert!(g.state.rules.effects.is_empty());
    checked(&g);
}

// CR 107.3, 611.2d, 613.4c: the paid X is frozen with its sign.
#[test]
fn resolved_continuous_signed_announced_x_is_frozen() {
    let mut g = with(
        board(&[DEATH_WIND]),
        &[
            ("Death Wind", 0, true),
            ("Grizzly Bears", 1, false),
            ("Swamp", 0, false),
            ("Swamp", 0, false),
        ],
    );
    let bears = object(&g, 1);
    Driver::cast(&mut g, CardId(0));
    let announce = Driver::request(&mut g, u64::MAX);
    assert_eq!(announce.reason, Reason::AnnounceX);
    g.respond(
        announce.player,
        announce.id,
        Answer::Number { value: 1.into() },
    )
    .unwrap();
    Driver::pick_object(&mut g, bears);
    Driver::pay(&mut g, &[2, 3]);
    settle(&mut g, &[]);
    assert_eq!(g.characteristics(bears).unwrap().power, Some(1.into()));
    assert_eq!(g.characteristics(bears).unwrap().toughness, Some(1.into()));
    let mut forged = g.fork();
    let effect = forged.state.rules.effects.first_mut().unwrap();
    let ResolvedEffectKind::PumpAnnouncedX { x, .. } = &mut effect.kind else {
        panic!("X pump");
    };
    *x = 2.into();
    assert!(forged.validate_state().is_err());
    next_turn(&mut g);
    assert_eq!(g.characteristics(bears).unwrap().power, Some(2.into()));
    checked(&g);
}

// CR 613.1e, 613.7: entry and resolution use one timestamp domain.
#[test]
fn resolved_continuous_setter_orders_between_two_static_sources() {
    let mut g = with(
        board(&[CRIMSON_WISPS, DARKEST_HOUR, THRAN_LENS]),
        &[
            ("Crimson Wisps", 0, true),
            ("Darkest Hour", 0, false),
            ("Thran Lens", 0, true),
            ("Grizzly Bears", 0, false),
            ("Mountain", 0, false),
            ("Mountain", 0, false),
            ("Mountain", 0, false),
        ],
    );
    let bears = object(&g, 3);
    assert_eq!(g.characteristics(bears).unwrap().colors, [Color::B]);
    cast_on(&mut g, 0, &[target(bears)], &[4]);
    assert_eq!(g.characteristics(bears).unwrap().colors, [Color::R]);
    cast_on(&mut g, 2, &[], &[5, 6]);
    assert!(g.characteristics(bears).unwrap().colors.is_empty());
    let mut forged = g.fork();
    let effect = forged.state.rules.effects.first_mut().unwrap();
    let ResolvedEffectKind::SetCharacteristics { timestamp, .. } = &mut effect.kind else {
        panic!("color setter");
    };
    *timestamp = u64::MAX;
    assert!(forged.validate_state().is_err());
}

// CR 302.6, 510.2, 613.1f, 702.10 and 702.111: haste permits the attack,
// menace prevents a single blocker, and the captured grants survive damage validation.
#[test]
fn resolved_continuous_stolen_creature_with_menace_deals_combat_damage() {
    let mut g = with(
        board(&[LOSE_CALM]),
        &[
            ("Lose Calm", 0, true),
            ("Grizzly Bears", 1, false),
            ("Grizzly Bears", 1, false),
            ("Grizzly Bears", 1, false),
            ("Mountain", 0, false),
            ("Mountain", 0, false),
            ("Mountain", 0, false),
            ("Mountain", 0, false),
        ],
    );
    let [attacker, first, second] = [1, 2, 3].map(|card| object(&g, card));
    cast_on(&mut g, 0, &[target(attacker)], &[4, 5, 6, 7]);
    let request = blockers(&mut g, &[attacker]);
    assert!(
        g.respond(
            request.player,
            request.id,
            request.block(vec![(first, attacker)])
        )
        .is_err()
    );
    g.fork()
        .respond(
            request.player,
            request.id,
            request.block(vec![(first, attacker), (second, attacker)]),
        )
        .unwrap();
    unblocked_damage(&mut g, &request, 2);
}

// CR 205.2b, 510.2, 613.1d: a land with the creature type deals combat damage.
#[test]
fn resolved_continuous_animated_land_deals_combat_damage() {
    let mut g = with(
        board(&[NATURAL_AFFINITY]),
        &[
            ("Natural Affinity", 0, true),
            ("Forest", 0, false),
            ("Forest", 0, false),
            ("Forest", 0, false),
            ("Forest", 0, false),
            ("Grizzly Bears", 1, false),
        ],
    );
    let land = object(&g, 1);
    cast_on(&mut g, 0, &[], &[2, 3, 4]);
    let request = blockers(&mut g, &[land]);

    let mut blocked = g.fork();
    let bears = object(&blocked, 5);
    blocked
        .respond(
            request.player,
            request.id,
            request.block(vec![(bears, land)]),
        )
        .unwrap();
    let graveyard = Zone::Graveyard { owner: PlayerId(0) };
    for _ in 0..20 {
        let decision = Driver::request(&mut blocked, u64::MAX);
        if blocked.zone(object(&blocked, 1)) == Some(graveyard) {
            break;
        }
        let pass = mtg_witness::temporary::pass(&decision);
        blocked.respond(decision.player, decision.id, pass).unwrap();
    }
    assert_eq!(blocked.zone(object(&blocked, 1)), Some(graveyard));
    checked(&blocked);

    unblocked_damage(&mut g, &request, 2);
}

// CR 122.1a, 122.2, 613.4c: a +1/+1 counter remains on the Forest when
// animation ends, with no power or toughness to modify until it animates again.
#[test]
fn resolved_continuous_animated_land_keeps_counter_after_cleanup() {
    let mut g = with(
        board(&[NATURAL_AFFINITY, RUBBLEBELT_MAVERICK]).card(
            "Rubblebelt Maverick",
            0,
            0,
            Zone::Graveyard { owner: PlayerId(0) },
        ),
        &[
            ("Natural Affinity", 0, true),
            ("Forest", 0, false),
            ("Forest", 0, false),
            ("Forest", 0, false),
            ("Forest", 0, false),
            ("Forest", 0, false),
        ],
    );
    let maverick = object(&g, 0);
    let land = object(&g, 2);
    cast_on(&mut g, 1, &[], &[3, 4, 5]);
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: maverick,
            ability: 0,
        },
    );
    Driver::pick_object(&mut g, land);
    Driver::pay(&mut g, &[6]);
    settle(&mut g, &[]);
    assert_eq!(g.characteristics(land).unwrap().power, Some(3.into()));

    next_turn(&mut g);
    let current = g.characteristics(land).unwrap();
    assert!(!current.has_type(CardKind::Creature));
    assert_eq!(current.power, None);
    assert_eq!(g.current(land).unwrap().plus_one_counters, 1.into());
    checked(&g);
}

// CR 205.2b, 601.2c, 613.1d: the simulator's permitted view retains the
// creature type of the animated land, and the engine accepts its selection.
#[test]
fn resolved_continuous_bolt_policy_targets_an_animated_land() {
    use mtg_core::query::{self, Tri};
    use mtg_sim::policy::{Policy, Strategy};

    let mut g = with(
        board(&[NATURAL_AFFINITY]),
        &[
            ("Natural Affinity", 0, true),
            ("Forest", 1, false),
            ("Forest", 0, false),
            ("Forest", 0, false),
            ("Forest", 0, false),
            ("Lightning Bolt", 0, true),
            ("Mountain", 0, false),
        ],
    );
    cast_on(&mut g, 0, &[], &[2, 3, 4]);
    Driver::cast(&mut g, CardId(5));
    let request = Driver::request(&mut g, u64::MAX);
    let session = g.player_session(request.player);
    let QueryResult::Complete(QueryValue::Observation(observation)) =
        session.observe(Budget::unlimited())
    else {
        panic!("expected permitted observation");
    };
    let decision = observation.decision.as_ref().unwrap();
    let Reason::Targets { specs } = &decision.reason else {
        panic!("expected Bolt target");
    };
    let view = &observation.view;
    let forest = view
        .objects
        .iter()
        .find(|object| object.owner == PlayerId(1) && object.zone == Zone::Battlefield)
        .unwrap();
    let offered = Candidate::Object {
        object: forest.handle.clone(),
    };
    assert!(decision.index(&offered).is_some());
    assert_eq!(
        query::matches_target_domain(&query::in_view(view, forest), view.viewer, specs[0].domain),
        Tri::Yes
    );
    let answer = Strategy::new(&g.prepared.catalog, Policy::Develop, [0; 4])
        .choose(&observation)
        .unwrap();
    assert_eq!(answer, decision.pick_one(&offered).unwrap());
    let QueryResult::Complete(QueryValue::Validated(receipt)) =
        session.check_response(decision.id.clone(), answer, Budget::unlimited())
    else {
        panic!("invalid permitted answer");
    };
    g.submit(receipt).unwrap();
    Driver::pay(&mut g, &[6]);
    settle(&mut g, &[]);
    assert_eq!(
        g.zone(object(&g, 1)),
        Some(Zone::Graveyard { owner: PlayerId(1) })
    );
}

fn unblocked_damage(g: &mut Game, request: &Decision<Rules>, damage: i64) {
    assert_eq!(request.player, PlayerId(1));
    let mut expected = g.state.rules.players[1].life.clone();
    expected.subtract(&damage.into());
    g.respond(request.player, request.id, request.block(vec![]))
        .unwrap();
    for _ in 0..20 {
        let decision = Driver::request(g, u64::MAX);
        if g.state.rules.players[1].life == expected {
            checked(g);
            return;
        }
        let pass = mtg_witness::temporary::pass(&decision);
        g.respond(decision.player, decision.id, pass).unwrap();
    }
    panic!("unblocked attacker failed to deal combat damage");
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
