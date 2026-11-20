//! Announcement operands for a bounded group of cards in one targeted player's
//! graveyard. CR 115.3, 601.2c, 608.2b and 701.20.
use super::*;
use serde_json::{Value, json};

/// Scope and cardinality are bound independently of the selected runtime values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraveyardShuffle {
    pub maximum: u8,
    pub player_selection: String,
    pub cards_selection: String,
}

impl GraveyardShuffle {
    pub fn from_clause(text: &str, step: &Value) -> Option<Self> {
        let (word, noun) = text
            .strip_prefix("Target player shuffles up to ")?
            .split_once(' ')?;
        let maximum = [
            "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
        ]
        .iter()
        .position(|n| *n == word)? as u8
            + 1;
        if noun
            != if maximum == 1 {
                "target card from their graveyard into their library."
            } else {
                "target cards from their graveyard into their library."
            }
        {
            return None;
        }
        let player_selection = step["who"]["selectionId"].as_str()?;
        let cards_selection = step["what"][0]["selectionId"].as_str()?;
        if player_selection.is_empty()
            || cards_selection.is_empty()
            || player_selection == cards_selection
        {
            return None;
        }
        let expected = json!({"effect":"shuffle-into-library","who":{"player":"target-player","selectionId":player_selection},"what":[{"ref":"target","filter":{"cls":"card","zone":{"zone":"graveyard","owner":"their"}},"count":{"amount":"fixed","value": maximum},"upTo":true,"selectionId":cards_selection}],"library":{"zone":"library","owner":"their"}});
        (step == &expected).then(|| Self {
            maximum,
            player_selection: player_selection.to_owned(),
            cards_selection: cards_selection.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn variable_graveyard_occurrence_binds_count_and_both_owners() {
        let source = "Target player shuffles up to two target cards from their graveyard into their library.";
        let step = json!({"effect":"shuffle-into-library","who":{"player":"target-player","selectionId":"chooser/seat"},"what":[{"ref":"target","filter":{"cls":"card","zone":{"zone":"graveyard","owner":"their"}},"count":{"amount":"fixed","value":2},"upTo":true,"selectionId":"chooser/cards"}],"library":{"zone":"library","owner":"their"}});
        assert_eq!(
            GraveyardShuffle::from_clause(source, &step)
                .unwrap()
                .maximum,
            2
        );
        for (pointer, value) in [
            ("/who/player", json!("you")),
            ("/what/0/upTo", json!(false)),
            ("/what/0/filter/zone/owner", json!("you")),
            ("/library/owner", json!("you")),
            ("/what/0/count/value", json!(3)),
            ("/what/0/selectionId", json!("chooser/seat")),
        ] {
            let mut altered = step.clone();
            *altered.pointer_mut(pointer).unwrap() = value;
            assert!(GraveyardShuffle::from_clause(source, &altered).is_none());
        }
    }
}

impl GraveyardShuffle {
    pub fn program(&self, timing: SpellTiming) -> SpellProgram {
        let target = |slot, selection_id, domain| TargetSpec {
            slot,
            selection_id,
            domain,
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
            filter: None,
        };
        SpellProgram {
            timing,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            targets: vec![
                target(0, self.player_selection.clone(), TargetDomain::AnyPlayer),
                target(
                    1,
                    self.cards_selection.clone(),
                    TargetDomain::PlayerGraveyardCards {
                        owner_slot: 0,
                        maximum: self.maximum,
                    },
                ),
            ],
            instructions: vec![Instruction::ShufflePlayerGraveyard { source_block: 0 }],
            modes: vec![],
            additional_sacrifice: None,
            optional_additional_costs: vec![],
        }
    }
}
pub fn offered_by(program: &SpellProgram) -> Option<u8> {
    let [who, cards] = program.targets.as_slice() else {
        return None;
    };
    let TargetDomain::PlayerGraveyardCards {
        owner_slot: 0,
        maximum,
    } = cards.domain
    else {
        return None;
    };
    (who.domain == TargetDomain::AnyPlayer
        && maximum > 0
        && matches!(
            program.instructions.as_slice(),
            [Instruction::ShufflePlayerGraveyard { source_block: 0 }]
        ))
    .then_some(maximum)
}

fn reserves(program: &SpellProgram) -> bool {
    program
        .targets
        .iter()
        .any(|t| matches!(t.domain, TargetDomain::PlayerGraveyardCards { .. }))
        || program
            .instructions
            .iter()
            .any(|i| matches!(i, Instruction::ShufflePlayerGraveyard { .. }))
        || program.modes.iter().any(|m| reserves(&m.program))
}
pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    let fail = || "requires the complete targeted graveyard shuffle".to_string();
    if d.activated.iter().any(|a|reserves(&a.program))||d.triggered.iter().any(|a|reserves(&a.program))||d.static_abilities.iter().any(|a|matches!(&a.effect,StaticEffect::GrantSpellCascade{program,..} if reserves(program))){return Err(fail());}
    if d.spell
        .as_ref()
        .is_some_and(|p| matches!(p.resolution, SpellResolution::ChooseModes { .. }))
    {
        return crate::modal_targets::validate_definition(d);
    }
    let src: Value = if d.spell.as_ref().is_some_and(reserves)
        || d.provenance.source_json.contains("shuffles up to")
        || d.provenance.abilities_json.contains("shuffle-into-library")
    {
        strict_read(&d.provenance.source_json)?
    } else {
        return Ok(());
    };
    let text = src["oracle_text"].as_str().unwrap_or("");
    let ast: Value = strict_read(&d.provenance.abilities_json)?;
    let step = &ast[0]["effects"][0]["steps"][0];
    if !d.spell.as_ref().is_some_and(reserves)
        && !text.starts_with("Target player shuffles up to ")
        && !(step["effect"] == "shuffle-into-library"
            && step["what"][0]["ref"] == "target"
            && step["what"][0]["filter"]["zone"]["zone"] == "graveyard")
    {
        return Ok(());
    }
    let shape = GraveyardShuffle::from_clause(text, step).ok_or_else(fail)?;
    let timing = match (d.characteristics.kind, src["type_line"].as_str()) {
        (CardKind::Instant, Some("Instant")) => SpellTiming::Instant,
        (CardKind::Sorcery, Some("Sorcery")) => SpellTiming::MainPhase,
        _ => return Err(fail()),
    };
    let c = &d.characteristics;
    let mut cost = String::new();
    if c.mana_cost.generic != Quantity::zero() || c.mana_cost.colored.is_empty() {
        cost.push_str(&format!("{{{}}}", c.mana_cost.generic));
    }
    for color in &c.mana_cost.colored {
        cost.push_str(&format!("{{{color:?}}}"));
    }
    if src["mana_cost"] != cost
        || src["id"] != d.provenance.printing_id
        || src["oracle_id"] != d.oracle_id
        || src["name"] != d.name
        || src["layout"] != "normal"
        || src["colors"] != json!(c.colors)
        || src["color_identity"] != json!(c.color_identity)
        || c.snow
        || c.legendary
        || !c.additional_types.is_empty()
        || !c.subtypes.is_empty()
        || ast != json!([{"kind":"spell","effects":[{"steps":[step]}]}])
        || d.spell.as_ref() != Some(&shape.program(timing))
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.static_abilities.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.keywords.is_empty()
        || d.x_mana_cost
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
    Ok(())
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let row: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/synthetic-target-graveyard-shuffle.json"
    ))
    .unwrap();
    let source = &row["source"];
    let ast = &row["faces"][0]["result"]["abilities"];
    let text = source["oracle_text"].as_str().unwrap();
    let shape = GraveyardShuffle::from_clause(text, &ast[0]["effects"][0]["steps"][0]).unwrap();
    let mut d = crate::targeted_counted_draw::witness();
    d.name = source["name"].as_str().unwrap().into();
    d.oracle_id = row["oracleIdentity"].as_str().unwrap().into();
    d.characteristics.kind = CardKind::Sorcery;
    d.characteristics.mana_cost = ManaCost {
        generic: 0.into(),
        colored: vec![Color::G],
    };
    d.characteristics.colors = vec![Color::G];
    d.characteristics.color_identity = vec![Color::G];
    d.provenance.printing_id = source["id"].as_str().unwrap().into();
    d.provenance.source_json = source.to_string();
    d.provenance.abilities_json = ast.to_string();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.provenance.binding_sha256 = row["bindingSha256"].as_str().unwrap().into();
    d.source_labels = None;
    d.spell = Some(shape.program(SpellTiming::MainPhase));
    d.capabilities = d.requirements();
    d
}

