//! Counted quantities read by the shared evaluator and the layer system,
//! from exact frozen R5 source on private boards: "+N/+M for each
//! [filter]" (CR 608.2h, 613.4c), lord scopes over a union of creature
//! filters, token counts read when the step is performed (a life total, a
//! cost's sacrificed object, a destroyed target's last-known mana value,
//! "that many"; CR 111.1, 608.2h), halves rounded as printed (CR 107.1a),
//! sacrifice costs by color (CR 105.2, 701.21a) and entry counters counted
//! as the permanent enters (CR 614.1c, 614.12). Each board is checked for
//! state validity, a snapshot round trip and journal replay.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_contracts::kir::{
    Amount, Atom, Cell, Change, EvaluatedVerb, Pred, Scope, SetExpr, StaticEffect, ZoneExpr,
    definition_cells,
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

const WIGHT_OF_PRECINCT_SIX: &str = record!("6397c046-4c59-4f0b-9b44-2a804eb95edf");
const BENALISH_HONOR_GUARD: &str = record!("0595b074-8f25-469d-9e82-ed60c4a7a2cd");
const ANCESTRAL_MASK: &str = record!("db5380ed-ba28-4ea2-abc3-4998e2022903");
const MOGG_SQUAD: &str = record!("4dac142a-c8f8-45c8-85d3-40e170dee112");
const DEATH_BARON: &str = record!("99024aa8-5687-4d38-8a4b-feef42d6c1ff");
const STORM_HERD: &str = record!("30aa362b-7ab1-486e-9802-1171e0dc2416");
const GHOULCALLER_GISA: &str = record!("0dd894cb-1968-471c-af08-ea7ec5ce8428");
const AURA_MUTATION: &str = record!("19bb51dd-2b93-45f0-927d-bbafddadd0a3");
const HORNET_NEST: &str = record!("123d2b6e-114e-4767-860d-50790382ca4a");
const ENDLESS_RANKS: &str = record!("69d4ecac-4735-4667-bfc1-c8800b436d08");
const INFERNAL_CONTRACT: &str = record!("4372205f-45e6-49c7-b1fd-13a41e20d4de");
const AYARA: &str = record!("388168b3-ec68-4af2-b88c-6a5ec88c15f6");
const GLIMMER_BAIRN: &str = record!("118cdf8b-44fb-4c3e-86c4-e7f4b1267646");
const JAN_JANSEN: &str = record!("cb491d8a-2e9f-46fe-9590-44c3b4a25f1b");
const DIREGRAF_COLOSSUS: &str = record!("fd62ad01-601f-4250-bc2a-8ef3982e45c4");
const GATEKEEPER_GARGOYLE: &str = record!("a175de44-3a7e-4165-8f9a-8173c0df13d6");

const P0: PlayerId = PlayerId(0);
const P1: PlayerId = PlayerId(1);

fn lowered(raw: &str) -> Definition {
    let result = mtg_witness::compile(mtg_witness::R5_MANIFEST, raw);
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("frozen counted-amount witness must lower: {result:?}");
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
    Board::new(catalog, "r5-counted-amounts-witness")
        .step(TurnStep::PrecombatMain)
        .seats(2)
}

