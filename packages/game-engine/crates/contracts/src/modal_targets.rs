//! CR 601.2b–c and 700.2: a mode's target occurrences belong to that printed
//! mode, including when two chosen modes select the same physical target.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeCount {
    OneOrMore,
    Exactly(u32),
}

impl ModeCount {
    pub fn selected(self, offered: usize, indices: &[u32]) -> Result<Vec<u32>, String> {
        if indices.is_empty()
            || indices.len() > offered
            || matches!(self, Self::Exactly(n) if usize::try_from(n).ok()!=Some(indices.len()))
        {
            return Err("wrong number of chosen modes".into());
        }
        let mut chosen = indices.to_vec();
        chosen.sort_unstable();
        if chosen.windows(2).any(|pair| pair[0] == pair[1])
            || chosen
                .last()
                .is_none_or(|index| usize::try_from(*index).map_or(true, |i| i >= offered))
        {
            return Err("unknown or repeated printed mode".into());
        }
        Ok(chosen)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModeTargetOccurrence<'a> {
    pub printed_mode: u32,
    pub target: &'a TargetSpec,
}

pub fn target_occurrences<'a>(
    root: &'a SpellProgram,
    count: ModeCount,
    chosen: &[u32],
) -> Result<Vec<ModeTargetOccurrence<'a>>, String> {
    let selected = count.selected(root.modes.len(), chosen)?;
    let mut occurrences = vec![];
    for mode in selected {
        for (index, target) in root.modes[mode as usize].program.targets.iter().enumerate() {
            if usize::try_from(target.slot).ok() != Some(index) {
                return Err("noncontiguous printed target occurrences".into());
            }
            if let TargetDomain::PlayerGraveyardCards { owner_slot, .. } = target.domain {
                if usize::try_from(owner_slot)
                    .ok()
                    .and_then(|s| root.modes[mode as usize].program.targets.get(s))
                    .is_none_or(|owner| {
                        owner.slot >= target.slot || owner.domain != TargetDomain::AnyPlayer
                    })
                {
                    return Err(
                        "dependent cards lack an earlier player target in the same mode".into(),
                    );
                }
            }
            occurrences.push(ModeTargetOccurrence {
                printed_mode: mode,
                target,
            });
        }
    }
    Ok(occurrences)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_mode_count_preserves_print_order_and_distinct_dependent_occurrences() {
        let mut root = crate::graveyard_shuffle::witness().spell.unwrap();
        let nested = root.clone();
        root.targets.clear();
        root.instructions.clear();
        root.modes = vec![
            SpellMode {
                program: nested.clone(),
            },
            SpellMode {
                program: nested.clone(),
            },
            SpellMode { program: nested },
        ];
        let choose = ModeCount::Exactly(2);
        let slots = target_occurrences(&root, choose, &[2, 0]).unwrap();
        assert_eq!(
            slots
                .iter()
                .map(|s| (s.printed_mode, s.target.slot))
                .collect::<Vec<_>>(),
            [(0, 0), (0, 1), (2, 0), (2, 1)]
        );
        for invalid in [&[][..], &[0], &[0, 0], &[0, 3], &[0, 1, 2]] {
            assert!(target_occurrences(&root, choose, invalid).is_err());
        }
        root.modes[2].program.targets[1].domain = TargetDomain::PlayerGraveyardCards {
            owner_slot: 1,
            maximum: 2,
        };
        assert!(target_occurrences(&root, choose, &[0, 2]).is_err());
    }
}

impl ModeCount {
    pub fn for_root(root: &SpellProgram) -> Option<Self> {
        match root.resolution {
            SpellResolution::ChooseModes { exactly } => Some(Self::Exactly(exactly)),
            SpellResolution::OneOrMoreModes => Some(Self::OneOrMore),
            _ => None,
        }
    }
}

