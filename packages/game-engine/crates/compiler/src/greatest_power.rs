//! Reconstruct the narrowly recognized complete creature and its bound event
//! identities. The old unparsed-face proof stays in archive provenance.
use super::*;
use crate::bindings::Bindings;
use serde_json::{Value, json};

fn full_graph<'a>(identity: &'a str, raw: &Value, ast: &Value) -> Bindings<'a> {
    let mut g = Bindings::new(identity, raw);
    let t = "/faces/0/abilities/0";
    let m = "/faces/0/abilities/1";
    let event = format!("{t}/trigger");
    let subject = format!("{event}/what");
    let draw = format!("{t}/effects/0/steps/0");
    let add = format!("{m}/effects/0/steps/0");
    let number = format!("{m}/effects/0/bindings/0");
    let actor = &ast[0]["effects"][0]["steps"][0]["who"];
    let it = &actor["of"];
    g.ability(t, "triggered", "trigger-controller");
    let select=g.node(&subject,"query","selection",json!({"scope":g.id(t,"ability"),"expression":ast[0]["trigger"]["what"],"cardinality":"one","timing":"trigger-match","selection":"evaluate-reference","rules":["608.2h"]}));
    g.reference(
        &format!("{subject}/filter/other"),
        &json!({"ref":"self"}),
        &g.id("/source", "object"),
        "201.5",
    );
    g.reference(&subject, &ast[0]["trigger"]["what"], &select, "608.2h");
    g.event(t, &event, "trigger:enters", &select, false);
    g.result(
        &event,
        "objects",
        "same-objects",
        Some(&ast[0]["trigger"]["what"]["filter"]),
    );
    for field in ["who", "decidedBy"] {
        let p = format!("{draw}/{field}");
        let q=g.node(&p,"query","selection",json!({"scope":g.id(t,"ability"),"expression":actor,"cardinality":"one","timing":"resolution","selection":"evaluate-reference","rules":["608.2h"]}));
        g.reference_rules(
            &format!("{p}/of"),
            it,
            &g.id(&event, "objects"),
            &["608.2k", "400.7j", "608.2h"],
        );
        g.reference(&p, actor, &q, "608.2h");
    }
    g.node(&draw,"event","event",json!({"scope":g.id(t,"ability"),"operation":"draw","inputs":[g.id(&format!("{draw}/who"),"query")],"availability":"when-performed","optional":true,"decider":g.id(&format!("{draw}/decidedBy"),"query"),"conditional":true,"rules":["608.2c"]}));
    g.result(
        &draw,
        "objects",
        "new-objects",
        Some(&json!({"cls":"card"})),
    );
    g.result(&draw, "quantity", "measured-quantity", None);
    g.reference_rules(
        &format!("{t}/effects/0/condition/what"),
        it,
        &g.id(&event, "objects"),
        &["608.2k", "400.7j", "608.2h"],
    );
    g.ability(m, "activated", "activator");
    let you = g.id(m, "you");
    let player = json!({"player":"you"});
    g.reference(&format!("{m}/costs/0/payer"), &player, &you, "109.5");
    g.reference(
        &format!("{m}/costs/1/source"),
        &json!({"ref":"self"}),
        &g.id("/source", "object"),
        "201.5",
    );
    g.reference(&format!("{m}/costs/1/payer"), &player, &you, "109.5");
    let n=g.node(&number,"number","defined-number",json!({"scope":g.id(m,"ability"),"definition":ast[1]["effects"][0]["bindings"][0]["value"],"rules":["107.3c"]}));
    g.reference(&format!("{add}/who"), &player, &you, "109.5");
    g.reference(
        &format!("{add}/mana/amount"),
        &json!({"amount":"x"}),
        &n,
        "107.3c",
    );
    g.reference(
        &format!("{number}/value/among/control"),
        &player,
        &you,
        "109.5",
    );
    g.event(m, &add, "add-mana", &you, false);
    g
}

