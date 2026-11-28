//! Counted and specified token creation from exact frozen R5 source on
//! private boards (CR 111.1-111.4, 111.10, 608.2h, 614.1c): predefined
//! Treasures from a reviewed trigger, a count read as the ability resolves,
//! "a token for each [filter]", keywords and tokens entering tapped. Tokens
//! one instruction creates enter in one event, so each sees the others
//! enter (CR 603.2c). Every board is checked for state validity, a snapshot
//! round trip and journal replay at every quantum.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_contracts::kir::{Cell, definition_cells};
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

const RAPACIOUS_DRAGON: &str = record!("0944ec2e-1dd9-459f-8f1d-667242cf52fe");
const GOBLIN_PIKER: &str = record!("50608184-90d3-43d2-a221-deb186c78323");
const KRENKO_MOB_BOSS: &str = record!("68418069-f615-40ef-ae0d-764192acae00");
const SECURE_THE_WASTES: &str = record!("b2347910-d6c6-4681-8316-7ef27056485c");
const OVERSEER_OF_THE_DAMNED: &str = record!("c1f085ce-5b52-45b3-aef0-f77f36b3da36");
const SPIDER_SPAWNING: &str = record!("da113cc9-bb2b-4af4-9c43-32c165b87363");

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);

fn lowered(raw: &str) -> Definition {
    let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, raw);
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("frozen token witness failed: {result:?}");
    };
    *definition
}

fn board(records: &[&str]) -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for raw in records {
        catalog.definitions.push(lowered(raw));
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    Board::new(catalog, "r5-token-creation-witness")
        .step(TurnStep::PrecombatMain)
        .seats(2)
}

/// Libraries come after the numbered cards, so card ids follow the board.
fn game(b: Board) -> Game {
    Game::with_trace(
        b.libraries("Island", 6).seed([7; 32]).prepare(),
        TraceMode::Detailed,
    )
}

fn checked(g: &Game) {
    g.validate_state().unwrap();
    let snapshot = g.snapshot().unwrap();
    let restored = Game::restore(g.prepared.as_ref().clone(), &snapshot).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
}

/// Pass priority until the stack is empty, then check and replay.
fn resolve(g: &mut Game) {
    for _ in 0..24 {
        let d = Driver::request(g, u64::MAX);
        assert_eq!(d.reason, Reason::Priority, "{d:?}");
        if g.state.rules.stack.is_empty() {
            break;
        }
        Driver::pass(g);
    }
    assert!(g.state.rules.stack.is_empty());
    checked(g);
    assert_recoverable((&*g).into());
}

/// The battlefield tokens `player` controls named `name`.
fn tokens(g: &Game, player: PlayerId, name: &str) -> Vec<Object> {
    g.state
        .rules
        .objects
        .values()
        .filter(|o| {
            o.zone == Zone::Battlefield
                && o.controller == player
                && g.state.rules.tokens.contains_key(&o.id.card)
                && g.copiable_definition(o.id).is_ok_and(|d| d.name == name)
        })
        .cloned()
        .collect()
}

fn bolt(g: &mut Game, bolt: u32, target: ObjectId, mana: &[u32]) {
    Driver::cast(g, CardId(bolt));
    Driver::pick_object(g, target);
    Driver::pay(g, mana);
}

