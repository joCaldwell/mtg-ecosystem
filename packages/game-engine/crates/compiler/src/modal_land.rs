//! Verify both raw archive faces and their ordered graph before lowering.
use super::*;
use serde_json::{Value, json};

pub(super) fn lower(
    rows: &Record,
    source: &Value,
    archive: &AcceptedArchive,
) -> Result<Definition, Failure> {
    let fail = || unsupported("source.layout", "multi_face_layout:modal_dfc");
    let [first, second] = rows.faces.as_slice() else {
        return Err(fail());
    };
    let [front, back] = source["card_faces"].as_array().ok_or_else(fail)?.as_slice() else {
        return Err(fail());
    };
    let ast_digest = rows.ast_sha256.as_ref().ok_or_else(fail)?;
    let mut results = vec![];
    for (index, (entry, face)) in [(first, front), (second, back)].into_iter().enumerate() {
        if entry.source_face_index as usize != index || entry.status != "attempted" {
            return Err(fail());
        }
        let decoded: ParseResult = decode(entry.result.get(), "faces.result")?;
        let expected_text = face["oracle_text"].as_str().ok_or_else(fail)?;
        if !decoded.ok
            || face["name"] != decoded.name
            || decoded.lines.iter().any(|line| !line.ok)
            || !decoded
                .lines
                .iter()
                .map(|line| line.text.as_str())
                .eq(expected_text.lines())
        {
            return Err(fail());
        }
        results.push(decoded);
    }
    let definition = mtg_contracts::modal_land::from_archive(SourceProvenance {
        printing_id: source["id"].as_str().ok_or_else(fail)?.into(),
        source_index: rows.source_index,
        archive_manifest_sha256: archive.manifest_sha256.clone(),
        parser_sha256: archive.parser_source_sha256.clone(),
        source_sha256: rows.source_sha256.clone(),
        ast_sha256: ast_digest.clone(),
        binding_sha256: rows.binding_sha256.clone(),
        source_json: rows.source.get().into(),
        abilities_json: format!(
            "{},{}",
            results[0].abilities.get(),
            results[1].abilities.get()
        ),
    })
    .map_err(|_| fail())?;
    if definition.oracle_id != rows.oracle_identity {
        return Err(fail());
    }

    let mut face_zero = front.clone();
    face_zero["layout"] = source["layout"].clone();
    let mut graph = bindings::Bindings::new(&rows.oracle_identity, &face_zero);
    if definition.characteristics.kind == CardKind::Land {
        bind_face(&mut graph, "/faces/0", &definition);
    } else {
        let ast: Value = decode(results[0].abilities.get(), "faces[0].abilities")?;
        let root = "/faces/0/abilities/0";
        let step = format!("{root}/effects/0/steps/0");
        let what = format!("{step}/what");
        graph.ability(root, "spell", "spell-controller");
        let actor = graph.id(root, "you");
        graph.reference(
            &format!("{what}/filter/zone/owner"),
            &json!({"player":"you"}),
            &actor,
            "109.5",
        );
        let target = graph.target(
            &graph.id(root, "ability"),
            &what,
            &ast[0]["effects"][0]["steps"][0]["what"],
            "line:0:1",
        );
        graph.event(root, &step, "return-to-hand", &target, false);
    }
    let object = graph.id("/source", "object");
    graph.node(
        "/faces/1",
        "face",
        "face",
        json!({
            "source":object,"sourceFaceIndex":1,"name":back["name"],"layout":source["layout"],
            "typeLine":back["type_line"],"manaCost":back["mana_cost"],"rules":["712"]
        }),
    );
    bind_face(
        &mut graph,
        "/faces/1",
        definition.modal_land_back().ok_or_else(fail)?,
    );
    let actual: Value = decode(rows.binding.get(), "binding")?;
    graph.validate_ordered(
        &actual,
        "both modal land faces require complete own mana bindings",
    )?;
    Ok(definition)
}

fn bind_face(graph: &mut bindings::Bindings<'_>, face: &str, definition: &Definition) {
    let source = graph.id("/source", "object");
    let face_id = graph.id(face, "face");
    let ability =
        |g: &mut bindings::Bindings<'_>, block: u32, kind: &str, actor: &str| {
            let root = format!("{face}/abilities/{block}");
            let you = g.id(&root, "you");
            let scope = g.node(&root, "ability", "ability", json!({
            "source":source,"face":face_id,"abilityKind":kind,"you":you,"rules":["113","109.5"]
        }));
            g.node(
                &root,
                "you",
                "runtime-player",
                json!({"scope":scope,"role":actor,"rules":["109.5"]}),
            );
            (root, you, scope)
        };
    if !definition.static_abilities.is_empty() {
        let (root, _, scope) = ability(graph, 0, "static", "static-source-controller");
        graph.node(
            &format!("{root}/effect"),
            "entering-controller",
            "runtime-player",
            json!({
                "scope":scope,"role":"entering-controller","rules":["110.2a","614.12"]
            }),
        );
        graph.reference(
            &format!("{root}/effect/what"),
            &json!({"ref":"this","noun":"land"}),
            &source,
            "201.5",
        );
    }
    let (root, you, _) = ability(
        graph,
        definition.mana_abilities[0].source_block,
        "activated",
        "activator",
    );
    let who = json!({"player":"you"});
    graph.reference(
        &format!("{root}/costs/0/source"),
        &json!({"ref":"self"}),
        &source,
        "201.5",
    );
    graph.reference(&format!("{root}/costs/0/payer"), &who, &you, "109.5");
    let step = format!("{root}/effects/0/steps/0");
    graph.reference(&format!("{step}/who"), &who, &you, "109.5");
    graph.event(&root, &step, "add-mana", &you, false);
}

#[test]
fn both_modal_land_ast_and_graph_faces_are_required() {
    const HEAD: &str = include_str!("../../../tests/fixtures/archive-manifest-phase1-r3.json");
    const ROW: &str = include_str!("../../../tests/fixtures/phase1-r3/branchloft-pathway.json");
    let result = compile(HEAD, ROW);
    let CompileResult::Lowered { definition } = result else {
        panic!("{result:?}")
    };
    let back = definition.modal_land_back().unwrap();
    assert_eq!(definition.source_face_name(), "Branchloft Pathway");
    assert_eq!(back.name, "Boulderloft Pathway");
    assert_eq!(
        back.mana_abilities[0].production,
        ManaProduction::Fixed {
            symbols: vec![Color::W]
        }
    );
    let changed = tests::changed_source(ROW, |row| {
        let refs = row["binding"]["references"].as_array_mut().unwrap();
        let from_front = refs[1]["target"].clone();
        let back_payer = refs
            .iter_mut()
            .find(|r| r["path"] == "/faces/1/abilities/0/costs/0/payer")
            .unwrap();
        back_payer["target"] = from_front;
    });
    let mut changed: Value = serde_json::from_str(&changed).unwrap();
    let both_faces = json!([
        changed["faces"][0]["result"]["abilities"],
        changed["faces"][1]["result"]["abilities"],
    ]);
    changed["astSha256"] = json!(sha256(canonical(&both_faces).unwrap().as_bytes()));
    let changed = compile(HEAD, &canonical(&changed).unwrap());
    assert!(
        matches!(&changed, CompileResult::Unsupported { path, .. } if path == "binding"),
        "{changed:?}"
    );
    let mut forged = *definition;
    forged.modal_land = None;
    forged.capabilities = forged.requirements();
    assert!(forged.validate().is_err());
}
