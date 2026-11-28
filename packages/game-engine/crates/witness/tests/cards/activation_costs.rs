//! An activated ability's own cost changes, from exact frozen R5 source on
//! private boards (CR 601.2f, 602.2b): "This ability costs {N} less/more to
//! activate for each [public count]" or "if [condition]". Each change is read
//! from its source as a static ability reads it, once, as the activation
//! begins, and locked into the total; increases apply before reductions and
//! generic mana is never reduced below zero (CR 118.7d). Each board is checked
//! for state validity, a snapshot round trip and journal recovery.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_contracts::kir::{
    Amount, Atom, CounterKind, Pred, Ref, SetExpr, SpellCostChange, ZoneExpr,
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

const TAMIYOS_LOGBOOK: &str = record!("d9356199-4ce3-44fa-9463-90fc134088f9");
const LORESEEKERS_STONE: &str = record!("28a21e56-b97f-487d-9a89-61c248192cec");
const DIARY_OF_DREAMS: &str = record!("98b43c61-4563-4a29-8cf0-fb8b34f337ce");
const ESQUIRE_OF_THE_KING: &str = record!("3ccaa78e-f3e5-4de4-b53c-cecd73a73b9c");
const STARPORT_SECURITY: &str = record!("5454dddf-c6e5-4b0d-b96d-4904c13606dc");
const BARU: &str = record!("1cf27d78-d6ae-4fa2-9c8d-c277913f821c");
const SURVEY_MECHAN: &str = record!("53745903-de8f-42cf-a949-842e5831de30");
const GHALTA_PRIMAL_HUNGER: &str = record!("b0b6be0c-41cf-4757-9f0e-87227b6ba6b3");
const THE_GREAT_HENGE: &str = record!("78427103-9543-41fb-b6d4-72963fe87275");
const VALIANT_CHANGELING: &str = record!("65d9b8b6-5215-4037-8e50-e0ca80c5dc9e");
const NYXBORN_BEHEMOTH: &str = record!("58e20ca7-5fe3-49ff-bee0-0388722e3cab");

fn lowered(raw: &str) -> Definition {
    let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, raw);
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("frozen activation-cost witness must lower: {result:?}");
    };
    definition.validate().unwrap();
    *definition
}

fn rejected(raw: &str) -> String {
    format!("{:?}", mtg_witness::compile(mtg_witness::R5_MANIFEST, raw))
}

fn board(records: &[&str]) -> Board {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    catalog
        .definitions
        .extend(records.iter().map(|raw| lowered(raw)));
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    Board::new(catalog, "r5-activation-cost-witness")
        .step(TurnStep::PrecombatMain)
        .seats(2)
}

fn game(b: Board) -> Game {
    Game::with_trace(
        b.libraries("Island", 8).seed([23; 32]).prepare(),
        TraceMode::Detailed,
    )
}

fn checked(g: &Game) {
    g.validate_state().unwrap();
    let snapshot = g.snapshot().unwrap();
    let restored = Game::restore(g.prepared.as_ref().clone(), &snapshot).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
}

/// Pass priority until the stack is empty, then check and recover.
fn resolve(g: &mut Game) {
    for _ in 0..32 {
        let d = Driver::request(g, u64::MAX);
        if g.state.rules.stack.is_empty() && d.reason == Reason::Priority {
            break;
        }
        match d.reason {
            Reason::Priority => Driver::pass(g),
            ref other => panic!("unexpected decision {other:?}"),
        }
    }
    assert!(g.state.rules.stack.is_empty());
    checked(g);
    assert_recoverable((&*g).into());
}

fn mana(generic: i64, colored: &[Color]) -> ManaCost {
    ManaCost {
        generic: Quantity::from(generic),
        colored: colored.to_vec(),
    }
}

/// Begin activating `ability` of board object `id`, then return the
/// locked total of the announced activation.
fn activate(g: &mut Game, id: u32, ability: u32) -> ManaCost {
    let action = Action::ActivateAbility {
        object: Driver::object(g, id),
        ability,
    };
    let d = Driver::request(g, u64::MAX);
    assert!(
        d.candidates.contains(&Candidate::Action {
            action: action.clone()
        }),
        "{d:?}"
    );
    Driver::action(g, action);
    // Advance to the mana window the announcement opens.
    Driver::request(g, u64::MAX);
    checked(g);
    g.state.rules.stack.last().unwrap().total_cost.clone()
}

