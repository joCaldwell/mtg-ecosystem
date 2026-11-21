//! Runtime witnesses for definitions admitted by the composition matrix.
//!
//! Each test exercises one newly opened cell (stack-object kind, cost,
//! announcement route, source type, instruction) with an exact frozen R3
//! archive row, through announcement, payment, resolution, snapshot restore
//! and journal replay. Card names identify witnesses, never mechanics.
//!
//! CR 20260925 113.3, 302.6, 305.2, 400.7, 602.2, 603.3, 603.6a, 608.2,
//! 611.2c, 700.4, 701.8, 701.21a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::aura_control_tests::prepared_with_sources;
use super::equipped_indestructible_tests::{answer, both, object, recover, request};
use super::*;

const R3: &str = include_str!("../../../../tests/fixtures/archive-manifest-phase1-r3.json");
const CHALICE: &str =
    include_str!("../../../../tests/fixtures/phase1-r3/compose-marble-chalice.json");
const PORT: &str = include_str!("../../../../tests/fixtures/phase1-r3/compose-rishadan-port.json");
const STRIP: &str = include_str!("../../../../tests/fixtures/phase1-r3/compose-strip-mine.json");
const SEAL: &str =
    include_str!("../../../../tests/fixtures/phase1-r3/compose-seal-of-primordium.json");
const KAMAHL: &str =
    include_str!("../../../../tests/fixtures/phase1-r3/compose-kamahl-pit-fighter.json");
const PRIEST: &str =
    include_str!("../../../../tests/fixtures/phase1-r3/compose-icatian-priest.json");
const MARSH: &str = include_str!("../../../../tests/fixtures/phase1-r3/compose-piranha-marsh.json");
const DRAGON: &str =
    include_str!("../../../../tests/fixtures/phase1-r3/compose-destructor-dragon.json");
const PIE: &str = include_str!("../../../../tests/fixtures/phase1-r3/compose-bake-into-a-pie.json");

