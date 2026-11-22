//! Source-relative entry triggers on artifacts, Equipment and Auras.
//! CR 603.3d, 603.6a, 608.2b–c and 608.2h.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub fn noun(kind: CardKind, subtypes: &[String]) -> Option<&'static str> {
    match kind {
        CardKind::Artifact if subtypes.iter().any(|s| s == "Equipment") => Some("Equipment"),
        CardKind::Artifact => Some("artifact"),
        CardKind::Enchantment if subtypes.iter().any(|s| s == "Aura") => Some("Aura"),
        _ => None,
    }
}

fn body<'a>(line: &'a str, noun: &str) -> Option<&'a str> {
    line.strip_prefix(&format!("When this {noun} enters, "))?
        .strip_suffix('.')
}

pub fn each_creature_sacrifice(line: &str) -> bool {
    line == "When this creature enters, each player sacrifices a creature of their choice."
}

pub fn candidate(line: &str, noun: &str) -> bool {
    if each_creature_sacrifice(line) {
        return true;
    }
    body(line, noun).is_some_and(|s| {
        [
            "draw ",
            "you draw ",
            "you gain ",
            "you lose ",
            "it deals ",
            "destroy target ",
        ]
        .iter()
        .any(|prefix| s.starts_with(prefix))
    })
}

pub fn printed(line: &str, noun: &str, ast: &Value, block: u32) -> Option<TriggeredAbility> {
    let mut program = SpellProgram {
        timing: SpellTiming::Instant,
        functioning_zone: FunctioningZone::Stack,
        resolution: SpellResolution::Instructions,
        optional_additional_costs: vec![],
        additional_sacrifice: None,
        targets: vec![],
        instructions: vec![],
        modes: vec![],
    };
    let mut add_target = |id: &str, domain: TargetDomain, filter: Option<PermanentFilter>| {
        let suffix = id.strip_prefix(&format!("line:{block}:"))?;
        if suffix.is_empty()
            || !suffix.bytes().all(|b| b.is_ascii_digit())
            || program
                .targets
                .iter()
                .any(|s: &TargetSpec| s.selection_id == id)
        {
            return None;
        }
        let slot = u32::try_from(program.targets.len()).ok()?;
        program.targets.push(TargetSpec {
            slot,
            selection_id: id.into(),
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
            domain,
            filter,
        });
        Some(slot)
    };
    let fixed = |n| json!({"amount":"fixed","value":n});
    let you = json!({"player":"you"});
    let mut sentences = vec![];
    for (i, sentence) in body(line, noun)?.split(". ").enumerate() {
        let mut steps = vec![];
        for (j, clause) in sentence
            .split(", then ")
            .flat_map(|s| s.split(" and "))
            .enumerate()
        {
            let clause = clause.to_ascii_lowercase();
            let archived = &ast["effects"][i]["steps"][j];
            let step = if each_creature_sacrifice(line) {
                program.instructions.push(Instruction::EachPlayerSacrifice {
                    source_block: block,
                });
                json!({"effect":"sacrifice","who":{"player":"each-player"},"what":{"ref":"filter","filter":{"types":["creature"]},"count":fixed(1)},"chosenBy":{"player":"each-player"}})
            } else if let Some(count) = clause
                .strip_prefix("draw ")
                .or_else(|| clause.strip_prefix("you draw "))
                .and_then(equipped_trigger::draw_count)
                .filter(|n| *n <= 10)
            {
                program.instructions.extend((0..count).map(|_| {
                    Instruction::draw(PlayerExpr::you(Context::Triggered), 1.into(), block)
                }));
                json!({"effect":"draw","who":you,"amount":fixed(count)})
            } else if let Some(n) = clause
                .strip_prefix("you gain ")
                .and_then(|s| s.strip_suffix(" life"))
                .and_then(equipped_trigger::count)
            {
                program.instructions.push(Instruction::act(
                    Verb::GainLife {
                        player: PlayerExpr::you(Context::Triggered),
                        amount: Amount::fixed(i64::from(n)),
                    },
                    block,
                ));
                json!({"effect":"gain-life","who":you,"amount":fixed(n)})
            } else if let Some(n) = clause
                .strip_prefix("you lose ")
                .and_then(|s| s.strip_suffix(" life"))
                .and_then(equipped_trigger::count)
            {
                program.instructions.push(Instruction::act(
                    Verb::LoseLife {
                        player: PlayerExpr::you(Context::Triggered),
                        amount: Amount::fixed(i64::from(n)),
                    },
                    block,
                ));
                json!({"effect":"lose-life","who":you,"amount":fixed(n)})
            } else if let Some(n) = clause
                .strip_prefix("it deals ")
                .and_then(|s| s.strip_suffix(" damage to any target"))
                .and_then(equipped_trigger::count)
            {
                let id = archived["to"]["selectionId"].as_str()?;
                let slot = add_target(id, TargetDomain::AnyDamageTarget, None)?;
                program.instructions.push(Instruction::damage_target(
                    slot,
                    kir::Amount::Fixed {
                        value: i64::from(n).into(),
                    },
                    block,
                ));
                json!({"effect":"damage","source":{"ref":"it"},"amount":fixed(n),"to":{"to":"any-target","selectionId":id}})
            } else {
                let (types, archive_filter) = match clause.strip_prefix("destroy target ")? {
                    "permanent" => (vec![], json!({"cls":"permanent"})),
                    "creature" => (vec![CardKind::Creature], json!({"types":["creature"]})),
                    _ => return None,
                };
                let id = archived["what"]["selectionId"].as_str()?;
                let slot = add_target(
                    id,
                    TargetDomain::BattlefieldPermanent,
                    Some(PermanentFilter {
                        types,
                        non_types: vec![],
                        non_colors: vec![],
                        opponent: false,
                        tapped: false,
                        power: None,
                    }),
                )?;
                program
                    .instructions
                    .push(Instruction::destroy_target(slot, false, block));
                json!({"effect":"destroy","what":{"ref":"target","filter":archive_filter,"count":fixed(1),"selectionId":id}})
            };
            steps.push(step);
        }
        sentences.push(json!({"steps":steps}));
    }
    let expected = json!({"kind":"triggered","trigger":{"trigger":"enters","what":{"ref":"this","noun":noun.to_ascii_lowercase()}},"effects":sentences});
    (*ast == expected).then_some(TriggeredAbility {
        functioning_zone: FunctioningZone::Battlefield,
        condition: TriggerCondition::SelfEntersBattlefield,
        intervening_if: None,
        death_filter: None,
        program,
    })
}

