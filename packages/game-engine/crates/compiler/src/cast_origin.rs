//! CR 109.5, 601.2a/i and 603.3a bind the cast and draw to the trigger
//! controller and test the zone the spell left during announcement.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const TRIGGER_TEXT: &str =
    "Whenever you cast a spell from anywhere other than your hand, draw a card.";

fn hand() -> Value {
    json!({"zone":"hand","owner":"your"})
}

fn unsupported_shape() -> Failure {
    unsupported(
        "trigger.cast.origin",
        "requires the complete controller cast-origin draw source and reference bindings",
    )
}

pub(super) fn lower_condition(
    caster: CasterRelation,
    spell: TypePredicate,
    zone: &Value,
) -> Result<TriggerCondition, Failure> {
    if caster != CasterRelation::Controller || spell != (TypePredicate::Any {}) || *zone != hand() {
        return Err(unsupported_shape());
    }
    Ok(TriggerCondition::SpellCastWithOrigin {
        caster,
        spell,
        origin: CastOriginPredicate::Outside {
            zone: CastOriginZone::ControllerHand,
        },
    })
}

pub(super) fn bind(
    abilities: &[Ability],
    binding: &Value,
    card: &Source,
    parsed: &ParseResult,
) -> Result<(), Failure> {
    let has_origin = abilities.iter().any(|ability| {
        matches!(ability, Ability::Triggered { trigger, .. }
            if matches!(trigger.as_ref(), SourceTrigger::Cast { not_from: Some(_), .. }))
    });
    // A forged AST that drops the origin must not take the ordinary cast path.
    let prints_origin = parsed.lines.iter().any(|line| {
        let text = normalization::literal_spelling(&line.text);
        text.starts_with("Whenever you cast ") && text.contains(" from anywhere other than ")
    });
    if !has_origin && !prints_origin {
        return Ok(());
    }

    // Match the complete raw clauses before the ordinary typed lowering can
    // collapse explicit nulls or defaults. Every printed block is accounted for.
    let you_player = json!({"player":"you"});
    let trigger_ability = json!({
        "kind":"triggered","trigger":{"trigger":"cast","who":you_player,"what":{"cls":"spell"},"notFrom":hand()},
        "effects":[{"steps":[{"effect":"draw","who":you_player,"amount":{"amount":"fixed","value":1}}]}]
    });
    let flying = json!({"kind":"keywords","keywords":[{"keyword":"flying"}]});
    let raw: Vec<Value> = decode(parsed.abilities.get(), "abilities")?;
    let identity = &card.oracle_id;
    let root = format!("card:{identity}");
    let source = format!("{root}/source#object");
    let face = format!("{root}/faces/0#face");
    let mut nodes = vec![
        json!({"id":source,"path":"/source","node":"source","identity":identity,"rules":["201.5"]}),
        json!({"id":face,"path":"/faces/0","node":"face","source":source,"sourceFaceIndex":0,
            "name":card.name,"layout":card.layout,"typeLine":card.type_line,"manaCost":card.mana_cost,"rules":["712"]}),
    ];
    let mut references = vec![];
    for (block, (raw, line)) in raw.iter().zip(&parsed.lines).enumerate() {
        let (kind, role) = match normalization::literal_spelling(&line.text).as_str() {
            TRIGGER_TEXT if *raw == trigger_ability => ("triggered", "trigger-controller"),
            "Flying" if *raw == flying => ("keywords", "static-source-controller"),
            _ => return Err(unsupported_shape()),
        };
        let path = format!("/faces/0/abilities/{block}");
        let scope = format!("{root}{path}#ability");
        let you = format!("{root}{path}#you");
        nodes.extend([
            json!({"id":scope,"path":path,"node":"ability","source":source,"face":face,
                "abilityKind":kind,"you":you,"rules":["113","109.5"]}),
            json!({"id":you,"path":path,"node":"runtime-player","scope":scope,"role":role,"rules":["109.5"]}),
        ]);
        if kind == "keywords" {
            let keyword = format!("{path}/keywords/0");
            let actor = format!("{root}{keyword}#you");
            nodes.extend([
                json!({"id":actor,"path":keyword,"node":"runtime-player","scope":scope,"role":role,"rules":["109.5","702.9"]}),
                json!({"id":format!("{root}{keyword}#keyword"),"path":keyword,"node":"keyword-ability",
                    "scope":scope,"keyword":"flying","abilities":["static"],"you":actor,"rules":["702.9"]}),
            ]);
            continue;
        }
        let trigger = format!("{path}/trigger");
        let effect = format!("{path}/effects/0/steps/0");
        for (path, operation, class, object_identity) in [
            (&trigger, "trigger:cast", "spell", "same-objects"),
            (&effect, "draw", "card", "new-objects"),
        ] {
            let event = format!("{root}{path}#event");
            nodes.extend([
                json!({"id":event,"path":path,"node":"event","scope":scope,"operation":operation,
                    "inputs":[you],"availability":"when-performed","rules":["608.2c"]}),
                json!({"id":format!("{root}{path}#objects"),"path":path,"node":"event-result","event":event,
                    "result":"objects","cardinality":"one","availability":"if-produced","identity":object_identity,
                    "description":{"cls":class},"rules":["608.2h"]}),
            ]);
        }
        nodes.push(
            json!({"id":format!("{root}{effect}#quantity"),"path":effect,"node":"event-result",
            "event":format!("{root}{effect}#event"),"result":"quantity","cardinality":"one",
            "availability":"if-produced","identity":"measured-quantity","rules":["608.2h"]}),
        );
        for path in [
            format!("{trigger}/who"),
            format!("{trigger}/notFrom/owner"),
            format!("{effect}/who"),
        ] {
            references.push(json!({"path":path,"expression":you_player,"binding":"bound","target":you,"rules":["109.5"]}));
        }
    }
    let count = references.len();
    if *binding
        != json!({
            "artifactKind":"oracle-reference-binding","schemaVersion":1,"executable":false,"identity":identity,
            "rules":{"edition":RULES_EDITION,"url":"https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt"},
            "nodes":nodes,"references":references,"diagnostics":[],
            "counts":{"obligations":count,"bound":count,"unresolved":0},"referencesBound":true,
            "engineFeatures":[],"fullyBound":true
        })
    {
        return Err(unsupported_shape());
    }
    Ok(())
}
