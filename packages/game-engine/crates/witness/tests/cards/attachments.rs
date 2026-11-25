//! CR 301.5, 303.4, 608.2b, 613.7e, 701.3, 702.6/11/18, 704.5m/n and 800.4a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use mtg_contracts::kir::{Cell, Tiers, definition_cells, definition_tiers};
use mtg_core::testkit::*;

fn game(cards: &[(&str, u8, bool)]) -> Game {
    game_with(cards, |_| {})
}

fn game_with(cards: &[(&str, u8, bool)], adapt: impl FnOnce(&mut Catalog)) -> Game {
    let mut catalog = FixtureCatalog::Witness.get().clone();
    for raw in [
        include_str!(
            "../../../../tests/fixtures/phase1-r4/c6c2f370-fc33-4907-975c-a07af3dc664d.json"
        ),
        include_str!(
            "../../../../tests/fixtures/phase1-r4/c1624d10-8838-4af8-aea1-a96c0fe6fd6b.json"
        ),
        include_str!(
            "../../../../tests/fixtures/phase1-r4/65986c1b-8e51-4604-b685-d82fa7d1263a.json"
        ),
        include_str!(
            "../../../../tests/fixtures/phase1-r4/5f5e0b10-c8cf-450c-bfd3-bcb0528ec330.json"
        ),
        include_str!(
            "../../../../tests/fixtures/phase1-r4/c8b143ad-43ec-4e0d-a440-e348daa31391.json"
        ),
        include_str!(
            "../../../../tests/fixtures/phase1-r4/ca204b66-8d0c-431a-8d34-282f7c2d17da.json"
        ),
    ] {
        let result = mtg_witness::compile(mtg_witness::R4_MANIFEST, raw);
        let mtg_compiler::CompileResult::Lowered { definition } = result else {
            panic!("{result:?}")
        };
        definition.validate().unwrap();
        assert_eq!(definition_tiers(&definition).unwrap(), Tiers::EXECUTES);
        assert!(
            definition_cells(&definition)
                .unwrap()
                .contains(&Cell::Attachment)
        );
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    adapt(&mut catalog);
    let mut board = Board::new(catalog, "attachment-witness")
        .step(TurnStep::PrecombatMain)
        .seats(3);
    for &(name, seat, hand) in cards {
        board = board.permanent_or_hand(name, seat, seat, hand);
    }
    Game::new(board.libraries("Forest", 4).prepare())
}

fn checked(g: &Game) {
    g.validate_state().unwrap();
    let snapshot = g.snapshot().unwrap();
    assert_eq!(
        Game::restore((*g.prepared).clone(), &snapshot)
            .unwrap()
            .snapshot()
            .unwrap(),
        snapshot
    );
    assert_recoverable(g.into());
}

fn equip(g: &mut Game, gear: ObjectId, host: ObjectId) {
    Driver::action(
        g,
        Action::ActivateAbility {
            object: gear,
            ability: 0,
        },
    );
    let r = Driver::request(g, u64::MAX);
    g.respond(r.player, r.id, r.targets(vec![host])).unwrap();
    mtg_witness::equipped::pay_and_resolve(g, u64::MAX, vec![]);
    checked(g);
}

#[test]
fn equipment_timestamps_and_targeting() {
    let mut g = game(&[
        ("Lightning Greaves", 0, false),
        ("Swiftfoot Boots", 0, false),
        ("Grizzly Bears", 0, false),
        ("Grizzly Bears", 0, false),
        ("Forest", 0, false),
        ("Forest", 0, false),
    ]);
    let [greaves, boots, a, b] = [0, 1, 2, 3].map(|i| Driver::object(&g, i));
    equip(&mut g, boots, a);
    assert!(g.targetable_by(a, PlayerId(0)));
    assert!(!g.targetable_by(a, PlayerId(1)));
    let timestamp = g.current(boots).unwrap().attachment_timestamp.unwrap();
    equip(&mut g, boots, a);
    assert_eq!(
        g.current(boots).unwrap().attachment_timestamp,
        Some(timestamp)
    );
    equip(&mut g, greaves, a);
    assert_eq!(g.current(greaves).unwrap().attached_to, Some(a));
    let old_timestamp = g.current(greaves).unwrap().attachment_timestamp;
    assert!(!g.targetable_by(a, PlayerId(0)));
    assert!(!g.targetable_by(a, PlayerId(1)));
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: greaves,
            ability: 0,
        },
    );
    let r = Driver::request(&mut g, u64::MAX);
    assert!(r.index(&Candidate::Object { object: a }).is_none());
    g.respond(r.player, r.id, r.targets(vec![b])).unwrap();
    mtg_witness::equipped::pay_and_resolve(&mut g, u64::MAX, vec![]);
    assert_eq!(g.current(greaves).unwrap().attached_to, Some(b));
    assert!(g.current(greaves).unwrap().attachment_timestamp > old_timestamp);
    checked(&g);
}

