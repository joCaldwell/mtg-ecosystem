//! Whole look/order/optional-shuffle/draw binding; CR401.4/608.2c/701.24.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};
pub(crate) fn coalesce(effects: &[Sentence]) -> Result<Option<Quantity>, Failure> {
    if !effects.iter().flat_map(|s| &s.steps).any(|s| {
        matches!(
            s,
            Step::Shuffle {
                optional: Some(true),
                ..
            }
        )
    }) {
        return Ok(None);
    }
    let fail = || {
        unsupported(
            "library order",
            "requires complete controller ordering and optional shuffle",
        )
    };
    let [
        Sentence {
            condition: None,
            condition_kind: None,
            bindings: a,
            steps: ordered,
        },
        Sentence {
            condition: None,
            condition_kind: None,
            bindings: b,
            steps: shuffled,
        },
    ] = effects
    else {
        return Err(fail());
    };
    let [
        Step::LookAt { who, what },
        Step::MoveZone {
            what: moved,
            to,
            tapped: None,
            order: Some(order),
        },
    ] = ordered.as_slice()
    else {
        return Err(fail());
    };
    let [
        Step::Shuffle {
            who: shuffler,
            library,
            optional: Some(true),
        },
    ] = shuffled.as_slice()
    else {
        return Err(fail());
    };
    if !a.is_empty()
        || !b.is_empty()
        || *who != json!({"player":"you"})
        || *what != json!({"top":what["top"],"of":{"zone":"library","owner":"your"}})
        || *moved != json!({"ref":"them"})
        || *to != json!({"zone":"library","owner":"your","position":"top"})
        || order != "any"
        || shuffler.player != "you"
        || shuffler.selection_id.is_some()
        || shuffler.of.is_some()
        || *library != json!({"zone":"library","owner":{"player":"you"}})
    {
        return Err(fail());
    }
    Ok(Some(controller_library::fixed_count(&what["top"])?))
}
pub(crate) fn validate_source(binding: &Value, identity: &str, raw: &str) -> Result<(), Failure> {
    let fail = || {
        unsupported(
            "binding",
            "requires complete controller order/shuffle/draw closure",
        )
    };
    let mut abilities: Value = decode(raw, "abilities")?;
    let n = abilities[0]["effects"][0]["steps"][0]["what"]["top"].clone();
    for ability in abilities.as_array_mut().ok_or_else(fail)? {
        for sentence in ability["effects"].as_array_mut().ok_or_else(fail)? {
            if let Some(bindings) = sentence.get("bindings") {
                if bindings != &json!([]) {
                    return Err(fail());
                }
                sentence
                    .as_object_mut()
                    .ok_or_else(fail)?
                    .remove("bindings");
            }
        }
    }
    let expected = json!([
    {"kind":"spell","effects":[{"steps":[{"effect":"look-at","who":{"player":"you"},"what":{"top":n,"of":{"zone":"library","owner":"your"}}},{"effect":"move-zone","what":{"ref":"them"},"to":{"zone":"library","owner":"your","position":"top"},"order":"any"}]},{"steps":[{"effect":"shuffle","who":{"player":"you"},"library":{"zone":"library","owner":{"player":"you"}},"optional":true}]}]},
    {"kind":"spell","effects":[{"steps":[{"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}]}]}
    ]);
    if abilities != expected {
        return Err(fail());
    }
    let path = "/faces/0/abilities";
    let root = format!("card:{identity}/faces/0/abilities");
    let you0 = format!("{root}/0#you");
    let you1 = format!("{root}/1#you");
    let look = format!("{root}/0/effects/0/steps/0");
    let refs = vec![
        (
            format!("{path}/0/effects/0/steps/0/who"),
            json!({"player":"you"}),
            you0.clone(),
        ),
        (
            format!("{path}/0/effects/0/steps/0/what/of/owner"),
            json!({"player":"you"}),
            you0.clone(),
        ),
        (
            format!("{path}/0/effects/0/steps/1/who"),
            json!({"player":"you"}),
            you0.clone(),
        ),
        (
            format!("{path}/0/effects/0/steps/1/what"),
            json!({"ref":"them"}),
            format!("{look}#objects"),
        ),
        (
            format!("{path}/0/effects/0/steps/1/to/owner"),
            json!({"player":"you"}),
            you0.clone(),
        ),
        (
            format!("{path}/0/effects/1/steps/0/who"),
            json!({"player":"you"}),
            you0.clone(),
        ),
        (
            format!("{path}/0/effects/1/steps/0/library/owner"),
            json!({"player":"you"}),
            you0.clone(),
        ),
        (
            format!("{path}/1/effects/0/steps/0/who"),
            json!({"player":"you"}),
            you1.clone(),
        ),
    ];
    let actual = binding["references"].as_array().ok_or_else(fail)?;
    let nodes = binding["nodes"].as_array().ok_or_else(fail)?;
    if binding["identity"] != identity
        || binding["fullyBound"] != true
        || binding["referencesBound"] != true
        || binding["diagnostics"] != json!([])
        || binding["counts"] != json!({"obligations":8,"bound":8,"unresolved":0})
        || actual.len() != 8
        || nodes.len() != 14
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
    for (block, you) in [(0, &you0), (1, &you1)] {
        if !nodes.iter().any(|n| {
            n["id"] == *you
                && n["node"] == "runtime-player"
                && n["scope"] == format!("{root}/{block}#ability")
                && n["role"] == "spell-controller"
        }) {
            return Err(fail());
        }
    }
    if !nodes.iter().any(|n| {
        n["id"] == format!("{look}#objects")
            && n["node"] == "event-result"
            && n["event"] == format!("{look}#event")
            && n["result"] == "objects"
            && n["identity"] == "same-objects"
    }) {
        return Err(fail());
    }
    if !nodes.iter().any(|n| {
        n["id"] == format!("{root}/0/effects/1/steps/0#event")
            && n["operation"] == "shuffle"
            && n["inputs"] == json!([you0])
            && n["optional"] == true
            && n["decider"] == you0
    }) {
        return Err(fail());
    }
    Ok(())
}
