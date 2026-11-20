//! Printed repeated choices retain a distinct target for each occurrence. CR 700.2d.
use super::*;
use serde_json::{Value, json};

pub const HEADER: &str = "Choose five. You may choose the same mode more than once.";

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
                Instruction::DrawCards {
                    recipient: Some(LifeRecipient::Target { target_slot: 0 }),
                    count: 1.into(),
                    source_block: 0,
                },
                json!({"effect":"draw","who":player,"amount":fixed(1)}),
            ),
            "Exile target player's graveyard." => (
                TargetDomain::AnyPlayer,
                Instruction::ExileTargetGraveyard {
                    target_slot: 0,
                    source_block: 0,
                },
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
                    Instruction::DestroyPermanent {
                        target_slot: 0,
                        source_block: 0,
                        optional: false,
                        regeneration_forbidden: false,
                    },
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
                    Instruction::DealDamage {
                        target_slot: 0,
                        amount: Quantity::from(i64::from(value)),
                        source_block: 0,
                    },
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
