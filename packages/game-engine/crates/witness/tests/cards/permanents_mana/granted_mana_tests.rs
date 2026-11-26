//! Quoted mana abilities granted by battlefield statics: Chromatic Lantern,
//! Gemhide Sliver and Abundant Growth from exact frozen R4 source. A granted
//! ability belongs to its recipient, which taps itself; "you" is the player
//! who activates it. Pinned CR 106.7, 109.5, 113.10, 113.10a, 302.6,
//! 605.1a, 605.3a, 613.1f, 613.7a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::kir::{
    Atom, AttachmentKind, Cell, Change, ExecutionTier, Pred, Scope, StaticEffect, definition_cells,
    definition_tiers,
};
use mtg_contracts::{ManaProduction, strict_read};
use mtg_core::ManaAbilityIdentity;

const LANTERN: &str = include_str!(
    "../../../../../tests/fixtures/phase1-r4/539f5396-d99a-417d-a84c-dff7930b5900.json"
);
const GEMHIDE: &str = include_str!(
    "../../../../../tests/fixtures/phase1-r4/2c09ca09-8e62-4fe3-9b3d-61573dd2ffbc.json"
);
const GROWTH: &str = include_str!(
    "../../../../../tests/fixtures/phase1-r4/947a2665-2f4d-4193-8768-118f85334549.json"
);

fn lowered(raw: &str) -> Definition {
    let result = mtg_witness::compile(mtg_witness::R4_MANIFEST, raw);
    let mtg_compiler::CompileResult::Lowered { definition } = result else {
        panic!("frozen granted-mana witness must lower: {result:?}");
    };
    definition.validate().unwrap();
    *definition
}

fn catalog() -> Catalog {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    catalog
        .definitions
        .extend([LANTERN, GEMHIDE, GROWTH].map(lowered));
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    catalog
}

/// Cards are numbered in order: `(name, controller, in hand)`.
fn game(cards: &[(&str, u8, bool)]) -> Game {
    let mut board = Board::new(catalog(), "granted-mana-witness")
        .step(TurnStep::PrecombatMain)
        .seats(2);
    for &(name, seat, hand) in cards {
        board = board.permanent_or_hand(name, seat, seat, hand);
    }
    let mut g = Game::with_trace(board.libraries("Forest", 4).prepare(), TraceMode::Detailed);
    Driver::request(&mut g, u64::MAX);
    g
}

fn checked(g: &Game) {
    g.validate_state().unwrap();
    let snapshot = g.snapshot().unwrap();
    let restored = Game::restore(g.prepared.as_ref().clone(), &snapshot).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
    assert_recoverable(g.into());
}

fn granted(index: u32) -> Candidate<Rules> {
    Candidate::ManaAbility {
        ability: ManaAbilityIdentity::Granted { index },
    }
}

fn any_color() -> Vec<Candidate<Rules>> {
    [Color::W, Color::U, Color::B, Color::R, Color::G]
        .map(|color| Candidate::Color { color })
        .to_vec()
}

fn pick(g: &mut Game, expected: &[Candidate<Rules>], choice: Candidate<Rules>) {
    let d = Driver::request(g, u64::MAX);
    assert_eq!(d.player, PlayerId(0));
    assert_eq!(d.candidates, expected, "{:?}", d.reason);
    let answer = d.pick_one(&choice).unwrap();
    g.respond(d.player, d.id, answer).unwrap();
}

fn pool(g: &Game, seat: usize) -> BTreeMap<Color, Quantity> {
    g.state.rules.players[seat]
        .mana
        .iter()
        .filter(|(_, n)| **n > Quantity::zero())
        .map(|(c, n)| (*c, n.clone()))
        .collect()
}