#[test]
fn aura_target_concedes_before_or_after_entry() {
    for before in [true, false] {
        let mut g = game(&[
            ("Pacifism", 0, true),
            ("Grizzly Bears", 1, false),
            ("Plains", 0, false),
            ("Plains", 0, false),
        ]);
        let creature = Driver::object(&g, 1);
        Driver::cast(&mut g, CardId(0));
        let r = Driver::request(&mut g, u64::MAX);
        g.respond(r.player, r.id, r.targets(vec![creature]))
            .unwrap();
        mtg_witness::equipped::pay_to_priority(&mut g, u64::MAX, vec![]);
        let on_stack = Driver::object(&g, 0);
        if !before {
            mtg_witness::equipped::pay_and_resolve(&mut g, u64::MAX, vec![]);
            assert_eq!(
                g.current(Driver::object(&g, 0)).unwrap().attached_to,
                Some(creature)
            );
            assert!(g.has_restriction(creature, CreatureRestriction::CantAttackOrBlock));
            checked(&g);
        }
        g.concede(PlayerId(1)).unwrap();
        mtg_witness::equipped::pay_and_resolve(&mut g, u64::MAX, vec![]);
        assert_eq!(
            g.current(Driver::object(&g, 0)).unwrap().zone,
            Zone::Graveyard { owner: PlayerId(0) }
        );
        assert_eq!(
            Driver::object(&g, 0).incarnation,
            on_stack.incarnation + if before { 1 } else { 2 }
        );
        checked(&g);
    }
}

#[test]
fn dying_host_triggers_skullclamp_before_equipment_detaches() {
    let mut g = game(&[
        ("Skullclamp", 0, false),
        ("Typhoid Rats", 0, false),
        ("Forest", 0, false),
    ]);
    let [gear, host] = [0, 1].map(|i| Driver::object(&g, i));
    equip(&mut g, gear, host);
    assert_eq!(g.current(gear).unwrap().attached_to, None);
    assert_eq!(g.current(gear).unwrap().zone, Zone::Battlefield);
    assert_eq!(
        g.state
            .rules
            .objects
            .values()
            .filter(|o| o.zone == Zone::Hand { owner: PlayerId(0) })
            .count(),
        2
    );
}

// Controlled variant: protection is granted by a resolved Equipment static.
// Pacifism and the Equip activations are the frozen R4 programs.
#[test]
fn gaining_protection_removes_aura_and_detaches_equipment() {
    let mut g = game_with(
        &[
            ("Pacifism", 0, true),
            ("Grizzly Bears", 0, false),
            ("Swiftfoot Boots", 0, false),
            ("Lightning Greaves", 0, false),
            ("Plains", 0, false),
            ("Plains", 0, false),
            ("Plains", 0, false),
        ],
        |c| {
            let boots = c
                .definitions
                .iter_mut()
                .find(|d| d.name == "Swiftfoot Boots")
                .unwrap();
            boots.characteristics.colors = vec![Color::W];
            let grants = c
                .definitions
                .iter_mut()
                .find(|d| d.name == "Lightning Greaves")
                .unwrap();
            let StaticEffect::Continuous { changes, .. } = &mut grants.static_abilities[0].effect
            else {
                panic!("static grants")
            };
            *changes = vec![kir::Change::AddKeywords {
                keywords: vec![KeywordAbility::ProtectionFromWhite],
            }];
            grants.capabilities = grants.requirements();
        },
    );
    let [host, boots, greaves] = [1, 2, 3].map(|id| Driver::object(&g, id));
    Driver::cast(&mut g, CardId(0));
    let r = Driver::request(&mut g, u64::MAX);
    g.respond(r.player, r.id, r.targets(vec![host])).unwrap();
    mtg_witness::equipped::pay_to_priority(&mut g, u64::MAX, vec![]);
    assert!(g.validate_activation(PlayerId(0), greaves, 0).is_err());
    mtg_witness::equipped::pay_and_resolve(&mut g, u64::MAX, vec![]);
    equip(&mut g, boots, host);
    equip(&mut g, greaves, host);
    assert_eq!(g.current(boots).unwrap().attached_to, None);
    assert_eq!(g.current(boots).unwrap().zone, Zone::Battlefield);
    assert_eq!(
        g.current(Driver::object(&g, 0)).unwrap().zone,
        Zone::Graveyard { owner: PlayerId(0) }
    );
    assert_eq!(g.current(greaves).unwrap().attached_to, Some(host));
}

