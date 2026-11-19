//! CR 102.2, 106.7, 109.5 and 605.1a: the activating player's opponents
//! define the eligible land colors at resolution.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const ORACLE_TEXT: &str =
    "{T}: Add one mana of any color that a land an opponent controls could produce.";

pub(super) fn lower(
    abilities: &[Ability],
    parsed: &ParseResult,
    binding: &Value,
    identity: &str,
    card: &Source,
) -> Result<Option<ManaAbility>, Failure> {
    let candidate = abilities.iter().any(|ability| {
        let Ability::Activated { effects, .. } = ability else {
            return false;
        };
        effects
            .iter()
            .flat_map(|sentence| &sentence.steps)
            .any(|step| {
                matches!(
                    step,
                    Step::AddMana {
                        mana: SourceMana::CouldProduce { .. },
                        ..
                    }
                )
            })
    });
    if !candidate {
        return Ok(None);
    }
    let bad = || unsupported("mana.family", "unsupported complete opponent-land mana source");
    let [Ability::Activated { effects, .. }] = abilities else {
        return Err(bad());
    };
    let [Sentence { steps, .. }] = effects.as_slice() else {
        return Err(bad());
    };
    let [Step::AddMana {
        mana:
            SourceMana::CouldProduce {
                amount,
                what,
                color_only,
            },
        ..
    }] = steps.as_slice() else {
        return Err(bad());
    };
    let query = json!({
        "ref":"filter",
        "filter":{"types":["land"],"control":"opponent"},
        "count":{"amount":"fixed","value":1}
    });
    if !matches!(
        (card.type_line.as_str(), card.mana_cost.as_str()),
        ("Land", "") | ("Artifact", "{2}")
    )
        || card.oracle_text != ORACLE_TEXT
        || !fixed_one(amount)
        || what != &query
        || !*color_only
    {
        return Err(bad());
    }

    // Raw equality closes optional/null fields as well as extra costs, blocks,
    // step effects and restrictions. The caller proves the line mirror.
    let raw: Value = decode(parsed.abilities.get(), "abilities")?;
    if raw != json!([{
        "kind":"activated","costs":[{"cost":"tap-self"}],
        "effects":[{"steps":[{"effect":"add-mana","who":{"player":"you"},"mana":{
            "mana":"could-produce","amount":{"amount":"fixed","value":1},"what":query,"colorOnly":true
        }}]}]
    }]) {
        return Err(bad());
    }
    bind(binding, identity, card, &query)?;
    Ok(Some(ManaAbility {
        costs: PermanentActivationCosts {
            tap: true,
            mana: None,
            sacrifice: false,
            remove_source_plus_one_counters: None,
        },
        production: ManaProduction::OpponentLandColors,
        restriction: None,
        self_damage: None,
        source_block: 0,
    }))
}

