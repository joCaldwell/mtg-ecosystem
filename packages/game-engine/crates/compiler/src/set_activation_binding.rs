//! Complete source/activator/recipient binding for source-sacrifice set grants.
//! CR109.5, 113.10, 602.2, 608.2h; frozen20260925.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn bind(
    binding: &Value,
    identity: &str,
    name: &str,
    layout: &str,
    type_line: &str,
    mana_cost: &str,
    raw: &str,
) -> Result<(), Failure> {
    let bad = || {
        unsupported(
            "binding",
            "requires complete source-sacrifice set-grant binding closure",
        )
    };
    let abilities: Value = decode(raw, "abilities")?;
    let [a] = abilities.as_array().ok_or_else(bad)?.as_slice() else {
        return Err(bad());
    };
    let step = &a["effects"][0]["steps"][0];
    let gained = step["abilities"].as_array().ok_or_else(bad)?;
    let what = &step["what"];
    let path = "/faces/0/abilities/0";
    let root = format!("card:{identity}");
    let source = format!("{root}/source#object");
    let face = format!("{root}/faces/0#face");
    let ability = format!("{root}{path}#ability");
    let you = format!("{root}{path}#you");
    let cost = format!("{path}/costs/0");
    let effect = format!("{path}/effects/0/steps/0");
    let query = format!("{root}{effect}/what#query");
    let mut nodes = vec![
        json!({"id":source,"path":"/source","node":"source","identity":identity}),
        json!({"id":face,"path":"/faces/0","node":"face","source":source,"sourceFaceIndex":0,"name":name,"layout":layout,"typeLine":type_line,"manaCost":mana_cost}),
        json!({"id":ability,"path":path,"node":"ability","source":source,"face":face,"abilityKind":"activated","you":you}),
        json!({"id":you,"path":path,"node":"runtime-player","scope":ability,"role":"activator"}),
        json!({"id":format!("{root}{cost}#event"),"path":cost,"node":"event","scope":ability,"operation":"cost:sacrifice","inputs":[],"availability":"when-performed"}),
        json!({"id":format!("{root}{cost}#quantity"),"path":cost,"node":"event-result","event":format!("{root}{cost}#event"),"result":"quantity","cardinality":"one","availability":"if-produced","identity":"measured-quantity"}),
        json!({"id":query,"path":format!("{effect}/what"),"node":"selection","scope":ability,"expression":what,"cardinality":"many","timing":"resolution","selection":"evaluate-reference"}),
        json!({"id":format!("{root}{effect}#event"),"path":effect,"node":"event","scope":ability,"operation":"gain-abilities","inputs":[],"availability":"when-performed"}),
    ];
    for index in 0..gained.len() {
        let p = format!("{effect}/abilities/{index}");
        let id = format!("{root}{p}");
        nodes.extend([
            json!({"id":format!("{id}#recipient"),"path":p,"node":"recipient","scope":ability,"of":query,"each":true}),
            json!({"id":format!("{id}#ability"),"path":p,"node":"ability","source":format!("{id}#recipient"),"face":face,"abilityKind":"keywords","you":format!("{id}#you")}),
            json!({"id":format!("{id}#you"),"path":p,"node":"runtime-player","scope":format!("{id}#ability"),"role":"static-source-controller"}),
            json!({"id":format!("{id}/keyword#you"),"path":format!("{p}/keyword"),"node":"runtime-player","scope":format!("{id}#ability"),"role":"static-source-controller"}),
        ]);
    }
    let refs = [
        (format!("{cost}/payer"), json!({"player":"you"}), you.clone()),
        (format!("{cost}/what"), json!({"ref":"this","noun":"creature"}), source),
        (format!("{effect}/what/filter/control"), json!({"player":"you"}), you),
        (format!("{effect}/what"), what.clone(), query),
    ].map(|(path,expression,target)| json!({"path":path,"expression":expression,"binding":"bound","target":target}));
    fn exact(value: &Value, expected: &[Value], key: &str) -> bool {
        let Some(rows) = value.as_array() else {
            return false;
        };
        rows.len() == expected.len()
            && expected.iter().all(|want| {
                let matching: Vec<_> = rows.iter().filter(|r| r[key] == want[key]).collect();
                matching.len() == 1 && {
                    let mut got = matching[0].clone();
                    got.as_object_mut().is_some_and(|o| {
                        o.remove("rules");
                        true
                    }) && got == *want
                }
            })
    }
    if binding["identity"] != identity
        || binding["fullyBound"] != true
        || binding["referencesBound"] != true
        || binding["diagnostics"] != json!([])
        || binding["engineFeatures"] != json!([])
        || binding["counts"] != json!({"obligations":4,"bound":4,"unresolved":0})
        || !exact(&binding["nodes"], &nodes, "id")
        || !exact(&binding["references"], &refs, "path")
    {
        return Err(bad());
    }
    Ok(())
}