fn hand(g: &Game, player: u8) -> usize {
    g.state
        .rules
        .objects
        .values()
        .filter(|o| {
            o.zone
                == Zone::Hand {
                    owner: PlayerId(player),
                }
        })
        .count()
}

/// Each printed change lowers beside the ability as written: Tamiyo's
/// Logbook counts other artifacts its controller controls, Loreseeker's
/// Stone the cards in its controller's hand (a public size, CR 402.3),
/// Diary of Dreams its own page counters and Esquire of the King a
/// legendary-creature condition, and Baru's "{X} less, where X is the
/// greatest power among Wurms you control" {1} less for each of that
/// greatest power (CR 107.3). Starport Security's condition reads counters
/// on other creatures, outside the static reader, and Survey Mechan's count
/// of differently named lands has no cost reader; both reject the whole
/// card.
#[test]
fn printed_activation_cost_changes_lower_with_their_counts_and_conditions() {
    let logbook = lowered(TAMIYOS_LOGBOOK);
    let [ability] = logbook.activated.as_slice() else {
        panic!("one activated ability");
    };
    let [change] = ability.cost_changes.as_slice() else {
        panic!("one cost change: {ability:?}");
    };
    assert_eq!(
        change.change,
        SpellCostChange::Reduce { mana: mana(1, &[]) }
    );
    assert!(change.condition.is_none());
    let Some(Amount::Count {
        set:
            SetExpr::Matching {
                zone: ZoneExpr::Battlefield,
                filter,
            },
    }) = &change.multiplier
    else {
        panic!("a battlefield count: {change:?}");
    };
    assert!(filter.0.contains(&Atom::IsNot {
        object: Ref::Source
    }));
    assert!(filter.0.contains(&Atom::Type {
        kind: CardKind::Artifact
    }));

    let stone = lowered(LORESEEKERS_STONE);
    assert!(matches!(
        stone.activated[0].cost_changes.as_slice(),
        [c] if matches!(c.change, SpellCostChange::Increase { .. })
            && c.multiplier == Some(Amount::Count {
                set: SetExpr::Matching {
                    zone: ZoneExpr::Hand { of: PlayerExpr::static_controller() },
                    filter: Pred::default(),
                },
            })
    ));
    let diary = lowered(DIARY_OF_DREAMS);
    assert!(matches!(
        diary.activated[0].cost_changes.as_slice(),
        [c] if c.multiplier == Some(Amount::CountersOn {
            object: Ref::Source,
            counter: CounterKind::Named { name: "page".into() },
        })
    ));
    let esquire = lowered(ESQUIRE_OF_THE_KING);
    assert!(matches!(
        esquire.activated[0].cost_changes.as_slice(),
        [c] if c.multiplier.is_none() && c.condition.is_some()
            && c.change == SpellCostChange::Reduce { mana: mana(2, &[]) }
    ));

    let starport = rejected(STARPORT_SECURITY);
    assert!(
        starport.contains("cell:activated:cost_change"),
        "{starport}"
    );
    let baru = lowered(BARU);
    assert!(matches!(
        baru.activated[0].cost_changes.as_slice(),
        [c] if c.change == SpellCostChange::Reduce { mana: mana(1, &[]) }
            && matches!(c.multiplier, Some(Amount::Greatest {
                stat: mtg_contracts::kir::Stat::Power, ..
            }))
    ));
    let mechan = rejected(SURVEY_MECHAN);
    assert!(
        mechan.contains("static:conditional:amount:distinct_count"),
        "{mechan}"
    );
}