pub(super) fn schema(schema: &mut Value) {
    let op = json!({"required":["op"],"properties":{"op":{"const":"shuffle_player_graveyard"}}});
    let dependent =
        json!({"properties":{"domain":{"type":"object","required":["player_graveyard_cards"]}}});
    let cap = json!({"contains":{"const":Capability::PlayerGraveyardShuffleV1}});
    let free = json!({"$ref":"#/$defs/PlayerGraveyardShuffleFree"});
    schema["$defs"]["PlayerGraveyardShuffleFree"] = json!({"properties":{"targets":{"not":{"contains":dependent}},"instructions":{"not":{"contains":op}},"modes":{"items":{"properties":{"program":free}}}}});
    let target = |slot, domain: Value| json!({"required":["slot","domain","stage","chooser","selection_id"],"properties":{"slot":{"const":slot},"domain":domain,"filter":{"type":"null"},"stage":{"const":"announcement"},"chooser":{"const":"controller"},"selection_id":{"minLength":1}}});
    let cards = json!({"type":"object","required":["player_graveyard_cards"],"properties":{"player_graveyard_cards":{"properties":{"owner_slot":{"const":0},"maximum":{"minimum":1,"maximum":10}}}}});
    let spell = json!({"type":"object","required":["targets","instructions"],"properties":{"targets":{"minItems":2,"maxItems":2,"prefixItems":[target(0,json!({"const":"any_player"})),target(1,cards)]},"instructions":{"const":[Instruction::ShufflePlayerGraveyard{source_block:0}]},"resolution":{"const":"instructions"},"functioning_zone":{"const":"stack"},"modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0}}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({"properties":{"activated":{"items":{"properties":{"program":free}}},"triggered":{"items":{"properties":{"program":free}}},"static_abilities":{"items":{"properties":{"effect":{"properties":{"grant_spell_cascade":{"properties":{"program":free}}}}}}}},"if":{"anyOf":[{"required":["capabilities"],"properties":{"capabilities":cap}},{"required":["spell"],"properties":{"spell":{"type":"object","not":free}}},{"properties":{"provenance":{"properties":{"source_json":{"pattern":"Target player shuffles up to "}}}}}]},"then":{"required":["spell","capabilities"],"properties":{"capabilities":{"allOf":[cap,{"contains":{"const":Capability::AnyPlayerTargetV1}}]},"spell":spell,"characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]}}},"activated":{"maxItems":0},"triggered":{"maxItems":0},"static_abilities":{"maxItems":0},"mana_abilities":{"maxItems":0},"keywords":{"maxItems":0}}}}));
}