fn game(b: Board) -> Game {
    Game::with_trace(
        b.libraries("Island", 8).seed([11; 32]).prepare(),
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
    for _ in 0..32 {
        let d = Driver::request(g, u64::MAX);
        if g.state.rules.stack.is_empty() && d.reason == Reason::Priority {
            break;
        }
        match d.reason {
            Reason::Priority => Driver::pass(g),
            Reason::OrderTriggers => {
                let order = (0..u32::try_from(d.candidates.len()).unwrap()).collect();
                g.respond(d.player, d.id, Answer::Order { order }).unwrap();
            }
            ref other => panic!("unexpected decision {other:?}"),
        }
    }
    assert!(g.state.rules.stack.is_empty());
    checked(g);
    assert_recoverable((&*g).into());
}

fn q(value: i64) -> Quantity {
    Quantity::from(value)
}

fn pt(g: &Game, id: u32) -> (Quantity, Quantity) {
    let c = g.characteristics(Driver::object(g, id)).unwrap();
    (c.power.unwrap(), c.toughness.unwrap())
}

fn graveyard(owner: u8) -> Zone {
    Zone::Graveyard {
        owner: PlayerId(owner),
    }
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

fn evaluated_tokens(d: &Definition) -> bool {
    definition_cells(d)
        .unwrap()
        .contains(&Cell::Evaluated(EvaluatedVerb::CreateTokens))
}

/// CR 608.2h, 613.4c: "This creature gets +1/+1 for each creature card in
/// your opponents' graveyards" lowers to an additive change over a public
/// count. Benalish Honor Guard counts legendary creatures its controller
/// controls; Ancestral Mask's "other enchantment" excludes itself. A
/// penalty ("-1/-1 for each") has no count form and rejects by name.
#[test]
fn counted_static_modifications_lower_over_public_counts() {
    let wight = lowered(WIGHT_OF_PRECINCT_SIX);
    let [ability] = wight.static_abilities.as_slice() else {
        panic!("one static");
    };
    let StaticEffect::Continuous {
        affects: Scope::Source,
        changes,
        condition: None,
    } = &ability.effect
    else {
        panic!("a source modification: {ability:?}");
    };
    let count = Amount::Count {
        set: SetExpr::Matching {
            zone: ZoneExpr::Graveyard {
                of: PlayerExpr::EachOpponent,
            },
            filter: Pred(vec![Atom::Type {
                kind: CardKind::Creature,
            }]),
        },
    };
    assert_eq!(
        changes,
        &[Change::PowerToughness {
            power: count.clone(),
            toughness: count,
        }]
    );
    let guard = lowered(BENALISH_HONOR_GUARD);
    assert!(guard.static_abilities.iter().any(|a| matches!(&a.effect,
        StaticEffect::Continuous { changes, .. }
            if matches!(changes.as_slice(), [Change::PowerToughness {
                power: Amount::Count { .. },
                toughness: Amount::Fixed { .. },
            }]))));
    let mask = lowered(ANCESTRAL_MASK);
    assert!(mask.static_abilities.iter().any(|a| matches!(&a.effect,
        StaticEffect::Continuous { affects: Scope::Attachment { .. }, changes, .. }
            if matches!(changes.as_slice(), [Change::PowerToughness {
                power: Amount::Times { of, .. }, ..
            }] if matches!(&**of, Amount::Count { set: SetExpr::Matching { filter, .. } }
                if filter.0.contains(&Atom::IsNot { object: mtg_contracts::kir::Ref::Source }))))));
    let squad = rejected(MOGG_SQUAD);
    assert!(squad.contains("static:modifier:for-each:negative"), "{squad}");
}

/// CR 608.2h, 611.3a: the count is read whenever the stat is: Wight of
/// Precinct Six grows as an opponent's creature dies and ignores its own
/// controller's graveyard. Benalish Honor Guard counts only the legendary
/// creatures its controller controls.
#[test]
fn counted_static_modifications_recount_as_objects_move() {
    let mut g = game(
        board(&[WIGHT_OF_PRECINCT_SIX, BENALISH_HONOR_GUARD])
            .permanent_or_hand("Wight of Precinct Six", 0, 0, false)
            .permanent_or_hand("Benalish Honor Guard", 0, 0, false)
            .permanent_or_hand("Isamaru, Hound of Konda", 0, 0, false)
            .permanent_or_hand("Isamaru, Hound of Konda", 1, 1, false)
            .permanent_or_hand("Grizzly Bears", 1, 1, false)
            .permanent_or_hand("Lightning Bolt", 0, 0, true)
            .permanent_or_hand("Mountain", 0, 0, false)
            .card("Grizzly Bears", 1, 1, graveyard(1))
            .card("Lightning Bolt", 1, 1, graveyard(1))
            .card("Grizzly Bears", 0, 0, graveyard(0)),
    );
    // One creature card in the opponent's graveyard; an instant and the
    // controller's own creature card do not count.
    assert_eq!(pt(&g, 0), (q(2), q(2)));
    // One legendary creature the guard's controller controls.
    assert_eq!(pt(&g, 1), (q(3), q(2)));
    let bears = Driver::object(&g, 4);
    Driver::cast(&mut g, CardId(5));
    Driver::pick_object(&mut g, bears);
    Driver::pay(&mut g, &[6]);
    resolve(&mut g);
    assert_eq!(g.zone(Driver::object(&g, 4)), Some(graveyard(1)));
    assert_eq!(pt(&g, 0), (q(3), q(3)));
    assert_eq!(pt(&g, 1), (q(3), q(2)));
}

/// CR 613.1f, 613.4c: "Skeletons you control and other Zombies you control
/// get +1/+1 and have deathtouch" is one creature scope over a union.
/// Death Baron is not another Zombie, an opponent's Zombie is not "you
/// control", and a non-Zombie is outside the union.
#[test]
fn union_lord_scope_affects_each_listed_creature() {
    let baron = lowered(DEATH_BARON);
    assert!(baron.static_abilities.iter().any(|a| matches!(&a.effect,
        StaticEffect::Continuous { affects: Scope::Matching { filter }, .. }
            if filter.0.iter().any(|atom| matches!(atom, Atom::AnyOf { alternatives }
                if alternatives.len() == 2)))));
    let g = game(
        board(&[DEATH_BARON, DIREGRAF_COLOSSUS])
            .permanent_or_hand("Death Baron", 0, 0, false)
            .permanent_or_hand("Diregraf Colossus", 0, 0, false)
            .permanent_or_hand("Diregraf Colossus", 1, 1, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false),
    );
    assert_eq!(pt(&g, 0), (q(2), q(2)));
    assert!(!g.has_keyword(Driver::object(&g, 0), KeywordAbility::Deathtouch));
    assert_eq!(pt(&g, 1), (q(3), q(3)));
    assert!(g.has_keyword(Driver::object(&g, 1), KeywordAbility::Deathtouch));
    assert_eq!(pt(&g, 2), (q(2), q(2)));
    assert!(!g.has_keyword(Driver::object(&g, 2), KeywordAbility::Deathtouch));
    assert_eq!(pt(&g, 3), (q(2), q(2)));
    checked(&g);
}

/// CR 111.1, 608.2h: Storm Herd ({8}{W}{W}) creates one Pegasus for each
/// point of its caster's life total as it resolves.
#[test]
fn storm_herd_creates_tokens_equal_to_the_life_total() {
    let herd = lowered(STORM_HERD);
    assert!(evaluated_tokens(&herd));
    let mut b = board(&[STORM_HERD])
        .permanent_or_hand("Storm Herd", 0, 0, true)
        .life(P0, q(6));
    for _ in 0..10 {
        b = b.permanent_or_hand("Plains", 0, 0, false);
    }
    let mut g = game(b);
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    resolve(&mut g);
    assert_eq!(g.zone(Driver::object(&g, 0)), Some(graveyard(0)));
    let pegasi = tokens(&g, P0, "Pegasus Token");
    assert_eq!(pegasi.len(), 6);
    assert!(g.has_keyword(pegasi[0].id, KeywordAbility::Flying));
}

/// CR 601.2h, 602.2b, 608.2h: Ghoulcaller Gisa's X is the power of the
/// creature sacrificed to pay the cost, read from its last-known
/// information after it left.
#[test]
fn ghoulcaller_gisa_reads_the_sacrificed_creatures_power() {
    let mut g = game(
        board(&[GHOULCALLER_GISA])
            .permanent_or_hand("Ghoulcaller Gisa", 0, 0, false)
            .permanent_or_hand("Colossal Dreadmaw", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false),
    );
    assert!(evaluated_tokens(g.copiable_definition(Driver::object(&g, 0)).unwrap()));
    let gisa = Driver::object(&g, 0);
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: gisa,
            ability: 0,
        },
    );
    Driver::pay(&mut g, &[2]);
    let d = Driver::request(&mut g, u64::MAX);
    assert_eq!(d.reason, Reason::SacrificeCost, "{d:?}");
    // "Another creature": Gisa is not offered.
    let dreadmaw = Driver::object(&g, 1);
    assert_eq!(d.candidates, vec![Candidate::Object { object: dreadmaw }]);
    Driver::pick_object(&mut g, dreadmaw);
    resolve(&mut g);
    assert_eq!(g.zone(Driver::object(&g, 1)), Some(graveyard(0)));
    assert_eq!(tokens(&g, P0, "Zombie Token").len(), 6);
}

