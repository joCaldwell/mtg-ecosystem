//! CR 714.2b-e. Each printed chapter symbol is a separate ability.
use super::*;
use serde_json::{Value, json};
use std::num::NonZeroU32;

mod composed;
pub use composed::FixedChapter;

pub struct LifeChapter {
    pub numbers: Vec<NonZeroU32>,
    pub effect: ChapterEffect,
    pub source_block: u32,
}
pub enum ChapterEffect {
    Gain { amount: u32 },
    DrawLose { draw: u32, loss: u32 },
    DelayedEntry { loss: u32, gain: u32 },
    Fixed(FixedChapter),
}
fn program(instructions: Vec<Instruction>) -> SpellProgram {
    SpellProgram {
        additional_sacrifice: None,
        optional_additional_costs: vec![],
        timing: SpellTiming::Instant,
        resolution: SpellResolution::Instructions,
        functioning_zone: FunctioningZone::Stack,
        targets: vec![],
        modes: vec![],
        instructions,
    }
}
impl LifeChapter {
    pub fn abilities(&self) -> impl Iterator<Item = TriggeredAbility> + '_ {
        self.numbers.iter().map(|&number| {
            let block = self.source_block;
            let gain = |amount| Instruction::GainLife {
                recipient: LifeRecipient::Controller,
                amount: Quantity::from(i64::from(amount)),
                source_block: block,
            };
            let lose = |recipient, amount| Instruction::LoseLife {
                recipient,
                amount: Quantity::from(i64::from(amount)),
                source_block: block,
            };
            let ops = match self.effect {
                ChapterEffect::Gain { amount } => vec![gain(amount)],
                ChapterEffect::DrawLose { draw, loss } => {
                    let mut steps = vec![
                        Instruction::DrawCards {
                            recipient: Some(LifeRecipient::Controller),
                            count: 1.into(),
                            source_block: block
                        };
                        draw as usize
                    ];
                    steps.push(lose(LifeRecipient::Controller, loss));
                    steps
                }
                ChapterEffect::Fixed(ref chapter) => chapter.instructions(block),
                ChapterEffect::DelayedEntry { loss, gain: life } => {
                    vec![Instruction::RegisterControllerCreatureEntryThisTurn {
                        source_block: block,
                        program: Box::new(program(vec![
                            lose(LifeRecipient::EachOpponent, loss),
                            gain(life),
                        ])),
                    }]
                }
            };
            let mut program = program(ops);
            if let ChapterEffect::Fixed(effect) = &self.effect
                && let Some(selection_id) = effect.selection_id(block)
            {
                program.targets.push(TargetSpec {
                    slot: 0,
                    selection_id,
                    domain: TargetDomain::ControlledBattlefieldCreature,
                    chooser: Chooser::Controller,
                    stage: EvaluationStage::Announcement,
                    filter: None,
                });
            }
            TriggeredAbility {
                condition: TriggerCondition::SagaChapter { number },
                functioning_zone: FunctioningZone::Battlefield,
                intervening_if: None,
                death_filter: None,
                program,
            }
        })
    }
    pub fn ast(&self) -> Value {
        let step = |effect: &str, player: &str, n: u32| json!({"effect":effect,"who":{"player":player},"amount":{"amount":"fixed","value":n}});
        let steps = match self.effect {
            ChapterEffect::Gain { amount } => vec![step("gain-life", "you", amount)],
            ChapterEffect::Fixed(ref chapter) => chapter.steps(self.source_block),
            ChapterEffect::DrawLose { draw, loss } => {
                vec![step("draw", "you", draw), step("lose-life", "you", loss)]
            }
            ChapterEffect::DelayedEntry { gain, loss } => vec![
                json!({"effect":"create-delayed-trigger","trigger":{"trigger":"enters","what":{"ref":"filter","filter":{"types":["creature"],"control":"you"},"count":{"amount":"fixed","value":1}}},"occurrence":"each","duration":{"duration":"this-turn"},"effects":[{"steps":[step("lose-life","each-opponent",loss),step("gain-life","you",gain)]}],"targets":"delayed-ability"}),
            ],
        };
        json!({"kind":"chapter","chapters":self.numbers,"effects":[{"steps":steps}]})
    }
}