pub(super) fn lower(r: &Record, archive: &AcceptedArchive) -> Result<Option<Definition>, Failure> {
    if !r.source.get().contains("each other creature") && !r.source.get().contains(r"\u") {
        return Ok(None);
    }
    let raw: Value = decode(r.source.get(), "source")?;
    let text = raw["oracle_text"].as_str().unwrap_or("");
    let Some(color) = greatest_power_source(text) else {
        return Ok(None);
    };
    let fail = || {
        unsupported(
            "greatest power source",
            "requires exact complete lines, face, and diagnostic bindings",
        )
    };
    if sha256(r.source.get().as_bytes()) != r.source_sha256
        || sha256(r.binding.get().as_bytes()) != r.binding_sha256
    {
        return Err(invalid("record", "source or binding hash mismatch"));
    }
    if r.faces.len() != 1 || r.faces[0].status != "attempted" || r.faces[0].source_face_index != 0 {
        return Err(fail());
    }
    let src: Source = decode(r.source.get(), "source")?;
    if src.oracle_id != r.oracle_identity || src.id.is_empty() || src.name.is_empty() {
        return Err(fail());
    }
    let ast = greatest_power_ast(color);
    let lines: Vec<_> = text.lines().collect();
    let face: Value = decode(r.faces[0].result.get(), "face")?;
    let binding: Value = decode(r.binding.get(), "binding")?;
    let (ast_raw, ast_sha) = match r.status.as_str() {
        "parsed" => {
            let expected = json!({"name":src.name,"ok":true,"lines":[{"text":lines[0],"ok":true,"ability":ast[0]},{"text":lines[1],"ok":true,"ability":ast[1]}],"abilities":ast});
            if face != expected {
                return Err(fail());
            }
            full_graph(&r.oracle_identity, &raw, &ast).validate_ordered(
                &binding,
                "incomplete entrant, controller or numeric binding",
            )?;
            let result: ParseResult = decode(r.faces[0].result.get(), "result")?;
            let s = result.abilities.get().to_owned();
            let hash = sha256(format!("[{s}]").as_bytes());
            if r.ast_sha256.as_ref() != Some(&hash) {
                return Err(fail());
            }
            (s, hash)
        }
        "unsupported" => {
            let expected = json!({"name":src.name,"ok":false,"lines":[{"text":lines[0],"ok":false,"error":"expected 'player's' | 'opponent's' at \"other\" (token 18)"},{"text":lines[1],"ok":true,"ability":ast[1]}]});
            let diagnostic = json!({"code":"unparsed-face","path":"/faces/0","message":"A missing or failed structural parse cannot have complete reference binding.","rules":[]});
            if face != expected
                || r.ast_sha256.is_some()
                || Bindings::new(&r.oracle_identity, &raw).with_scope_diagnostic(diagnostic)
                    != binding
            {
                return Err(fail());
            }
            let s = canonical(&ast).map_err(|_| fail())?;
            let hash = sha256(format!("[{s}]").as_bytes());
            (s, hash)
        }
        _ => return Err(fail()),
    };
    let types = type_line::parse(&src.type_line)?;
    let (spell, trigger, mana) = greatest_power_programs(color);
    let stat = |v: Option<String>| -> Result<Quantity, Failure> {
        v.ok_or_else(fail)?.parse::<Quantity>().map_err(|_| fail())
    };
    let mut d = Definition {
        oracle_id: src.oracle_id,
        name: src.name,
        x_mana_cost: false,
        composed: false,
        intrinsic_zone: None,
        activated: vec![],
        static_abilities: vec![],
        keywords: vec![],
        capabilities: BTreeSet::new(),
        characteristics: Characteristics {
            kind: types.kind,
            additional_types: types.additional_types,
            subtypes: types.subtypes,
            basic: types.basic,
            legendary: types.legendary,
            snow: types.snow,
            hybrid_mana: vec![],
            mana_cost: mana_cost(&src.mana_cost)?,
            colors: src.colors,
            color_identity: src.color_identity,
            power: Some(stat(src.power)?),
            toughness: Some(stat(src.toughness)?),
        },
        spell: Some(spell),
        triggered: vec![trigger],
        mana_abilities: vec![mana],
        modal_land: None,
        transform_faces: None,
        source_labels: Some(SourceLabels {
            text: text.to_owned(),
            blocks: lines.into_iter().map(str::to_owned).collect(),
            triggered: vec![0],
            mana: vec![1],
            ..SourceLabels::default()
        }),
        provenance: SourceProvenance {
            printing_id: src.id,
            source_index: r.source_index,
            source_sha256: r.source_sha256.clone(),
            source_json: r.source.get().into(),
            binding_sha256: r.binding_sha256.clone(),
            archive_manifest_sha256: archive.manifest_sha256.clone(),
            parser_sha256: archive.parser_source_sha256.clone(),
            ast_sha256: ast_sha,
            abilities_json: ast_raw,
        },
    };
    d.capabilities = d.requirements();
    d.validate().map_err(|msg| unsupported("definition", msg))?;
    Ok(Some(d))
}

#[cfg(test)]
mod tests {
    use super::*;
    const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    const SYNTHETIC: &str =
        include_str!("../../../tests/fixtures/synthetic-copperleaf-surveyor.json");
    #[test]
    fn greatest_power_requires_complete_current_or_historical_binding() {
        let historic =
            include_str!("../../../tests/fixtures/ramos/1d725121-e50c-42f0-9128-56802f07c89e.json");
        for row in [historic, SYNTHETIC] {
            let result = compile(MANIFEST, row);
            assert!(
                matches!(result, CompileResult::Lowered { .. }),
                "{result:?}"
            );
        }
        let detached = crate::tests::changed_source(SYNTHETIC, |r| {
            r["binding"]["references"][0]["target"] = json!("different-source");
        });
        assert!(matches!(
            compile(MANIFEST, &detached),
            CompileResult::Unsupported { .. }
        ));
        // Preserve exact archive source and graph layout through the mutation.
        let altered = historic.replace("expected 'player's' | 'opponent's'", "different");
        assert_ne!(altered, historic);
        assert!(matches!(
            compile(MANIFEST, &altered),
            CompileResult::Unsupported { .. }
        ));
    }
}
