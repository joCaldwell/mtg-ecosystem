//! A standalone spell with one announced player target. CR 104.3e, 601.2c, 608.2b.
use super::*;
use serde_json::{Value, json};

/// Parsed complete source operands for the two-block artifact family. Basic
/// colored or decimal symbols are the fixed mana subset supported by the
/// existing permanent-cost validator.
pub struct ArtifactLossSource {
    pub mana: ManaCost,
    pub selection: String,
    pub abilities: Value,
}

fn fixed_symbols(mut symbols_text: &str, allow_colorless: bool) -> Option<(Vec<&str>, ManaCost)> {
    let mut symbols = Vec::new();
    let mut mana = ManaCost {
        generic: Quantity::zero(),
        colored: vec![],
    };
    while !symbols_text.is_empty() {
        let (symbol, rest) = symbols_text.strip_prefix('{')?.split_once('}')?;
        match symbol {
            "C" if allow_colorless => mana.colored.push(Color::C),
            "W" => mana.colored.push(Color::W),
            "U" => mana.colored.push(Color::U),
            "B" => mana.colored.push(Color::B),
            "R" => mana.colored.push(Color::R),
            "G" => mana.colored.push(Color::G),
            digits if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) => {
                mana.generic.add(&digits.parse().ok()?);
            }
            _ => return None,
        }
        symbols.push(symbol);
        symbols_text = rest;
    }
    Some((symbols, mana))
}

impl ArtifactLossSource {
    pub fn parse(text: &str) -> Option<Self> {
        let (entry, activation) = text.split_once('\n')?;
        if entry != "This artifact enters tapped." {
            return None;
        }
        let (cost, effect) = activation.split_once(": ")?;
        if effect.strip_prefix("Target player ")? != "loses the game." {
            return None;
        }
        let symbols_text = cost.strip_suffix(", {T}, Sacrifice this artifact")?;
        let (symbols, mana) = fixed_symbols(symbols_text, false)?;
        if symbols.is_empty() {
            return None;
        }
        // Each mana symbol is a token. Before the target are two commas, the
        // tap symbol, "Sacrifice this artifact" and the colon.
        let selection = format!("line:1:{}", symbols.len().checked_add(7)?);
        let artifact = json!({"ref":"this","noun":"artifact"});
        let abilities = json!([
            {"kind":"static","effect":{"static":"enters-tapped","what":artifact}},
            {"kind":"activated","costs":[{"cost":"mana","symbols":symbols},{"cost":"tap-self"},
                {"cost":"sacrifice","what":artifact}],"effects":[{"steps":[{"effect":"lose-game",
                "who":{"player":"target-player","selectionId":selection}}]}]}
        ]);
        Some(Self {
            mana,
            selection,
            abilities,
        })
    }
}

fn program(p: &SpellProgram, timing: SpellTiming, block: u32) -> bool {
    p.timing == timing
        && p.functioning_zone == FunctioningZone::Stack
        && p.resolution == SpellResolution::Instructions
        && p.modes.is_empty()
        && p.additional_sacrifice.is_none()
        && p.optional_additional_costs.is_empty()
        && matches!(p.targets.as_slice(), [t] if t.slot == 0 && t.domain == TargetDomain::AnyPlayer
            && t.filter.is_none() && t.chooser == Chooser::Controller && t.stage == EvaluationStage::Announcement && !t.selection_id.is_empty())
        && matches!(p.instructions.as_slice(), [Instruction::LoseGameTargetPlayer {target_slot:0,source_block}] if *source_block == block)
}

