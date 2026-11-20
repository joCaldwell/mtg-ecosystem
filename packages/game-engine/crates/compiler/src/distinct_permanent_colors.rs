//! CR 105.1–2, 109.5, 110.1, 605.3b, 608.2h, 611.3 and 613.4c.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const MANA_LINE: &str =
    "{T}: For each color among permanents you control, add one mana of that color.";

pub(super) fn lower(
    abilities: &[Ability],
    parsed: &ParseResult,
    binding: &Value,
    card: &Source,
) -> Result<Option<(StaticAbility, ManaAbility)>, Failure> {
    let has_color_mana = abilities.iter().any(|ability| {
        let Ability::Activated { effects, .. } = ability else {
            return false;
        };
        effects
            .iter()
            .flat_map(|sentence| &sentence.steps)
            .any(|step| {
                matches!(
                    step,
                    Step::AddMana {
                        spend_restriction: None,
                        mana: SourceMana::EachColorAmong { .. },
                        ..
                    }
                )
            })
    });
    if !has_color_mana
        && !parsed
            .lines
            .iter()
            .any(|line| normalization::literal_spelling(&line.text) == MANA_LINE)
    {
        return Ok(None);
    }

    let bad = || {
        unsupported(
            "mana.family",
            "unsupported complete distinct permanent colors source",
        )
    };
    let [
        Ability::Keywords { .. },
        Ability::Static {
            effect: SourceStaticEffect::Modify { modifiers, .. },
            ..
        },
        Ability::Activated { costs, .. },
    ] = abilities
    else {
        return Err(bad());
    };
    let [modifier] = modifiers.as_slice() else {
        return Err(bad());
    };
    let stat = |field: &str| {
        let value: SignedAmount =
            serde_json::from_value(modifier[field].clone()).map_err(|_| bad())?;
        if value.sign != 1 {
            return Err(bad());
        }
        fixed_positive(&value.amount)
    };
    let power_per_color = stat("power")?;
    let toughness_per_color = stat("toughness")?;
    let among = json!({"ref":"each","filter":{"cls":"permanent","control":"you"}});
    let what = json!({"ref":"this","noun":"creature"});
    let raw: Value = decode(parsed.abilities.get(), "abilities")?;
    if raw != json!([
        {"kind":"keywords","keywords":[{"keyword":"vigilance"}]},
        {"kind":"static","effect":{"static":"modify","what":what,"modifiers":[{
            "modifier":"pt","power":modifier["power"],"toughness":modifier["toughness"],
            "multiplier":{"amount":"distinct-count","characteristic":"color","among":among},
            "valueScope":"each-affected-object"
        }]}},
        {"kind":"activated","costs":[{"cost":"tap-self"}],"effects":[{"steps":[{
            "effect":"add-mana","who":{"player":"you"},"mana":{"mana":"each-color-among","among":among}
        }]}]}
    ]) || !parsed.lines.iter().map(|line| normalization::literal_spelling(&line.text)).eq([
        "Vigilance".to_owned(),
        format!("This creature gets +{power_per_color}/+{toughness_per_color} for each color among permanents you control."),
        MANA_LINE.to_owned(),
    ]) {
        return Err(bad());
    }
    validate_binding(binding, card, &among, &what)?;
    let scope = PermanentColorScope::Controller;
    Ok(Some((
        StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::ModifySourceByDistinctPermanentColors {
                scope,
                power_per_color,
                toughness_per_color,
            },
        },
        ManaAbility {
            costs: lower_permanent_costs(costs, CardKind::Creature)?,
            production: ManaProduction::EachPermanentColor { scope },
            restriction: None,
            self_damage: None,
            source_block: 2,
        },
    )))
}