pub fn fixed_player_modes(
    text: &str,
    ast: &serde_json::Value,
    timing: SpellTiming,
) -> Option<SpellProgram> {
    use serde_json::json;
    let mut lines = text.lines();
    let header = lines.next()?;
    let exactly = match header {
        "Choose two —" => 2,
        "Choose three —" => 3,
        _ => return None,
    };
    let mut root = SpellProgram {
        timing,
        functioning_zone: FunctioningZone::Stack,
        resolution: SpellResolution::ChooseModes { exactly },
        targets: vec![],
        instructions: vec![],
        modes: vec![],
        additional_sacrifice: None,
        optional_additional_costs: vec![],
    };
    let mut options = vec![];
    let mut ids = BTreeSet::new();
    for (i, line) in lines.enumerate() {
        let clause = line.strip_prefix("• Target player ")?;
        let (verb, number) = clause.strip_suffix(" life.")?.split_once(' ')?;
        let amount = number
            .parse::<u32>()
            .ok()
            .filter(|n| *n > 0 && n.to_string() == number)?;
        let step = &ast[0]["effects"][0]["steps"][0]["options"][i]["effects"][0]["steps"][0];
        let selection = step["who"]["selectionId"].as_str()?;
        if selection.is_empty() || !ids.insert(selection.to_owned()) {
            return None;
        }
        let recipient = LifeRecipient::Target { target_slot: 0 };
        let (operation, instruction) = match verb {
            "gains" => (
                "gain-life",
                Instruction::GainLife {
                    recipient,
                    amount: Quantity::from(i64::from(amount)),
                    source_block: 0,
                },
            ),
            "loses" => (
                "lose-life",
                Instruction::LoseLife {
                    recipient,
                    amount: Quantity::from(i64::from(amount)),
                    source_block: 0,
                },
            ),
            _ => return None,
        };
        let who = json!({"player":"target-player","selectionId":selection});
        options.push(json!({"effects":[{"steps":[{"effect":operation,"who":who,"amount":{"amount":"fixed","value":amount}}]}]}));
        root.modes.push(SpellMode {
            program: SpellProgram {
                timing,
                functioning_zone: FunctioningZone::Stack,
                resolution: SpellResolution::Instructions,
                targets: vec![TargetSpec {
                    slot: 0,
                    selection_id: selection.into(),
                    chooser: Chooser::Controller,
                    stage: EvaluationStage::Announcement,
                    domain: TargetDomain::AnyPlayer,
                    filter: None,
                }],
                instructions: vec![instruction],
                modes: vec![],
                additional_sacrifice: None,
                optional_additional_costs: vec![],
            },
        });
    }
    let expected = json!([{"kind":"spell","effects":[{"steps":[{"effect":"modal","count":{"exactly":exactly},"options":options}]}]}]);
    (root.modes.len() >= exactly as usize && *ast == expected).then_some(root)
}

