//! CR 601.2c, 608.2b, 613.4c (frozen 20260925).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::CreaturePowerFilter;
use std::io::{BufRead, BufReader};

fn scenario() -> Prepared {
    let mut rows = vec![
        ("Doom Blade", 0, 0, true),
        ("Grizzly Bears", 1, 1, false),
        ("Cursed Minotaur", 1, 1, false),
        ("Glorious Anthem", 1, 1, false),
        ("Giant Growth", 1, 1, true),
    ];
    lands(&mut rows, "Swamp", 0, 3);
    lands(&mut rows, "Forest", 1, 1);
    if let Some(path) = std::env::var_os("MTG_POWER_FILTERED_REMOVAL_ARCHIVE_PROBE") {
        let file = std::fs::File::open(path).expect("open frozen archive probe");
        let row = BufReader::new(file)
            .lines()
            .map(|line| line.expect("read frozen archive row"))
            .find(|line| {
                line.contains("Reaver Ambush")
                    && mtg_contracts::sha256(line.as_bytes())
                        == "9ce18b7fc3bc7e5bf2230c08f9449c0b325f73224d75a74614a6fd5ed3fdef55"
            })
            .expect("pinned Reaver Ambush, frozen physical row 561");
        rows[0].0 = "Reaver Ambush";
        return prepared_with_sources(&rows, &[&row]);
    }

    let mut pod = prepared(&rows);
    let catalog = Arc::make_mut(&mut pod.catalog);
    let removal = catalog
        .definitions
        .iter_mut()
        .find(|d| d.name == "Doom Blade")
        .unwrap();
    removal.oracle_id = "synthetic-power-filtered-exile".into();
    removal.name = "Synthetic power-filtered exile".into();
    removal.source_labels = None;
    removal.characteristics.mana_cost.generic = 2.into();
    let program = removal.spell.as_mut().unwrap();
    let filter = program.targets[0].filter.as_mut().unwrap();
    filter.non_colors.clear();
    filter.power = Some(CreaturePowerFilter::AtMost { value: 3.into() });
    program.instructions = vec![Instruction::ExilePermanent {
        target_slot: 0,
        source_block: 0,
    }];
    removal.capabilities = removal.requirements();
    pod.config.cards[0].oracle_id = removal.oracle_id.clone();
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    pod.catalog_digest = digest(catalog).unwrap();
    pod.profile_digest = digest(&pod.config).unwrap();
    pod
}

#[test]
fn power_filtered_removal_rechecks_current_power_after_paid_pump() {
    let mut g = Game::with_trace(scenario(), TraceMode::Detailed);
    let bear = object(&g, 1);
    let minotaur = object(&g, 2);
    assert_eq!(g.characteristics(bear).unwrap().power, Some(3.into()));
    assert_eq!(g.characteristics(minotaur).unwrap().power, Some(4.into()));

    answer(&mut g, 1, Response::Cast { card: CardId(0) });
    assert!(
        matches!(request(&mut g, 1), Request::Targets { candidates, .. } if candidates == [bear])
    );
    answer(
        &mut g,
        1,
        Response::Targets {
            objects: vec![bear],
        },
    );
    fund(&mut g, 1, "Swamp", 3);
    answer(&mut g, 1, Response::FinishMana { undo_mana: false });
    answer(
        &mut g,
        1,
        Response::SpendMana {
            spending: BTreeMap::from([(Color::B, 3.into())]),
        },
    );
    until_player(&mut g, 1, PlayerId(1));
    let removal = g.top().unwrap();
    assert_eq!(removal.targets, [bear]);
    assert_eq!(
        removal.paid,
        Some(ManaCost {
            generic: 2.into(),
            colored: vec![Color::B]
        })
    );
    let removal_id = removal.id;

    announce(&mut g, 1, 4, vec![TargetEntity::Object { object: bear }]);
    assert_eq!(
        g.top().unwrap().paid,
        Some(ManaCost {
            generic: 0.into(),
            colored: vec![Color::G]
        })
    );
    for _ in 0..4 {
        answer(&mut g, 1, Response::Pass);
    }
    assert!(matches!(request(&mut g, 1), Request::Priority { .. }));
    assert_eq!(g.top().unwrap().id, removal_id);
    assert_eq!(g.characteristics(bear).unwrap().power, Some(6.into()));
    recover(&g);

    resolve(&mut g, 1, |_| {});
    assert!(g.events().iter().any(|event| matches!(
        event, Event::SpellCompleted { stack_id, resolved: false } if *stack_id == removal_id
    )));
    assert_eq!(g.zone(bear), Some(Zone::Battlefield));
    assert_eq!(
        g.zone(object(&g, 0)),
        Some(Zone::Graveyard { owner: PlayerId(0) })
    );
    recover(&g);
}
