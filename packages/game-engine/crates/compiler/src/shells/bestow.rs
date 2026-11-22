//! Fixed-mana bestow announcement operands, CR 118.9 and 702.103a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

/// The cost belongs to the prospective caster. A static source controller is
/// a distinct bound actor even when both happen to be the same player.
pub(crate) fn cast_plan(
    source: &Value,
    result: &ParseResult,
    binding: &Value,
    identity: &str,
) -> Result<Option<BestowCastPlan>, Failure> {
    let text = source["oracle_text"].as_str().unwrap_or_default();
    if !text.lines().any(|line| line.starts_with("Bestow ")) {
        return Ok(None);
    }
    let unsupported = || {
        super::unsupported(
            "abilities.bestow",
            "requires one separately printed fixed-mana bestow cost and bound prospective caster",
        )
    };
    let lines = source_lines(
        text,
        result.name.as_str(),
        source["type_line"].as_str().ok_or_else(unsupported)?,
    )?;
    let mut matching = lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| l.strip_prefix("Bestow ").map(|mana| (i, mana)));
    let Some((index, printed)) = matching.next() else {
        return Err(unsupported());
    };
    if matching.next().is_some() {
        return Err(unsupported());
    }
    let mana = mana_cost(printed)?;
    let mut symbols = Vec::new();
    if mana.generic != Quantity::zero() || mana.colored.is_empty() {
        symbols.push(mana.generic.to_string())
    }
    symbols.extend(mana.colored.iter().map(|c| format!("{c:?}")));
    if symbols
        .iter()
        .map(|s| format!("{{{s}}}"))
        .collect::<String>()
        != printed
    {
        return Err(unsupported());
    }
    let ast = json!({"kind":"keywords","keywords":[{"keyword":"bestow","cost":[{"cost":"mana","symbols":symbols}]}]});
    let blocks: Vec<Value> = decode(result.abilities.get(), "abilities")?;
    let line = result.lines.get(index).ok_or_else(unsupported)?;
    if blocks.len() != lines.len()
        || result.lines.len() != lines.len()
        || blocks.get(index) != Some(&ast)
        || !line.ok
        || line.text != lines[index]
        || decode::<Value>(line.ability.get(), "abilities.bestow")? != ast
    {
        return Err(unsupported());
    }
    let root = format!("/faces/0/abilities/{index}");
    let keyword = format!("{root}/keywords/0");
    let mut graph = Bindings::new(identity, source);
    graph.ability(&root, "keywords", "static-source-controller");
    let scope = graph.id(&root, "ability");
    let caster = graph.node(
        &keyword,
        "you",
        "runtime-player",
        json!({"scope":scope,"role":"prospective-caster","rules":["109.5","702.103"]}),
    );
    graph.node(&keyword,"keyword","keyword-ability",json!({"scope":scope,"keyword":"bestow","abilities":["static"],"you":caster,"rules":["702.103"]}));
    graph.reference(
        &format!("{keyword}/cost/0/payer"),
        &json!({"player":"you"}),
        &caster,
        "109.5",
    );
    if index != 0
        || blocks.len() != 3
        || source["layout"] != "normal"
        || !source["type_line"].as_str().is_some_and(|ty| {
            type_line::exactly(ty, &[CardKind::Enchantment, CardKind::Creature], false)
        })
    {
        return Err(unsupported());
    }
    validate_siblings(&mut graph, &lines, &blocks, result)?;
    validate_keyword_inventory(source, &lines[1])?;
    graph.validate_ordered(
        binding,
        "requires the complete bestow payer, intrinsic keywords and enchanted-creature grant graph",
    )?;
    let types = type_line::parse(source["type_line"].as_str().ok_or_else(unsupported)?)?;
    let stat = |field: &str| -> Result<Option<Quantity>, Failure> {
        let digits = source[field].as_str().ok_or_else(unsupported)?;
        let value: Quantity = digits.parse().map_err(|_| unsupported())?;
        if value.to_string() != digits {
            return Err(unsupported());
        }
        Ok(Some(value))
    };
    let colors =
        |field: &str| serde_json::from_value(source[field].clone()).map_err(|_| unsupported());
    let printed = Characteristics {
        hybrid_mana: vec![],
        kind: types.kind,
        additional_types: types.additional_types,
        subtypes: types.subtypes,
        basic: types.basic,
        legendary: types.legendary,
        snow: types.snow,
        mana_cost: mana_cost(source["mana_cost"].as_str().ok_or_else(unsupported)?)?,
        colors: colors("colors")?,
        color_identity: colors("color_identity")?,
        power: stat("power")?,
        toughness: stat("toughness")?,
    };
    BestowCastPlan::new(&printed, mana)
        .map(Some)
        .map_err(|_| unsupported())
}