pub(super) fn validate_definition(d: &Definition) -> Result<(), String> {
    fn fixed(p: &SpellProgram) -> bool {
        matches!(p.resolution, SpellResolution::ChooseModes { .. })
            || p.modes.iter().any(|m| fixed(&m.program))
    }
    let fail =
        || "fixed target modes require the complete ordinary source and target operands".to_owned();
    if d.activated.iter().any(|a| fixed(&a.program))
        || d.triggered.iter().any(|a| fixed(&a.program))
        || d.static_abilities.iter().any(
            |a| matches!(&a.effect,StaticEffect::GrantSpellCascade{program,..} if fixed(program)),
        )
    {
        return Err(fail());
    }
    let uses = d.spell.as_ref().is_some_and(fixed);
    if !uses
        && !d.provenance.source_json.contains("Choose two")
        && !d.provenance.source_json.contains("Choose three")
    {
        return Ok(());
    }
    let src: serde_json::Value = strict_read(&d.provenance.source_json)?;
    let text = src["oracle_text"].as_str().unwrap_or("");
    let ast: serde_json::Value = strict_read(&d.provenance.abilities_json)?;
    let timing = match d.characteristics.kind {
        CardKind::Instant => SpellTiming::Instant,
        CardKind::Sorcery => SpellTiming::MainPhase,
        _ if !uses => return Ok(()),
        _ => return Err(fail()),
    };
    let candidate =
        fixed_player_modes(text, &ast, timing).or_else(|| mixed_target_modes(text, &ast, timing));
    let source_candidate = text
        .lines()
        .next()
        .is_some_and(|h| matches!(h, "Choose two —" | "Choose three —"))
        && text.lines().skip(1).all(|l| {
            l.starts_with("• Target player gains ") || l.starts_with("• Target player loses ")
        });
    if !uses && !source_candidate && candidate.is_none() {
        return Ok(());
    }
    let c = &d.characteristics;
    let mut cost = String::new();
    if c.mana_cost.generic != Quantity::zero() || c.mana_cost.colored.is_empty() {
        cost.push_str(&format!("{{{}}}", c.mana_cost.generic));
    }
    for color in &c.mana_cost.colored {
        cost.push_str(&format!("{{{color:?}}}"));
    }
    if src["id"] != d.provenance.printing_id
        || src["oracle_id"] != d.oracle_id
        || src["name"] != d.name
        || src["layout"] != "normal"
        || src["mana_cost"] != cost
        || src["type_line"] != format!("{:?}", c.kind)
        || src["colors"] != serde_json::json!(c.colors)
        || src["color_identity"] != serde_json::json!(c.color_identity)
        || c.basic
        || c.legendary
        || c.snow
        || !c.additional_types.is_empty()
        || !c.subtypes.is_empty()
    {
        return Err(fail());
    }
    if candidate.is_none()
        || d.spell != candidate
        || d.x_mana_cost
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.static_abilities.is_empty()
        || !d.mana_abilities.is_empty()
        || !d.keywords.is_empty()
        || d.source_labels.as_ref().is_some_and(|l| {
            l != &SourceLabels {
                text: text.into(),
                blocks: vec![text.into()],
                spell_modes: text.lines().skip(1).map(str::to_owned).collect(),
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
    let raw: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/synthetic-fixed-target-modes.json"
    ))
    .unwrap();
    let source = &raw["source"];
    let ast = &raw["faces"][0]["result"]["abilities"];
    let text = source["oracle_text"].as_str().unwrap();
    let mut d = crate::graveyard_shuffle::witness();
    d.name = source["name"].as_str().unwrap().into();
    d.oracle_id = raw["oracleIdentity"].as_str().unwrap().into();
    d.provenance.printing_id = source["id"].as_str().unwrap().into();
    d.provenance.source_json = source.to_string();
    d.provenance.abilities_json = ast.to_string();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.provenance.binding_sha256 = raw["bindingSha256"].as_str().unwrap().into();
    d.spell = Some(fixed_player_modes(text, ast, SpellTiming::MainPhase).unwrap());
    d.capabilities = d.requirements();
    d
}

pub(super) fn schema(schema: &mut serde_json::Value) {
    use serde_json::json;
    let marker = json!({"type":"object","required":["choose_modes"]});
    let cap = json!({"contains":{"const":Capability::FixedTargetModesV1}});
    let free = json!({"$ref":"#/$defs/FixedTargetModesFree"});
    schema["$defs"]["FixedTargetModesFree"] = json!({"properties":{"resolution":{"not":marker},"modes":{"items":{"properties":{"program":free}}}}});
    let child = json!({"properties":{"program":{"properties":{"targets":{"minItems":1,"maxItems":1,"items":{"properties":{"domain":{"const":"any_player"},"slot":{"const":0},"filter":{"type":"null"}}}},"instructions":{"minItems":1,"maxItems":1,"items":{"properties":{"op":{"enum":["gain_life","lose_life"]},"recipient":{"const":{"target":{"target_slot":0}}},"source_block":{"const":0}}}},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0}}}}});
    let child = json!({"anyOf":[child,{"properties":{"program":{"$ref":"#/$defs/UnionReturnChild"}}},{"properties":{"program":{"$ref":"#/$defs/GraveyardShuffleChild"}}},{"properties":{"program":{"$ref":"#/$defs/TypedCounterChild"}}},{"properties":{"program":{"$ref":"#/$defs/FixedCountersChild"}}}]});
    let basic = |domain: TargetDomain, filter: serde_json::Value, op: serde_json::Value| json!({"properties":{"resolution":{"const":"instructions"},"targets":{"minItems":1,"maxItems":1,"items":{"properties":{"domain":{"const":domain},"slot":{"const":0},"filter":filter}}},"instructions":{"minItems":1,"maxItems":1,"items":op},"modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0}}});
    let filter = PermanentFilter {
        types: vec![CardKind::Artifact, CardKind::Enchantment],
        non_types: vec![],
        non_colors: vec![],
        opponent: false,
        power: None,
        tapped: false,
    };
    schema["$defs"]["TypedCounterChild"] = basic(
        TargetDomain::StackSpell,
        json!({"const":filter}),
        json!({"const":Instruction::CounterStackObject{target_slot:0,source_block:0}}),
    );
    schema["$defs"]["FixedCountersChild"] = basic(
        TargetDomain::BattlefieldCreature,
        json!({"type":"null"}),
        json!({"properties":{"op":{"const":"put_plus_one_counters"},"source_block":{"const":0},"on":{"const":CounterRecipient::Target{target_slot:0}},"amount":{"enum":(2..=10).map(Quantity::from).collect::<Vec<_>>()}}}),
    );
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({"properties":{"activated":{"items":{"properties":{"program":free}}},"triggered":{"items":{"properties":{"program":free}}},"static_abilities":{"items":{"properties":{"effect":{"properties":{"grant_spell_cascade":{"properties":{"program":free}}}}}}}},"if":{"anyOf":[{"required":["capabilities"],"properties":{"capabilities":cap}},{"required":["spell"],"properties":{"spell":{"type":"object","not":free}}}]},"then":{"required":["spell","capabilities"],"properties":{"capabilities":{"allOf":[cap,{"contains":{"const":Capability::ModalChoiceV1}}]},"activated":{"maxItems":0},"triggered":{"maxItems":0},"static_abilities":{"maxItems":0},"spell":{"type":"object","properties":{"resolution":marker,"modes":{"minItems":2,"items":child},"targets":{"maxItems":0},"instructions":{"maxItems":0},"additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0}}}}}}));
}

/// Compile each printed clause independently. The variable graveyard group
/// occupies the tail of this bounded target layout so its runtime arity cannot
/// shift any following instruction operand.
pub fn mixed_target_modes(
    text: &str,
    ast: &serde_json::Value,
    timing: SpellTiming,
) -> Option<SpellProgram> {
    use serde_json::json;
    let mut lines = text.lines();
    let exactly = match lines.next()? {
        "Choose two —" => 2,
        "Choose three —" => 3,
        _ => return None,
    };
    let mut ids = BTreeSet::new();
    let mut options = vec![];
    let mut modes = vec![];
    let mut variable = false;
    for (i, line) in lines.enumerate() {
        if variable {
            return None;
        }
        let text = line.strip_prefix("• ")?;
        let step = &ast[0]["effects"][0]["steps"][0]["options"][i]["effects"][0]["steps"][0];
        let child = if let Some(p) =
            crate::creature_planeswalker_return::program(text, step, timing, 0)
        {
            p
        } else if let Some(shape) =
            crate::graveyard_shuffle::GraveyardShuffle::from_clause(text, step)
        {
            variable = true;
            shape.program(timing)
        } else {
            let (key, domain, filter, op, expected) = match text {
                "Counter target artifact or enchantment spell." => {
                    let id = step["what"]["selectionId"].as_str()?;
                    let f = PermanentFilter {
                        types: vec![CardKind::Artifact, CardKind::Enchantment],
                        non_types: vec![],
                        non_colors: vec![],
                        opponent: false,
                        tapped: false,
                        power: None,
                    };
                    (
                        "what",
                        TargetDomain::StackSpell,
                        Some(f),
                        Instruction::CounterStackObject {
                            target_slot: 0,
                            source_block: 0,
                        },
                        json!({"effect":"counter","what":{"ref":"target","filter":{"allOf":[{"anyOf":[{"types":["artifact"]},{"types":["enchantment"]}]}],"cls":"spell"},"count":{"amount":"fixed","value":1},"selectionId":id}}),
                    )
                }
                clause => {
                    let word = clause
                        .strip_prefix("Put ")?
                        .strip_suffix(" +1/+1 counters on target creature.")?;
                    let n = [
                        "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
                    ]
                    .iter()
                    .position(|s| *s == word)? as u32
                        + 2;
                    let id = step["on"]["selectionId"].as_str()?;
                    (
                        "on",
                        TargetDomain::BattlefieldCreature,
                        None,
                        Instruction::PutPlusOneCounters {
                            on: CounterRecipient::Target { target_slot: 0 },
                            amount: Quantity::from(i64::from(n)),
                            source_block: 0,
                        },
                        json!({"effect":"put-counters","on":{"ref":"target","filter":{"types":["creature"]},"count":{"amount":"fixed","value":1},"selectionId":id},"counter":{"type":"+1/+1"},"count":{"amount":"fixed","value":n}}),
                    )
                }
            };
            if step != &expected {
                return None;
            }
            SpellProgram {
                timing,
                functioning_zone: FunctioningZone::Stack,
                resolution: SpellResolution::Instructions,
                targets: vec![TargetSpec {
                    slot: 0,
                    selection_id: step[key]["selectionId"].as_str()?.into(),
                    chooser: Chooser::Controller,
                    stage: EvaluationStage::Announcement,
                    domain,
                    filter,
                }],
                instructions: vec![op],
                modes: vec![],
                additional_sacrifice: None,
                optional_additional_costs: vec![],
            }
        };
        if child
            .targets
            .iter()
            .any(|t| t.selection_id.is_empty() || !ids.insert(t.selection_id.clone()))
        {
            return None;
        }
        options.push(json!({"effects":[{"steps":[step]}]}));
        modes.push(SpellMode { program: child });
    }
    let expected = json!([{"kind":"spell","effects":[{"steps":[{"effect":"modal","count":{"exactly":exactly},"options":options}]}]}]);
    (modes.len() >= exactly as usize && ast == &expected).then_some(SpellProgram {
        timing,
        functioning_zone: FunctioningZone::Stack,
        resolution: SpellResolution::ChooseModes { exactly },
        targets: vec![],
        instructions: vec![],
        modes,
        additional_sacrifice: None,
        optional_additional_costs: vec![],
    })
}