/// Rapacious Dragon: "When this creature enters, create two Treasure
/// tokens." The two Treasures come from one reviewed enters trigger
/// (CR 111.10a); one then pays for Lightning Bolt and is sacrificed.
#[test]
fn rapacious_dragon_creates_two_treasures_and_one_pays_for_a_spell() {
    let mut b = board(&[RAPACIOUS_DRAGON])
        .permanent_or_hand("Rapacious Dragon", 0, 0, true)
        .permanent_or_hand("Lightning Bolt", 0, 0, true);
    for _ in 0..5 {
        b = b.permanent_or_hand("Mountain", 0, 0, false);
    }
    let mut g = game(b.permanent_or_hand("Grizzly Bears", 1, 1, false));
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[2, 3, 4, 5, 6]);
    resolve(&mut g);
    let treasures = tokens(&g, P0, "Treasure Token");
    assert_eq!(treasures.len(), 2);
    assert!(treasures.iter().all(|t| !t.tapped));
    let c = g.characteristics(treasures[0].id).unwrap();
    assert_eq!(c.kind, CardKind::Artifact);
    assert_eq!(c.subtypes, vec!["Treasure".to_string()]);
    let bear = Driver::object(&g, 7);
    Driver::cast(&mut g, CardId(1));
    Driver::pick_object(&mut g, bear);
    // CR 111.10a: "{T}, Sacrifice this token: Add one mana of any color."
    let d = Driver::request(&mut g, u64::MAX);
    let source = d
        .index(&Candidate::Action {
            action: Action::ActivateMana {
                object: treasures[0].id,
            },
        })
        .unwrap();
    Driver::answer(
        &mut g,
        u64::MAX,
        Answer::Pay {
            pay: PayAnswer::Activate { source },
        },
    );
    let d = Driver::request(&mut g, u64::MAX);
    assert!(matches!(d.reason, Reason::ManaColor { .. }), "{d:?}");
    let red = d.pick_one(&Candidate::Color { color: Color::R }).unwrap();
    g.respond(d.player, d.id, red).unwrap();
    Driver::answer(
        &mut g,
        u64::MAX,
        Answer::Pay {
            pay: PayAnswer::Finish {
                undo_on_failure: true,
            },
        },
    );
    g.complete_test_mana();
    resolve(&mut g);
    assert!(g.object_id(treasures[0].id.card).is_none());
    assert_eq!(tokens(&g, P0, "Treasure Token").len(), 1);
    assert_eq!(
        g.state.rules.objects[&bear.card].zone,
        Zone::Graveyard { owner: P1 }
    );
}

/// Krenko, Mob Boss: "{T}: Create X 1/1 red Goblin creature tokens, where
/// X is the number of Goblins you control." X is read as the ability
/// resolves (CR 608.2h): a Goblin bolted in response is not counted. The
/// tokens enter in one event, so Soul's Attendant triggers once for each
/// (CR 603.2c).
#[test]
fn krenko_counts_goblins_as_the_ability_resolves() {
    for respond in [false, true] {
        let mut g = game(
            board(&[KRENKO_MOB_BOSS, GOBLIN_PIKER])
                .permanent_or_hand("Lightning Bolt", 0, 0, true)
                .permanent_or_hand("Krenko, Mob Boss", 0, 0, false)
                .permanent_or_hand("Goblin Piker", 0, 0, false)
                .permanent_or_hand("Mountain", 0, 0, false)
                .permanent_or_hand("Soul's Attendant", 0, 0, false),
        );
        assert!(
            definition_cells(g.copiable_definition(Driver::object(&g, 1)).unwrap())
                .unwrap()
                .contains(&Cell::Evaluated(
                    mtg_contracts::kir::EvaluatedVerb::CreateTokens
                ))
        );
        let krenko = Driver::object(&g, 1);
        Driver::action(
            &mut g,
            Action::ActivateAbility {
                object: krenko,
                ability: 0,
            },
        );
        if respond {
            let piker = Driver::object(&g, 2);
            bolt(&mut g, 0, piker, &[3]);
        }
        // Resolve the bolt (if any) and Krenko's ability, stopping when the
        // Goblins have entered. Two simultaneous entries put two Soul's
        // Attendant triggers up for ordering; one goes straight to the stack.
        let expected = if respond { 1 } else { 2 };
        let mut triggered = 0;
        for _ in 0..16 {
            let d = Driver::request(&mut g, u64::MAX);
            if d.reason == Reason::OrderTriggers {
                triggered = d.candidates.len();
                break;
            }
            assert_eq!(d.reason, Reason::Priority, "{d:?}");
            if !tokens(&g, P0, "Goblin Token").is_empty() {
                triggered = g.state.rules.stack.len();
                break;
            }
            Driver::pass(&mut g);
        }
        assert_eq!(triggered, expected, "one trigger per token");
        let goblins = tokens(&g, P0, "Goblin Token");
        assert_eq!(goblins.len(), expected, "respond {respond}");
        let created = g
            .trace
            .records
            .iter()
            .find_map(|record| {
                let EvidenceRecord::Batch { batch } = &record.record else {
                    return None;
                };
                batch.occurrences.iter().find_map(|occurrence| {
                    if let EventOutcome::TokensCreated { objects, .. } = &occurrence.actual {
                        Some(objects)
                    } else {
                        None
                    }
                })
            })
            .unwrap();
        assert_eq!(
            created,
            &goblins.iter().map(|goblin| goblin.id).collect::<Vec<_>>()
        );
        let c = g.characteristics(goblins[0].id).unwrap();
        assert_eq!((c.power, c.toughness), (Some(1.into()), Some(1.into())));
        assert_eq!(c.colors, vec![Color::R]);
        assert_eq!(c.subtypes, vec!["Goblin".to_string()]);
        checked(&g);
        assert_recoverable((&g).into());
    }
}