fn validate_binding(
    binding: &Value,
    card: &Source,
    among: &Value,
    what: &Value,
) -> Result<(), Failure> {
    let root = format!("card:{}", card.oracle_id);
    let source = format!("{root}/source#object");
    let face = format!("{root}/faces/0#face");
    let keyword = "/faces/0/abilities/0/keywords/0";
    let stats = "/faces/0/abilities/1/effect/modifiers/0";
    let stats_what = "/faces/0/abilities/1/effect/what";
    let stats_among = format!("{stats}/multiplier/among");
    let stats_control = format!("{stats_among}/filter/control");
    let activation = "/faces/0/abilities/2";
    let step = format!("{activation}/effects/0/steps/0");
    let mana_among = format!("{step}/mana/among");
    let mana_control = format!("{mana_among}/filter/control");
    let mana_scope = format!("{root}{activation}#ability");
    let mana_you = format!("{root}{activation}#you");
    let query = format!("{root}{mana_among}#query");
    let you = json!({"player":"you"});

    let mut nodes = vec![
        json!({"id":source,"path":"/source","node":"source","identity":card.oracle_id,"rules":["201.5"]}),
        json!({"id":face,"path":"/faces/0","node":"face","source":source,"sourceFaceIndex":0,
            "name":card.name,"layout":card.layout,"typeLine":card.type_line,"manaCost":card.mana_cost,"rules":["712"]}),
    ];
    for (block, kind, role) in [
        (0, "keywords", "static-source-controller"),
        (1, "static", "static-source-controller"),
        (2, "activated", "activator"),
    ] {
        let path = format!("/faces/0/abilities/{block}");
        let scope = format!("{root}{path}#ability");
        let player = format!("{root}{path}#you");
        nodes.extend([
            json!({"id":scope,"path":path,"node":"ability","source":source,"face":face,
                "abilityKind":kind,"you":player,"rules":["113","109.5"]}),
            json!({"id":player,"path":path,"node":"runtime-player","scope":scope,"role":role,"rules":["109.5"]}),
        ]);
        if block == 0 {
            let keyword_you = format!("{root}{keyword}#you");
            nodes.extend([
                json!({"id":keyword_you,"path":keyword,"node":"runtime-player","scope":scope,
                    "role":role,"rules":["109.5","702.20"]}),
                json!({"id":format!("{root}{keyword}#keyword"),"path":keyword,"node":"keyword-ability",
                    "scope":scope,"keyword":"vigilance","abilities":["static"],"you":keyword_you,"rules":["702.20"]}),
            ]);
        }
    }
    nodes.extend([
        json!({"id":query,"path":mana_among,"node":"selection","scope":mana_scope,"expression":among,
            "cardinality":"many","timing":"resolution","selection":"evaluate-reference","rules":["608.2h"]}),
        json!({"id":format!("{root}{step}#event"),"path":step,"node":"event","scope":mana_scope,
            "operation":"add-mana","inputs":[mana_you],"availability":"when-performed","rules":["608.2c"]}),
    ]);

    // The retained parser could not bind the modifier's affected-object scope.
    // Exact self-reference plus the complete selector above make that scope
    // unambiguous: the affected object is the source and "you" is its controller
    // (CR 109.5, 611.3). Only these two known unresolved references are consumed.
    let diagnostic =
        "A dynamic characteristic value needs a distinct affected-object binding for each match.";
    let expected = json!({
        "artifactKind":"oracle-reference-binding","schemaVersion":1,"executable":false,
        "identity":card.oracle_id,
        "rules":{"edition":RULES_EDITION,"url":"https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt"},
        "nodes":nodes,
        "references":[
            {"path":stats_what,"expression":what,"binding":"bound","target":source,"rules":["201.5"]},
            {"path":stats_control,"expression":you,"binding":"unresolved","code":"unsupported-scope",
                "candidates":[],"rules":["109.5"]},
            {"path":stats_among,"expression":among,"binding":"unresolved","code":"unsupported-scope",
                "candidates":[],"rules":["201.5","607"]},
            {"path":format!("{activation}/costs/0/source"),"expression":{"ref":"self"},"binding":"bound",
                "target":source,"rules":["201.5"]},
            {"path":format!("{activation}/costs/0/payer"),"expression":you,"binding":"bound",
                "target":mana_you,"rules":["109.5"]},
            {"path":format!("{step}/who"),"expression":you,"binding":"bound","target":mana_you,"rules":["109.5"]},
            {"path":mana_control,"expression":you,"binding":"bound","target":mana_you,"rules":["109.5"]},
            {"path":mana_among,"expression":among,"binding":"bound","target":query,"rules":["608.2h"]}
        ],
        "diagnostics":[
            {"code":"unsupported-scope","path":stats,"message":diagnostic,"rules":["611.3","613.6"]},
            {"code":"unsupported-scope","path":stats_control,"message":diagnostic,"rules":["109.5"]},
            {"code":"unsupported-scope","path":stats_among,"message":diagnostic,"rules":["201.5","607"]}
        ],
        "counts":{"obligations":8,"bound":6,"unresolved":2},
        "referencesBound":false,"engineFeatures":[],"fullyBound":false
    });
    if *binding != expected {
        return Err(unsupported(
            "binding.mana",
            "incomplete distinct permanent colors binding closure",
        ));
    }
    Ok(())
}