/// `(name, owner, controller, in hand)`; card IDs follow row order.
type Row = (&'static str, u8, u8, bool);

/// The shared paid-scenario catalog plus composed R3 rows, each compiled in
/// its own accepted archive context.
fn prepared(rows: &[Row], sources: &[&str]) -> Prepared {
    let mut p = prepared_with_sources(&[], &[]);
    let mut catalog = (*p.catalog).clone();
    for raw in sources {
        let mtg_compiler::CompileResult::Lowered { definition } = mtg_compiler::compile(R3, raw)
        else {
            panic!("composed frozen source must lower")
        };
        assert!(definition.composed);
        assert!(
            definition
                .capabilities
                .contains(&Capability::ComposedAbilityV1)
        );
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let library = std::mem::take(&mut p.config.cards);
    p.config.cards = rows
        .iter()
        .map(|(name, owner, controller, hand)| CardSetup {
            id: CardId(0),
            oracle_id: catalog
                .definitions
                .iter()
                .find(|d| d.name == *name)
                .unwrap()
                .oracle_id
                .clone(),
            owner: PlayerId(*owner),
            controller: PlayerId(*controller),
            zone: if *hand {
                Zone::Hand {
                    owner: PlayerId(*owner),
                }
            } else {
                Zone::Battlefield
            },
        })
        .chain(library)
        .collect();
    for (id, card) in p.config.cards.iter_mut().enumerate() {
        card.id = CardId(id as u32);
    }
    p.catalog_digest = digest(&catalog).unwrap();
    p.catalog = Arc::new(catalog);
    p.profile_digest = digest(&p.config).unwrap();
    p
}

/// Answer every announcement, payment and placement decision, passing
/// priority until the stack and pending triggers are empty. `announcements`
/// are the target responses, consumed in the order the runtime asks for them.
fn settle(g: &mut Game, q: u64, announcements: Vec<Response>) {
    let mut announcements = announcements.into_iter();
    for _ in 0..400 {
        let r = request(g, q);
        let response = match &r {
            Request::Priority { .. }
                if g.state.rules.stack.is_empty() && g.state.rules.pending_triggers.is_empty() =>
            {
                assert!(announcements.next().is_none(), "unused target announcement");
                return;
            }
            Request::Priority { .. } => Response::Pass,
            Request::Targets { .. }
            | Request::EntityTargets { .. }
            | Request::PlayerTargets { .. } => {
                announcements.next().expect("an expected target request")
            }
            Request::OrderTriggers { candidates, .. } => Response::OrderTriggers {
                order: candidates.clone(),
            },
            Request::Mana { cost, .. } => {
                if g.payment_pool(r.player(), cost).is_ok() {
                    Response::FinishMana { undo_mana: false }
                } else {
                    let land = g
                        .state
                        .rules
                        .objects
                        .values()
                        .find(|o| {
                            o.controller == r.player()
                                && o.zone == Zone::Battlefield
                                && !o.tapped
                                && g.definition(o.id.card).unwrap().characteristics.basic
                        })
                        .expect("an untapped basic land");
                    Response::ActivateMana { object: land.id }
                }
            }
            Request::SpendMana {
                cost, available, ..
            } => {
                let mut rest = available.clone();
                let mut spending = BTreeMap::<Color, Quantity>::new();
                for c in &cost.colored {
                    rest.get_mut(c).unwrap().subtract(&1.into());
                    spending.entry(*c).or_default().add(&1.into());
                }
                let mut generic = cost.generic.clone();
                for (c, n) in rest {
                    let used = n.min(generic.clone());
                    generic.subtract(&used);
                    if used > Quantity::zero() {
                        spending.entry(c).or_default().add(&used);
                    }
                }
                assert_eq!(generic, Quantity::zero());
                Response::SpendMana { spending }
            }
            x => panic!("unexpected composed decision {x:?}"),
        };
        g.respond(r.player(), r.decision_id(), response).unwrap();
    }
    panic!("bounded settlement");
}

fn objects(objects: &[ObjectId]) -> Response {
    Response::Targets {
        objects: objects.to_vec(),
    }
}

fn activate(g: &mut Game, q: u64, source: u32, announcements: Vec<Response>) {
    let source = object(g, source);
    answer(
        g,
        q,
        Response::ActivateAbility {
            object: source,
            ability: 0,
        },
    );
    settle(g, q, announcements);
}

fn graveyard(g: &Game, card: u32) -> bool {
    matches!(
        g.current(object(g, card)).unwrap().zone,
        Zone::Graveyard { .. }
    )
}

fn life(g: &Game, player: usize) -> Quantity {
    g.state.rules.players[player].life.clone()
}

fn stats(g: &Game, id: ObjectId) -> (i64, i64) {
    let c = g.characteristics(id).unwrap();
    let number = |q: &Option<Quantity>| q.as_ref().unwrap().to_string().parse().unwrap();
    (number(&c.power), number(&c.toughness))
}

fn plus(mut life: Quantity, amount: i64) -> Quantity {
    life.add(&amount.into());
    life
}

#[test]
fn composed_tap_activation_on_a_noncreature_source_resolves_a_targetless_program() {
    // CR 302.6 applies to creatures only; an artifact taps the turn it arrives.
    both(
        prepared(&[("Marble Chalice", 0, 0, false)], &[CHALICE]),
        |g, q| {
            let before = life(g, 0);
            activate(g, q, 0, vec![]);
            assert!(g.current(object(g, 0)).unwrap().tapped);
            assert_eq!(life(g, 0), plus(before, 1));
            recover(g);
        },
    );
}

#[test]
fn composed_paid_tap_activation_on_a_land_taps_an_announced_permanent() {
    both(
        prepared(
            &[
                ("Forest", 0, 0, false),
                ("Rishadan Port", 0, 0, false),
                ("Forest", 1, 1, false),
            ],
            &[PORT],
        ),
        |g, q| {
            let target = object(g, 2);
            assert!(!g.current(target).unwrap().tapped);
            activate(g, q, 1, vec![objects(&[target])]);
            assert!(g.current(target).unwrap().tapped);
            assert!(g.current(object(g, 1)).unwrap().tapped);
            assert!(g.current(object(g, 0)).unwrap().tapped);
            recover(g);
        },
    );
}

#[test]
fn composed_tap_and_sacrifice_cost_without_mana_destroys_an_announced_land() {
    // CR 602.2b, 701.21a: the complete cost is paid after the target is chosen.
    both(
        prepared(
            &[("Strip Mine", 0, 0, false), ("Forest", 1, 1, false)],
            &[STRIP],
        ),
        |g, q| {
            let target = object(g, 1);
            activate(g, q, 0, vec![objects(&[target])]);
            assert!(graveyard(g, 0));
            assert!(graveyard(g, 1));
            recover(g);
        },
    );
}

#[test]
fn composed_sacrifice_of_an_enchantment_source_destroys_an_announced_artifact() {
    both(
        prepared(
            &[
                ("Seal of Primordium", 0, 0, false),
                ("Sol Ring", 1, 1, false),
                ("Grizzly Bears", 1, 1, false),
            ],
            &[SEAL],
        ),
        |g, q| {
            let target = object(g, 1);
            activate(g, q, 0, vec![objects(&[target])]);
            assert!(graveyard(g, 0));
            assert!(graveyard(g, 1));
            assert_eq!(g.current(object(g, 2)).unwrap().zone, Zone::Battlefield);
            recover(g);
        },
    );
}

#[test]
fn composed_creature_tap_activation_deals_damage_to_an_announced_player() {
    both(
        prepared(&[("Kamahl, Pit Fighter", 0, 0, false)], &[KAMAHL]),
        |g, q| {
            let before = life(g, 1);
            let player = TargetEntity::Player {
                player: PlayerId(1),
            };
            activate(
                g,
                q,
                0,
                vec![Response::EntityTargets {
                    entities: vec![player],
                }],
            );
            assert!(g.current(object(g, 0)).unwrap().tapped);
            assert_eq!(before, plus(life(g, 1), 3));
            recover(g);
        },
    );
}

#[test]
fn composed_mana_activation_pumps_an_announced_creature_until_cleanup() {
    both(
        prepared(
            &[
                ("Plains", 0, 0, false),
                ("Plains", 0, 0, false),
                ("Plains", 0, 0, false),
                ("Icatian Priest", 0, 0, false),
                ("Grizzly Bears", 0, 0, false),
            ],
            &[PRIEST],
        ),
        |g, q| {
            let bears = object(g, 4);
            assert_eq!(stats(g, bears), (2, 2));
            activate(g, q, 3, vec![objects(&[bears])]);
            assert_eq!(stats(g, bears), (3, 3));
            assert!(!g.current(object(g, 3)).unwrap().tapped);
            recover(g);
        },
    );
}

#[test]
fn composed_land_entry_trigger_enters_tapped_and_drains_an_announced_player() {
    // CR 603.6a, 614.12: the land's own entry replacement and entry trigger.
    both(
        prepared(&[("Piranha Marsh", 0, 0, true)], &[MARSH]),
        |g, q| {
            let before = life(g, 2);
            answer(g, q, Response::PlayLand { card: CardId(0) });
            settle(
                g,
                q,
                vec![Response::PlayerTargets {
                    players: vec![PlayerId(2)],
                }],
            );
            let marsh = g.current(object(g, 0)).unwrap();
            assert_eq!(marsh.zone, Zone::Battlefield);
            assert!(marsh.tapped);
            assert_eq!(before, plus(life(g, 2), 1));
            recover(g);
        },
    );
}

#[test]
fn composed_dies_trigger_destroys_an_announced_noncreature_permanent() {
    // CR 700.4, 603.10a: the trigger looks back to the battlefield; its
    // program needs nothing from the departed source.
    both(
        prepared(
            &[
                ("Swamp", 0, 0, false),
                ("Swamp", 0, 0, false),
                ("Doom Blade", 0, 0, true),
                ("Destructor Dragon", 0, 0, false),
                ("Sol Ring", 1, 1, false),
                ("Grizzly Bears", 1, 1, false),
            ],
            &[DRAGON],
        ),
        |g, q| {
            let dragon = object(g, 3);
            let ring = object(g, 4);
            answer(g, q, Response::Cast { card: CardId(2) });
            // The spell announces the Dragon; its dies trigger then announces
            // the artifact.
            settle(g, q, vec![objects(&[dragon]), objects(&[ring])]);
            assert!(graveyard(g, 3));
            assert!(graveyard(g, 4));
            assert_eq!(g.current(object(g, 5)).unwrap().zone, Zone::Battlefield);
            recover(g);
        },
    );
}

#[test]
fn composed_spell_sequence_performs_its_clauses_in_printed_order() {
    // CR 608.2c, 701.8, 111.10: the spell destroys its target, then its
    // controller creates the predefined token.
    both(
        prepared(
            &[
                ("Swamp", 0, 0, false),
                ("Swamp", 0, 0, false),
                ("Swamp", 0, 0, false),
                ("Swamp", 0, 0, false),
                ("Bake into a Pie", 0, 0, true),
                ("Grizzly Bears", 1, 1, false),
            ],
            &[PIE],
        ),
        |g, q| {
            let bears = object(g, 5);
            answer(g, q, Response::Cast { card: CardId(4) });
            settle(g, q, vec![objects(&[bears])]);
            assert!(graveyard(g, 5));
            assert!(graveyard(g, 4));
            assert_eq!(g.state.rules.tokens.len(), 1);
            let token = *g.state.rules.tokens.keys().next().unwrap();
            let token = g.current(g.object_id(token).unwrap()).unwrap();
            assert_eq!(token.controller, PlayerId(0));
            assert_eq!(token.zone, Zone::Battlefield);
            let food = g.characteristics(token.id).unwrap();
            assert_eq!(food.kind, CardKind::Artifact);
            assert_eq!(food.subtypes, ["Food"]);
            recover(g);
        },
    );
}