#[test]
fn combat_triggers_match_only_the_equipped_attacker() {
    for host_attacks in [false, true] {
        let mut g = game(&[
            ("Goldvein Pick", 0, false),
            ("Shard of Broken Glass", 0, false),
            ("Grizzly Bears", 0, false),
            ("Grizzly Bears", 0, false),
            ("Forest", 0, false),
            ("Forest", 0, false),
        ]);
        let [pick, shard, host, other] = [0, 1, 2, 3].map(|i| Driver::object(&g, i));
        equip(&mut g, pick, host);
        equip(&mut g, shard, host);
        let mut milled = false;
        for _ in 0..80 {
            let r = Driver::request(&mut g, u64::MAX);
            if g.state.rules.turn.step == TurnStep::PostcombatMain {
                break;
            }
            let answer = match r.reason {
                Reason::Priority => mtg_witness::temporary::pass(&r),
                Reason::Attackers => {
                    r.attack(vec![(if host_attacks { host } else { other }, PlayerId(1))])
                }
                Reason::Blockers => r.block(vec![]),
                Reason::OptionalEffect => {
                    assert!(!milled);
                    milled = true;
                    checked(&g);
                    r.pick_one(&Candidate::Flag { flag: Flag::Accept }).unwrap()
                }
                _ => panic!("unexpected combat decision {:?}", r.reason),
            };
            g.respond(r.player, r.id, answer).unwrap();
        }
        assert_eq!(g.state.rules.turn.step, TurnStep::PostcombatMain);
        assert_eq!(milled, host_attacks);
        assert_eq!(g.state.rules.tokens.len(), usize::from(host_attacks));
        checked(&g);
    }
}

// Isolate the noncreature Aura target domains. Their R4 cards also contain
// other printed clauses, whose independent rejection must be preserved.
#[test]
fn aura_filter_governs_announcement_and_entry_on_noncreatures() {
    for (types, target) in [
        (vec![CardKind::Artifact], 1),
        (vec![CardKind::Land], 2),
        (vec![], 1),
    ] {
        let mut g = game_with(
            &[
                ("Pacifism", 0, true),
                ("Swiftfoot Boots", 1, false),
                ("Plains", 0, false),
                ("Plains", 0, false),
            ],
            |catalog| {
                let aura = catalog
                    .definitions
                    .iter_mut()
                    .find(|d| d.name == "Pacifism")
                    .unwrap();
                let spec = &mut aura.spell.as_mut().unwrap().targets[0];
                spec.domain = TargetDomain::BattlefieldPermanent;
                spec.filter = Some(PermanentFilter {
                    types: types.clone(),
                    non_types: vec![],
                    non_colors: vec![],
                    opponent: false,
                    tapped: false,
                    power: None,
                });
                aura.capabilities = aura.requirements();
            },
        );
        Driver::cast(&mut g, CardId(0));
        let r = Driver::request(&mut g, u64::MAX);
        let gear = Driver::object(&g, 1);
        let land = Driver::object(&g, 2);
        assert_eq!(
            r.objects_offered().contains(&gear),
            types.is_empty() || types.contains(&CardKind::Artifact)
        );
        assert_eq!(
            r.objects_offered().contains(&land),
            types.is_empty() || types.contains(&CardKind::Land)
        );
        let host = Driver::object(&g, target);
        g.respond(r.player, r.id, r.targets(vec![host])).unwrap();
        mtg_witness::equipped::pay_and_resolve(&mut g, u64::MAX, vec![]);
        assert_eq!(
            g.current(Driver::object(&g, 0)).unwrap().attached_to,
            Some(host)
        );
        checked(&g);
    }
}

// The control effect is injected between paid activation and resolution.
// This isolates CR 608.2b from the admission of a printed control spell.
#[test]
fn equip_rechecks_control_and_keeps_its_previous_host() {
    let mut g = game(&[
        ("Swiftfoot Boots", 0, false),
        ("Grizzly Bears", 0, false),
        ("Grizzly Bears", 0, false),
        ("Forest", 0, false),
        ("Forest", 0, false),
    ]);
    let [boots, first, next] = [0, 1, 2].map(|i| Driver::object(&g, i));
    equip(&mut g, boots, first);
    let timestamp = g.current(boots).unwrap().attachment_timestamp;
    Driver::action(
        &mut g,
        Action::ActivateAbility {
            object: boots,
            ability: 0,
        },
    );
    let r = Driver::request(&mut g, u64::MAX);
    g.respond(r.player, r.id, r.targets(vec![next])).unwrap();
    mtg_witness::equipped::pay_to_priority(&mut g, u64::MAX, vec![]);
    for (id, host) in [first, next].into_iter().enumerate() {
        g.state.rules.control_effects.push(ControlEffect {
            id: u32::try_from(id).unwrap(),
            object: host,
            controller: PlayerId(1),
            source: ControlSource::Persistent,
        });
    }
    g.reconcile_controllers().unwrap();
    mtg_witness::equipped::pay_and_resolve(&mut g, u64::MAX, vec![]);
    assert_eq!(g.current(boots).unwrap().attached_to, Some(first));
    assert_eq!(g.current(boots).unwrap().attachment_timestamp, timestamp);
    assert_eq!(g.current(first).unwrap().controller, PlayerId(1));
    g.validate_state().unwrap();
}
