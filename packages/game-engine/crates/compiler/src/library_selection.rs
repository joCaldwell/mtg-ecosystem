//! Complete look/pick-one/remainder source and binding closure, CR608.2c/h/k.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};
pub(super) fn coalesce(
    effects: &[Sentence],
) -> Result<Option<(Quantity, LibraryRemainder)>, Failure> {
    if !effects
        .iter()
        .flat_map(|s| &s.steps)
        .any(|s| matches!(s, Step::LookAt { .. }))
    {
        return Ok(None);
    }
    let fail = || {
        unsupported(
            "library selection",
            "requires complete look/pick-one/remainder source",
        )
    };
    let [
        Sentence {
            bindings: a,
            steps: look,
        },
        Sentence {
            bindings: b,
            steps: rest,
        },
    ] = effects
    else {
        return Err(fail());
    };
    let [Step::LookAt { who, what }] = look.as_slice() else {
        return Err(fail());
    };
    let [Step::Simultaneous { steps }] = rest.as_slice() else {
        return Err(fail());
    };
    if !a.is_empty()
        || !b.is_empty()
        || *who != json!({"player":"you"})
        || *what != json!({"top":what["top"],"of":{"zone":"library","owner":"your"}})
    {
        return Err(fail());
    }
    let amount = controller_library::fixed_count(&what["top"])?;
    let [hand, remainder] = steps.as_slice() else {
        return Err(fail());
    };
    if *hand
        != json!({"effect":"move-zone","what":{"ref":"from-pool","pool":{"ref":"them"},"count":{"amount":"fixed","value":1}},"to":{"zone":"hand","owner":"your"}})
    {
        return Err(fail());
    }
    let grave = json!({"effect":"move-zone","what":{"ref":"rest"},"to":{"zone":"graveyard","owner":"your"}});
    let bottom = json!({"effect":"move-zone","what":{"ref":"rest"},"to":{"zone":"library","owner":"your","position":"bottom"}});
    let mut ordered = bottom.clone();
    ordered["order"] = json!("any");
    let destination = if *remainder == grave {
        LibraryRemainder::Graveyard
    } else if *remainder == ordered || (*remainder == bottom && amount <= 2.into()) {
        LibraryRemainder::Bottom
    } else {
        return Err(fail());
    };
    Ok(Some((amount, destination)))
}
pub(super) fn validate_bindings(binding: &Value, identity: &str, raw: &str) -> Result<(), Failure> {
    let fail = || {
        unsupported(
            "binding",
            "requires complete library selection reference closure",
        )
    };
    let abilities: Value = decode(raw, "abilities")?;
    let [a] = abilities.as_array().ok_or_else(fail)?.as_slice() else {
        return Err(fail());
    };
    if a.as_object()
        .is_none_or(|o| o.len() != 2 || !o.contains_key("kind") || !o.contains_key("effects"))
        || a["kind"] != "spell"
    {
        return Err(fail());
    }
    let scope = format!("card:{identity}/faces/0/abilities/0#ability");
    let you = format!("card:{identity}/faces/0/abilities/0#you");
    let prefix = format!("card:{identity}/faces/0/abilities/0/effects/");
    let look = format!("{prefix}0/steps/0");
    let picked = format!("{prefix}1/steps/0/steps/0");
    let rest = format!("{prefix}1/steps/0/steps/1");
    let steps = &a["effects"][1]["steps"][0]["steps"];
    let expected = vec![
        (
            "/faces/0/abilities/0/effects/0/steps/0/who",
            json!({"player":"you"}),
            you.clone(),
        ),
        (
            "/faces/0/abilities/0/effects/0/steps/0/what/of/owner",
            json!({"player":"you"}),
            you.clone(),
        ),
        (
            "/faces/0/abilities/0/effects/1/steps/0/steps/0/who",
            json!({"player":"you"}),
            you.clone(),
        ),
        (
            "/faces/0/abilities/0/effects/1/steps/0/steps/0/what/pool",
            json!({"ref":"them"}),
            format!("{look}#objects"),
        ),
        (
            "/faces/0/abilities/0/effects/1/steps/0/steps/0/what",
            steps[0]["what"].clone(),
            format!("{picked}/what#query"),
        ),
        (
            "/faces/0/abilities/0/effects/1/steps/0/steps/0/to/owner",
            json!({"player":"you"}),
            you.clone(),
        ),
        (
            "/faces/0/abilities/0/effects/1/steps/0/steps/1/who",
            json!({"player":"you"}),
            you.clone(),
        ),
        (
            "/faces/0/abilities/0/effects/1/steps/0/steps/1/what",
            json!({"ref":"rest"}),
            format!("{rest}/what#remainder"),
        ),
        (
            "/faces/0/abilities/0/effects/1/steps/0/steps/1/to/owner",
            json!({"player":"you"}),
            you.clone(),
        ),
    ];
    let refs = binding["references"].as_array().ok_or_else(fail)?;
    if binding["identity"] != identity
        || binding["fullyBound"] != true
        || binding["referencesBound"] != true
        || binding["diagnostics"] != json!([])
        || binding["counts"] != json!({"obligations":9,"bound":9,"unresolved":0})
        || refs.len() != 9
        || expected.iter().any(|(p, e, t)| {
            let matches = refs.iter().filter(|r| r["path"] == *p).collect::<Vec<_>>();
            matches.len() != 1
                || matches[0]["expression"] != *e
                || matches[0]["target"] != *t
                || matches[0]["binding"] != "bound"
        })
    {
        return Err(fail());
    }
    let nodes = binding["nodes"].as_array().ok_or_else(fail)?;
    let find = |id: String| -> Result<&Value, Failure> {
        let m = nodes.iter().filter(|n| n["id"] == id).collect::<Vec<_>>();
        if m.len() == 1 { Ok(m[0]) } else { Err(fail()) }
    };
    // CR400.7j: only the public graveyard move exports successor objects.
    let expected_nodes = if steps[1]["to"]["zone"] == "graveyard" {
        12
    } else {
        11
    };
    if nodes.len() != expected_nodes {
        return Err(fail());
    }
    let n = find(you.clone())?;
    if n["node"] != "runtime-player" || n["scope"] != scope || n["role"] != "spell-controller" {
        return Err(fail());
    }
    let n = find(format!("{look}#event"))?;
    if n["node"] != "event"
        || n["operation"] != "look-at"
        || n["scope"] != scope
        || n["inputs"] != json!([you])
    {
        return Err(fail());
    }
    let n = find(format!("{look}#objects"))?;
    if n["node"] != "event-result"
        || n["event"] != format!("{look}#event")
        || n["result"] != "objects"
        || n["identity"] != "same-objects"
    {
        return Err(fail());
    }
    let n = find(format!("{picked}/what#query"))?;
    if n["node"] != "selection"
        || n["scope"] != scope
        || n["expression"] != steps[0]["what"]
        || n["cardinality"] != "one"
        || n["timing"] != "resolution"
        || n["selection"] != "evaluate-reference"
    {
        return Err(fail());
    }
    let n = find(format!("{rest}/what#remainder"))?;
    if n["node"] != "remainder"
        || n["scope"] != scope
        || n["pool"] != format!("{look}#objects")
        || n["excluding"] != format!("{picked}/what#query")
    {
        return Err(fail());
    }
    for (path, input) in [(&picked, "query"), (&rest, "remainder")] {
        let n = find(format!("{path}#event"))?;
        if n["node"] != "event"
            || n["operation"] != "move-zone"
            || n["scope"] != scope
            || n["inputs"] != json!([format!("{path}/what#{input}")])
        {
            return Err(fail());
        }
    }
    Ok(())
}