fn number(digits: &str) -> Result<u32, String> {
    digits
        .parse::<u32>()
        .ok()
        .filter(|n| *n > 0 && n.to_string() == digits)
        .ok_or_else(|| "invalid fixed chapter amount".into())
}
fn effect(line: &str) -> Result<ChapterEffect, String> {
    let invalid = || "unsupported complete chapter instruction".to_owned();
    if let Some(program) = FixedChapter::parse(line) {
        return Ok(ChapterEffect::Fixed(program));
    }
    if let Some(digits) = line
        .strip_prefix("You gain ")
        .and_then(|s| s.strip_suffix(" life."))
    {
        return Ok(ChapterEffect::Gain {
            amount: number(digits)?,
        });
    }
    if let Some(rest) = line.strip_prefix("You draw ") {
        let (draw, loss) = rest
            .strip_suffix(" life.")
            .and_then(|s| s.split_once(" and you lose "))
            .ok_or_else(invalid)?;
        let draw = match draw {
            "a card" => 1,
            "two cards" => 2,
            "three cards" => 3,
            _ => return Err(invalid()),
        };
        return Ok(ChapterEffect::DrawLose {
            draw,
            loss: number(loss)?,
        });
    }
    let (event, consequence) = line.split_once(", ").ok_or_else(invalid)?;
    let (subject, duration) = event.split_once(" enters ").ok_or_else(invalid)?;
    if subject != "Whenever a creature you control" || duration != "this turn" {
        return Err(invalid());
    }
    let (loss, gain) = consequence
        .strip_prefix("each opponent loses ")
        .and_then(|s| s.strip_suffix(" life."))
        .and_then(|s| s.split_once(" life and you gain "))
        .ok_or_else(invalid)?;
    Ok(ChapterEffect::DelayedEntry {
        loss: number(loss)?,
        gain: number(gain)?,
    })
}
pub fn chapter_lines(text: &str) -> &str {
    let Some((reminder, rest)) = text.split_once('\n') else {
        return text;
    };
    let prefix = "(As this Saga enters and after your draw step, ";
    let suffix = "add a lore counter. Sacrifice after III.)";
    if reminder.strip_prefix(prefix) == Some(suffix) {
        rest
    } else {
        text
    }
}
pub fn life_chapters(text: &str) -> Result<Vec<LifeChapter>, String> {
    let lines = chapter_lines(text);
    let mut chapters = vec![];
    let mut next = 1;
    for (block, line) in lines.split('\n').enumerate() {
        let (symbols, clause) = line.split_once(" — ").ok_or("invalid chapter line")?;
        let mut numbers = vec![];
        for symbol in symbols.split(", ") {
            let n = match symbol {
                "I" => 1,
                "II" => 2,
                "III" => 3,
                _ => return Err("unsupported chapter symbol".into()),
            };
            if n != next {
                return Err("expected consecutive printed chapter symbols".into());
            }
            next += 1;
            numbers.push(NonZeroU32::new(n).unwrap());
        }
        chapters.push(LifeChapter {
            numbers,
            effect: effect(clause)?,
            source_block: u32::try_from(block).map_err(|_| "chapter block overflow")?,
        });
    }
    if lines != text && next != 4 {
        return Err("reminder and final chapter disagree".into());
    }
    Ok(chapters)
}
pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let raw = &d.provenance.source_json;
    let typed = d
        .capabilities
        .contains(&Capability::DelayedControllerCreatureEntryV1)
        || d.characteristics.subtypes.iter().any(|s| s == "Saga")
        || d.capabilities.contains(&Capability::SagaChaptersV1)
        || d.triggered
            .iter()
            .any(|t| matches!(t.condition, TriggerCondition::SagaChapter { .. }));
    if !typed && !raw.contains("saga") && !raw.contains(r"\u") {
        return Ok(());
    }
    let src: Value = strict_read(raw)?;
    if !typed && src["layout"] != "saga" {
        return Ok(());
    }
    let text = src["oracle_text"]
        .as_str()
        .ok_or("Saga source missing text")?;
    let chapters = life_chapters(text)?;
    let c = &d.characteristics;
    let mana = c.mana_cost.printed_fixed_cost();
    if src["layout"] != "saga"
        || src["type_line"] != "Enchantment — Saga"
        || src["id"] != d.provenance.printing_id
        || src["oracle_id"] != d.oracle_id
        || src["name"] != d.name
        || src["mana_cost"] != mana
        || src["colors"] != json!(c.colors)
        || src["color_identity"] != json!(c.color_identity)
        || src["keywords"] != json!([])
        || ["card_faces", "power", "toughness", "loyalty", "defense"]
            .iter()
            .any(|key| src.get(*key).is_some())
        || c.kind != CardKind::Enchantment
        || c.subtypes != ["Saga"]
        || !c.additional_types.is_empty()
        || c.legendary
        || c.snow
        || c.basic
        || d.x_mana_cost
        || !d.keywords.is_empty()
        || !d.activated.is_empty()
        || !d.static_abilities.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.capabilities.contains(&Capability::SagaChaptersV1)
        || d.spell.as_ref().is_none_or(|p| {
            p.resolution != SpellResolution::Permanent
                || p.timing != SpellTiming::MainPhase
                || p.functioning_zone != FunctioningZone::Stack
                || !p.instructions.is_empty()
                || !p.targets.is_empty()
                || !p.modes.is_empty()
                || !p.optional_additional_costs.is_empty()
                || p.additional_sacrifice.is_some()
        })
        || !d
            .triggered
            .iter()
            .cloned()
            .eq(chapters.iter().flat_map(|c| c.abilities()))
        || strict_read::<Value>(&d.provenance.abilities_json)?
            != Value::Array(chapters.iter().map(|c| c.ast()).collect())
        || d.source_labels.as_ref().is_some_and(|l| {
            l.text != text
                || !l
                    .blocks
                    .iter()
                    .map(String::as_str)
                    .eq(chapter_lines(text).lines())
                || !l.triggered.iter().copied().eq(chapters
                    .iter()
                    .flat_map(|c| c.numbers.iter().map(|_| c.source_block)))
                || !l.activated.is_empty()
                || !l.mana.is_empty()
                || !l.statics.is_empty()
                || !l.spell_modes.is_empty()
        })
    {
        return Err("invalid complete ordinary Saga source".into());
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    use serde_json::value::RawValue;
    #[derive(Deserialize)]
    struct Archive {
        source: Box<RawValue>,
        faces: Vec<Face>,
    }
    #[derive(Deserialize)]
    struct Face {
        result: Parsed,
    }
    #[derive(Deserialize)]
    struct Parsed {
        abilities: Box<RawValue>,
    }
    let archive =
        include_str!("../../../tests/fixtures/ramos/8419d1d5-bb0e-4a2d-bb6f-67957d035dde.json");
    let bytes: Archive = serde_json::from_str(archive).unwrap();
    let record: Value = serde_json::from_str(archive).unwrap();
    let source = &record["source"];
    let mut d = entry_graveyard_exile::witness(CardKind::Creature);
    d.name = source["name"].as_str().unwrap().into();
    d.oracle_id = source["oracle_id"].as_str().unwrap().into();
    d.provenance.printing_id = source["id"].as_str().unwrap().into();
    d.provenance.source_json = bytes.source.get().to_owned();
    d.provenance.source_sha256 = record["sourceSha256"].as_str().unwrap().into();
    d.provenance.abilities_json = bytes.faces[0].result.abilities.get().to_owned();
    d.provenance.ast_sha256 = record["astSha256"].as_str().unwrap().into();
    d.provenance.binding_sha256 = record["bindingSha256"].as_str().unwrap().into();
    d.characteristics.kind = CardKind::Enchantment;
    d.characteristics.subtypes = vec!["Saga".into()];
    d.characteristics.power = None;
    d.characteristics.toughness = None;
    d.characteristics.colors = serde_json::from_value(source["colors"].clone()).unwrap();
    d.characteristics.color_identity =
        serde_json::from_value(source["color_identity"].clone()).unwrap();
    d.characteristics.mana_cost = ManaCost {
        generic: 0.into(),
        colored: vec![Color::R, Color::W, Color::B],
    };
    d.keywords.clear();
    let text = source["oracle_text"].as_str().unwrap();
    let chapters = life_chapters(text).unwrap();
    d.triggered = chapters.iter().flat_map(|c| c.abilities()).collect();
    d.source_labels = Some(SourceLabels {
        text: text.into(),
        blocks: chapter_lines(text).lines().map(str::to_string).collect(),
        triggered: vec![0, 1, 1],
        ..Default::default()
    });
    d.capabilities = d.requirements();
    d
}

