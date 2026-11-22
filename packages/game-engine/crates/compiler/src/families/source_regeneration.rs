//! The printed creature's activator pays fixed mana. The replacement names
//! the source object of that invocation (CR 109.5, 201.5, 701.19a).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::Value;

pub(crate) fn bind(d: &Definition, source: &Value, binding: &Value) -> Result<(), Failure> {
    if !d.capabilities.contains(&Capability::SourceRegenerationV1) {
        return Ok(());
    }
    let mut graph = bindings::Bindings::new(&d.oracle_id, source);
    let text = source["oracle_text"].as_str().unwrap_or_default();
    let blocks =
        mtg_contracts::source_regeneration::source_keywords(text, &d.provenance.abilities_json)
            .map_err(|error| unsupported("binding", error))?;
    for (block, keywords) in blocks.iter().enumerate() {
        let root = format!("/faces/0/abilities/{block}");
        graph.ability(&root, "keywords", "static-source-controller");
        for (index, keyword) in keywords.iter().enumerate() {
            let (word, rule) = mtg_contracts::source_regeneration::keyword(*keyword)
                .ok_or_else(|| unsupported("binding", "unbound regeneration source keyword"))?;
            cast_entry::bind_keyword(&mut graph, &root, index, *keyword, word, rule);
        }
    }
    let line = mtg_contracts::source_regeneration::activation_line(
        text.lines().last().unwrap_or_default(),
    );
    let block = source_block(blocks.len())?;
    let spec = fixed_creature_activation(line, &d.name, block)
        .ok_or_else(|| unsupported("binding", "missing complete printed regeneration"))?;
    cast_entry::bind_activation(&mut graph, block, &spec);
    graph.validate(
        binding,
        "requires complete regeneration source and activator",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const MANIFEST: &str = include_str!("../../../../tests/fixtures/archive-manifest-current.json");
    const SOURCE: &str =
        include_str!("../../../../tests/fixtures/8661d773-3b92-4fcf-8ffc-aced1ff7be8a.json");

    #[test]
    fn fixed_source_regeneration_binds_the_payer_and_exact_incarnation() {
        assert_eq!(
            sha256(SOURCE.trim_end().as_bytes()),
            "730fbbc2973c1d8f03218c3a0e04ffa529f84ff8f6f4536499b864e71d94d7bf"
        );
        let result = compile(MANIFEST, SOURCE);
        let CompileResult::Lowered { definition } = result else {
            panic!("expected Trestle Troll to lower: {result:?}");
        };
        assert_eq!(
            definition.keywords,
            [KeywordAbility::Defender, KeywordAbility::Reach]
        );
        assert_eq!(
            definition.activated[0].cost,
            ActivationCost::Mana {
                cost: ManaCost {
                    generic: 1.into(),
                    colored: vec![Color::B, Color::G]
                }
            }
        );
        assert_eq!(
            definition.activated[0].program.instructions,
            [Instruction::RegenerateSource { source_block: 2 }]
        );
        assert!(definition.activated[0].program.targets.is_empty());
        assert!(
            definition
                .capabilities
                .contains(&Capability::SourceRegenerationV1)
        );

        let wrong_subject = crate::tests::changed_source(SOURCE, |record| {
            let subject = record["binding"]["references"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|r| r["path"] == "/faces/0/abilities/2/effects/0/steps/0/what")
                .unwrap();
            subject["target"] = json!(
                subject["target"]
                    .as_str()
                    .unwrap()
                    .replace("/source#object", "/faces/0/abilities/2#you")
            );
        });
        assert!(matches!(compile(MANIFEST, &wrong_subject),
            CompileResult::Unsupported {path, ..} if path == "binding"));

        let added_target = crate::tests::changed_source(SOURCE, |record| {
            let result = &mut record["faces"][0]["result"];
            result["abilities"][2]["effects"][0]["steps"][0]["what"] =
                json!({"ref":"target","filter":{"types":["creature"]}});
            result["lines"][2]["ability"] = result["abilities"][2].clone();
        });
        assert!(matches!(
            compile(MANIFEST, &added_target),
            CompileResult::Unsupported { .. }
        ));
    }
}
