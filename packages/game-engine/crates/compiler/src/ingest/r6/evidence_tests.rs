use super::*;
use crate::ingest::evidence::{Basis, Check, Coverage, EvidenceError, StructuralPunctuation::*};
use serde_json::json;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};

const RETURN: &str = "/effects/0/steps/0";
const COUNT: &str = "/effects/0/steps/0/what/count";
const FILTER: &str = "/effects/0/steps/0/what/filter";
const RESTRICTION: &str = "/effects/0/steps/0/restriction";
const RIDER: &str = "/effects/0/steps/0/riders/0";
const ITERATION: &str = "/effects/1/steps/0/each/filter/relation";
const CREATE: &str = "/effects/1/steps/0/effects/0/steps/0";

const WYLULI_WOLF: usize = 15;
const SAHEELI: usize = 35;
const THROTTLE: usize = 252;
const SPARA: usize = 406;
const ERTHA: usize = 1589;
const JUNK: usize = 1344;
const TERA: usize = 3602;
const SECRET: usize = 7870;
const BOSEIJU: usize = 8135;
const HALFLING: usize = 37759;

fn verified(raw: &str) -> Result<CardEvidence, Failure> {
    Ok(verify_row(raw, &mut printed::NameClaims::default())?
        .evidence
        .unwrap())
}

fn block(row: &Value, index: usize) -> &Value {
    &row["printed"]["faces"][0]["blocks"][index]
}

fn block_mut(row: &mut Value, index: usize) -> &mut Value {
    &mut row["printed"]["faces"][0]["blocks"][index]
}

fn ability(row: &Value, index: usize) -> &Value {
    &row["faces"][0]["result"]["lines"][index]["ability"]
}

fn ast_mut(row: &mut Value, index: usize) -> &mut Value {
    &mut row["faces"][0]["result"]["lines"][index]["ability"]
}

fn fact_mut<'a>(row: &'a mut Value, index: usize, key: &str, value: &str) -> &'a mut Value {
    block_mut(row, index)["facts"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|fact| fact[key] == value)
        .unwrap()
}