pub(super) fn source_matches(d: &Definition, ability: &TriggeredAbility) -> bool {
    if ability.condition != TriggerCondition::SelfEntersBattlefield {
        return false;
    }
    let Some(noun) = noun(d.characteristics.kind, &d.characteristics.subtypes)
        .or_else(|| (d.characteristics.kind == CardKind::Creature).then_some("creature"))
    else {
        return false;
    };
    let Some(first) = ability.program.instructions.first() else {
        return false;
    };
    let block = first.source_block();
    let Ok(source) = strict_read::<Value>(&d.provenance.source_json) else {
        return false;
    };
    let Ok(ast) = strict_read::<Vec<Value>>(&d.provenance.abilities_json) else {
        return false;
    };
    let Some(line) = source["oracle_text"]
        .as_str()
        .and_then(|s| s.lines().nth(block as usize))
    else {
        return false;
    };
    source["layout"] == "normal"
        && source["oracle_id"] == d.oracle_id
        && source["id"] == d.provenance.printing_id
        && source["name"] == d.name
        && ast
            .get(block as usize)
            .and_then(|a| printed(line, noun, a, block))
            .as_ref()
            == Some(ability)
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let Some(noun) = noun(d.characteristics.kind, &d.characteristics.subtypes)
        .or_else(|| (d.characteristics.kind == CardKind::Creature).then_some("creature"))
    else {
        return Ok(());
    };
    let raw = &d.provenance.source_json;
    if !raw.contains(&format!("When this {noun} enters, ")) && !raw.contains(r"\u") {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let text = source["oracle_text"].as_str().unwrap_or_default();
    let ast: Vec<Value> = strict_read(&d.provenance.abilities_json)?;
    let trigger_count = ast.iter().filter(|a| a["kind"] == "triggered").count();
    for (index, line) in text.lines().enumerate() {
        if !candidate(line, noun) || (noun == "creature" && !each_creature_sacrifice(line)) {
            continue;
        }
        let block = u32::try_from(index).map_err(|_| "entry block limit")?;
        if let Some(expected) = ast.get(index).and_then(|a| printed(line, noun, a, block)) {
            let ordinal = ast[..index]
                .iter()
                .filter(|a| a["kind"] == "triggered")
                .count();
            if d.triggered.len() != trigger_count
                || d.triggered.get(ordinal) != Some(&expected)
                || !source_matches(d, &expected)
                || d.source_labels
                    .as_ref()
                    .is_some_and(|labels| labels.triggered.get(ordinal) != Some(&block))
            {
                return Err("requires complete permanent entry ability".into());
            }
        }
    }
    Ok(())
}
