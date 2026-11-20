//! CR 109.5, 110.2a, 608.2c–d and 702.85a–c.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

const ABILITY_TEXT: &str = "As you cascade, you may put a land card from among the exiled cards onto the battlefield tapped.";
const ORACLE_TEXT: &str = concat!(
    "As you cascade, you may put a land card from among the exiled cards onto the battlefield tapped. ",
    "(Do this after the last card is exiled but before deciding whether to cast a nonland card.)"
);

pub(super) fn validate(
    abilities: &[Ability],
    parsed: &ParseResult,
    source: &Value,
    binding: &Value,
    card: &Source,
) -> Result<(), Failure> {
    if !card.oracle_text.contains(ABILITY_TEXT)
        && !abilities.iter().any(|ability| {
            matches!(
                ability,
                Ability::Static {
                    effect: SourceStaticEffect::AsCascade { .. },
                    ..
                }
            )
        })
    {
        return Ok(());
    }
    let bad = || {
        unsupported(
            "static.cascade_land_entry",
            "unsupported complete cascade land entry source",
        )
    };
    let [
        Ability::Static {
            effect: SourceStaticEffect::AsCascade { controller, choice },
            ..
        },
    ] = abilities
    else {
        return Err(bad());
    };
    let pool = json!({"ref":"cascade-exiled-cards"});
    let expected = json!([{"kind":"static","effect":{
        "static":"as-cascade","controller":{"player":"you"},"choice":{
            "optional":true,"from":pool,"filter":{"types":["land"],"cls":"card"},
            "count":1,"to":"battlefield","tapped":true
        }
    }}]);
    if card.oracle_text != ORACLE_TEXT
        || !matches!(parsed.lines.as_slice(), [line] if line.text == ABILITY_TEXT)
        || decode::<Value>(parsed.abilities.get(), "abilities")? != expected
    {
        return Err(bad());
    }

    // The source controller constrains which occurrence qualifies. The
    // occurrence separately supplies both the exiled pool and the decider.
    let mut graph = Bindings::new(&card.oracle_id, source);
    let base = "/faces/0/abilities/0";
    let effect = "/faces/0/abilities/0/effect";
    let controller_path = "/faces/0/abilities/0/effect/controller";
    let choice_path = "/faces/0/abilities/0/effect/choice";
    let pool_path = "/faces/0/abilities/0/effect/choice/from";
    graph.ability(base, "static", "static-source-controller");
    let scope = graph.id(base, "ability");
    let you = graph.id(base, "you");
    let occurrence = graph.node(
        effect,
        "cascade",
        "cascade-occurrence",
        json!({"scope":scope,"controllerMatches":you,"rules":["608.2c","702.85b","702.85c"]}),
    );
    let decider = graph.node(
        controller_path,
        "cascade-controller",
        "derived-player",
        json!({"scope":scope,"role":"controller","of":occurrence,"rules":["608.2c","702.85b"]}),
    );
    let exiled = graph.node(
        pool_path,
        "pool",
        "cascade-exiled-pool",
        json!({"occurrence":occurrence,"rules":["702.85a","702.85b","702.85c"]}),
    );
    graph.node(
        choice_path,
        "event",
        "event",
        json!({"scope":scope,"operation":"choose-and-put","inputs":[exiled],
            "availability":"when-performed","optional":true,"decider":decider,"rules":["608.2c"]}),
    );
    graph.reference(controller_path, controller, &you, "109.5");
    graph.reference(pool_path, &choice["from"], &exiled, "702.85b");
    graph.validate_with_features(
        binding,
        &[json!({"feature":"cascade-resolution","path":effect,
            "message":"Offer the choice from this cascade's cards still in exile after exiling, before the cast decision or bottom placement.",
            "rules":["608.2d","702.85b"]})],
        "incomplete cascade land entry binding graph",
    )
}