/// CR 608.2b, 608.2h: Aura Mutation's X is the destroyed enchantment's
/// mana value from last-known information. "That enchantment" names the
/// target: the Saprolings the step creates do not exist when X is read.
#[test]
fn aura_mutation_counts_the_destroyed_enchantments_mana_value() {
    let mut g = game(
        board(&[AURA_MUTATION])
            .permanent_or_hand("Aura Mutation", 0, 0, true)
            .permanent_or_hand("Glorious Anthem", 1, 1, false)
            .permanent_or_hand("Forest", 0, 0, false)
            .permanent_or_hand("Plains", 0, 0, false),
    );
    let anthem = Driver::object(&g, 1);
    Driver::cast(&mut g, CardId(0));
    Driver::pick_object(&mut g, anthem);
    Driver::pay(&mut g, &[2, 3]);
    resolve(&mut g);
    assert_eq!(g.zone(Driver::object(&g, 1)), Some(graveyard(1)));
    let saprolings = tokens(&g, P0, "Saproling Token");
    assert_eq!(saprolings.len(), 3);
}

/// CR 603.2, 608.2h: Hornet Nest creates one Insect for each point of
/// damage dealt to it ("that many").
#[test]
fn hornet_nest_creates_that_many_insects() {
    let mut g = game(
        board(&[HORNET_NEST])
            .permanent_or_hand("Hornet Nest", 1, 1, false)
            .permanent_or_hand("Lightning Bolt", 0, 0, true)
            .permanent_or_hand("Mountain", 0, 0, false),
    );
    let nest = Driver::object(&g, 0);
    Driver::cast(&mut g, CardId(1));
    Driver::pick_object(&mut g, nest);
    Driver::pay(&mut g, &[2]);
    resolve(&mut g);
    assert_eq!(g.zone(Driver::object(&g, 0)), Some(graveyard(1)));
    let insects = tokens(&g, P1, "Insect Token");
    assert_eq!(insects.len(), 3);
    assert!(g.has_keyword(insects[0].id, KeywordAbility::Deathtouch));
    assert!(g.has_keyword(insects[0].id, KeywordAbility::Flying));
}

