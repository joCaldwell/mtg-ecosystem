//! CR121.2/401.4/608.2c: draw attempts precede a selection from the entire hand.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};
pub(super) fn coalesce(effects: &[Sentence]) -> Result<Option<(Quantity, Quantity)>, Failure> {
    if !effects.iter().flat_map(|s|&s.steps).any(|s|matches!(s,Step::MoveZone{what,order:Some(_),..} if what["filter"]["zone"]["zone"]=="hand")){return Ok(None);}
    let fail = || {
        unsupported(
            "hand return",
            "requires whole controller draw and ordered hand return",
        )
    };
    let [Sentence { bindings, steps, condition: None, condition_kind: None }] = effects else {
        return Err(fail());
    };
    let [
        Step::Draw {
            who,
            amount: DrawAmount::Fixed { value },
            optional: false,
        },
        Step::MoveZone {
            what,
            to,
            tapped: None,
            order: Some(order),
        },
    ] = steps.as_slice()
    else {
        return Err(fail());
    };
    if !bindings.is_empty()
        || who.player != "you"
        || who.selection_id.is_some()
        || who.of.is_some()
        || *to != json!({"zone":"library","owner":"your","position":"top"})
        || order != "any"
        || *what
            != json!({"ref":"filter","filter":{"cls":"card","zone":{"zone":"hand","owner":"your"}},"count":what["count"]})
    {
        return Err(fail());
    }
    Ok(Some((
        (*value as i64).into(),
        controller_library::fixed_count(&what["count"])?,
    )))
}
pub(super) fn validate_source(binding: &Value, identity: &str, raw: &str) -> Result<(), Failure> {
    let fail = || {
        unsupported(
            "binding",
            "requires complete controller draw/hand-return closure",
        )
    };
    let mut abilities: Value = decode(raw, "abilities")?;
    let draw = abilities[0]["effects"][0]["steps"][0]["amount"].clone();
    let count = abilities[0]["effects"][0]["steps"][1]["what"]["count"].clone();
    if abilities[0]["effects"][0].get("bindings").is_some() {
        if abilities[0]["effects"][0]["bindings"] != json!([]) {
            return Err(fail());
        }
        abilities[0]["effects"][0]
            .as_object_mut()
            .ok_or_else(fail)?
            .remove("bindings");
    }
    let selected = json!({"ref":"filter","filter":{"cls":"card","zone":{"zone":"hand","owner":"your"}},"count":count});
    if abilities
        != json!([{"kind":"spell","effects":[{"steps":[{"effect":"draw","who":{"player":"you"},"amount":draw},{"effect":"move-zone","what":selected,"to":{"zone":"library","owner":"your","position":"top"},"order":"any"}]}]}])
    {
        return Err(fail());
    }
    let base = "/faces/0/abilities/0";
    let root = format!("card:{identity}{base}");
    let you = format!("{root}#you");
    let query = format!("{root}/effects/0/steps/1/what#query");
    let refs = [
        (
            format!("{base}/effects/0/steps/0/who"),
            json!({"player":"you"}),
            you.clone(),
        ),
        (
            format!("{base}/effects/0/steps/1/who"),
            json!({"player":"you"}),
            you.clone(),
        ),
        (
            format!("{base}/effects/0/steps/1/what/filter/zone/owner"),
            json!({"player":"you"}),
            you.clone(),
        ),
        (
            format!("{base}/effects/0/steps/1/what"),
            selected.clone(),
            query.clone(),
        ),
        (
            format!("{base}/effects/0/steps/1/to/owner"),
            json!({"player":"you"}),
            you.clone(),
        ),
    ];
    let actual = binding["references"].as_array().ok_or_else(fail)?;
    let nodes = binding["nodes"].as_array().ok_or_else(fail)?;
    if binding["identity"] != identity
        || binding["fullyBound"] != true
        || binding["referencesBound"] != true
        || binding["diagnostics"] != json!([])
        || binding["counts"] != json!({"obligations":5,"bound":5,"unresolved":0})
        || actual.len() != 5
        || nodes.len() != 10
        || refs.iter().any(|(p, e, t)| {
            let m = actual
                .iter()
                .filter(|r| r["path"] == *p)
                .collect::<Vec<_>>();
            m.len() != 1
                || m[0]["expression"] != *e
                || m[0]["target"] != *t
                || m[0]["binding"] != "bound"
        })
    {
        return Err(fail());
    }
    if !nodes.iter().any(|n| {
        n["id"] == you
            && n["node"] == "runtime-player"
            && n["scope"] == format!("{root}#ability")
            && n["role"] == "spell-controller"
    }) || !nodes.iter().any(|n| {
        n["id"] == query
            && n["node"] == "selection"
            && n["expression"] == selected
            && n["timing"] == "resolution"
            && n["selection"] == "evaluate-reference"
    }) {
        return Err(fail());
    }
    Ok(())
}
