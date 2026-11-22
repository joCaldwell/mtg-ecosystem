//! Printed repeated choices retain a distinct target for each occurrence. CR 700.2d.
use super::*;
use serde_json::{Value, json};

pub const HEADER: &str = "Choose five. You may choose the same mode more than once.";

/// Archive parser line labels replace the subject of a self-damage clause
/// with its canonical self-reference; all other printed text stays literal.
pub fn parser_block(name: &str, text: &str) -> String {
    let prefix = format!("• {name} deals ");
    text.lines()
        .map(|line| match line.strip_prefix(&prefix) {
            Some(amount) => format!("• ~ deals {amount}"),
            None => line.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn program(name: &str, text: &str, ast: &Value, timing: SpellTiming) -> Option<SpellProgram> {
    let mut lines = text.lines();
    if lines.next() != Some(HEADER) {
        return None;
    }
    let mut options = vec![];
    let mut modes = vec![];
    let mut selection_ids = BTreeSet::new();
    let fixed = |v: u32| json!({"amount":"fixed","value":v});
    let target = |id: &str, filter: Value| json!({"ref":"target","count":fixed(1),"filter":filter,"selectionId":id});
    for (i, line) in lines.enumerate() {
        let clause = line.strip_prefix("• ")?;
        let step = &ast[0]["effects"][0]["steps"][0]["options"][i]["effects"][0]["steps"][0];
        let key = match step["effect"].as_str()? {
            "phase-out" | "destroy" => "/what",
            "draw" => "/who",
            "exile" => "/what/owner",
            "damage" => "/to",
            _ => return None,
        };
        let id = step.pointer(key)?["selectionId"].as_str()?;
        if id.is_empty() || !selection_ids.insert(id.to_owned()) {
            return None;
        }
        let player = json!({"player":"target-player","selectionId":id});
        let mut filter = None;
        let (domain, op, expected) = match clause {
            "Target permanent phases out." => {
                filter = Some(PermanentFilter {
                    types: vec![],
                    non_types: vec![],
                    non_colors: vec![],
                    opponent: false,
                    power: None,
                    tapped: false,
                });
                (
                    TargetDomain::BattlefieldPermanent,
                    Instruction::PhaseOutTargetPermanent {
                        target_slot: 0,
                        source_block: 0,
                    },
                    json!({"effect":"phase-out","what":target(id,json!({"cls":"permanent"}))}),
                )
            }
            "Target player draws a card." => (
                TargetDomain::AnyPlayer,
                Instruction::draw(PlayerExpr::target(0), 1.into(), 0),
                json!({"effect":"draw","who":player,"amount":fixed(1)}),
            ),
            "Exile target player's graveyard." => (
                TargetDomain::AnyPlayer,
                Instruction::exile_target_graveyard(0, 0),
                json!({"effect":"exile","what":{"ref":"zone-cards","zone":"graveyard","owner":player}}),
            ),
            "Destroy target artifact or enchantment." => {
                filter = Some(PermanentFilter {
                    types: vec![CardKind::Artifact, CardKind::Enchantment],
                    non_types: vec![],
                    non_colors: vec![],
                    opponent: false,
                    power: None,
                    tapped: false,
                });
                (
                    TargetDomain::BattlefieldPermanent,
                    Instruction::destroy_target(0, false, 0),
                    json!({"effect":"destroy","what":target(id,json!({"allOf":[{"anyOf":[{"types":["artifact"]},{"types":["enchantment"]}]}]}))}),
                )
            }
            _ => {
                let n = clause
                    .strip_prefix(name)?
                    .strip_prefix(" deals ")?
                    .strip_suffix(" damage to any target.")?;
                let value = n
                    .parse::<u32>()
                    .ok()
                    .filter(|v| *v > 0 && v.to_string() == n)?;
                (
                    TargetDomain::AnyDamageTarget,
                    Instruction::damage_target(
                        0,
                        kir::Amount::Fixed {
                            value: Quantity::from(i64::from(value)),
                        },
                        0,
                    ),
                    json!({"effect":"damage","source":{"ref":"self"},"amount":fixed(value),"to":{"to":"any-target","selectionId":id}}),
                )
            }
        };
        if step != &expected {
            return None;
        }
        options.push(json!({"effects":[{"steps":[step]}]}));
        modes.push(SpellMode {
            program: SpellProgram {
                timing,
                functioning_zone: FunctioningZone::Stack,
                resolution: SpellResolution::Instructions,
                targets: vec![TargetSpec {
                    slot: 0,
                    selection_id: id.to_owned(),
                    stage: EvaluationStage::Announcement,
                    chooser: Chooser::Controller,
                    domain,
                    filter,
                }],
                instructions: vec![op],
                modes: vec![],
                additional_sacrifice: None,
                optional_additional_costs: vec![],
            },
        });
    }
    let exact = json!([{"kind":"spell","effects":[{"steps":[{"effect":"modal","count":{"exactly":5},"allowRepeat":true,"options":options}]}]}]);
    (modes.len() >= 2 && ast == &exact).then_some(SpellProgram {
        timing,
        functioning_zone: FunctioningZone::Stack,
        resolution: SpellResolution::ChooseRepeatedModes { exactly: 5 },
        targets: vec![],
        instructions: vec![],
        modes,
        additional_sacrifice: None,
        optional_additional_costs: vec![],
    })
}

pub(super) fn schema(s: &mut Value) {
    let cap = Capability::RepeatedTargetModesV1;
    let variants = [
        (
            TargetDomain::BattlefieldPermanent,
            "phase_out_target_permanent",
            Some(Capability::TargetedPhasingV1),
        ),
        (TargetDomain::AnyPlayer, "draw", None),
        (
            TargetDomain::AnyPlayer,
            "exile_graveyard",
            Some(Capability::EntryGraveyardExileV1),
        ),
        (
            TargetDomain::AnyDamageTarget,
            "damage",
            Some(Capability::NoncombatDamageV1),
        ),
        (TargetDomain::BattlefieldPermanent, "destroy", None),
    ];
    let mut children = vec![];
    for (domain, op, required) in variants {
        let operand =
            json!({"op":{"const":op},"source_block":{"const":0},"target_slot":{"const":0}});
        let mut filter = json!({"type":"null"});
        match op {
            "phase_out_target_permanent" | "destroy" => {
                let types = if op == "destroy" {
                    vec![CardKind::Artifact, CardKind::Enchantment]
                } else {
                    vec![]
                };
                filter = json!({"const":PermanentFilter{types,non_types:vec![],non_colors:vec![],opponent:false,power:None,tapped:false}});
            }
            "draw" => {}
            _ => {}
        }
        let item = if op == "draw" {
            instruction_schema::at(instruction_schema::target_draw(0, json!({"const":"1"})), 0)
        } else if op == "damage" {
            instruction_schema::at(instruction_schema::target_damage(Some(0)), 0)
        } else if op == "exile_graveyard" {
            instruction_schema::at(instruction_schema::exile_target_graveyard(Some(0)), 0)
        } else if op == "destroy" {
            instruction_schema::at(instruction_schema::target_destroy(Some(0), Some(false)), 0)
        } else {
            json!({"properties":operand})
        };
        let child = json!({"required":["program"],"properties":{"program":{"properties":{"resolution":{"const":"instructions"},"targets":{"minItems":1,"maxItems":1,"items":{"properties":{"domain":{"const":domain},"slot":{"const":0},"filter":filter}}},"instructions":{"minItems":1,"maxItems":1,"items":item},"modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0}}}}});
        if let Some(required) = required {
            s["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({"if":{"required":["spell"],"properties":{"spell":{"type":"object","properties":{"resolution":{"const":SpellResolution::ChooseRepeatedModes{exactly:5}},"modes":{"contains":child}}}}},"then":{"properties":{"capabilities":{"contains":{"const":required}}}}}));
        }
        children.push(child);
    }
    let resolution = json!({"const":SpellResolution::ChooseRepeatedModes{exactly:5}});
    s["$defs"]["RepeatedTargetRoot"] = json!({"type":"object","required":["resolution","modes"],"properties":{"functioning_zone":{"const":"stack"},"resolution":resolution,"modes":{"minItems":2,"items":{"oneOf":children}},"targets":{"maxItems":0},"instructions":{"maxItems":0},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0}}});
    s["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({"if":{"anyOf":[{"properties":{"capabilities":{"contains":{"const":cap}}}},{"required":["spell"],"properties":{"spell":{"type":"object","properties":{"resolution":{"type":"object","required":["choose_repeated_modes"]}}}}}]},"then":{"required":["spell"],"properties":{"spell":{"$ref":"#/$defs/RepeatedTargetRoot"},"capabilities":{"allOf":[{"contains":{"const":cap}},{"contains":{"const":Capability::ModalChoiceV1}}]},"activated":{"maxItems":0},"triggered":{"maxItems":0},"static_abilities":{"maxItems":0},"mana_abilities":{"maxItems":0},"keywords":{"maxItems":0}}}}));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    use serde_json::value::RawValue;
    use std::collections::BTreeMap;

    // Use the original compact source and AST subvalues, preserving the exact
    // frozen provenance that the build's compiler/runtime witnesses use.
    type RawFields = BTreeMap<String, Box<RawValue>>;
    let raw: RawFields = strict_read(include_str!(
        "../../../tests/fixtures/ramos/164651c5-b831-4070-8491-16d30ac8395b.json"
    ))
    .unwrap();
    let source_json = raw["source"].get();
    let source: Value = strict_read(source_json).unwrap();
    let faces: Vec<RawFields> = strict_read(raw["faces"].get()).unwrap();
    let result: RawFields = strict_read(faces[0]["result"].get()).unwrap();
    let abilities_json = result["abilities"].get();
    let ast = strict_read(abilities_json).unwrap();
    let archive = accepted_manifest(include_str!(
        "../../../tests/fixtures/archive-manifest-current.json"
    ))
    .unwrap();

    let mut d = power_filtered_removal::witness();
    d.source_labels = None;
    d.name = source["name"].as_str().unwrap().into();
    d.oracle_id = source["oracle_id"].as_str().unwrap().into();
    d.characteristics.mana_cost = ManaCost {
        generic: 2.into(),
        colored: vec![Color::W, Color::U, Color::B, Color::R, Color::G],
    };
    d.characteristics.colors = strict_read(&source["colors"].to_string()).unwrap();
    d.characteristics.color_identity = strict_read(&source["color_identity"].to_string()).unwrap();
    d.spell = Some(
        program(
            &d.name,
            source["oracle_text"].as_str().unwrap(),
            &ast,
            SpellTiming::Instant,
        )
        .unwrap(),
    );
    d.provenance = SourceProvenance {
        printing_id: source["id"].as_str().unwrap().into(),
        source_index: strict_read(raw["sourceIndex"].get()).unwrap(),
        source_sha256: strict_read(raw["sourceSha256"].get()).unwrap(),
        ast_sha256: strict_read(raw["astSha256"].get()).unwrap(),
        binding_sha256: strict_read(raw["bindingSha256"].get()).unwrap(),
        archive_manifest_sha256: archive.manifest_sha256.clone(),
        parser_sha256: archive.parser_source_sha256.clone(),
        source_json: source_json.into(),
        abilities_json: abilities_json.into(),
    };
    d.capabilities = d.requirements();
    d
}