fn grant(d: &Definition) -> (&Scope, &[mtg_contracts::ManaAbility]) {
    let [ability] = d
        .static_abilities
        .iter()
        .filter_map(|s| match &s.effect {
            StaticEffect::Continuous {
                affects,
                changes,
                condition: None,
            } => match changes.as_slice() {
                [Change::AddManaAbilities { abilities }] => Some((affects, abilities.as_slice())),
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>()[..]
    else {
        panic!("{} has one mana grant", d.name);
    };
    ability
}

#[test]
fn quoted_mana_grants_lower_with_distinct_recipients() {
    let c = catalog();
    let find = |name| c.definitions.iter().find(|d| d.name == name).unwrap();
    let you = Atom::ControlledBy {
        player: mtg_contracts::kir::PlayerExpr::static_controller(),
    };
    let land = Atom::Type {
        kind: CardKind::Land,
    };
    let lantern = find("Chromatic Lantern");
    let (scope, abilities) = grant(lantern);
    assert_eq!(
        *scope,
        Scope::Matching {
            filter: Pred(vec![land.clone(), you])
        }
    );
    // The printed ability stays printed; the quoted one is granted only.
    assert_eq!(lantern.mana_abilities.len(), 1);
    for a in abilities.iter().chain(&lantern.mana_abilities) {
        assert_eq!(a.production, ManaProduction::AnyColor);
        assert!(a.costs.tap && a.riders.is_empty() && a.costs.mana.is_none());
    }
    // "All Slivers": every controller's Slivers.
    let (scope, _) = grant(find("Gemhide Sliver"));
    assert_eq!(
        *scope,
        Scope::Matching {
            filter: Pred(vec![Atom::Subtype {
                subtype: "Sliver".into()
            }])
        }
    );
    let (scope, _) = grant(find("Abundant Growth"));
    assert_eq!(
        *scope,
        Scope::Attachment {
            kind: AttachmentKind::Enchanted,
            filter: Pred(vec![land]),
        }
    );
    for name in ["Chromatic Lantern", "Gemhide Sliver", "Abundant Growth"] {
        let d = find(name);
        assert!(d.capabilities.contains(&Capability::GrantedManaAbilitiesV1));
        assert!(
            definition_cells(d)
                .unwrap()
                .contains(&Cell::ManaAbilitySelection)
        );
        assert!(
            !definition_tiers(d)
                .unwrap()
                .admits(ExecutionTier::HeadlessQualified)
        );
    }
}

/// CR 113.10a, 605.3a, 106.7: the Forest chooses between its intrinsic {G}
/// and the granted ability, taps itself for blue, and the blue pays for a
/// spell. The opponent's Island gains nothing.
#[test]
fn lantern_land_taps_itself_for_granted_mana_and_spends_it() {
    let mut g = game(&[
        ("Chromatic Lantern", 0, false),
        ("Forest", 0, false),
        ("Island", 1, false),
        ("Storm Crow", 0, true),
        ("Forest", 0, false),
    ]);
    let forest = Driver::object(&g, 1);
    let island = Driver::object(&g, 2);
    assert_eq!(g.granted_mana_abilities(forest).len(), 1);
    assert!(g.granted_mana_abilities(island).is_empty());
    // The Lantern is not a land: it keeps only its printed ability.
    assert!(g.granted_mana_abilities(Driver::object(&g, 0)).is_empty());
    assert_eq!(g.mana_abilities(island).unwrap(), vec![Color::U]);
    assert_eq!(
        g.mana_abilities(forest).unwrap(),
        vec![Color::G, Color::W, Color::U, Color::B, Color::R]
    );

    Driver::cast(&mut g, CardId(3));
    for (land, color) in [(1, Color::U), (4, Color::B)] {
        let object = Driver::object(&g, land);
        let d = Driver::request(&mut g, u64::MAX);
        let source = d
            .index(&Candidate::Action {
                action: Action::ActivateMana { object },
            })
            .unwrap();
        g.respond(
            d.player,
            d.id,
            Answer::Pay {
                pay: PayAnswer::Activate { source },
            },
        )
        .unwrap();
        pick(
            &mut g,
            &[
                Candidate::ManaAbility {
                    ability: ManaAbilityIdentity::Intrinsic { color: Color::G },
                },
                granted(0),
            ],
            granted(0),
        );
        pick(&mut g, &any_color(), Candidate::Color { color });
        assert!(g.current(object).unwrap().tapped);
        Driver::request(&mut g, u64::MAX);
    }
    assert_eq!(
        pool(&g, 0),
        BTreeMap::from([(Color::U, 1.into()), (Color::B, 1.into())])
    );
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
    assert!(pool(&g, 0).is_empty());
    assert_eq!(g.state.rules.stack.len(), 1);
    for _ in g.live_players() {
        Driver::pass(&mut g);
    }
    Driver::request(&mut g, u64::MAX);
    assert_eq!(
        g.current(Driver::object(&g, 3)).unwrap().zone,
        Zone::Battlefield
    );
    checked(&g);
}

/// CR 302.6, 109.5: each Gemhide grants every Sliver an ability; a Sliver
/// taps only once its controller has controlled it since the turn began,
/// and the mana goes to the player who activates it. An opponent's Sliver
/// has the abilities too and publishes them.
#[test]
fn gemhide_slivers_grant_every_sliver_and_respect_summoning_sickness() {
    let mut g = game(&[("Gemhide Sliver", 0, false), ("Gemhide Sliver", 1, false)]);
    let mine = Driver::object(&g, 0);
    let theirs = Driver::object(&g, 1);
    assert_eq!(g.granted_mana_abilities(mine).len(), 2);
    assert_eq!(g.granted_mana_abilities(theirs).len(), 2);
    let QueryResult::Complete(QueryValue::Observation(view)) =
        g.observe(PlayerId(0), Budget::unlimited())
    else {
        panic!("observation");
    };
    for o in view
        .view
        .objects
        .iter()
        .filter(|o| o.zone == Zone::Battlefield)
    {
        assert_eq!(o.granted_mana.len(), 2);
        assert!(
            o.granted_mana
                .iter()
                .all(|a| a.production == ManaProduction::AnyColor)
        );
    }
    let mut sick = g.fork();
    sick.state
        .rules
        .objects
        .get_mut(&CardId(0))
        .unwrap()
        .attack_ready = false;
    assert!(
        !sick
            .next_test_decision()
            .candidates
            .contains(&Candidate::Action {
                action: Action::ActivateMana { object: mine }
            })
    );
    let answer = g.next_test_decision().activate_mana(mine);
    Driver::answer(&mut g, u64::MAX, answer);
    pick(&mut g, &[granted(0), granted(1)], granted(1));
    pick(&mut g, &any_color(), Candidate::Color { color: Color::R });
    Driver::request(&mut g, u64::MAX);
    assert_eq!(pool(&g, 0), BTreeMap::from([(Color::R, 1.into())]));
    assert!(pool(&g, 1).is_empty());
    assert!(g.current(mine).unwrap().tapped && !g.current(theirs).unwrap().tapped);
    checked(&g);
}

/// CR 303.4, 613.1f: the enchanted land has the ability, the other land
/// does not, and the draw trigger resolves.
#[test]
fn abundant_growth_grants_only_its_enchanted_land() {
    let mut g = game(&[
        ("Abundant Growth", 0, true),
        ("Forest", 0, false),
        ("Plains", 0, false),
        ("Mountain", 0, false),
    ]);
    let plains = Driver::object(&g, 2);
    let mountain = Driver::object(&g, 3);
    let hand = |g: &Game| {
        g.state
            .rules
            .objects
            .values()
            .filter(|o| o.zone == Zone::Hand { owner: PlayerId(0) })
            .count()
    };
    Driver::cast(&mut g, CardId(0));
    let r = Driver::request(&mut g, u64::MAX);
    assert!(r.objects_offered().contains(&plains));
    g.respond(r.player, r.id, r.targets(vec![plains])).unwrap();
    Driver::pay(&mut g, &[1]);
    let before = hand(&g);
    while !g.state.rules.stack.is_empty() || !g.state.rules.pending_triggers.is_empty() {
        Driver::pass(&mut g);
        Driver::request(&mut g, u64::MAX);
    }
    assert_eq!(hand(&g), before + 1);
    assert_eq!(
        g.current(Driver::object(&g, 0)).unwrap().attached_to,
        Some(plains)
    );
    assert_eq!(g.granted_mana_abilities(plains).len(), 1);
    assert!(g.granted_mana_abilities(mountain).is_empty());
    let answer = g.next_test_decision().activate_mana(plains);
    Driver::answer(&mut g, u64::MAX, answer);
    pick(
        &mut g,
        &[
            Candidate::ManaAbility {
                ability: ManaAbilityIdentity::Intrinsic { color: Color::W },
            },
            granted(0),
        ],
        granted(0),
    );
    pick(&mut g, &any_color(), Candidate::Color { color: Color::G });
    Driver::request(&mut g, u64::MAX);
    assert_eq!(pool(&g, 0), BTreeMap::from([(Color::G, 1.into())]));
    checked(&g);
}

/// Saved state names an exact current ability: a granted activation whose
/// slot, recipient or ability no longer matches is rejected on restore.
#[test]
fn forged_granted_mana_activations_reject() {
    let mut g = game(&[("Chromatic Lantern", 0, false), ("Forest", 0, false)]);
    let forest = Driver::object(&g, 1);
    let answer = g.next_test_decision().activate_mana(forest);
    Driver::answer(&mut g, u64::MAX, answer);
    pick(
        &mut g,
        &[
            Candidate::ManaAbility {
                ability: ManaAbilityIdentity::Intrinsic { color: Color::G },
            },
            granted(0),
        ],
        granted(0),
    );
    let d = Driver::request(&mut g, u64::MAX);
    assert_eq!(d.candidates, any_color());
    checked(&g);
    let valid = g.snapshot().unwrap();
    let base: serde_json::Value = strict_read(&valid).unwrap();
    let activation = &base["state"]["frame"]["activation"];
    assert_eq!(activation["granted"], true);
    let forge = |edit: &dyn Fn(&mut serde_json::Value)| {
        let mut forged = base.clone();
        edit(&mut forged["state"]["frame"]["activation"]);
        Game::restore(
            g.prepared.as_ref().clone(),
            &serde_json::to_string(&forged).unwrap(),
        )
        .is_err()
    };
    assert!(forge(&|a| a["index"] = 1.into()));
    assert!(forge(&|a| {
        a.as_object_mut().unwrap().remove("granted");
    }));
    assert!(forge(
        &|a| a["ability"]["production"] = serde_json::json!({"mana": "fixed", "symbols": ["G"]})
    ));
}

/// CR 113.10a, 602.1: only the admitted grant shape validates. A recipient
/// filter that reads tapped status, a guarding condition, or a cost, rider
/// or restriction the activation could not keep stable rejects by name.
#[test]
fn unadmitted_mana_grant_shapes_reject() {
    let lantern = lowered(LANTERN);
    let index = lantern
        .static_abilities
        .iter()
        .position(|s| matches!(s.effect, StaticEffect::Continuous { .. }))
        .unwrap();
    let edit = |change: &dyn Fn(&mut StaticEffect)| {
        let mut d = lantern.clone();
        change(&mut d.static_abilities[index].effect);
        d.validate()
    };
    let ability = |edit_ability: &dyn Fn(&mut mtg_contracts::ManaAbility)| {
        edit(&|effect: &mut StaticEffect| {
            let StaticEffect::Continuous { changes, .. } = effect else {
                unreachable!()
            };
            let [Change::AddManaAbilities { abilities }] = changes.as_mut_slice() else {
                unreachable!()
            };
            edit_ability(&mut abilities[0]);
        })
    };
    assert!(edit(&|_| {}).is_ok());
    let rejected = |result: Result<(), String>| {
        assert_eq!(result.unwrap_err(), "cell:static:continuous-change");
    };
    rejected(edit(&|effect| {
        let StaticEffect::Continuous { affects, .. } = effect else {
            unreachable!()
        };
        let Scope::Matching { filter } = affects else {
            unreachable!()
        };
        filter.0.push(Atom::Untapped);
    }));
    rejected(ability(&|a| a.costs.sacrifice = true));
    rejected(ability(&|a| {
        a.costs.mana = Some(mtg_contracts::ManaCost {
            generic: 1.into(),
            colored: vec![],
        })
    }));
    rejected(ability(&|a| a.costs.tap = false));
    rejected(ability(&|a| {
        a.restriction = Some(mtg_contracts::ManaActivationRestriction::OnceEachTurn)
    }));
    rejected(ability(&|a| {
        a.production = ManaProduction::Fixed { symbols: vec![] }
    }));
    rejected(edit(&|effect| {
        let StaticEffect::Continuous { changes, .. } = effect else {
            unreachable!()
        };
        changes[0] = Change::AddManaAbilities { abilities: vec![] };
    }));
    // A guarded grant is rejected before its changes are read.
    let guarded = edit(&|effect| {
        let StaticEffect::Continuous { condition, .. } = effect else {
            unreachable!()
        };
        *condition = Some(mtg_contracts::kir::Cond::ActivePlayer {
            player: mtg_contracts::kir::PlayerExpr::static_controller(),
        });
    });
    assert!(guarded.is_err());
}
