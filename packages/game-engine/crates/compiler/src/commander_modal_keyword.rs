//! The frozen binder labels the commander query as a generic reference. Its
//! timing in the executable is fixed by the printed cast condition and complete
//! conditionalCount AST (CR 601.2b, 700.2a), never by that diagnostic label.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::bindings::Bindings;
use mtg_contracts::commander_modal_keyword::{TEXT, abilities};
use serde_json::{Value, json};

const BASE: &str = "/faces/0/abilities/0";

pub(super) fn validate(
    source: &Value,
    result: &ParseResult,
    binding: &Value,
) -> Result<bool, Failure> {
    if source["oracle_text"] != TEXT {
        return Ok(false);
    }
    let ast: Value = decode(result.abilities.get(), "abilities")?;
    if ast != abilities()
        || source["type_line"] != "Instant"
        || source["keywords"] != json!([])
        || !matches!(result.lines.as_slice(), [line] if line.text == TEXT)
    {
        return Err(unsupported(
            "commander_modal_keyword",
            "requires the complete printed cast condition and both keyword choices",
        ));
    }
    let identity = source["oracle_id"]
        .as_str()
        .ok_or_else(|| invalid("source.oracle_id", "missing identity"))?;
    let step_path = format!("{BASE}/effects/0/steps/0");
    let modal = &ast[0]["effects"][0]["steps"][0];
    let mut graph = Bindings::new(identity, source);
    graph.ability(BASE, "spell", "spell-controller");
    let scope = graph.id(BASE, "ability");
    let you = graph.id(BASE, "you");
    let condition = &modal["conditionalCount"]["condition"];
    let what = format!("{step_path}/conditionalCount/condition/what");
    let query = graph.node(
        &what,
        "query",
        "selection",
        json!({
        "scope":scope,"expression":condition["what"],"cardinality":"one","timing":"resolution",
        "selection":"evaluate-reference","rules":["608.2h"]}),
    );
    graph.reference(&what, &condition["what"], &query, "608.2h");
    graph.reference(
        &format!("{step_path}/conditionalCount/condition/who"),
        &condition["who"],
        &you,
        "109.5",
    );

    for (index, rules) in [["702.9", "702.20", "702.4"], ["702.15", "702.12", "702.16"]]
        .iter()
        .enumerate()
    {
        let path = format!("{step_path}/options/{index}/effects/0/steps/0");
        let step = &modal["options"][index]["effects"][0]["steps"][0];
        let what = format!("{path}/what");
        let query = graph.node(
            &what,
            "query",
            "selection",
            json!({
            "scope":scope,"expression":step["what"],"cardinality":"many","timing":"resolution",
            "selection":"evaluate-reference","rules":["608.2h"]}),
        );
        graph.reference(
            &format!("{what}/filter/control"),
            &condition["who"],
            &you,
            "109.5",
        );
        graph.reference(&what, &step["what"], &query, "608.2h");
        for (k, rule) in rules.iter().enumerate() {
            let gained = format!("{path}/abilities/{k}");
            let recipient = graph.node(
                &gained,
                "recipient",
                "recipient",
                json!({
                "scope":scope,"of":query,"each":true,"rules":["113.10","201.5a"]}),
            );
            let ability = graph.node(
                &gained,
                "ability",
                "ability",
                json!({
                "source":recipient,"face":graph.id("/faces/0","face"),"abilityKind":"keywords",
                "you":graph.id(&gained,"you"),"rules":["113","113.10"]}),
            );
            graph.node(
                &gained,
                "you",
                "runtime-player",
                json!({"scope":ability,
                "role":"static-source-controller","rules":["109.5","301.5d","303.4e"]}),
            );
            let keyword = format!("{gained}/keyword");
            let controller = graph.node(
                &keyword,
                "you",
                "runtime-player",
                json!({
                "scope":ability,"role":"static-source-controller","rules":["109.5",rule]}),
            );
            graph.node(
                &keyword,
                "keyword",
                "keyword-ability",
                json!({
                "scope":ability,"keyword":step["abilities"][k]["keyword"]["keyword"],
                "abilities":["static"],"you":controller,"rules":[rule]}),
            );
        }
        graph.event(BASE, &path, "gain-abilities", &query, false);
    }
    graph.validate_ordered(
        binding,
        "incomplete commander condition or modal recipient graph",
    )?;
    Ok(true)
}

#[test]
fn commander_modal_keyword_exact_archive_and_binding() {
    use crate::tests::changed_source;
    const RECORD: &str =
        include_str!("../../../tests/fixtures/ramos/fd949f82-fc10-4e37-8aa9-6c7569fe3c55.json");
    const HEADER: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    assert_eq!(
        sha256(RECORD.trim_end_matches('\n').as_bytes()),
        "89895674f37bd335de47bc846e8751637334d4197fcef721b65cd0046b55f0a9"
    );
    let result = compile(HEADER, RECORD);
    let CompileResult::Lowered { definition } = result else {
        panic!("{result:?}")
    };
    let root = definition.spell.as_ref().unwrap();
    assert!(akromas_will_program(root));
    assert_eq!(
        definition.source_labels.as_ref().unwrap().spell_modes,
        mtg_contracts::commander_modal_keyword::mode_labels()
    );
    let grant = |keywords| Instruction::GrantKeywordsSet {
        set: ObjectSet::ControllerCreatures,
        modifier: None,
        keywords,
        duration: Duration::UntilEndOfTurn,
        source_block: 0,
    };
    use KeywordAbility::*;
    let first = grant(vec![Flying, Vigilance, DoubleStrike]);
    let second = grant(vec![Lifelink, Indestructible, ProtectionFromEachColor]);
    assert_eq!(root.modes[0].program.instructions, [first.clone()]);
    assert_eq!(root.modes[1].program.instructions, [second.clone()]);
    assert_eq!(root.modes[2].program.instructions, [first, second]);
    for change in [
        "condition",
        "order",
        "quality",
        "reference",
        "recipient",
        "keyword",
        "extra",
        "unbound",
        "text",
    ] {
        let raw = changed_source(RECORD, |r| {
            let result = &mut r["faces"][0]["result"];
            let modal = &mut result["abilities"][0]["effects"][0]["steps"][0];
            match change {
                "condition" => modal["conditionalCount"]["count"]["min"] = json!(0),
                "order" => modal["options"].as_array_mut().unwrap().reverse(),
                "quality" => {
                    modal["options"][1]["effects"][0]["steps"][0]["abilities"][2]["keyword"]["from"]
                        [0]["scope"] = json!("all")
                }
                "reference" => r["binding"]["references"][0]["target"] = json!("elsewhere"),
                "recipient" | "keyword" => {
                    let kind = if change == "recipient" {
                        "recipient"
                    } else {
                        "keyword-ability"
                    };
                    let n = r["binding"]["nodes"]
                        .as_array_mut()
                        .unwrap()
                        .iter_mut()
                        .find(|n| n["node"] == kind)
                        .unwrap();
                    n["scope"] = json!("elsewhere");
                }
                "extra" => {
                    modal["options"][0]["ignored"] = json!(true);
                }
                "unbound" => r["binding"]["fullyBound"] = json!(false),
                _ => {
                    let edited = TEXT.replace("a commander", "your commander");
                    r["source"]["oracle_text"] = json!(edited);
                    r["faces"][0]["result"]["lines"][0]["text"] = json!(edited);
                }
            }
            let result = &mut r["faces"][0]["result"];
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        assert!(
            !matches!(compile(HEADER, &raw), CompileResult::Lowered { .. }),
            "{change}"
        );
    }
}
