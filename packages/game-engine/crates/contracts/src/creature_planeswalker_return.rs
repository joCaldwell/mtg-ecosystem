//! A creature-or-planeswalker union remains explicit even though the current
//! definition type system cannot construct a planeswalker. CR 109.2, 115.1a.
use super::*;
use serde_json::{Value, json};

pub const CLAUSE: &str = "Return target creature or planeswalker to its owner's hand.";

pub fn program(text: &str, step: &Value, timing: SpellTiming, block: u32) -> Option<SpellProgram> {
    if text != CLAUSE {
        return None;
    }
    let id = step["what"]["selectionId"].as_str()?;
    let expected = json!({"effect":"return-to-hand","what":{"ref":"target","filter":{"allOf":[{"anyOf":[{"types":["creature"]},{"types":["planeswalker"]}]}]},"count":{"amount":"fixed","value":1},"selectionId":id},"to":"owner"});
    if id.is_empty() || step != &expected {
        return None;
    }
    Some(SpellProgram {
        timing,
        resolution: SpellResolution::Instructions,
        functioning_zone: FunctioningZone::Stack,
        targets: vec![TargetSpec {
            slot: 0,
            selection_id: id.into(),
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
            domain: TargetDomain::CreatureOrPlaneswalker,
            filter: None,
        }],
        instructions: vec![Instruction::ReturnToOwnerHand {
            target_slot: 0,
            source_block: block,
        }],
        modes: vec![],
        additional_sacrifice: None,
        optional_additional_costs: vec![],
    })
}
fn uses(p: &SpellProgram) -> bool {
    p.targets
        .iter()
        .any(|s| s.domain == TargetDomain::CreatureOrPlaneswalker)
        || p.modes.iter().any(|m| uses(&m.program))
}
pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let fail = || "requires the exact creature-or-planeswalker owner-hand return".to_string();
    if d.activated.iter().any(|a| uses(&a.program))
        || d.triggered.iter().any(|a| uses(&a.program))
        || d.static_abilities.iter().any(
            |a| matches!(&a.effect,StaticEffect::GrantSpellCascade{program,..} if uses(program)),
        )
    {
        return Err(fail());
    }
    let active = d.spell.as_ref().is_some_and(uses);
    if !active && !d.provenance.source_json.contains(CLAUSE) {
        return Ok(());
    }
    let src: Value = strict_read(&d.provenance.source_json)?;
    let text = src["oracle_text"].as_str().unwrap_or("");
    if !active && text != CLAUSE {
        return Ok(());
    }
    let timing = match (d.characteristics.kind, src["type_line"].as_str()) {
        (CardKind::Instant, Some("Instant")) => SpellTiming::Instant,
        (CardKind::Sorcery, Some("Sorcery")) => SpellTiming::MainPhase,
        _ => return Err(fail()),
    };
    let ast: Value = strict_read(&d.provenance.abilities_json)?;
    let step = &ast[0]["effects"][0]["steps"][0];
    let expected = program(text, step, timing, 0).ok_or_else(fail)?;
    let c = &d.characteristics;
    let mut cost = String::new();
    if c.mana_cost.generic != Quantity::zero() || c.mana_cost.colored.is_empty() {
        cost.push_str(&format!("{{{}}}", c.mana_cost.generic));
    }
    for color in &c.mana_cost.colored {
        cost.push_str(&format!("{{{color:?}}}"));
    }
    if src["id"] != d.provenance.printing_id
        || src["name"] != d.name
        || src["oracle_id"] != d.oracle_id
        || src["layout"] != "normal"
        || src["mana_cost"] != cost
        || src["colors"] != json!(c.colors)
        || src["color_identity"] != json!(c.color_identity)
        || c.snow
        || c.basic
        || c.legendary
        || !c.additional_types.is_empty()
        || !c.subtypes.is_empty()
        || d.source_labels.as_ref().is_some_and(|l| {
            l != &SourceLabels {
                text: text.into(),
                blocks: vec![text.into()],
                ..Default::default()
            }
        })
    {
        return Err(fail());
    }
    if ast != json!([{"kind":"spell","effects":[{"steps":[step]}]}])
        || d.spell.as_ref() != Some(&expected)
        || d.x_mana_cost
        || !d.activated.is_empty()
        || !d.static_abilities.is_empty()
        || !d.triggered.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.keywords.is_empty()
    {
        return Err(fail());
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let raw: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/synthetic-creature-planeswalker-return.json"
    ))
    .unwrap();
    let source = &raw["source"];
    let ast = &raw["faces"][0]["result"]["abilities"];
    let mut d = crate::graveyard_shuffle::witness();
    d.name = source["name"].as_str().unwrap().into();
    d.oracle_id = raw["oracleIdentity"].as_str().unwrap().into();
    d.provenance.printing_id = source["id"].as_str().unwrap().into();
    d.provenance.source_json = source.to_string();
    d.provenance.abilities_json = ast.to_string();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.provenance.binding_sha256 = raw["bindingSha256"].as_str().unwrap().into();
    d.spell = Some(
        program(
            CLAUSE,
            &ast[0]["effects"][0]["steps"][0],
            SpellTiming::MainPhase,
            0,
        )
        .unwrap(),
    );
    d.capabilities = d.requirements();
    d
}
pub(super) fn schema(schema: &mut Value) {
    let cap = json!({"contains":{"const":Capability::CreatureOrPlaneswalkerTargetV1}});
    let target = json!({"required":["domain"],"properties":{"domain":{"const":TargetDomain::CreatureOrPlaneswalker},"slot":{"const":0},"filter":{"type":"null"}}});
    let free = json!({"$ref":"#/$defs/CreaturePlaneswalkerReturnFree"});
    schema["$defs"]["CreaturePlaneswalkerReturnFree"] = json!({"properties":{"targets":{"not":{"contains":target}},"modes":{"items":{"properties":{"program":free}}}}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({"properties":{"activated":{"items":{"properties":{"program":free}}},"triggered":{"items":{"properties":{"program":free}}},"static_abilities":{"items":{"properties":{"effect":{"properties":{"grant_spell_cascade":{"properties":{"program":free}}}}}}}},"if":{"anyOf":[{"required":["capabilities"],"properties":{"capabilities":cap}},{"required":["spell"],"properties":{"spell":{"type":"object","not":free}}}]},"then":{"required":["spell","capabilities"],"properties":{"capabilities":{"allOf":[cap,{"contains":{"const":Capability::OwnerHandMoveV1}}]},"spell":{"type":"object","properties":{"targets":{"minItems":1,"maxItems":1,"items":target},"instructions":{"const":[Instruction::ReturnToOwnerHand{target_slot:0,source_block:0}]},"modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0}}}}}}));
}
