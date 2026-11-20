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