fn bind(binding: &Value, identity: &str, card: &Source, query: &Value) -> Result<(), Failure> {
    let bad = || unsupported("binding.mana", "incomplete opponent-land mana binding closure");
    let root = format!("card:{identity}");
    let source = format!("{root}/source#object");
    let face = format!("{root}/faces/0#face");
    let path = "/faces/0/abilities/0";
    let step = format!("{path}/effects/0/steps/0");
    let what = format!("{step}/mana/what");
    let control = format!("{what}/filter/control");
    let ability = format!("{root}{path}#ability");
    let you = format!("{root}{path}#you");
    let opponents = format!("{root}{control}#opponents");
    let selection = format!("{root}{what}#query");
    let mut expected = json!({
        "artifactKind":"oracle-reference-binding","schemaVersion":1,"executable":false,
        "identity":identity,
        "rules":{"edition":RULES_EDITION,"url":"https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt"},
        "nodes":[
            {"id":source,"path":"/source","node":"source","identity":identity,"rules":["201.5"]},
            {"id":face,"path":"/faces/0","node":"face","source":source,"sourceFaceIndex":0,
             "name":card.name,"layout":card.layout,"typeLine":card.type_line,"manaCost":card.mana_cost,"rules":["712"]},
            {"id":ability,"path":path,"node":"ability","source":source,"face":face,"abilityKind":"activated",
             "you":you,"rules":["113","109.5"]},
            {"id":you,"path":path,"node":"runtime-player","scope":ability,"role":"activator","rules":["109.5"]},
            {"id":opponents,"path":control,"node":"derived-player","scope":ability,"role":"opponents","of":you,
             "rules":["102.2","109.5"]},
            {"id":selection,"path":what,"node":"selection","scope":ability,"expression":query,"cardinality":"one",
             "timing":"resolution","selection":"evaluate-reference","rules":["608.2h"]},
            {"id":format!("{root}{step}#event"),"path":step,"node":"event","scope":ability,"operation":"add-mana",
             "inputs":[you],"availability":"when-performed","rules":["608.2c"]}
        ],
        "references":[
            {"path":format!("{path}/costs/0/source"),"expression":{"ref":"self"},"binding":"bound","target":source,
             "rules":["201.5"]},
            {"path":format!("{path}/costs/0/payer"),"expression":{"player":"you"},"binding":"bound","target":you,
             "rules":["109.5"]},
            {"path":format!("{step}/who"),"expression":{"player":"you"},"binding":"bound","target":you,
             "rules":["109.5"]},
            {"path":control,"expression":{"implicit":"opponents"},"binding":"bound","target":opponents,"rules":["102.2"]},
            {"path":what,"expression":query,"binding":"bound","target":selection,"rules":["608.2h"]}
        ],
        "diagnostics":[],"counts":{"obligations":5,"bound":5,"unresolved":0},
        "referencesBound":true,"engineFeatures":[],"fullyBound":true
    });
    let mut actual = binding.clone();
    for (key, by) in [("nodes", "id"), ("references", "path")] {
        for graph in [&mut actual, &mut expected] {
            graph[key]
                .as_array_mut()
                .ok_or_else(bad)?
                .sort_by(|a, b| a[by].as_str().cmp(&b[by].as_str()));
        }
    }
    if actual != expected {
        return Err(bad());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str =
        include_str!("../../../tests/fixtures/ramos/archive-manifest-current.json");
    const ORCHARD: &str =
        include_str!("../../../tests/fixtures/ramos/27b047e3-0d41-45e2-98e9-9391d7923a1e.json");
    const STONE: &str =
        include_str!("../../../tests/fixtures/ramos/95560508-7ac9-4be9-8a3f-3c7d5b52807b.json");

    #[test]
    fn opponent_land_mana_frozen_sources_and_closed_binding() {
        let registry: Vec<AcceptedArchive> =
            strict_read(include_str!("../../../tests/fixtures/ramos/accepted-archives.json"))
                .unwrap();
        let manifest_pin = accepted_manifest(MANIFEST).unwrap();
        let archive = registry
            .iter()
            .find(|archive| archive.manifest_sha256 == manifest_pin.manifest_sha256)
            .unwrap();
        for (raw, kind, capability) in [
            (ORCHARD, CardKind::Land, Capability::LandPlayV1),
            (STONE, CardKind::Artifact, Capability::CastArtifactV1),
        ] {
            assert!(archive.record_sha256.contains(&sha256(raw.trim_end().as_bytes())));
            let result = compile(MANIFEST, raw);
            let CompileResult::Lowered { definition } = result else {
                panic!("{result:?}");
            };
            assert_eq!(definition.characteristics.kind, kind);
            assert!(opponent_land_mana_family(&definition));
            assert_eq!(definition.source_labels.as_ref().unwrap().mana, [0]);
            assert_eq!(
                definition.capabilities,
                BTreeSet::from([
                    Capability::NormalLayoutV1,
                    Capability::PermanentManaV1,
                    Capability::OpponentLandManaV1,
                    capability,
                ])
            );
        }

        let wrong_opponents = crate::tests::changed_source(STONE, |record| {
            let opponents = record["binding"]["nodes"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|node| node["role"] == "opponents")
                .unwrap();
            opponents["of"] = opponents["scope"].clone();
        });
        assert!(matches!(
            compile(MANIFEST, &wrong_opponents),
            CompileResult::Unsupported { path, .. } if path == "binding.mana"
        ));

        let colorless_allowed = crate::tests::changed_source(ORCHARD, |record| {
            let parsed = &mut record["faces"][0]["result"];
            parsed["abilities"][0]["effects"][0]["steps"][0]["mana"]["colorOnly"] = false.into();
            parsed["lines"][0]["ability"] = parsed["abilities"][0].clone();
        });
        assert!(matches!(
            compile(MANIFEST, &colorless_allowed),
            CompileResult::Unsupported { path, .. } if path == "mana.family"
        ));
    }
}
