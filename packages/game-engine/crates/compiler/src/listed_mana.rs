//! CR 106.1, 109.5 and 605.1a: tap the source and add one mana of a listed
//! color to the activator's pool.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn colors(options: &[SourceMana]) -> Option<Vec<Color>> {
    if !(2..=5).contains(&options.len()) {
        return None;
    }
    let mut colors = Vec::with_capacity(options.len());
    for option in options {
        let SourceMana::Fixed { symbols } = option else {
            return None;
        };
        let [color] = symbols.as_slice() else {
            return None;
        };
        if *color == Color::C || colors.contains(color) {
            return None;
        }
        colors.push(*color);
    }
    Some(colors)
}

pub(super) fn printed_list(colors: &[Color]) -> String {
    let symbols: Vec<_> = colors
        .iter()
        .map(|color| format!("{{{color:?}}}"))
        .collect();
    if let [first, second] = symbols.as_slice() {
        format!("{first} or {second}")
    } else {
        let (last, rest) = symbols.split_last().expect("validated listed colors");
        format!("{}, or {last}", rest.join(", "))
    }
}

pub(super) fn lower(
    ability: &Ability,
    block: usize,
    line: &str,
    source: &Value,
    binding: &Value,
    identity: &str,
) -> Result<Option<ManaAbility>, Failure> {
    let Ability::Activated {
        costs,
        effects,
        restriction,
        ability_word,
    } = ability
    else {
        return Ok(None);
    };
    let [
        Sentence {
            bindings,
            steps,
            condition,
            condition_kind,
        },
    ] = effects.as_slice()
    else {
        return Ok(None);
    };
    let [
        Step::AddMana {
            who,
            mana: SourceMana::Choice { options },
        },
    ] = steps.as_slice()
    else {
        return Ok(None);
    };
    let bad = || unsupported("mana", "unsupported complete listed mana ability");
    if !matches!(costs.as_slice(), [SourceCost::TapSelf {}])
        || restriction.is_some()
        || ability_word.is_some()
        || condition.is_some()
        || condition_kind.is_some()
        || !bindings.is_empty()
        || who.player != "you"
        || who.selection_id.is_some()
        || who.of.is_some()
    {
        return Err(bad());
    }
    let colors = colors(options).ok_or_else(bad)?;
    if normalization::literal_spelling(line) != format!("{{T}}: Add {}.", printed_list(&colors)) {
        return Err(invalid(
            "lines",
            "listed mana does not match complete printed ability",
        ));
    }

    let root = format!("/faces/0/abilities/{block}");
    let mana = format!("{root}/effects/0/steps/0");
    let mut graph = bindings::Bindings::new(identity, source);
    graph.ability(&root, "activated", "activator");
    let activator = graph.id(&root, "you");
    let object = graph.id("/source", "object");
    let you = json!({"player":"you"});
    graph.event(&root, &mana, "add-mana", &activator, false);
    graph.reference(
        &format!("{root}/costs/0/source"),
        &json!({"ref":"self"}),
        &object,
        "201.5",
    );
    graph.reference(&format!("{root}/costs/0/payer"), &you, &activator, "109.5");
    graph.reference(&format!("{mana}/who"), &you, &activator, "109.5");
    graph.validate_block(binding, &root, "incomplete listed mana binding graph")?;

    Ok(Some(ManaAbility {
        self_damage: None,
        restriction: None,
        costs: PermanentActivationCosts {
            tap: true,
            mana: None,
            sacrifice: false,
            remove_source_plus_one_counters: None,
        },
        production: ManaProduction::Choice { colors },
        source_block: source_block(block)?,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    const NOMAD_OUTPOST: &str =
        include_str!("../../../tests/fixtures/4619de7e-3d6e-4c6b-8e6e-e24db324839d.json");

    #[test]
    fn listed_mana_requires_exact_printed_colors_and_activator() {
        assert_eq!(
            sha256(NOMAD_OUTPOST.trim_end().as_bytes()),
            "25e2c6fbf672f3c1699bfd1e8b7c7d3c1cfe59493b9da662391dd22a646e15a1"
        );
        let result = compile(MANIFEST, NOMAD_OUTPOST);
        let CompileResult::Lowered { definition } = result else {
            panic!("expected Nomad Outpost to lower: {result:?}");
        };
        assert_eq!(
            definition.static_abilities[0].effect,
            StaticEffect::EntersTapped
        );
        assert!(definition.spell.is_none());
        let [mana] = definition.mana_abilities.as_slice() else {
            panic!("expected one mana ability");
        };
        assert_eq!(
            mana.production,
            ManaProduction::Choice {
                colors: vec![Color::R, Color::W, Color::B],
            },
        );
        assert_eq!(definition.source_labels.as_ref().unwrap().mana, [1]);

        let wrong_color = crate::tests::changed_source(NOMAD_OUTPOST, |record| {
            let result = &mut record["faces"][0]["result"];
            result["abilities"][1]["effects"][0]["steps"][0]["mana"]["options"][0]["symbols"][0] =
                json!("U");
            result["lines"][1]["ability"] = result["abilities"][1].clone();
        });
        assert!(matches!(
            compile(MANIFEST, &wrong_color),
            CompileResult::Invalid { path, .. } if path == "lines"
        ));

        let wrong_activator = crate::tests::changed_source(NOMAD_OUTPOST, |record| {
            let references = record["binding"]["references"].as_array_mut().unwrap();
            let recipient = references
                .iter_mut()
                .find(|reference| reference["path"] == "/faces/0/abilities/1/effects/0/steps/0/who")
                .unwrap();
            let identity = recipient["target"]
                .as_str()
                .unwrap()
                .split('/')
                .next()
                .unwrap();
            recipient["target"] = json!(format!("{identity}/faces/0/abilities/0#you"));
        });
        assert!(matches!(
            compile(MANIFEST, &wrong_activator),
            CompileResult::Unsupported { path, .. } if path == "binding"
        ));
    }
}
