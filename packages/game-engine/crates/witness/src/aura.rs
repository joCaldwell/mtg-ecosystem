//! Aura-control exact-source catalog preparation shared across binaries.
use mtg_core::testkit::*;
use std::sync::Arc;

pub const MIND: &str =
    include_str!("../../../tests/fixtures/5912546a-acc2-448c-b042-64bdac5ec129.json");
pub const MAGIC: &str =
    include_str!("../../../tests/fixtures/cd0d7141-46d2-4aa3-bc77-6b3b4513803e.json");
pub type Row = (&'static str, u8, u8, bool);
const CURRENT: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
pub fn prepared_with_sources(rows: &[Row], extras: &[&str]) -> Prepared {
    let extras: Vec<_> = extras.iter().map(|raw| (CURRENT, *raw)).collect();
    prepared_with_archived_sources(rows, &extras)
}
/// As `prepared_with_sources`, each extra row compiled under its own archive
/// manifest `(manifest, row)`.
pub fn prepared_with_archived_sources(rows: &[Row], extras: &[(&str, &str)]) -> Prepared {
    let mut catalog = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap();
    // Every added definition is compiled in its matching accepted archive
    // context. Never compare provenance from distinct parser/manifest identities.
    for (manifest, raw) in [
        MIND,
        MAGIC,
        include_str!("../../../tests/fixtures/9d8fe3f8-5027-4f9e-999e-e277caa96478.json"),
        include_str!("../../../tests/fixtures/2f990b54-fbf3-4949-85bb-9ba39710e72a.json"),
        include_str!("../../../tests/fixtures/1cb419cc-d8e4-4676-987f-089cebccc374.json"),
    ]
    .into_iter()
    .map(|raw| (CURRENT, raw))
    .chain(extras.iter().copied())
    {
        let mtg_compiler::CompileResult::Lowered { definition } = crate::compile(manifest, raw)
        else {
            panic!("complete frozen source must lower")
        };
        if raw == MIND || raw == MAGIC {
            assert!(mtg_contracts::aura_control_source(&definition));
            assert!(definition.capabilities.contains(&Capability::AuraControlV1));
        }
        catalog.definitions.push(*definition);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    let mut config = ScenarioConfig::turns_permanents();
    config.profile = "aura-control-private-source-witness".into();
    config.initial.turn_step = TurnStep::PrecombatMain;
    config.commanders.clear();
    config.cards = rows
        .iter()
        .enumerate()
        .map(|(i, (name, owner, controller, hand))| CardSetup {
            id: CardId(i as u32),
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
        .collect();
    for seat in 0..4 {
        for _ in 0..12 {
            config.cards.push(CardSetup {
                id: CardId(config.cards.len() as u32),
                oracle_id: FOREST.into(),
                owner: PlayerId(seat),
                controller: PlayerId(seat),
                zone: Zone::Library {
                    owner: PlayerId(seat),
                },
            });
        }
    }
    Prepared {
        catalog_digest: digest(&catalog).unwrap(),
        catalog: Arc::new(catalog),
        profile_digest: digest(&config).unwrap(),
        config,
        match_config: None,
    }
}