pub(super) fn extend_schema(schema: &mut Value) {
    use Capability::*;
    let chapter = json!({"required":["condition"],"properties":{"condition":{"type":"object","required":["saga_chapter"]}}});
    let allowed = [
        SagaChaptersV1,
        CastEnchantmentV1,
        TriggeredAbilityV1,
        LifeGainV1,
        LifeLossV1,
        TargetPlayerDrawV1,
        SacrificeV1,
        DelayedControllerCreatureEntryV1,
        CreatureTokensV1,
        CreatureTargetV1,
        PlusOneCountersV1,
        TemporaryCreatureKeywordProgramV1,
        TemporaryPowerToughnessV1,
        AttachmentsV1,
        TrampleV1,
    ];
    let fixed = |op: &str, recipient: &str, field: &str| json!({"type":"object","required":["op","recipient",field],"properties":{"op":{"const":op},"recipient":{"const":recipient},(field):{"pattern":"^[1-9][0-9]*$"}}});
    let gain = fixed("gain_life", "controller", "amount");
    let sequence =
        |ops: Vec<Value>| json!({"minItems":ops.len(),"maxItems":ops.len(),"prefixItems":ops});
    let child = sequence(vec![
        fixed("lose_life", "each_opponent", "amount"),
        gain.clone(),
    ]);
    let registration = json!({"required":["op","program"],"properties":{"op":{"const":"register_controller_creature_entry_this_turn"},"program":{"properties":{"resolution":{"const":"instructions"},"timing":{"const":"instant"},"functioning_zone":{"const":"stack"},"instructions":child,"targets":{"maxItems":0},"modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0}}}}});
    let mut variants = vec![sequence(vec![gain]), sequence(vec![registration])];
    let mut draw = fixed("draw_cards", "controller", "count");
    draw["properties"]["count"] = json!({"const":"1"});
    for n in 1..=3 {
        let mut steps = vec![draw.clone(); n];
        steps.push(fixed("lose_life", "controller", "amount"));
        variants.push(sequence(steps));
    }
    variants.push(sequence(vec![
        json!({"required":["op","count"],"properties":{
            "op":{"const":"create_creature_tokens"},"count":{"const":1}
        }}),
    ]));
    let programs = json!({"oneOf":variants});
    let target = json!({"required":["slot","domain","chooser","stage","selection_id"],"properties":{
        "slot":{"const":0},"domain":{"const":"controlled_battlefield_creature"},
        "selection_id":{"minLength":1},"chooser":{"const":"controller"},
        "stage":{"const":"announcement"},"filter":{"type":"null"}
    }});
    let grant = sequence(vec![json!({
        "required":["op","target_slot","modifier","keywords","duration"],
        "properties":{"op":{"const":"grant_keywords"},"target_slot":{"const":0},
            "modifier":{"type":"object"},"keywords":{"const":["trample"]},"duration":{"const":"until_end_of_turn"}}
    })]);
    let targeted = |instructions: Value| {
        json!({"type":"object","required":["targets","instructions"],
        "properties":{
            "targets":{"minItems":1,"maxItems":1,"items":target},"instructions":instructions,
            "functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
            "timing":{"const":"instant"},"modes":{"maxItems":0},
            "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0}
        }})
    };
    let grant_program = targeted(grant);
    let counter_program = targeted(sequence(vec![json!({
        "required":["op","on","amount"],"properties":{"op":{"const":"put_plus_one_counters"},
            "on":{"const":{"recipient":"target","target_slot":0}},"amount":{"pattern":"^[1-9][0-9]*$"}}
    })]));
    schema["$defs"]["SagaTargetedGrantProgram"] = grant_program.clone();
    schema["$defs"]["SagaTargetedGrantChapter"] = json!({"allOf":[chapter],
        "required":["program"],"properties":{"program":{"$ref":"#/$defs/SagaTargetedGrantProgram"}}});
    let chapter_programs = json!({"oneOf":[
        {"properties":{"targets":{"maxItems":0},"instructions":programs}},
        grant_program, counter_program
    ]});
    let registration_chapter = json!({
        "required":["program"],"properties":{"program":{
            "required":["instructions"],"properties":{"instructions":{"contains":{
                "required":["op"],"properties":{"op":{"const":"register_controller_creature_entry_this_turn"}}
            }}}
        }}
    });
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "if":{"anyOf":[{"properties":{"capabilities":{"contains":{"const":SagaChaptersV1}}}},
            {"required":["triggered"],"properties":{"triggered":{"contains":chapter}}},
            {"properties":{"characteristics":{"properties":{"subtypes":{"contains":{"const":"Saga"}}}}}},
            {"properties":{"provenance":{"properties":{"source_json":{"pattern":r#""layout"\s*:\s*"saga""#}}}}}]},
        "then":{"required":["triggered","spell"],"allOf":[{
            "if":{"properties":{"triggered":{"contains":registration_chapter}}},
            "then":{"properties":{"capabilities":{"allOf":[
                {"contains":{"const":SacrificeV1}},{"contains":{"const":DelayedControllerCreatureEntryV1}}
            ]}}},
            "else":{"properties":{"capabilities":{"not":{"contains":{
                "enum":[SacrificeV1,DelayedControllerCreatureEntryV1]
            }}}}}
        }],"properties":{
            "capabilities":{"minItems":4,"maxItems":allowed.len(),"allOf":[{"contains":{"const":SagaChaptersV1}},{"contains":{"const":TriggeredAbilityV1}},{"contains":{"const":CastEnchantmentV1}}],"uniqueItems":true,"items":{"enum":allowed}},
            "characteristics":{"properties":{"kind":{"const":"enchantment"},"subtypes":{"const":["Saga"]},"basic":{"const":false},
                "legendary":{"const":false},"snow":{"const":false},"additional_types":{"maxItems":0}}},
            "x_mana_cost":{"const":false},"activated":{"maxItems":0},"mana_abilities":{"maxItems":0},"static_abilities":{"maxItems":0},"keywords":{"maxItems":0},
            "spell":{"type":"object","properties":{"resolution":{"const":"permanent"},"timing":{"const":"main_phase"},"functioning_zone":{"const":"stack"},
                "instructions":{"maxItems":0},"targets":{"maxItems":0},"modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0}}},
            "triggered":{"minItems":1,"maxItems":3,"items":{"allOf":[chapter],"properties":{"functioning_zone":{"const":"battlefield"},
                "intervening_if":{"type":"null"},"death_filter":{"type":"null"},"program":{"allOf":[chapter_programs],"properties":{"functioning_zone":{"const":"stack"},
                "timing":{"const":"instant"},"resolution":{"const":"instructions"},"modes":{"maxItems":0},
                "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0}}}}}},
            "provenance":{"properties":{"source_json":{"pattern":r#""layout"\s*:\s*"saga""#}}}
        }}
    }));
}