fn validate_keyword_inventory(source: &Value, intrinsic: &str) -> Result<(), Failure> {
    let fail = || {
        unsupported(
            "source.keywords",
            "requires exactly the printed bestow and intrinsic keywords",
        )
    };
    let raw = source["keywords"].as_array().ok_or_else(fail)?;
    let mut expected = std::collections::BTreeSet::from([String::from("bestow")]);
    expected.extend(intrinsic.split(", ").map(str::to_ascii_lowercase));
    let mut actual = std::collections::BTreeSet::new();
    for word in raw {
        if !actual.insert(word.as_str().ok_or_else(fail)?.to_ascii_lowercase()) {
            return Err(fail());
        }
    }
    if actual != expected {
        return Err(fail());
    }
    Ok(())
}

fn keyword_rule(word: &str) -> Option<&'static str> {
    Some(match word {
        "flying" => "702.9",
        "first strike" => "702.7",
        "vigilance" => "702.20",
        "trample" => "702.19",
        "lifelink" => "702.15",
        "reach" => "702.17",
        _ => return None,
    })
}
fn keyword_node(graph: &mut Bindings<'_>, scope: &str, path: &str, word: &str, rule: &str) {
    let actor = graph.node(
        path,
        "you",
        "runtime-player",
        json!({"scope":scope,"role":"static-source-controller","rules":["109.5",rule]}),
    );
    graph.node(
        path,
        "keyword",
        "keyword-ability",
        json!({"scope":scope,"keyword":word,"abilities":["static"],"you":actor,"rules":[rule]}),
    );
}
fn validate_siblings(
    graph: &mut Bindings<'_>,
    lines: &[String],
    blocks: &[Value],
    result: &ParseResult,
) -> Result<(), Failure> {
    let fail = || {
        unsupported(
            "abilities.bestow",
            "requires complete intrinsic keywords and enchanted creature modifiers",
        )
    };
    let printed = lines[1].to_ascii_lowercase();
    let words: Vec<_> = printed.split(", ").collect();
    let mut canonical = printed.clone();
    canonical
        .get_mut(..1)
        .ok_or_else(fail)?
        .make_ascii_uppercase();
    if canonical != lines[1] {
        return Err(fail());
    }
    let mut distinct = std::collections::BTreeSet::new();
    let intrinsic = "/faces/0/abilities/1";
    graph.ability(intrinsic, "keywords", "static-source-controller");
    for (i, word) in words.iter().enumerate() {
        if !distinct.insert(word) {
            return Err(fail());
        }
        let rule = keyword_rule(word).ok_or_else(fail)?;
        let scope = graph.id(intrinsic, "ability");
        keyword_node(
            graph,
            &scope,
            &format!("{intrinsic}/keywords/{i}"),
            word,
            rule,
        );
    }
    let intrinsic_ast = json!({"kind":"keywords","keywords":words.iter().map(|word|json!({"keyword":word})).collect::<Vec<_>>()});
    let static_root = "/faces/0/abilities/2";
    graph.ability(static_root, "static", "static-source-controller");
    let target = graph.enchanted_creature(static_root);
    let (stats, grant) = lines[2]
        .strip_prefix("Enchanted creature gets ")
        .and_then(|s| s.strip_suffix('.'))
        .and_then(|s| s.split_once(" and has "))
        .ok_or_else(fail)?;
    let (power, toughness) = stats.split_once('/').ok_or_else(fail)?;
    let amount = |s: &str| -> Result<Value, Failure> {
        let value = s
            .strip_prefix('+')
            .and_then(|s| s.parse::<u32>().ok())
            .filter(|v| *v > 0 && format!("+{v}") == s)
            .ok_or_else(fail)?;
        Ok(json!({"sign":1,"amount":{"amount":"fixed","value":value}}))
    };
    let mut grants = Vec::new();
    if let Some((rest, last)) = grant.rsplit_once(", and ") {
        grants.extend(rest.split(", "));
        grants.push(last);
    } else if let Some((a, b)) = grant.split_once(" and ") {
        grants.extend([a, b]);
    } else {
        grants.push(grant);
    }
    if grants.len() == 2 && grant.contains(", ") {
        return Err(fail());
    }
    let mut seen = std::collections::BTreeSet::new();
    for (i, word) in grants.iter().enumerate() {
        let rule = keyword_rule(word).ok_or_else(fail)?;
        if !seen.insert(word) {
            return Err(fail());
        }
        let path = format!("{static_root}/effect/modifiers/1/abilities/{i}");
        let source=graph.node(&path,"recipient","recipient",json!({"scope":graph.id(static_root,"ability"),"of":target,"each":false,"rules":["113.10","201.5a"]}));
        let scope=graph.node(&path,"ability","ability",json!({"source":source,"face":graph.id("/faces/0","face"),"abilityKind":"keywords","you":graph.id(&path,"you"),"rules":["113","113.10"]}));
        graph.node(&path,"you","runtime-player",json!({"scope":scope,"role":"static-source-controller","rules":["109.5","301.5d","303.4e"]}));
        keyword_node(graph, &scope, &format!("{path}/keyword"), word, rule);
    }
    let modifier = json!({"kind":"static","effect":{"static":"modify","what":{"ref":"enchanted","noun":"creature"},"modifiers":[
        {"modifier":"pt","power":amount(power)?,"toughness":amount(toughness)?},
        {"modifier":"abilities","abilities":grants.iter().map(|word|json!({"gained":"keyword","keyword":{"keyword":word}})).collect::<Vec<_>>()}]}});
    for (index, ast) in [(1, intrinsic_ast), (2, modifier)] {
        if result.lines[index].text != lines[index]
            || !result.lines[index].ok
            || blocks[index] != ast
            || decode::<Value>(result.lines[index].ability.get(), "abilities.bestow")? != ast
        {
            return Err(fail());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bestow_cost_binds_printed_mana_and_distinct_caster() {
        let record: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/ramos/695ee4fe-356d-47fb-9442-d171b618151c.json"
        ))
        .unwrap();
        let parse: ParseResult =
            decode(&record["faces"][0]["result"].to_string(), "result").unwrap();
        let identity = record["oracleIdentity"].as_str().unwrap();
        let source = &record["source"];
        let binding = &record["binding"];
        let plan = cast_plan(source, &parse, binding, identity)
            .unwrap()
            .unwrap();
        let found = plan.base_cost(BestowForm::Aura);
        assert_eq!(found.generic, 2.into());
        assert_eq!(
            found.colored,
            [Color::W, Color::U, Color::B, Color::R, Color::G]
        );
        let compiled = compile(
            include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json"),
            include_str!(
                "../../../../tests/fixtures/ramos/695ee4fe-356d-47fb-9442-d171b618151c.json"
            ),
        );
        let CompileResult::Lowered { definition } = compiled else {
            panic!("{compiled:?}")
        };
        assert_eq!(
            bestow_alternative(&definition),
            Some((found, &canonical_bestow_program()))
        );
        assert_eq!(definition.source_labels.as_ref().unwrap().statics, [0, 2]);
        assert!(
            matches!(&definition.static_abilities[1].effect,StaticEffect::ModifyCreatures{scope:CreatureScope::Enchanted,power,toughness,keywords,..}
            if power == &Quantity::from(4) && toughness == power && keywords == &definition.keywords && keywords.len()==5)
        );
        // A second source has different printed and alternative costs, base stats,
        // subtype and an attached keyword that the creature itself does not have.
        let raw = include_str!(
            "../../../../tests/fixtures/ramos/695ee4fe-356d-47fb-9442-d171b618151c.json"
        )
        .replace("Chromanticore", "Test Griffin")
        .replace("Manticore", "Griffin")
        .replace("{2}{W}{U}{B}{R}{G}", "{3}{W}")
        .replace("{W}{U}{B}{R}{G}", "{W}")
        .replace(r#"["2","W","U","B","R","G"]"#, r#"["3","W"]"#)
        .replace("and has flying", "and has reach")
        .replace("+4/+4", "+2/+3");
        let changed = crate::tests::changed_source(&raw, |r| {
            r["source"]["power"] = json!("1");
            r["source"]["toughness"] = json!("2");
            for key in ["colors", "color_identity"] {
                r["source"][key] = json!(["W"]);
            }
            let result = &mut r["faces"][0]["result"];
            let mut ability = result["abilities"][2].clone();
            let mods = &mut ability["effect"]["modifiers"];
            mods[0]["power"]["amount"]["value"] = json!(2);
            mods[0]["toughness"]["amount"]["value"] = json!(3);
            mods[1]["abilities"][0]["keyword"]["keyword"] = json!("reach");
            result["lines"][2]["ability"] = ability.clone();
            result["abilities"][2] = ability;
            for node in r["binding"]["nodes"].as_array_mut().unwrap() {
                if node["path"]
                    .as_str()
                    .unwrap()
                    .starts_with("/faces/0/abilities/2/")
                {
                    if node["keyword"] == "flying" {
                        node["keyword"] = json!("reach");
                    }
                    for rule in node["rules"].as_array_mut().unwrap() {
                        if rule.as_str() == Some("702.9") {
                            *rule = json!("702.17");
                        }
                    }
                }
            }
        });
        let compiled = compile(
            include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json"),
            &changed,
        );
        let CompileResult::Lowered { definition } = compiled else {
            panic!("{compiled:?}")
        };
        assert_eq!(bestow_alternative(&definition).unwrap().0.generic, 3.into());
        assert!(!definition.keywords.contains(&KeywordAbility::Reach));
        assert!(
            matches!(&definition.static_abilities[1].effect,StaticEffect::ModifyCreatures{keywords,..} if keywords.contains(&KeywordAbility::Reach))
        );
        let mut forged = binding.clone();
        forged["references"][0]["target"] = forged["nodes"][3]["id"].clone();
        assert!(cast_plan(source, &parse, &forged, identity).is_err());
        let mut detached = binding.clone();
        let recipient = detached["nodes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|node| node["node"] == "recipient")
            .unwrap();
        recipient["of"] = json!(format!("card:{identity}/source#object"));
        assert!(cast_plan(source, &parse, &detached, identity).is_err());
        let mut forged = source.clone();
        forged["oracle_text"] = json!(
            source["oracle_text"]
                .as_str()
                .unwrap()
                .replacen("{2}", "{3}", 1)
        );
        assert!(cast_plan(&forged, &parse, binding, identity).is_err());
    }
}