fn rehash(row: &mut Value) -> String {
    let result = &mut row["faces"][0]["result"];
    if result["ok"] == true {
        let ast: Vec<_> = result["lines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|line| line["ability"].clone())
            .collect();
        result["abilities"] = json!(ast);
        row["astSha256"] = sha256(json!([ast]).to_string().as_bytes()).into();
    }
    for (value, hash) in [
        ("source", "sourceSha256"),
        ("printed", "printedSha256"),
        ("binding", "bindingSha256"),
        ("resolution", "resolutionSha256"),
    ] {
        row[hash] = sha256(row[value].to_string().as_bytes()).into();
    }
    row.to_string()
}

fn change_field(row: &mut Value, index: usize, path: &str, value: Value) {
    *ast_mut(row, index).pointer_mut(path).unwrap() = value;
}

fn expect_check(label: &str, row: &mut Value, expected: &str) {
    let raw = rehash(row);
    let failure = verified(&raw).err().expect("mutation passed the verifier");
    assert!(
        !failure.unsupported && failure.path.ends_with(expected),
        "{label}: {failure:?}"
    );
    println!("{label}\t{expected}\t{}", super::failure(failure));
}

fn check_field(evidence: &CardEvidence, index: usize, field: &str) -> Result<(), EvidenceError> {
    evidence.block(0, index, field)?.check_family(|coverage| {
        coverage.field(field)?;
        Ok(())
    })
}

fn bind_forms(
    row: &Value,
    block_index: usize,
    coverage: &mut Coverage<'_, '_>,
    bindings: &[(&str, u32, &str)],
) -> Result<(), EvidenceError> {
    for (index, fact) in block(row, block_index)["facts"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let Some([_, start, _]) = fact["tok"]
            .as_array()
            .map(|parts| [0, 1, 2].map(|part| parts[part].as_u64().unwrap() as u32))
        else {
            continue;
        };
        for &(form, token, field) in bindings {
            if fact["form"] == form && start == token {
                coverage.bind_fact(index, field)?;
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Controller {
    BeforeMove,
    AnnouncedTarget,
    GraveyardObject,
}

/// A test resolver for the frozen evidence. Its expected meanings are fixed
/// by the witness, independent of any fields or facts added by a mutation.
fn check_terastodon(
    row: &Value,
    evidence: &CardEvidence,
    controller: Controller,
) -> Result<(), EvidenceError> {
    let each = "/effects/1/steps/0";
    let what = format!("{RETURN}/what");
    let who = format!("{CREATE}/who");
    evidence.block(0, 0, RETURN)?.check_family(|coverage| {
        for (field, start, end, spelling, expected) in [
            ("/kind".into(), 0, 1, "When", json!("triggered")),
            ("/trigger/what/ref".into(), 1, 2, "this", json!("this")),
            (
                "/trigger/what/noun".into(),
                2,
                3,
                "creature",
                json!("creature"),
            ),
            ("/trigger/trigger".into(), 3, 4, "enters", json!("enters")),
            (format!("{RETURN}/effect"), 5, 6, "you", json!("destroy")),
            (
                format!("{RETURN}/effect"),
                7,
                8,
                "destroy",
                json!("destroy"),
            ),
            (
                format!("{what}/count/amount"),
                10,
                11,
                "three",
                json!("fixed"),
            ),
            (format!("{what}/ref"), 11, 12, "target", json!("target")),
            (
                format!("{FILTER}/cls"),
                13,
                14,
                "permanents",
                json!("permanent"),
            ),
            (
                format!("{each}/effect"),
                15,
                17,
                "For each",
                json!("for-each"),
            ),
            (
                format!("{each}/processing"),
                15,
                17,
                "For each",
                json!("instruction-wise"),
            ),
            (format!("{each}/each/ref"), 16, 17, "each", json!("each")),
            (
                format!("{each}/each/filter/cls"),
                17,
                18,
                "permanent",
                json!("permanent"),
            ),
            (
                format!("{ITERATION}/relation"),
                18,
                20,
                "put into",
                json!("zone-change"),
            ),
            (format!("{ITERATION}/to/owner"), 20, 21, "a", json!("any")),
            (
                format!("{ITERATION}/to/zone"),
                21,
                22,
                "graveyard",
                json!("graveyard"),
            ),
            (
                format!("{who}/player"),
                25,
                27,
                "its controller",
                json!("controller"),
            ),
            (
                format!("{CREATE}/effect"),
                27,
                28,
                "creates",
                json!("create-token"),
            ),
            (
                format!("{CREATE}/count/amount"),
                28,
                29,
                "a",
                json!("fixed"),
            ),
            (
                format!("{CREATE}/token/power/amount"),
                29,
                30,
                "3",
                json!("fixed"),
            ),
            (
                format!("{CREATE}/token/toughness/amount"),
                31,
                32,
                "3",
                json!("fixed"),
            ),
            (
                format!("{CREATE}/token/colors"),
                32,
                33,
                "green",
                json!(["green"]),
            ),
            (
                format!("{CREATE}/token/subtypes"),
                33,
                34,
                "Elephant",
                json!(["Elephant"]),
            ),
            (
                format!("{CREATE}/token/types"),
                34,
                35,
                "creature",
                json!(["creature"]),
            ),
            (
                format!("{CREATE}/token/supertypes"),
                32,
                36,
                "green Elephant creature token",
                json!([]),
            ),
            (
                format!("{CREATE}/token/keywords"),
                32,
                36,
                "green Elephant creature token",
                json!([]),
            ),
        ] {
            coverage.meaning(&field, [0, start, end], |value, text| {
                value == &expected && text == spelling
            })?;
        }
        // CR 400.7 and 608.2h. The possessive is inside the iteration over
        // actual moves. Read the controller of each permanent before its move.
        coverage.meaning(&format!("{who}/of/ref"), [0, 25, 26], |value, text| {
            controller == Controller::BeforeMove && value == "it" && text == "its"
        })?;
        for field in [
            format!("{what}/upTo"),
            format!("{what}/count/value"),
            format!("{what}/selectionId"),
            format!("{FILTER}/nonTypes"),
            format!("{ITERATION}/scope"),
            format!("{CREATE}/count/value"),
            format!("{CREATE}/token/power/value"),
            format!("{CREATE}/token/toughness/value"),
        ] {
            coverage.field(&field)?;
        }
        let optional = format!("{RETURN}/optional");
        if ability(row, 0).pointer(&optional).is_some() {
            coverage.field(&optional)?;
        }
        bind_forms(
            row,
            0,
            coverage,
            &[
                ("this", 1, "/trigger/what/ref"),
                ("you", 5, &format!("{RETURN}/effect")),
                ("up-to", 8, &format!("{what}/upTo")),
                ("numeral", 10, &format!("{what}/count/value")),
                ("target", 11, &format!("{what}/selectionId")),
                ("each", 16, &format!("{each}/each/ref")),
                ("article", 20, &format!("{ITERATION}/to/owner")),
                ("a", 20, &format!("{ITERATION}/to/owner")),
                ("this", 22, &format!("{ITERATION}/scope")),
                ("its", 25, &format!("{who}/of/ref")),
                ("a", 28, &format!("{CREATE}/count/value")),
                ("numeral", 29, &format!("{CREATE}/token/power/value")),
                ("numeral", 31, &format!("{CREATE}/token/toughness/value")),
            ],
        )?;
        let create = format!("{CREATE}/effect");
        coverage.meaning(&create, [0, 29, 32], |value, text| {
            value == "create-token" && text == "3/3"
        })?;
        let slash = block(row, 0)["facts"]
            .as_array()
            .unwrap()
            .iter()
            .position(|fact| fact["fact"] == "punct" && fact["tok"] == json!([0, 30, 31]))
            .unwrap();
        coverage.bind_fact(slash, &create)?;
        for (index, role) in [
            (4, ClauseComma),
            (14, SentenceTerminator),
            (24, ClauseComma),
            (36, SentenceTerminator),
        ] {
            coverage.punctuation(0, index, role)?;
        }
        // Exploratory reads of all facts and tokens must never close a claim.
        for index in 0..block(row, 0)["facts"].as_array().unwrap().len() {
            coverage.fact(index)?;
        }
        for index in 0..37 {
            coverage.token(0, index)?;
        }
        Ok(())
    })
}

fn expect_family(label: &str, error: EvidenceError, expected: Check, range: [u32; 2]) {
    assert_eq!(error.check, expected, "{label}: {error:?}");
    assert_eq!(error.range, Some(range), "{label}: {error:?}");
    let failure: Failure = error.into();
    assert!(failure.unsupported && failure.path.starts_with("printed/faces/0/blocks/"));
    println!("{label}\t{expected:?}\t{}", super::failure(failure));
}

fn check_witnesses(rows: &BTreeMap<usize, String>) {
    let cases = [
        (JUNK, 0, "/keywords/0".into(), [0, 6]),
        (JUNK, 1, "/trigger/what".into(), [12, 25]),
        (JUNK, 1, "/trigger".into(), [12, 30]),
        (JUNK, 1, FILTER.into(), [39, 87]),
        (JUNK, 1, format!("{FILTER}/zone"), [73, 87]),
        (TERA, 0, COUNT.into(), [49, 54]),
        (TERA, 0, FILTER.into(), [62, 84]),
        (TERA, 0, ITERATION.into(), [105, 134]),
        (TERA, 0, format!("{CREATE}/who"), [136, 150]),
        (TERA, 0, format!("{CREATE}/token/power"), [161, 162]),
        (TERA, 0, format!("{CREATE}/token/toughness"), [163, 164]),
        (SECRET, 0, format!("{RETURN}/who/members/0"), [0, 3]),
        (SECRET, 0, format!("{RETURN}/who/members/1"), [8, 23]),
        (SECRET, 0, format!("{RETURN}/who"), [0, 28]),
        (SECRET, 0, format!("{RETURN}/amount"), [34, 39]),
        (BOSEIJU, 0, "/effect/what".into(), [0, 7]),
        (BOSEIJU, 1, "/costs/0".into(), [23, 26]),
        (BOSEIJU, 1, "/costs/1".into(), [28, 38]),
        (BOSEIJU, 1, format!("{RIDER}/spentOn/0/filter"), [77, 101]),
        (
            BOSEIJU,
            1,
            format!("{RIDER}/effects/0/steps/0/what"),
            [103, 113],
        ),
        (HALFLING, 0, "/costs/0".into(), [0, 3]),
        (HALFLING, 1, "/costs/0".into(), [14, 17]),
        (HALFLING, 1, RESTRICTION.into(), [46, 92]),
        (
            HALFLING,
            1,
            format!("{RIDER}/effects/0/steps/0/what"),
            [98, 108],
        ),
    ];
    for (line, index, path, range) in cases {
        let card = verified(&rows[&line]).unwrap();
        let block = card.block(0, index, &path).unwrap();
        let proof = block
            .node(&path)
            .unwrap_or_else(|error| panic!("{line}:{path}: {error:?}"));
        assert_eq!(proof.range, range, "{line}:{path}");
        assert_eq!(proof.basis, Basis::Recorded, "{line}:{path}");
        assert_eq!(proof.owner, path, "{line}:{path}");
    }

    for (line, index, field, range) in [
        (JUNK, 1, format!("{FILTER}/other"), [39, 46]),
        (JUNK, 1, format!("{RETURN}/what/selectionId"), [47, 53]),
        (TERA, 0, format!("{RETURN}/optional"), [31, 34]),
        (TERA, 0, format!("{RETURN}/what/upTo"), [43, 48]),
        (TERA, 0, format!("{FILTER}/nonTypes"), [62, 73]),
        (TERA, 0, format!("{ITERATION}/scope"), [126, 134]),
        (TERA, 0, format!("{CREATE}/token/power/value"), [161, 162]),
        (
            TERA,
            0,
            format!("{CREATE}/token/toughness/value"),
            [163, 164],
        ),
        (SECRET, 0, format!("{RETURN}/who/members/0/player"), [0, 3]),
        (BOSEIJU, 1, "/costs/1/amount/value".into(), [32, 33]),
        (HALFLING, 1, format!("{RESTRICTION}/restriction"), [62, 66]),
    ] {
        let row: Value = serde_json::from_str(&rows[&line]).unwrap();
        let evidence = verified(&rows[&line]).unwrap();
        let block = evidence.block(0, index, &field).unwrap();
        let fact_index = self::block(&row, index)["facts"]
            .as_array()
            .unwrap()
            .iter()
            .position(|fact| fact["field"] == field)
            .unwrap();
        let proof = block.fact(fact_index).unwrap();
        assert_eq!(proof.range, range, "{line}:{field}");
        assert_eq!(proof.basis, Basis::Recorded);
        if let PrintedFact::Target(target) = proof.value {
            assert_eq!(target.target_index, 0);
            assert_eq!(target.scope, "");
            assert_eq!(target.selection_id, "line:1:7");
        }
    }

    let junk = verified(&rows[&JUNK]).unwrap();
    let block = junk.block(0, 1, COUNT).unwrap();
    let count = block.node(COUNT).unwrap();
    assert_eq!(
        count.basis,
        Basis::Implicit(&ImplicitReason::ImpliedSingularTarget)
    );
    assert_eq!(count.range, [39, 87]);
    let halfling = verified(&rows[&HALFLING]).unwrap();
    let block = halfling.block(0, 1, RESTRICTION).unwrap();
    for (path, target) in [
        (
            format!("{RIDER}/spentOn/0"),
            format!("{RESTRICTION}/uses/0"),
        ),
        (
            format!("{RIDER}/spentOn/0/filter"),
            format!("{RESTRICTION}/uses/0/filter"),
        ),
    ] {
        let alias = block.node(&path).unwrap();
        assert_eq!(
            alias.basis,
            Basis::Alias {
                target: &target,
                default: None
            }
        );
        assert_eq!(alias.range, block.node(&target).unwrap().range);
        assert_eq!(alias.owner, target);
        assert_eq!(
            block
                .node_facts(&path)
                .unwrap()
                .map(|result| result.unwrap().0)
                .collect::<Vec<_>>(),
            block
                .node_facts(&target)
                .unwrap()
                .map(|result| result.unwrap().0)
                .collect::<Vec<_>>()
        );
    }

    let boseiju = verified(&rows[&BOSEIJU]).unwrap();
    let block = boseiju.block(0, 1, RIDER).unwrap();
    expect_family(
        "Boseiju rider hull",
        block.node(RIDER).unwrap_err(),
        Check::P11,
        [49, 133],
    );
    let that = block
        .facts()
        .find(|(_, value)| matches!(value, PrintedFact::Mention(v) if v.at == RIDER))
        .unwrap()
        .0;
    let fact = block.fact(that).unwrap();
    assert_eq!(fact.range, [52, 56]);
    assert_eq!(fact.owner, RIDER);
    println!("four witness groups passed");
}

fn remove_field(row: &mut Value, index: usize, owner: &str, field: &str, form: &str) {
    ast_mut(row, index)
        .pointer_mut(owner)
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove(field)
        .unwrap();
    for fact in block_mut(row, index)["facts"].as_array_mut().unwrap() {
        if fact["form"] == form && fact["field"] == format!("{owner}/{field}") {
            fact["field"] = Value::Null;
        }
    }
}

fn check_mutations(rows: &BTreeMap<usize, String>) {
    let get = |line: usize| serde_json::from_str::<Value>(&rows[&line]).unwrap();
    let mut row = get(JUNK);
    row["printed"]["faces"][0]["lines"][1]["tokens"][7]["o"] = json!([48, 54]);
    expect_check("shift token O", &mut row, "P4");
    let mut row = get(BOSEIJU);
    row["printed"]["faces"][0]["edits"][0]["o"] = json!([0, 6]);
    expect_check("shrink Boseiju name edit", &mut row, "P3");

    let mut row = get(JUNK);
    let field = format!("{RETURN}/what/selectionId");
    change_field(&mut row, 1, &field, json!("line:1:6"));
    fact_mut(&mut row, 1, "fact", "target")["selectionId"] = json!("line:1:6");
    expect_check("change Junk Diver selectionId", &mut row, "P9");
    let mut row = get(SAHEELI);
    for fact in block_mut(&mut row, 0)["facts"].as_array_mut().unwrap() {
        if let Some(index) = fact["targetIndex"].as_u64() {
            fact["targetIndex"] = json!(1 - index);
        }
    }
    expect_check("swap target indices", &mut row, "P9");
    let mut row = get(JUNK);
    fact_mut(&mut row, 1, "fact", "target")["scope"] = json!(RETURN);
    expect_check("move target to nested scope", &mut row, "P9");
    let mut row = get(TERA);
    fact_mut(&mut row, 0, "form", "this-way")["at"] = json!("/effects/0");
    expect_check("reassign this way", &mut row, "P7");
    let mut row = get(TERA);
    change_field(&mut row, 0, &format!("{COUNT}/value"), json!(2));
    expect_check("change Terastodon AST three to two", &mut row, "P10");
    let mut row = get(TERA);
    fact_mut(&mut row, 0, "form", "word")["value"] = json!(2);
    expect_check("change Terastodon fact three to two", &mut row, "P8");
    let mut row = get(BOSEIJU);
    change_field(&mut row, 1, "/costs/1/amount/value", json!(3));
    expect_check("change Boseiju life to three", &mut row, "P10");
    let mut row = get(JUNK);
    change_field(&mut row, 1, &format!("{COUNT}/value"), json!(2));
    // Withdraw the now inapplicable implicit proof to exercise P11 after P6.
    let node = block_mut(&mut row, 1)["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["path"] == COUNT)
        .unwrap();
    *node = json!({"basis":"unlocated","path":COUNT});
    let card = verified(&rehash(&mut row)).unwrap();
    let block = card.block(0, 1, COUNT).unwrap();
    expect_family(
        "invent Junk Diver count two",
        block.node(COUNT).unwrap_err(),
        Check::P11,
        [39, 87],
    );

    let mut row = get(TERA);
    let filter = ast_mut(&mut row, 0)
        .pointer_mut(FILTER)
        .unwrap()
        .as_object_mut()
        .unwrap();
    let types = filter.remove("nonTypes").unwrap();
    filter.insert("types".into(), types);
    expect_check("invert noncreature polarity", &mut row, "P10");
    let mut row = get(HALFLING);
    ast_mut(&mut row, 1)
        .pointer_mut(RETURN)
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("restriction")
        .unwrap();
    // Withdrawal of the restriction's proof leaves the equal rider fields
    // without their alias target. Retain their complete AST shape.
    let block = block_mut(&mut row, 1);
    let spent = format!("{RIDER}/spentOn/");
    let nodes = block["nodes"].as_array_mut().unwrap();
    nodes.retain(|node| !node["path"].as_str().unwrap().starts_with(RESTRICTION));
    for node in nodes {
        if node["path"].as_str().unwrap().starts_with(&spent) {
            *node = json!({"basis":"unlocated","path":node["path"]});
        }
    }
    for fact in block["facts"].as_array_mut().unwrap() {
        if fact["at"]
            .as_str()
            .is_some_and(|owner| owner.starts_with(RESTRICTION))
        {
            fact["at"] = json!(RETURN);
            fact["field"] = Value::Null;
        }
    }
    let card = verified(&rehash(&mut row)).unwrap();
    let field = format!("{RESTRICTION}/restriction");
    expect_family(
        "omit Halfling restriction",
        card.block(0, 1, &field)
            .unwrap()
            .check_family(|coverage| {
                coverage.punctuation(1, 1, CostEffectColon)?;
                coverage.field(&format!("{RETURN}/mana/amount/value"))?;
                coverage.meaning(&format!("{RETURN}/mana/mana"), [1, 4, 8], |value, text| {
                    value == "any-color" && text == "mana of any color"
                })?;
                coverage.meaning(&format!("{RETURN}/effect"), [1, 9, 12], |value, text| {
                    value == "add-mana" && text == "Spend this mana"
                })?;
                bind_forms(
                    &row,
                    1,
                    coverage,
                    &[
                        ("numeral", 3, &format!("{RETURN}/mana/amount/value")),
                        ("any", 6, &format!("{RETURN}/mana/mana")),
                        ("this", 10, &format!("{RETURN}/effect")),
                    ],
                )?;
                coverage.punctuation(1, 8, SentenceTerminator)?;
                Ok(())
            })
            .unwrap_err(),
        Check::P12,
        [62, 66],
    );
    let mut row = get(BOSEIJU);
    let halfling = get(HALFLING);
    ast_mut(&mut row, 1).pointer_mut(RETURN).unwrap()["restriction"] =
        ability(&halfling, 1).pointer(RESTRICTION).unwrap().clone();
    let fields = self::block(&halfling, 1)["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|node| node["path"].as_str().unwrap().starts_with(RESTRICTION))
        .map(|node| json!({"basis":"unlocated","path":node["path"]}));
    block_mut(&mut row, 1)["nodes"]
        .as_array_mut()
        .unwrap()
        .extend(fields);
    let card = verified(&rehash(&mut row)).unwrap();
    expect_family(
        "invent Boseiju restriction",
        check_field(&card, 1, &field).unwrap_err(),
        Check::P11,
        [40, 133],
    );

    for (name, line, index, owner, property, form, add, range) in [
        (
            "omit Terastodon may",
            TERA,
            0,
            RETURN,
            "optional",
            "may",
            false,
            [31, 34],
        ),
        (
            "invent Junk Diver may",
            JUNK,
            1,
            RETURN,
            "optional",
            "may",
            true,
            [32, 100],
        ),
        (
            "omit Junk Diver another",
            JUNK,
            1,
            FILTER,
            "other",
            "another",
            false,
            [39, 46],
        ),
        (
            "invent Terastodon other",
            TERA,
            0,
            FILTER,
            "other",
            "other",
            true,
            [62, 84],
        ),
    ] {
        let mut row = get(line);
        if add {
            ast_mut(&mut row, index).pointer_mut(owner).unwrap()[property] = json!(true);
        } else {
            remove_field(&mut row, index, owner, property, form);
        }
        let card = verified(&rehash(&mut row)).unwrap();
        let field = format!("{owner}/{property}");
        expect_family(
            name,
            match line {
                TERA => check_terastodon(&row, &card, Controller::BeforeMove),
                JUNK if !add => card.block(0, 1, &field).unwrap().check_family(|coverage| {
                    coverage.meaning("/trigger/what/ref", [1, 1, 2], |value, text| {
                        value == "this" && text == "this"
                    })?;
                    coverage.punctuation(1, 4, ClauseComma)?;
                    bind_forms(&row, 1, coverage, &[("this", 1, "/trigger/what/ref")])?;
                    Ok(())
                }),
                _ => check_field(&card, index, &field),
            }
            .unwrap_err(),
            if add { Check::P11 } else { Check::P12 },
            range,
        );
    }
    let mut row = get(TERA);
    remove_field(&mut row, 0, RETURN, "optional", "may");
    block_mut(&mut row, 0)["facts"]
        .as_array_mut()
        .unwrap()
        .retain(|fact| fact["form"] != "may");
    let card = verified(&rehash(&mut row)).unwrap();
    expect_family(
        "omit Terastodon may and delete fact",
        check_terastodon(&row, &card, Controller::BeforeMove).unwrap_err(),
        Check::P12,
        [31, 34],
    );

    let row = get(TERA);
    let card = verified(&rows[&TERA]).unwrap();
    check_terastodon(&row, &card, Controller::BeforeMove).unwrap();
    for (label, candidate) in [
        (
            "use announced target controller",
            Controller::AnnouncedTarget,
        ),
        (
            "use graveyard object controller",
            Controller::GraveyardObject,
        ),
    ] {
        expect_family(
            label,
            check_terastodon(&row, &card, candidate).unwrap_err(),
            Check::P11,
            [136, 139],
        );
    }

    let mut row = get(HALFLING);
    fact_mut(&mut row, 1, "form", "this")["tok"] = json!([0, 3, 4]);
    expect_check(
        "cross Halfling mana mention into first activation",
        &mut row,
        "P7",
    );
    let mut row = get(JUNK);
    fact_mut(&mut row, 1, "fact", "exception")["at"] = json!("/trigger/what");
    expect_check("attach another to wrong child", &mut row, "P7");
    let mut row = get(HALFLING);
    fact_mut(&mut row, 1, "form", "only")["at"] = json!(format!("{RESTRICTION}/uses/0"));
    expect_check("attach only to wrong child", &mut row, "P7");
    let mut row = get(HALFLING);
    ast_mut(&mut row, 1)
        .pointer_mut(&format!("{RIDER}/effects/0/steps/0"))
        .unwrap()["effect"] = json!("counter");
    expect_check("clear can't polarity", &mut row, "P10");

    let mut row = get(JUNK);
    let keyword = ability(&row, 0)["keywords"][0].clone();
    ast_mut(&mut row, 0)["keywords"]
        .as_array_mut()
        .unwrap()
        .push(keyword);
    block_mut(&mut row, 0)["nodes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"basis":"alias","path":"/keywords/1","of":"/keywords/0"}));
    assert_eq!(self::block(&row, 0)["nodes"][1]["span"], json!([[0, 0, 1]]));
    assert_eq!(
        row["printed"]["faces"][0]["lines"][0]["tokens"][0]["o"],
        json!([0, 6])
    );
    expect_check("duplicate Flying with same token", &mut row, "P6");
    let mut row = get(HALFLING);
    let rider = ability(&row, 1).pointer(RIDER).unwrap().clone();
    ast_mut(&mut row, 1).pointer_mut(RETURN).unwrap()["riders"]
        .as_array_mut()
        .unwrap()
        .push(rider);
    expect_check("duplicate Halfling rider", &mut row, "P6");
    let mut row = get(HALFLING);
    change_field(
        &mut row,
        1,
        &format!("{RIDER}/spentOn/0/filter/supertypes"),
        json!(["snow"]),
    );
    expect_check("diverge Halfling alias", &mut row, "P6");
}

fn check_punctuation_mutations(rows: &BTreeMap<usize, String>) {
    let mut throttle: Value = serde_json::from_str(&rows[&THROTTLE]).unwrap();
    assert_eq!(throttle["source"]["name"], "Throttle");
    assert_eq!(
        throttle["source"]["oracle_text"],
        "Target creature gets -4/-4 until end of turn."
    );
    ast_mut(&mut throttle, 0).pointer_mut(RETURN).unwrap()["toughness"]
        .as_object_mut()
        .unwrap()
        .remove("sign")
        .unwrap();
    let card = verified(&rehash(&mut throttle)).unwrap();
    let block = card.block(0, 0, RETURN).unwrap();
    let check = |claim_minus| {
        block.check_family(|coverage| {
            for (field, start, end, literal, expected) in [
                ("/kind".to_owned(), 0, 1, "Target", json!("spell")),
                (
                    format!("{RETURN}/what/ref"),
                    0,
                    1,
                    "Target",
                    json!("target"),
                ),
                (
                    format!("{FILTER}/types"),
                    1,
                    2,
                    "creature",
                    json!(["creature"]),
                ),
                (format!("{RETURN}/effect"), 2, 3, "gets", json!("pump")),
                (format!("{RETURN}/power/sign"), 3, 4, "-", json!(-1)),
                (
                    format!("{RETURN}/power/amount/amount"),
                    4,
                    5,
                    "4",
                    json!("fixed"),
                ),
                (format!("{RETURN}/effect"), 4, 6, "4/", json!("pump")),
                (
                    format!("{RETURN}/toughness/amount/amount"),
                    7,
                    8,
                    "4",
                    json!("fixed"),
                ),
                (
                    format!("{RETURN}/duration/duration"),
                    8,
                    12,
                    "until end of turn",
                    json!("end-of-turn"),
                ),
            ] {
                coverage.meaning(&field, [0, start, end], |value, text| {
                    value == &expected && text == literal
                })?;
            }
            for field in [
                format!("{RETURN}/what/selectionId"),
                format!("{RETURN}/power/amount/value"),
                format!("{RETURN}/toughness/amount/value"),
            ] {
                coverage.field(&field)?;
            }
            for property in ["amount", "value"] {
                coverage.implicit(
                    &format!("{COUNT}/{property}"),
                    ImplicitReason::ImpliedSingularTarget,
                )?;
            }
            bind_forms(
                &throttle,
                0,
                coverage,
                &[
                    ("target", 0, &format!("{RETURN}/what/selectionId")),
                    ("numeral", 4, &format!("{RETURN}/power/amount/value")),
                    ("numeral", 7, &format!("{RETURN}/toughness/amount/value")),
                ],
            )?;
            for (index, field) in [
                (2, format!("{RETURN}/power/sign")),
                (5, format!("{RETURN}/effect")),
            ] {
                coverage.bind_fact(index, &field)?;
            }
            coverage.punctuation(0, 12, SentenceTerminator)?;
            if claim_minus {
                coverage.punctuation(0, 6, SentenceTerminator)?;
            }
            Ok(())
        })
    };
    expect_family(
        "omit Throttle toughness sign",
        check(false).unwrap_err(),
        Check::P12,
        [24, 25],
    );
    expect_family(
        "claim Throttle minus as structural",
        check(true).unwrap_err(),
        Check::P11,
        [24, 25],
    );

    let wolf: Value = serde_json::from_str(&rows[&WYLULI_WOLF]).unwrap();
    assert_eq!(wolf["source"]["name"], "Wyluli Wolf");
    assert_eq!(
        wolf["source"]["oracle_text"],
        "{T}: Target creature gets +1/+1 until end of turn."
    );
    let card = verified(&rows[&WYLULI_WOLF]).unwrap();
    let block = card.block(0, 0, RETURN).unwrap();
    for (label, index, role, range) in [
        (
            "claim Wyluli Wolf plus as structural",
            8,
            CostEffectColon,
            [29, 30],
        ),
        (
            "claim Wyluli Wolf P/T slash as structural",
            7,
            ClauseComma,
            [28, 29],
        ),
    ] {
        expect_family(
            label,
            block
                .check_family(|coverage| coverage.punctuation(0, index, role))
                .unwrap_err(),
            Check::P11,
            range,
        );
    }

    let card = verified(&rows[&BOSEIJU]).unwrap();
    let block = card.block(0, 1, "/costs/0").unwrap();
    expect_family(
        "claim Boseiju cost separator as a clause comma",
        block
            .check_family(|coverage| {
                coverage.punctuation(1, 1, CostListComma)?;
                coverage.punctuation(1, 5, CostEffectColon)?;
                coverage.punctuation(1, 1, ClauseComma)
            })
            .unwrap_err(),
        Check::P11,
        [26, 27],
    );
}

fn check_quoted_punctuation(rows: &BTreeMap<usize, String>) {
    let mut escaped = Vec::new();
    let mut inner_claims = Vec::new();
    for (row_number, name, block_index, line, index, role, range) in [
        (
            ERTHA,
            "Ertha Jo, Frontier Mentor",
            0,
            0,
            31,
            SentenceTerminator,
            [131, 132],
        ),
        (
            SPARA,
            "Spara's Adjudicators",
            1,
            1,
            14,
            CostEffectColon,
            [167, 168],
        ),
    ] {
        let raw = &rows[&row_number];
        let mut row: Value = serde_json::from_str(raw).unwrap();
        assert_eq!(row["source"]["name"], name);
        let source = row["source"].clone();
        let legacy = row["faces"].clone();
        let printed = block_mut(&mut row, block_index);
        let mut outer_owner = "/effects/0";
        for node in printed["nodes"].as_array_mut().unwrap() {
            let path = node["path"].as_str().unwrap();
            if row_number == ERTHA {
                if path == "/effects/0" {
                    node["span"] = json!([[0, 4, 32]]);
                    node["folds"] = json!([[0, 32, 39]]);
                } else if path.starts_with("/effects/0/") {
                    if let Some(runs) = node.get_mut("span").and_then(Value::as_array_mut) {
                        let mut clipped = Vec::new();
                        for run in runs.iter() {
                            let start = run[1].as_u64().unwrap();
                            let end = run[2].as_u64().unwrap();
                            if !(start <= 31 && 31 < end) {
                                clipped.push(run.clone());
                                continue;
                            }
                            if start < 31 {
                                clipped.push(json!([0, start, 31]));
                            }
                            if 32 < end {
                                clipped.push(json!([0, 32, end]));
                            }
                        }
                        *runs = clipped;
                    }
                }
            } else {
                const GRANT: &str = "/effects/0/steps/0/abilities/0";
                outer_owner = "";
                match path {
                    "/costs/1" => node["span"] = json!([[1, 2, 14]]),
                    "/effects/0" => node["span"] = json!([[1, 15, 31]]),
                    "/effects/0/steps/0" | GRANT => node["span"] = json!([[1, 15, 23]]),
                    "/effects/0/steps/0/abilities/0/ability" => {
                        node["span"] = json!([[1, 15, 22]]);
                    }
                    "/effects/0/steps/0/abilities/0/ability/costs/0"
                    | "/effects/0/steps/0/what"
                    | "/effects/0/steps/0/what/filter" => {
                        *node = json!({"path": path, "basis": "unlocated"});
                    }
                    _ => {}
                }
            }
        }
        for fact in printed["facts"].as_array_mut().unwrap() {
            if fact["tok"] == json!([line, index, index + 1]) {
                fact["at"] = json!(outer_owner);
            } else if row_number == SPARA
                && (fact["tok"] == json!([1, 8, 9])
                    || fact["tok"] == json!([1, 9, 10])
                    || fact["tok"] == json!([1, 12, 13]))
            {
                fact["at"] = json!("/costs/1");
            }
        }
        let mutated = rehash(&mut row);
        assert_eq!(row["source"], source);
        assert_eq!(row["faces"], legacy);
        let claim = |card: &CardEvidence| {
            let block = card.block(0, block_index, "quoted punctuation").unwrap();
            let mut consumed = false;
            let _ = block.check_family(|coverage| {
                assert_eq!(coverage.token(line, index)?.range, range);
                coverage.punctuation(line, index, role)?;
                consumed = true;
                Ok(())
            });
            consumed
        };
        match verified(&mutated) {
            Err(error) => {
                assert!(
                    !error.unsupported && error.path.ends_with("P6"),
                    "{error:?}"
                );
                println!("{name} outer delimiter\tP6\t{}", super::failure(error));
            }
            Ok(card) => escaped.push((name, claim(&card))),
        }
        let card = verified(raw).unwrap();
        inner_claims.push((name, range, claim(&card)));
    }
    assert!(
        escaped.is_empty(),
        "quote-boundary mutations passed intake (name, consumed): {escaped:?}"
    );
    for (name, range, consumed) in inner_claims {
        assert!(consumed, "{name}: failed to consume the inner delimiter");
        println!("{name} inner delimiter\taccepted\tO{range:?}");
    }
}

#[test]
#[ignore = "opt-in frozen R6 candidate; set MTG_R6_VERIFIER_ROWS"]
fn frozen_family_evidence() {
    let archive = std::env::var_os("MTG_R6_VERIFIER_ROWS").expect("set MTG_R6_VERIFIER_ROWS");
    let reader = BufReader::new(std::fs::File::open(archive).unwrap());
    let rows: BTreeMap<_, _> = reader
        .lines()
        .enumerate()
        .take(HALFLING)
        .filter_map(|(index, line)| {
            let number = index + 1;
            let line = line.unwrap();
            [
                WYLULI_WOLF,
                SAHEELI,
                THROTTLE,
                SPARA,
                ERTHA,
                JUNK,
                TERA,
                SECRET,
                BOSEIJU,
                HALFLING,
            ]
            .contains(&number)
            .then_some((number, line))
        })
        .collect();
    check_witnesses(&rows);
    check_mutations(&rows);
    check_punctuation_mutations(&rows);
    check_quoted_punctuation(&rows);
    check_source_edits(&rows);
}

#[test]
fn legacy_requires_r6_for_the_missing_obligation() {
    let card = CardEvidence::legacy();
    let failure: Failure = card.block(0, 1, COUNT).unwrap_err().into();
    assert!(failure.unsupported);
    assert_eq!(failure.path, "printed/faces/0/blocks/1");
    assert_eq!(failure.reason, format!("evidence:requires-r6:{COUNT}"));
}

struct SourceEdit {
    label: &'static str,
    row: usize,
    block: usize,
    from: &'static str,
    to: &'static str,
    field: String,
    expected: Option<Value>,
}

/// Exercise real text changes separately from fixed-O verifier mutations. A
/// syntactically rejected edit has to retain a located block diagnostic.
fn check_source_edits(rows: &BTreeMap<usize, String>) {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let read = |label, row, block, from, to, field, expected| SourceEdit {
        label,
        row,
        block,
        from,
        to,
        field,
        expected,
    };
    let edits = [
        read(
            "shift token O",
            JUNK,
            1,
            "Flying\n",
            "Flying (Reminder text.)\n",
            format!("{RETURN}/what/selectionId"),
            Some(json!("line:1:7")),
        ),
        read(
            "shrink Boseiju name edit",
            BOSEIJU,
            0,
            "Boseiju enters",
            "Boseij enters",
            "/effect/what/ref".into(),
            None,
        ),
        read(
            "change Junk Diver selectionId",
            JUNK,
            1,
            "another target",
            "target",
            format!("{RETURN}/what/selectionId"),
            Some(json!("line:1:6")),
        ),
        read(
            "swap target indices",
            SAHEELI,
            0,
            "target artifact.\n• Create a token that's a copy of target creature",
            "target creature.\n• Create a token that's a copy of target artifact",
            "/effects/0/steps/0/options/0/effects/0/steps/0/token/copyOf/filter/types".into(),
            Some(json!(["creature"])),
        ),
        read(
            "move target to nested scope",
            JUNK,
            1,
            "another target artifact card from your graveyard to your hand.",
            "another artifact card from your graveyard to your hand if you control target artifact.",
            "/effects/0/condition/what/selectionId".into(),
            Some(json!("line:1:18")),
        ),
        read(
            "reassign this way",
            TERA,
            0,
            "permanents. For each permanent put into a graveyard this way,",
            "permanents this way. For each permanent put into a graveyard,",
            format!("{ITERATION}/scope"),
            None,
        ),
        read(
            "change Terastodon AST three to two",
            TERA,
            0,
            "up to three",
            "up to two",
            format!("{COUNT}/value"),
            Some(json!(2)),
        ),
        read(
            "change Terastodon fact three to two",
            TERA,
            0,
            "up to three",
            "up to two",
            format!("{COUNT}/value"),
            Some(json!(2)),
        ),
        read(
            "change Boseiju life to three",
            BOSEIJU,
            1,
            "Pay 2 life",
            "Pay 3 life",
            "/costs/1/amount/value".into(),
            Some(json!(3)),
        ),
        read(
            "invent Junk Diver count two",
            JUNK,
            1,
            "another target artifact card",
            "two target artifact cards",
            format!("{COUNT}/value"),
            Some(json!(2)),
        ),
        read(
            "invert noncreature polarity",
            TERA,
            0,
            "noncreature permanents",
            "creature permanents",
            format!("{FILTER}/types"),
            Some(json!(["creature"])),
        ),
        read(
            "omit Halfling restriction",
            HALFLING,
            1,
            "only to cast",
            "to cast",
            format!("{RESTRICTION}/restriction"),
            None,
        ),
        read(
            "invent Boseiju restriction",
            BOSEIJU,
            1,
            "If that mana is spent on an instant or sorcery spell, that spell can't be countered.",
            "Spend this mana only to cast an instant or sorcery spell, and that spell can't be countered.",
            format!("{RESTRICTION}/restriction"),
            Some(json!("only")),
        ),
        read(
            "omit Terastodon may",
            TERA,
            0,
            "you may destroy",
            "destroy",
            format!("{RETURN}/optional"),
            None,
        ),
        read(
            "omit Terastodon may and delete fact",
            TERA,
            0,
            "you may destroy",
            "destroy",
            format!("{RETURN}/optional"),
            None,
        ),
        read(
            "invent Junk Diver may",
            JUNK,
            1,
            "dies, return",
            "dies, you may return",
            format!("{RETURN}/optional"),
            Some(json!(true)),
        ),
        read(
            "omit Junk Diver another",
            JUNK,
            1,
            "another target",
            "target",
            format!("{FILTER}/other"),
            None,
        ),
        read(
            "invent Terastodon other",
            TERA,
            0,
            "three target",
            "three other target",
            format!("{FILTER}/other"),
            Some(json!(true)),
        ),
        read(
            "cross Halfling mana mention into first activation",
            HALFLING,
            0,
            "Add {C}.\n{T}: Add one mana of any color. Spend this mana only to cast a legendary spell, and that spell can't be countered.",
            "Add {C}. Spend this mana only to cast a legendary spell, and that spell can't be countered.\n{T}: Add one mana of any color.",
            format!("{RESTRICTION}/restriction"),
            Some(json!("only")),
        ),
        read(
            "attach another to wrong child",
            JUNK,
            1,
            "this creature dies, return another target",
            "another creature dies, return target",
            "/trigger/what/filter/other".into(),
            Some(json!(true)),
        ),
        read(
            "attach only to wrong child",
            HALFLING,
            1,
            "only to cast a legendary spell, and that spell",
            "to cast a legendary spell, and only that spell",
            format!("{RESTRICTION}/restriction"),
            None,
        ),
        read(
            "clear can't polarity",
            HALFLING,
            1,
            "can't be countered",
            "can be countered",
            format!("{RIDER}/effects/0/steps/0/effect"),
            None,
        ),
        read(
            "use announced target controller",
            TERA,
            0,
            "its controller creates",
            "each target's controller creates",
            format!("{CREATE}/who/player"),
            None,
        ),
        read(
            "use graveyard object controller",
            TERA,
            0,
            "its controller creates",
            "its controller in that graveyard creates",
            format!("{CREATE}/who/player"),
            None,
        ),
        read(
            "duplicate Flying with same token",
            JUNK,
            0,
            "Flying\n",
            "Flying, flying\n",
            "/keywords/1/keyword".into(),
            Some(json!("flying")),
        ),
        read(
            "duplicate Halfling rider",
            HALFLING,
            1,
            "and that spell can't be countered.",
            "and that spell can't be countered. That spell can't be countered.",
            "/effects/1/steps/0/effect".into(),
            Some(json!("cant")),
        ),
        read(
            "diverge Halfling alias",
            HALFLING,
            1,
            "legendary spell",
            "snow spell",
            format!("{RESTRICTION}/uses/0/filter/supertypes"),
            Some(json!(["snow"])),
        ),
    ];
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    // Spawn once per edit, with small detached inputs. The archive is
    // never opened for writing and verifier-only corruptions never reach TS.
    for edit in edits {
        let mut child = Command::new("node")
            .arg(root.join("../../tests/reread-printed-evidence.ts"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let row: Value = serde_json::from_str(&rows[&edit.row]).unwrap();
        let mut input = child.stdin.take().unwrap();
        writeln!(
            input,
            "{}",
            json!({"row":row, "from":edit.from, "to":edit.to})
        )
        .unwrap();
        drop(input);
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "{}", edit.label);
        let raw = std::str::from_utf8(&output.stdout)
            .unwrap()
            .trim_end_matches('\n');
        let altered: Value = serde_json::from_str(raw).unwrap();
        let card = verified(raw).unwrap_or_else(|error| panic!("{}: {error:?}", edit.label));
        match card.block(0, edit.block, &edit.field) {
            Err(error) => {
                assert_eq!(error.check, Check::P11);
                assert!(error.range.is_some(), "{}: {error:?}", edit.label);
                println!("{}\treread\t{}", edit.label, super::failure(error.into()));
            }
            Ok(block) => {
                assert!(
                    !matches!(
                        edit.label,
                        "reassign this way"
                            | "attach only to wrong child"
                            | "use announced target controller"
                            | "use graveyard object controller"
                    ),
                    "{}: expected a located read rejection",
                    edit.label
                );
                let actual = ability(&altered, edit.block).pointer(&edit.field);
                assert_eq!(
                    actual,
                    edit.expected.as_ref(),
                    "{}: {}",
                    edit.label,
                    ability(&altered, edit.block)
                );
                if actual.is_some() {
                    let owner = edit.field.rsplit_once('/').unwrap().0;
                    assert!(block.node(owner).is_ok(), "{}: {owner}", edit.label);
                }
                match edit.label {
                    "shift token O" => {
                        let proof = block
                            .facts()
                            .find_map(|(index, fact)| {
                                matches!(fact, PrintedFact::Target(_))
                                    .then(|| block.fact(index).unwrap())
                            })
                            .unwrap();
                        assert_eq!(proof.range, [64, 70]);
                    }
                    "move target to nested scope"
                    | "attach another to wrong child"
                    | "cross Halfling mana mention into first activation" => {
                        let (old_block, old_field) = match edit.label {
                            "move target to nested scope" => {
                                (1, format!("{RETURN}/what/selectionId"))
                            }
                            "attach another to wrong child" => (1, format!("{FILTER}/other")),
                            _ => (1, format!("{RESTRICTION}/restriction")),
                        };
                        assert!(ability(&altered, old_block).pointer(&old_field).is_none());
                        let error = check_field(&card, old_block, &old_field).unwrap_err();
                        assert_eq!(error.check, Check::P11);
                        assert!(error.range.is_some());
                        match edit.label {
                            "move target to nested scope" => {
                                let (_, fact) = block
                                    .facts()
                                    .find(|(_, fact)| matches!(fact, PrintedFact::Target(_)))
                                    .unwrap();
                                let PrintedFact::Target(target) = fact else {
                                    unreachable!()
                                };
                                assert_eq!(target.field, Nullable::Value(edit.field.clone()));
                            }
                            "attach another to wrong child" => {
                                assert!(
                                    block.facts().any(
                                        |(_, fact)| matches!(fact, PrintedFact::Exception(value)
                                    if value.field == Nullable::Value(edit.field.clone()))
                                    )
                                );
                            }
                            _ => {
                                assert!(block.facts().any(|(_, fact)| matches!(fact, PrintedFact::Mention(value)
                                    if value.form == PrintedFactMentionForm::This && value.tok.0[0] == 0 && value.at == RESTRICTION)));
                            }
                        }
                    }
                    "swap target indices" => {
                        let indices: Vec<_> = block
                            .facts()
                            .filter_map(|(_, fact)| match fact {
                                PrintedFact::Target(target) => Some(target.target_index),
                                _ => None,
                            })
                            .collect();
                        assert_eq!(indices, [0, 1]);
                    }
                    "duplicate Flying with same token" => {
                        let first = block.node("/keywords/0").unwrap();
                        let second = block.node("/keywords/1").unwrap();
                        assert_eq!([first.range, second.range], [[0, 6], [8, 14]]);
                    }
                    "duplicate Halfling rider" => {
                        let first = block.node(&format!("{RIDER}/effects/0/steps/0")).unwrap();
                        let second = block.node("/effects/1/steps/0").unwrap();
                        assert!(first.range[1] < second.range[0]);
                    }
                    "diverge Halfling alias" => {
                        let canonical = format!("{RESTRICTION}/uses/0/filter");
                        let alias = block.node(&format!("{RIDER}/spentOn/0/filter")).unwrap();
                        assert_eq!(alias.owner, canonical);
                        assert_eq!(alias.range, block.node(&canonical).unwrap().range);
                        assert_eq!(
                            ability(&altered, edit.block)
                                .pointer(&format!("{RIDER}/spentOn/0/filter/supertypes")),
                            edit.expected.as_ref()
                        );
                    }
                    _ => {}
                }
                println!("{}\treread\tchecked AST {}", edit.label, edit.field);
            }
        }
    }
}