/// Spider Spawning: "Create a 1/2 green Spider creature token with reach
/// for each creature card in your graveyard." Two creature cards and an
/// instant: two Spiders with reach.
#[test]
fn spider_spawning_creates_a_spider_for_each_creature_card() {
    let mut b = board(&[SPIDER_SPAWNING]).permanent_or_hand("Spider Spawning", 0, 0, true);
    for _ in 0..5 {
        b = b.permanent_or_hand("Forest", 0, 0, false);
    }
    let graveyard = Zone::Graveyard { owner: P0 };
    let mut g = game(
        b.card("Grizzly Bears", 0, 0, graveyard)
            .card("Colossal Dreadmaw", 0, 0, graveyard)
            .card("Lightning Bolt", 0, 0, graveyard),
    );
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2, 3, 4, 5]);
    resolve(&mut g);
    let spiders = tokens(&g, P0, "Spider Token");
    assert_eq!(spiders.len(), 2);
    let c = g.characteristics(spiders[0].id).unwrap();
    assert_eq!((c.power, c.toughness), (Some(1.into()), Some(2.into())));
    assert_eq!(c.colors, vec![Color::G]);
    let keywords = &g.copiable_definition(spiders[0].id).unwrap().keywords;
    assert_eq!(keywords, &vec![KeywordAbility::Reach]);
}

/// Overseer of the Damned: "Whenever a nontoken creature an opponent
/// controls dies, create a tapped 2/2 black Zombie creature token." The
/// token enters tapped (CR 614.1c).
#[test]
fn overseer_of_the_damned_creates_a_tapped_zombie() {
    let mut g = game(
        board(&[OVERSEER_OF_THE_DAMNED])
            .permanent_or_hand("Lightning Bolt", 0, 0, true)
            .permanent_or_hand("Overseer of the Damned", 0, 0, false)
            .permanent_or_hand("Mountain", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 1, 1, false),
    );
    let bear = Driver::object(&g, 3);
    bolt(&mut g, 0, bear, &[2]);
    resolve(&mut g);
    let zombies = tokens(&g, P0, "Zombie Token");
    assert_eq!(zombies.len(), 1);
    assert!(zombies[0].tapped);
    let c = g.characteristics(zombies[0].id).unwrap();
    assert_eq!((c.power, c.toughness), (Some(2.into()), Some(2.into())));
    assert_eq!(c.colors, vec![Color::B]);
}

/// Secure the Wastes: "Create X 1/1 white Warrior creature tokens." The
/// announced X opens the counted token cell; no fixed count is printed.
#[test]
fn secure_the_wastes_reads_the_announced_x() {
    let mut b = board(&[SECURE_THE_WASTES]).permanent_or_hand("Secure the Wastes", 0, 0, true);
    for _ in 0..3 {
        b = b.permanent_or_hand("Plains", 0, 0, false);
    }
    let mut g = game(b);
    Driver::cast(&mut g, CardId(0));
    let decision = Driver::request(&mut g, u64::MAX);
    assert_eq!(decision.reason, Reason::AnnounceX);
    g.respond(
        decision.player,
        decision.id,
        Answer::Number { value: 2.into() },
    )
    .unwrap();
    Driver::pay(&mut g, &[1, 2, 3]);
    resolve(&mut g);
    assert_eq!(tokens(&g, P0, "Warrior Token").len(), 2);
}
