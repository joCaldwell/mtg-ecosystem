use super::*;
use crate::ingest::evidence::{Basis, Check, EvidenceError};
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

const SAHEELI: usize = 35;
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

/// An opt-in fixture family reads the operative field when present. Other
/// printed material is outside the selected test obligation. Drive the
/// remaining claims explicitly so P12 has to find the exact unused occurrence.
fn check_field(
    row: &Value,
    evidence: &CardEvidence,
    index: usize,
    field: &str,
    mention: &str,
) -> Result<(), EvidenceError> {
    let block = evidence.block(0, index, field)?;
    block.check_family(|coverage| {
        if ability(row, index).pointer(field).is_some() {
            coverage.field(field)?;
        }
        let wire = self::block(row, index);
        let mut omitted = None;
        for (fact_index, fact) in wire["facts"].as_array().unwrap().iter().enumerate() {
            if fact["form"] == mention {
                omitted = fact["tok"]
                    .as_array()
                    .map(|parts| [0, 1, 2].map(|part| parts[part].as_u64().unwrap() as u32));
            } else {
                coverage.fact(fact_index)?;
            }
        }
        let [first, end] = [0, 1].map(|i| wire["lines"][i].as_u64().unwrap() as u32);
        for line in first..end {
            let tokens = row["printed"]["faces"][0]["lines"][line as usize]["tokens"]
                .as_array()
                .unwrap();
            for token in 0..tokens.len() as u32 {
                if !omitted.is_some_and(|[l, s, e]| l == line && (s..e).contains(&token)) {
                    coverage.token(line, token)?;
                }
            }
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
        check_field(&row, &card, 1, &field, "only").unwrap_err(),
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
        check_field(&row, &card, 1, &field, "only").unwrap_err(),
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
            check_field(&row, &card, index, &field, form).unwrap_err(),
            if add { Check::P11 } else { Check::P12 },
            range,
        );
    }
    let mut row = get(JUNK);
    let keyword = ability(&row, 0)["keywords"][0].clone();
    ast_mut(&mut row, 0)["keywords"]
        .as_array_mut()
        .unwrap()
        .push(keyword);
    expect_check("duplicate Flying", &mut row, "P6");
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
            [SAHEELI, JUNK, TERA, SECRET, BOSEIJU, HALFLING]
                .contains(&number)
                .then_some((number, line))
        })
        .collect();
    check_witnesses(&rows);
    check_mutations(&rows);
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
            "move target O with reminder",
            JUNK,
            1,
            "Flying\n",
            "Flying (Reminder text.)\n",
            format!("{RETURN}/what/selectionId"),
            Some(json!("line:1:7")),
        ),
        read(
            "misspell Boseiju",
            BOSEIJU,
            0,
            "Boseiju enters",
            "Boseij enters",
            "/effect/what/ref".into(),
            None,
        ),
        read(
            "move Junk Diver selection token",
            JUNK,
            1,
            "another target",
            "target",
            format!("{RETURN}/what/selectionId"),
            Some(json!("line:1:6")),
        ),
        read(
            "swap modal target heads",
            SAHEELI,
            0,
            "target artifact.\n• Create a token that's a copy of target creature",
            "target creature.\n• Create a token that's a copy of target artifact",
            "/effects/0/steps/0/options/0/effects/0/steps/0/token/copyOf/filter/types".into(),
            Some(json!(["creature"])),
        ),
        read(
            "remove target introduction",
            JUNK,
            1,
            "another target artifact card",
            "an artifact card",
            format!("{RETURN}/what/selectionId"),
            None,
        ),
        read(
            "remove this way linkage",
            TERA,
            0,
            "graveyard this way",
            "graveyard",
            format!("{ITERATION}/scope"),
            None,
        ),
        read(
            "print two on Terastodon",
            TERA,
            0,
            "up to three",
            "up to two",
            format!("{COUNT}/value"),
            Some(json!(2)),
        ),
        read(
            "print three life on Boseiju",
            BOSEIJU,
            1,
            "Pay 2 life",
            "Pay 3 life",
            "/costs/1/amount/value".into(),
            Some(json!(3)),
        ),
        read(
            "print two on Junk Diver",
            JUNK,
            1,
            "another target artifact card",
            "two target artifact cards",
            format!("{COUNT}/value"),
            Some(json!(2)),
        ),
        read(
            "print creature instead of noncreature",
            TERA,
            0,
            "noncreature permanents",
            "creature permanents",
            format!("{FILTER}/types"),
            Some(json!(["creature"])),
        ),
        read(
            "remove Halfling spend clause",
            HALFLING,
            1,
            " Spend this mana only to cast a legendary spell, and that spell can't be countered.",
            "",
            RESTRICTION.into(),
            None,
        ),
        read(
            "print Boseiju spend restriction",
            BOSEIJU,
            1,
            "If that mana is spent on an instant or sorcery spell, that spell can't be countered.",
            "Spend this mana only to cast an instant or sorcery spell, and that spell can't be countered.",
            format!("{RESTRICTION}/restriction"),
            Some(json!("only")),
        ),
        read(
            "remove printed may",
            TERA,
            0,
            "you may destroy",
            "destroy",
            format!("{RETURN}/optional"),
            None,
        ),
        read(
            "print Junk Diver may",
            JUNK,
            1,
            "dies, return",
            "dies, you may return",
            format!("{RETURN}/optional"),
            Some(json!(true)),
        ),
        read(
            "remove printed another",
            JUNK,
            1,
            "another target",
            "target",
            format!("{FILTER}/other"),
            None,
        ),
        read(
            "print Terastodon other",
            TERA,
            0,
            "three target",
            "three other target",
            format!("{FILTER}/other"),
            Some(json!(true)),
        ),
        read(
            "print Flying twice",
            JUNK,
            0,
            "Flying\n",
            "Flying, flying\n",
            "/keywords/1/keyword".into(),
            Some(json!("flying")),
        ),
        read(
            "print Halfling rider twice",
            HALFLING,
            1,
            "and that spell can't be countered.",
            "and that spell can't be countered. That spell can't be countered.",
            "/effects/1/steps/0/effect".into(),
            Some(json!("cant")),
        ),
        read(
            "change Halfling alias source",
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
                    "move target O with reminder" => {
                        let proof = block
                            .facts()
                            .find_map(|(index, fact)| {
                                matches!(fact, PrintedFact::Target(_))
                                    .then(|| block.fact(index).unwrap())
                            })
                            .unwrap();
                        assert_eq!(proof.range, [64, 70]);
                    }
                    "swap modal target heads" => {
                        let indices: Vec<_> = block
                            .facts()
                            .filter_map(|(_, fact)| match fact {
                                PrintedFact::Target(target) => Some(target.target_index),
                                _ => None,
                            })
                            .collect();
                        assert_eq!(indices, [0, 1]);
                    }
                    "print Flying twice" => {
                        let first = block.node("/keywords/0").unwrap();
                        let second = block.node("/keywords/1").unwrap();
                        assert_eq!([first.range, second.range], [[0, 6], [8, 14]]);
                    }
                    "print Halfling rider twice" => {
                        let first = block.node(&format!("{RIDER}/effects/0/steps/0")).unwrap();
                        let second = block.node("/effects/1/steps/0").unwrap();
                        assert!(first.range[1] < second.range[0]);
                    }
                    "change Halfling alias source" => {
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
