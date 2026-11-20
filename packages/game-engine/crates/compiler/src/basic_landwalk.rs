//! Whole-source gate for printed basic subtype landwalk, CR 702.14.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

pub(super) fn lower(
    source: &Source,
    shape: &Value,
    kind: CardKind,
    result: &ParseResult,
    binding: &Value,
) -> Result<Option<Vec<KeywordAbility>>, Failure> {
    let path = "abilities.landwalk";
    let Some(blocks) =
        mtg_contracts::basic_landwalk::source_keywords(&source.oracle_text, result.abilities.get())
            .map_err(|message| unsupported(path, message))?
    else {
        return Ok(None);
    };
    if kind != CardKind::Creature || source.layout != "normal" {
        return Err(unsupported(
            path,
            "basic landwalk requires a normal creature",
        ));
    }

    bind_intrinsic_keywords(
        &source.oracle_id,
        shape,
        binding,
        &blocks,
        path,
        "requires complete basic-landwalk keyword bindings",
    )?;
    Ok(Some(blocks.into_iter().flatten().collect()))
}

pub(super) fn bind_intrinsic_keywords(
    identity: &str,
    shape: &Value,
    binding: &Value,
    blocks: &[Vec<KeywordAbility>],
    path: &str,
    message: &str,
) -> Result<(), Failure> {
    let mut graph = Bindings::new(identity, shape);
    for (block, keywords) in blocks.iter().enumerate() {
        let root = format!("/faces/0/abilities/{block}");
        graph.ability(&root, "keywords", "static-source-controller");
        let scope = graph.id(&root, "ability");
        for (index, &keyword) in keywords.iter().enumerate() {
            let keyword_path = format!("{root}/keywords/{index}");
            let (word, rule) = keyword_metadata(keyword)
                .ok_or_else(|| unsupported(path, "no exact archived keyword scope"))?;
            let role = if keyword == KeywordAbility::Flash {
                "prospective-caster"
            } else {
                "static-source-controller"
            };
            let you = graph.node(
                &keyword_path,
                "you",
                "runtime-player",
                json!({"scope":scope,"role":role,"rules":["109.5",rule]}),
            );
            graph.node(
                &keyword_path, "keyword", "keyword-ability",
                json!({"scope":scope,"keyword":word,"abilities":["static"],"you":you,"rules":[rule]}),
            );
        }
    }
    graph.validate_ordered(binding, message)
}

pub(super) fn keyword_metadata(keyword: KeywordAbility) -> Option<(&'static str, &'static str)> {
    use KeywordAbility::*;
    Some(match keyword {
        Deathtouch => ("deathtouch", "702.2"),
        Defender => ("defender", "702.3"),
        DoubleStrike => ("double strike", "702.4"),
        FirstStrike => ("first strike", "702.7"),
        Flash => ("flash", "702.8"),
        Flying => ("flying", "702.9"),
        Haste => ("haste", "702.10"),
        Hexproof => ("hexproof", "702.11"),
        Indestructible => ("indestructible", "702.12"),
        Plainswalk | Islandwalk | Swampwalk | Mountainwalk | Forestwalk => ("landwalk", "702.14"),
        Lifelink => ("lifelink", "702.15"),
        Reach => ("reach", "702.17"),
        Shroud => ("shroud", "702.18"),
        Trample => ("trample", "702.19"),
        Vigilance => ("vigilance", "702.20"),
        Menace => ("menace", "702.111"),
        ProtectionFromWhite | ProtectionFromBlue | ProtectionFromBlack | ProtectionFromRed
        | ProtectionFromGreen => ("protection", "702.16"),
        ProtectionFromEachColor | ProtectionFromEverything => return None,
    })
}

#[cfg(test)]
#[test]
fn basic_landwalk_lowers_exact_frozen_source_and_rejects_changed_graph() {
    const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    const ROW: &str = include_str!("../../../tests/fixtures/basic-landwalk-zendikar-farguide.json");
    let compiled = compile(MANIFEST, ROW);
    let CompileResult::Lowered { definition } = compiled else {
        panic!("complete frozen Forestwalk source did not lower: {compiled:?}");
    };
    assert_eq!(definition.keywords, [KeywordAbility::Forestwalk]);
    assert!(
        definition
            .capabilities
            .contains(&Capability::BasicLandwalkV1)
    );
    let mut unbound = (*definition).clone();
    unbound.keywords.clear();
    unbound.capabilities = unbound.requirements();
    assert!(unbound.validate().is_err());

    let mut changed: Value = decode(ROW, "test").unwrap();
    let graph = changed["binding"]["nodes"].as_array_mut().unwrap();
    graph.last_mut().unwrap()["keyword"] = json!("flying");
    changed["bindingSha256"] = json!(sha256(canonical(&changed["binding"]).unwrap().as_bytes()));
    changed["sourceSha256"] = json!(sha256(canonical(&changed["source"]).unwrap().as_bytes()));
    changed["astSha256"] = json!(sha256(
        format!(
            "[{}]",
            canonical(&changed["faces"][0]["result"]["abilities"]).unwrap()
        )
        .as_bytes(),
    ));
    assert!(matches!(
        compile(MANIFEST, &canonical(&changed).unwrap()),
        CompileResult::Unsupported { path, .. } if path == "binding"
    ));
}