/// "This spell costs {X} less to cast, where X is [amount]" is {1} less for
/// each of that amount, read as the total is determined (CR 107.3,
/// 601.2f): Ghalta, Primal Hunger totals the power of creatures its caster
/// controls and The Great Henge reads the greatest. Valiant Changeling's
/// "can't reduce ... by more than {5}" and Nyxborn Behemoth's total mana
/// value are outside the cost reader and reject.
#[test]
fn spell_cost_bindings_lower_to_counted_generic_reductions() {
    use mtg_contracts::kir::{Stat, StaticRule};
    let multiplier = |d: &Definition| {
        d.static_abilities
            .iter()
            .find_map(|a| match &a.effect {
                StaticEffect::Rule {
                    rule:
                        StaticRule::SpellCost {
                            change: SpellCostChange::Reduce { mana: m },
                            multiplier,
                            ..
                        },
                    ..
                } if *m == mana(1, &[]) => multiplier.clone(),
                _ => None,
            })
            .expect("a counted spell cost reduction")
    };
    assert!(matches!(
        multiplier(&lowered(GHALTA_PRIMAL_HUNGER)),
        Amount::Total {
            stat: Stat::Power,
            ..
        }
    ));
    assert!(matches!(
        multiplier(&lowered(THE_GREAT_HENGE)),
        Amount::Greatest {
            stat: Stat::Power,
            ..
        }
    ));
    let changeling = rejected(VALIANT_CHANGELING);
    assert!(
        changeling.contains("static:cost-modification:qualifier"),
        "{changeling}"
    );
    let behemoth = rejected(NYXBORN_BEHEMOTH);
    assert!(
        behemoth.contains("static:cost-modification:amount:mana-value"),
        "{behemoth}"
    );
}

/// Ghalta, Primal Hunger ({10}{G}{G}) with Colossal Dreadmaw (6/6) and
/// Grizzly Bears (2/2) locks {2}{G}{G}; The Great Henge ({7}{G}{G}) with
/// the same creatures reads only the Dreadmaw's 6 and locks {1}{G}{G}. Both
/// resolve with exactly that mana.
#[test]
fn spell_cost_bindings_read_current_power_as_the_cost_locks() {
    for (raw, name, total) in [
        (
            GHALTA_PRIMAL_HUNGER,
            "Ghalta, Primal Hunger",
            mana(2, &[Color::G, Color::G]),
        ),
        (
            THE_GREAT_HENGE,
            "The Great Henge",
            mana(1, &[Color::G, Color::G]),
        ),
    ] {
        let lands: u32 = 4 + u32::from(name == "Ghalta, Primal Hunger");
        let mut b = board(&[raw])
            .permanent_or_hand(name, 0, 0, true)
            .permanent_or_hand("Colossal Dreadmaw", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false);
        for _ in 0..lands {
            b = b.permanent_or_hand("Forest", 0, 0, false);
        }
        let mut g = game(b);
        Driver::cast(&mut g, CardId(0));
        Driver::request(&mut g, u64::MAX);
        assert_eq!(g.state.rules.stack.last().unwrap().total_cost, total);
        let paid: Vec<u32> =
            (3..3 + total.colored.len() as u32 + total.generic.to_u32().unwrap()).collect();
        Driver::pay(&mut g, &paid);
        resolve(&mut g);
        assert_eq!(
            g.state.rules.objects[&CardId(0)].zone,
            Zone::Battlefield,
            "{name}"
        );
    }
}

/// Tamiyo's Logbook: "{5}{U}, {T}: Draw a card. This ability costs {1} less
/// to activate for each other artifact you control." Two other artifacts
/// lock {3}{U}; six would reduce more than the generic {5}, leaving only
/// {U} (CR 118.7d: a generic reduction never reduces colored mana).
#[test]
fn tamiyos_logbook_counts_other_artifacts_and_keeps_its_colored_mana() {
    let mut g = game(
        board(&[TAMIYOS_LOGBOOK])
            .permanent_or_hand("Tamiyo's Logbook", 0, 0, false)
            .permanent_or_hand("Darksteel Relic", 0, 0, false)
            .permanent_or_hand("Wooden Sphere", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false)
            // An opponent's artifact is not counted.
            .permanent_or_hand("Sol Ring", 1, 1, false),
    );
    let before = hand(&g, 0);
    assert_eq!(activate(&mut g, 0, 0), mana(3, &[Color::U]));
    Driver::pay(&mut g, &[3, 4, 5, 6]);
    resolve(&mut g);
    assert_eq!(hand(&g, 0), before + 1);

    let mut g = game(
        board(&[TAMIYOS_LOGBOOK])
            .permanent_or_hand("Tamiyo's Logbook", 0, 0, false)
            .permanent_or_hand("Darksteel Relic", 0, 0, false)
            .permanent_or_hand("Wooden Sphere", 0, 0, false)
            .permanent_or_hand("Sol Ring", 0, 0, false)
            .permanent_or_hand("Izzet Signet", 0, 0, false)
            .permanent_or_hand("Pristine Talisman", 0, 0, false)
            .permanent_or_hand("Standing Stones", 0, 0, false)
            .permanent_or_hand("Island", 0, 0, false),
    );
    let before = hand(&g, 0);
    assert_eq!(activate(&mut g, 0, 0), mana(0, &[Color::U]));
    Driver::pay(&mut g, &[7]);
    resolve(&mut g);
    assert_eq!(hand(&g, 0), before + 1);
}

