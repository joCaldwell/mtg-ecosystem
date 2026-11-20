//! CR702.29a supplies the implicit self discard and mandatory controller draw;
//! the frozen AST intentionally contains only the keyword and its mana cost.
//! CR305.6 intrinsic typed mana and CR614.12 entry remain independent programs.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn lower(keyword: &SourceKeyword, block: u32) -> Result<ActivatedAbility, Failure> {
    let bad = || {
        unsupported(
            "cycling.cost",
            "cycling requires one complete fixed mana cost",
        )
    };
    let Some([SourceCost::Mana { symbols }]) = keyword.cost.as_deref() else {
        return Err(bad());
    };
    if keyword.keyword != "cycling" || keyword.filter.is_some() || symbols.is_empty() {
        return Err(bad());
    }
    let text = symbols
        .iter()
        .map(|s| format!("{{{s}}}"))
        .collect::<String>();
    let mana = mana_cost(&text)?;
    Ok(ActivatedAbility {
        functioning_zone: FunctioningZone::Hand,
        cost: ActivationCost::DiscardSource { mana },
        program: SpellProgram {
            optional_additional_costs: vec![],
            additional_sacrifice: None,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            targets: vec![],
            instructions: vec![Instruction::DrawCards {
                recipient: None,
                count: 1.into(),
                source_block: block,
            }],
            modes: vec![],
        },
    })
}

pub(super) fn bind(
    abilities: &[Ability],
    binding: &Value,
    identity: &str,
    card: &Source,
    parsed: &ParseResult,
) -> Result<(), Failure> {
    if !abilities.iter().any(|a| matches!(a, Ability::Keywords { keywords, .. } if keywords.iter().any(|k| k.keyword == "cycling"))) {
        return Ok(());
    }
    let bad = || {
        unsupported(
            "cycling.binding",
            "incomplete tapped typed-land cycling source/binding closure",
        )
    };
    let [
        Ability::Static {
            effect:
                SourceStaticEffect::EntersTapped {
                    what,
                    condition_kind: None,
                    condition: None,
                },
            ability_word: None,
        },
        Ability::Keywords {
            keywords,
            ability_word: None,
        },
    ] = abilities
    else {
        return Err(bad());
    };
    let [keyword] = keywords.as_slice() else {
        return Err(bad());
    };
    lower(keyword, 1)?;
    let Some([SourceCost::Mana { symbols }]) = keyword.cost.as_deref() else {
        return Err(bad());
    };
    let costs = json!([{"cost":"mana","symbols":symbols}]);
    let raw: Value = decode(parsed.abilities.get(), "cycling.abilities")?;
    // Reject explicit null optional fields as well as tails that serde Option
    // alone would erase. No parser/fixture rewrite or reminder-text execution.
    if *what != json!({"ref":"this","noun":"land"})
        || raw
            != json!([
                {"kind":"static","effect":{"static":"enters-tapped","what":what}},
                {"kind":"keywords","keywords":[{"keyword":"cycling","cost":costs}]}
            ])
        || parsed.lines.len() != 2
        || normalization::literal_spelling(&parsed.lines[0].text) != "This land enters tapped."
        || normalization::literal_spelling(&parsed.lines[1].text)
            != format!(
                "Cycling {}",
                symbols
                    .iter()
                    .map(|s| format!("{{{s}}}"))
                    .collect::<String>()
            )
    {
        return Err(bad());
    }
    let root = format!("card:{identity}");
    let source = format!("{root}/source#object");
    let face = format!("{root}/faces/0#face");
    let mut nodes = vec![
        json!({"id":source,"path":"/source","node":"source","identity":identity}),
        json!({"id":face,"path":"/faces/0","node":"face","source":source,"sourceFaceIndex":0,"name":card.name,"layout":card.layout,"typeLine":card.type_line,"manaCost":card.mana_cost}),
    ];
    for (block, kind) in [(0, "static"), (1, "keywords")] {
        let path = format!("/faces/0/abilities/{block}");
        let ability = format!("{root}{path}#ability");
        let you = format!("{root}{path}#you");
        nodes.push(json!({"id":ability,"path":path,"node":"ability","source":source,"face":face,"abilityKind":kind,"you":you}));
        nodes.push(json!({"id":you,"path":path,"node":"runtime-player","scope":ability,"role":"static-source-controller"}));
        if block == 0 {
            nodes.push(json!({"id":format!("{root}{path}/effect#entering-controller"),"path":format!("{path}/effect"),"node":"runtime-player","scope":ability,"role":"entering-controller"}));
        } else {
            let path = format!("{path}/keywords/0");
            let you = format!("{root}{path}#you");
            nodes.push(json!({"id":you,"path":path,"node":"runtime-player","scope":ability,"role":"activator"}));
            nodes.push(json!({"id":format!("{root}{path}#keyword"),"path":path,"node":"keyword-ability","scope":ability,"keyword":"cycling","abilities":["activated"],"you":you}));
        }
    }
    let refs = vec![
        json!({"path":"/faces/0/abilities/0/effect/what","expression":what,"binding":"bound","target":source}),
        json!({"path":"/faces/0/abilities/1/keywords/0/cost/0/payer","expression":{"player":"you"},"binding":"bound","target":format!("{root}/faces/0/abilities/1/keywords/0#you")}),
    ];
    fn same(actual: &Value, expected: &[Value]) -> bool {
        actual.as_array().is_some_and(|rows| {
            let stripped: Vec<_> = rows
                .iter()
                .map(|r| {
                    let mut r = r.clone();
                    if let Some(o) = r.as_object_mut() {
                        o.remove("rules");
                    }
                    r
                })
                .collect();
            stripped.len() == expected.len()
                && expected
                    .iter()
                    .all(|e| stripped.iter().filter(|r| *r == e).count() == 1)
        })
    }
    if binding["identity"] != identity
        || binding["fullyBound"] != true
        || binding["referencesBound"] != true
        || binding["diagnostics"] != json!([])
        || binding["engineFeatures"] != json!([])
        || binding["counts"] != json!({"obligations":2,"bound":2,"unresolved":0})
        || !same(&binding["nodes"], &nodes)
        || !same(&binding["references"], &refs)
    {
        return Err(bad());
    }
    Ok(())
}