/// CR 107.1a: Infernal Contract loses half the life total, rounded up;
/// Endless Ranks of the Dead lowers half its Zombie count, rounded down.
#[test]
fn halves_round_as_printed() {
    let ranks = lowered(ENDLESS_RANKS);
    let printed = format!("{:?}", ranks.triggered);
    assert!(printed.contains("Half"), "{printed}");
    assert!(!printed.contains("round_up: true"), "{printed}");
    assert!(evaluated_tokens(&ranks));
    let mut g = game(
        board(&[INFERNAL_CONTRACT])
            .permanent_or_hand("Infernal Contract", 0, 0, true)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .life(P0, q(7)),
    );
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2, 3]);
    resolve(&mut g);
    assert_eq!(g.state.rules.players[0].life, q(3));
}

/// CR 105.2, 701.21a: Ayara's "Sacrifice another black creature" offers
/// only the other black creatures its controller controls. Glimmer Bairn's
/// "Sacrifice a token" requires a token. "An artifact creature" is a
/// conjunction of two types and still rejects.
#[test]
fn sacrifice_costs_filter_by_color_and_token_status() {
    let bairn = lowered(GLIMMER_BAIRN);
    assert!(format!("{:?}", bairn.activated).contains("token: Some(true)"));
    let jansen = rejected(JAN_JANSEN);
    assert!(
        jansen.contains("unsupported complete sacrifice filter"),
        "{jansen}"
    );
    let mut g = game(
        board(&[AYARA])
            .permanent_or_hand("Ayara, First of Locthwain", 0, 0, false)
            .permanent_or_hand("Typhoid Rats", 0, 0, false)
            .permanent_or_hand("Grizzly Bears", 0, 0, false)
            .permanent_or_hand("Vampire Nighthawk", 1, 1, false),
    );
    let ayara = Driver::object(&g, 0);
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: ayara,
            ability: 0,
        },
    );
    let d = Driver::request(&mut g, u64::MAX);
    assert_eq!(d.reason, Reason::SacrificeCost, "{d:?}");
    let rats = Driver::object(&g, 1);
    assert_eq!(d.candidates, vec![Candidate::Object { object: rats }]);
    Driver::pick_object(&mut g, rats);
    resolve(&mut g);
    assert_eq!(g.zone(Driver::object(&g, 1)), Some(graveyard(0)));
    assert_eq!(g.zone(Driver::object(&g, 2)), Some(Zone::Battlefield));
}

/// CR 614.1c, 614.12, 122.6: Diregraf Colossus enters with a counter for
/// each Zombie card in its incoming controller's graveyard; an opponent's
/// Zombie card does not count. Gatekeeper Gargoyle counts Gates on the
/// battlefield the same way.
#[test]
fn entry_counters_count_as_the_permanent_enters() {
    let gargoyle = lowered(GATEKEEPER_GARGOYLE);
    assert!(gargoyle.static_abilities.iter().any(|a| matches!(&a.effect,
        StaticEffect::EntersWithPlusOneCounters {
            for_each: Some(SetExpr::Matching { zone: ZoneExpr::Battlefield, .. }),
            ..
        })));
    let mut g = game(
        board(&[DIREGRAF_COLOSSUS, DEATH_BARON])
            .permanent_or_hand("Diregraf Colossus", 0, 0, true)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .permanent_or_hand("Swamp", 0, 0, false)
            .card("Death Baron", 0, 0, graveyard(0))
            .card("Diregraf Colossus", 0, 0, graveyard(0))
            .card("Grizzly Bears", 0, 0, graveyard(0))
            .card("Death Baron", 1, 1, graveyard(1)),
    );
    Driver::cast(&mut g, CardId(0));
    Driver::pay(&mut g, &[1, 2, 3]);
    resolve(&mut g);
    let colossus = Driver::object(&g, 0);
    assert_eq!(g.zone(colossus), Some(Zone::Battlefield));
    assert_eq!(g.current(colossus).unwrap().plus_one_counters, q(2));
    assert_eq!(pt(&g, 0), (q(4), q(4)));
}