/// Loreseeker's Stone: "{3}, {T}: Draw three cards. This ability costs {1}
/// more to activate for each card in your hand." Two cards in hand lock
/// {5}; the cards drawn later do not change the paid total.
#[test]
fn loreseekers_stone_increases_by_its_controllers_hand_size() {
    let mut g = game(
        board(&[LORESEEKERS_STONE])
            .permanent_or_hand("Loreseeker's Stone", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, true)
            .permanent_or_hand("Unsummon", 0, 0, true)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            // An opponent's hand is not counted.
            .permanent_or_hand("Murder", 1, 1, true),
    );
    assert_eq!(hand(&g, 0), 2);
    assert_eq!(activate(&mut g, 0, 0), mana(5, &[]));
    Driver::pay(&mut g, &[3, 4, 5, 6, 7]);
    resolve(&mut g);
    assert_eq!(hand(&g, 0), 5);
}

/// Diary of Dreams: "Whenever you cast an instant or sorcery spell, put a
/// page counter on this artifact. {5}, {T}: Draw a card. This ability costs
/// {1} less to activate for each page counter on this artifact." One cast
/// instant puts one page counter on it; its activation then locks {4}.
#[test]
fn diary_of_dreams_reads_its_own_page_counters() {
    let mut g = game(
        board(&[DIARY_OF_DREAMS])
            .permanent_or_hand("Diary of Dreams", 0, 0, false)
            .permanent_or_hand("Giant Growth", 0, 0, true)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Forest", 0, 0, false),
    );
    Driver::cast(&mut g, CardId(1));
    let d = Driver::request(&mut g, u64::MAX);
    let bears = Driver::object(&g, 2);
    let pick = d.index(&Candidate::Object { object: bears }).unwrap();
    g.respond(
        d.player,
        d.id,
        Answer::Pick {
            picks: vec![vec![pick]],
        },
    )
    .unwrap();
    Driver::pay(&mut g, &[3]);
    resolve(&mut g);
    let diary = Driver::object(&g, 0);
    let page = CounterKind::Named {
        name: "page".into(),
    };
    assert_eq!(
        g.current(diary).unwrap().counter_count(&page),
        Quantity::from(1)
    );
    assert_eq!(activate(&mut g, 0, 0), mana(4, &[]));
    Driver::pay(&mut g, &[4, 5, 6, 7]);
    resolve(&mut g);
}

/// Esquire of the King: "{4}{W}, {T}: Creatures you control get +1/+1 until
/// end of turn. This ability costs {2} less to activate if you control a
/// legendary creature." The condition holds with Isamaru and locks {2}{W};
/// without a legendary creature the printed {4}{W} remains.
#[test]
fn esquire_of_the_king_reduces_only_while_its_condition_holds() {
    let mut g = game(
        board(&[ESQUIRE_OF_THE_KING])
            .permanent_or_hand("Esquire of the King", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false),
    );
    assert_eq!(activate(&mut g, 0, 0), mana(4, &[Color::W]));

    let mut g = game(
        board(&[ESQUIRE_OF_THE_KING])
            .permanent_or_hand("Esquire of the King", 0, 0, false)
            .permanent_or_hand("Isamaru, Hound of Konda", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false),
    );
    assert_eq!(activate(&mut g, 0, 0), mana(2, &[Color::W]));
    Driver::pay(&mut g, &[2, 3, 4]);
    resolve(&mut g);
    let isamaru = g.characteristics(Driver::object(&g, 1)).unwrap();
    assert_eq!(isamaru.power, Some(Quantity::from(3)));
}