#[test]
fn cascade_land_entry() {
    let old_manifest = include_str!("../../../tests/fixtures/ramos/archive-manifest-current.json");
    let old =
        include_str!("../../../tests/fixtures/ramos/23695bc2-86d3-4d78-b374-9e83c4cc9cc4.json");
    let old_row: Value = strict_read(old).unwrap();
    assert!(matches!(
        compile(old_manifest, old),
        CompileResult::Unsupported { path, .. } if path == "status"
    ));

    let manifest =
        include_str!("../../../tests/fixtures/archive-manifest-averna-cascade-entry.json");
    let archive = accepted_manifest(manifest).unwrap();
    let Some(path) = std::env::var_os("MTG_CASCADE_LAND_ENTRY_ARCHIVE_PROBE") else {
        eprintln!(
            "Averna's frozen parse remains unsupported; private archive probe was not supplied"
        );
        return;
    };
    let raw = std::fs::read_to_string(path).unwrap();
    assert_eq!(
        sha256(raw.as_bytes()),
        *archive.file_sha256.as_ref().unwrap()
    );
    let lines: Vec<_> = raw.lines().collect();
    let [header, card] = lines.as_slice() else {
        panic!("expected the pinned manifest and one complete card row");
    };
    assert_eq!(*header, manifest.trim_end());
    assert_eq!(
        archive.record_sha256,
        BTreeSet::from([sha256(card.as_bytes())])
    );
    let row: Value = strict_read(card).unwrap();
    assert_eq!(row["source"], old_row["source"]);
    assert_eq!(row["sourceSha256"], old_row["sourceSha256"]);
    let result = compile(header, card);
    let CompileResult::Lowered { definition: d } = result else {
        panic!("{result:?}");
    };
    assert_eq!(d.name, "Averna, the Chaos Bloom");
    assert_eq!(d.characteristics.kind, CardKind::Creature);
    assert!(d.characteristics.legendary);
    assert_eq!(d.characteristics.subtypes, ["Elemental", "Shaman"]);
    assert_eq!(d.characteristics.power, Some(4.into()));
    assert_eq!(d.characteristics.toughness, Some(2.into()));
    assert_eq!(d.characteristics.mana_cost, mana_cost("{G}{U}{R}").unwrap());
    assert_eq!(
        d.characteristics.color_identity,
        [Color::G, Color::R, Color::U]
    );
    assert_eq!(
        d.static_abilities,
        [StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::CascadeLandEntry,
        }]
    );
    let spell = d.spell.as_ref().unwrap();
    assert_eq!(spell.resolution, SpellResolution::Permanent);
    assert_eq!(spell.timing, SpellTiming::MainPhase);
    assert!(spell.instructions.is_empty());
    assert_eq!(
        d.capabilities,
        BTreeSet::from([
            Capability::NormalLayoutV1,
            Capability::BaseCreatureV1,
            Capability::CastCreatureV1,
            Capability::CommanderIdentityV1,
            Capability::CascadeLandEntryV1,
        ])
    );
    let labels = d.source_labels.as_ref().unwrap();
    assert_eq!(labels.text, ORACLE_TEXT);
    assert_eq!(labels.blocks, [ABILITY_TEXT]);
    assert_eq!(labels.statics, [0]);
    assert_eq!(
        d.provenance.binding_sha256,
        row["bindingSha256"].as_str().unwrap()
    );

    for (pointer, value) in [
        ("/nodes/4/controllerMatches", json!("unbound")),
        ("/nodes/5/of", json!("a different occurrence")),
        ("/nodes/6/occurrence", json!("a different occurrence")),
        ("/nodes/7/inputs", json!([])),
        ("/nodes/7/optional", json!(false)),
        ("/nodes/7/decider", row["binding"]["nodes"][3]["id"].clone()),
        ("/references/1/target", json!("global exile")),
        ("/engineFeatures", json!([])),
        ("/fullyBound", json!(true)),
    ] {
        let bad = tests::changed_source(card, |r| {
            *r["binding"].pointer_mut(pointer).unwrap() = value;
        });
        assert!(
            matches!(compile(header, &bad), CompileResult::Unsupported { path, .. }
            if path == "binding"),
            "{pointer}"
        );
    }
    for (field, value) in [
        ("optional", json!(false)),
        ("from", json!({"ref":"exile"})),
        (
            "filter",
            json!({"types":["land", "creature"], "cls":"card"}),
        ),
        ("count", json!(2)),
        ("to", json!("hand")),
        ("tapped", Value::Null),
    ] {
        let bad = tests::changed_source(card, |r| {
            let p = &mut r["faces"][0]["result"];
            p["abilities"][0]["effect"]["choice"][field] = value;
            p["lines"][0]["ability"] = p["abilities"][0].clone();
        });
        assert!(
            matches!(compile(header, &bad), CompileResult::Unsupported { path, .. }
            if path == "static.cascade_land_entry"),
            "{field}"
        );
    }
}