pub fn artifact_loss_activation(a: &ActivatedAbility) -> bool {
    a.functioning_zone == FunctioningZone::Battlefield
        && matches!(&a.cost, ActivationCost::Composite {costs} if costs.tap && costs.sacrifice
            && costs.mana.is_some() && costs.remove_source_plus_one_counters.is_none() && validate_permanent_costs(costs).is_ok())
        && program(&a.program, SpellTiming::Instant, 1)
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    let has_loss = d.capabilities.contains(&Capability::TargetedGameLossV1);
    let raw = &d.provenance.source_json;
    if !has_loss && !raw.contains("loses the game") && !raw.contains(r"\u") {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let text = source["oracle_text"].as_str().unwrap_or_default();
    let bound = ArtifactLossSource::parse(text);
    if !has_loss && bound.is_none() {
        return Ok(());
    }
    let c = &d.characteristics;
    let ordinary = c.subtypes.is_empty()
        && c.additional_types.is_empty()
        && !c.basic
        && !c.snow
        && !c.legendary
        && !d.x_mana_cost
        && d.intrinsic_zone.is_none()
        && d.keywords.is_empty()
        && d.triggered.is_empty()
        && d.mana_abilities.is_empty();
    let ok = ordinary
        && match (bound, c.kind, d.spell.as_ref(), d.activated.as_slice()) {
            (None, CardKind::Sorcery, Some(p), []) => {
                d.static_abilities.is_empty() && program(p, SpellTiming::MainPhase, 0)
            }
            (Some(bound), CardKind::Artifact, Some(p), [a]) => {
                let ast: Value = strict_read(&d.provenance.abilities_json)?;
                ast == bound.abilities
                    && source["oracle_id"] == d.oracle_id
                    && source["id"] == d.provenance.printing_id
                    && source["name"] == d.name
                    && source["type_line"] == "Artifact"
                    && source["layout"] == "normal"
                    && ["card_faces", "loyalty", "defense", "power", "toughness"]
                        .iter()
                        .all(|key| source[*key].is_null())
                    && c.power.is_none()
                    && c.toughness.is_none()
                    && source["colors"] == json!(c.colors)
                    && source["color_identity"] == json!(c.color_identity)
                    && source["mana_cost"]
                        .as_str()
                        .and_then(|s| fixed_symbols(s, true))
                        .is_some_and(|(_, mana)| mana == c.mana_cost)
                    && matches!(
                        d.static_abilities.as_slice(),
                        [StaticAbility {
                            functioning_zone: FunctioningZone::Battlefield,
                            effect: StaticEffect::EntersTapped
                        }]
                    )
                    && p.functioning_zone == FunctioningZone::Stack
                    && p.timing == SpellTiming::MainPhase
                    && p.resolution == SpellResolution::Permanent
                    && p.targets.is_empty()
                    && p.modes.is_empty()
                    && p.instructions.is_empty()
                    && p.additional_sacrifice.is_none()
                    && p.optional_additional_costs.is_empty()
                    && artifact_loss_activation(a)
                    && a.program.targets[0].selection_id == bound.selection
                    && matches!(&a.cost, ActivationCost::Composite {costs} if costs.mana.as_ref() == Some(&bound.mana))
                    && d.source_labels.as_ref().is_none_or(|l| {
                        l.blocks.iter().map(String::as_str).eq(text.lines())
                            && l.statics == [0]
                            && l.activated == [1]
                            && l.triggered.is_empty()
                            && l.mana.is_empty()
                            && l.spell_modes.is_empty()
                    })
            }
            _ => false,
        };
    if ok {
        Ok(())
    } else {
        Err("unsupported complete targeted game loss definition".into())
    }
}

pub(super) fn extend_schema(schema: &mut Value) {
    let instruction =
        json!({"required":["op"],"properties":{"op":{"const":"lose_game_target_player"}}});
    let free = json!({"$ref":"#/$defs/GameLossFreeProgram"});
    schema["$defs"]["GameLossFreeProgram"] = json!({"properties":{
        "instructions":{"items":{"not":instruction}},"modes":{"items":{"properties":{"program":free}}}
    }});
    let spell = json!({"required":["spell"],"properties":{"spell":{"type":"object","not":free}}});
    let activated = json!({"required":["activated"],"properties":{"activated":{"contains":{"required":["program"],"properties":{"program":{"not":free}}}}}});
    let loss = |timing, block| {
        json!({"properties":{
            "timing":{"const":timing},"functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
            "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},"modes":{"maxItems":0},
            "targets":{"minItems":1,"maxItems":1,"items":{"properties":{"domain":{"const":"any_player"},"slot":{"const":0},
                "selection_id":{"minLength":1},"chooser":{"const":"controller"},"stage":{"const":"announcement"},"filter":{"type":"null"}}}},
            "instructions":{"const":[Instruction::LoseGameTargetPlayer{target_slot:0,source_block:block}]}
        }})
    };
    let caps = |extra: &[Capability]| {
        let mut caps = vec![
            Capability::NormalLayoutV1,
            Capability::AnyPlayerTargetV1,
            Capability::TargetedGameLossV1,
        ];
        caps.extend_from_slice(extra);
        json!({"minItems":caps.len(),"maxItems":caps.len(),"uniqueItems":true,"items":{"enum":caps}})
    };
    let ability = json!({"required":["functioning_zone","program","cost"],"properties":{
        "functioning_zone":{"const":"battlefield"},"program":loss(SpellTiming::Instant,1),
        "cost":{"required":["composite"],"properties":{"composite":{"properties":{"costs":{
            "required":["mana"],"properties":{"tap":{"const":true},"sacrifice":{"const":true},"remove_source_plus_one_counters":{"type":"null"},
                "mana":{"type":"object","properties":{"generic":{"pattern":"^(0|[1-9][0-9]*)$"},"colored":{"items":{"not":{"const":"C"}}}}}}
        }}}}}
    }});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().extend([
        json!({"properties":{"triggered":{"items":{"properties":{"program":free}}}}}),
        json!({"if":{"anyOf":[spell,activated]},"then":{"properties":{
            "characteristics":{"properties":{"subtypes":{"maxItems":0},"additional_types":{"maxItems":0},"basic":{"const":false},"snow":{"const":false},"legendary":{"const":false}}},
            "intrinsic_zone":{"type":"null"},"x_mana_cost":{"const":false},"keywords":{"maxItems":0},
            "triggered":{"maxItems":0},"mana_abilities":{"maxItems":0}
        },"oneOf":[
            {"required":["spell"],"properties":{"capabilities":caps(&[Capability::CastSorceryV1]),
                "characteristics":{"properties":{"kind":{"const":"sorcery"}}},"spell":loss(SpellTiming::MainPhase,0),
                "static_abilities":{"maxItems":0},"activated":{"maxItems":0}}},
            {"required":["spell","activated","static_abilities"],"properties":{
                "source_labels":{"properties":{"statics":{"const":[0]},"activated":{"const":[1]},"blocks":{"minItems":2,"maxItems":2}}},
                "capabilities":caps(&[Capability::CastArtifactV1,Capability::ActivatedAbilityV1,Capability::CompositeActivationCostV1,Capability::EntersTappedV1]),
                "characteristics":{"properties":{"kind":{"const":"artifact"}}},
                "spell":{"type":"object","properties":{"timing":{"const":"main_phase"},"functioning_zone":{"const":"stack"},"resolution":{"const":"permanent"},
                    "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},"targets":{"maxItems":0},"instructions":{"maxItems":0},"modes":{"maxItems":0}}},
                "static_abilities":{"const":[StaticAbility{functioning_zone:FunctioningZone::Battlefield,effect:StaticEffect::EntersTapped}]},
                "activated":{"minItems":1,"maxItems":1,"items":ability}
            }}
        ]},"else":{"properties":{"capabilities":{"not":{"contains":{"const":Capability::TargetedGameLossV1}}}}}})
    ]);
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let mut d = targeted_counted_draw::witness();
    d.characteristics.kind = CardKind::Sorcery;
    d.characteristics.subtypes.clear();
    d.name = "Synthetic Targeted Loss".into();
    d.source_labels = None;
    let p = d.spell.as_mut().unwrap();
    p.timing = SpellTiming::MainPhase;
    p.instructions = vec![Instruction::LoseGameTargetPlayer {
        target_slot: 0,
        source_block: 0,
    }];
    d.capabilities = d.requirements();
    d
}

#[test]
fn targeted_game_loss_contract_rejects_added_and_unbound_effects() {
    let d = witness();
    d.validate().unwrap();
    for variant in 0..3 {
        let mut malformed = d.clone();
        let p = malformed.spell.as_mut().unwrap();
        match variant {
            0 => p.instructions.push(p.instructions[0].clone()),
            1 => p.targets[0].domain = TargetDomain::OpponentPlayer,
            _ => {
                p.instructions[0] = Instruction::LoseGameTargetPlayer {
                    target_slot: 1,
                    source_block: 0,
                }
            }
        }
        malformed.capabilities = malformed.requirements();
        assert!(malformed.validate().is_err());
    }
}
