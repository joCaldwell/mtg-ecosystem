//! Source-relative scalar ability blocks. CR 109.5, 602.2, 603.3, 608.2c.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(crate) const SCHEMA: &str = "#/$defs/ScalarAbilityProgram";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BoundAbility {
    Trigger(TriggeredAbility),
    Activation(ActivatedAbility),
}

pub fn subject(kind: CardKind) -> Option<&'static str> {
    Some(match kind {
        CardKind::Creature => "creature",
        CardKind::Artifact => "artifact",
        CardKind::Enchantment => "enchantment",
        _ => return None,
    })
}

pub fn candidate(line: &str, kind: CardKind) -> bool {
    let Some(noun) = subject(kind) else {
        return false;
    };
    let lower = line.to_ascii_lowercase();
    let look = lower.contains("scry ") || lower.contains("surveil ");
    let token = PredefinedToken::appears(line);
    if token
        && ((kind == CardKind::Creature && line.starts_with("When this creature dies, "))
            || (line.starts_with("{T}: Create ") && kind == CardKind::Creature))
    {
        return true;
    }

    ((look || token)
        && (line.starts_with(&format!("When this {noun} enters, "))
            || (kind == CardKind::Creature
                && line.starts_with("Whenever this creature attacks, "))))
        || (line.starts_with('{')
            && line.contains(&format!(", Sacrifice this {noun}: "))
            && (look || line.contains(" life and draw a card.")))
}

fn fixed(n: u32) -> Value {
    json!({"amount":"fixed","value":n})
}

pub(super) fn reminder_valid(text: &str, instructions: &[Instruction]) -> bool {
    let Some(text) = text.strip_suffix(')') else {
        return false;
    };
    if text
        == "To scry X, look at the top X cards of your library, then put any number of them on the bottom and the rest on top in any order."
        && instructions
            .iter()
            .filter(|i| matches!(i, Instruction::Scry { .. }))
            .count()
            > 1
    {
        return true;
    }
    instructions.iter().any(|i| {
        let (verb, amount) = match i {
            Instruction::Scry { amount, .. } => ("scry", amount),
            Instruction::Surveil { amount, .. } => ("surveil", amount),
            _ => return false,
        };
        let prefix = format!("To {verb} {amount}, look at the top ");
        let Some(rest) = text
            .strip_prefix("Look at the top ")
            .or_else(|| text.strip_prefix(&prefix))
        else {
            return false;
        };
        if *amount == Quantity::from(1) {
            return match verb {
                "scry" => rest == "card of your library. You may put that card on the bottom.",
                _ => matches!(
                    rest,
                    "card of your library. You may put it into your graveyard."
                        | "card of your library. You may put that card into your graveyard."
                ),
            };
        }
        let Some((n, tail)) =
            rest.split_once(" cards of your library, then put any number of them ")
        else {
            return false;
        };
        equipped_trigger::count(n).is_some_and(|n| *amount == Quantity::from(i64::from(n)))
            && tail
                == if verb == "scry" {
                    "on the bottom and the rest on top in any order."
                } else {
                    "into your graveyard and the rest on top of your library in any order."
                }
    })
}

fn effects(body: &str, block: u32, context: Context) -> Option<(Vec<Instruction>, Value)> {
    let (body, reminder) = body
        .split_once(" (")
        .map_or((body, None), |(b, r)| (b, Some(r)));
    let mut instructions = Vec::new();
    let mut effects = Vec::new();
    for sentence in body.strip_suffix('.')?.split(". ") {
        let mut steps = Vec::new();
        for clause in sentence.split(", then ").flat_map(|s| s.split(" and ")) {
            let lower = clause.to_ascii_lowercase();
            let (ast, inst) = if let Some(step) = predefined_tokens::printed(clause, context, block)
            {
                step
            } else if let Some(n) = lower
                .strip_prefix("scry ")
                .and_then(equipped_trigger::count)
            {
                (
                    json!({"effect":"scry","amount":fixed(n)}),
                    Instruction::Scry {
                        amount: i64::from(n).into(),
                        source_block: block,
                    },
                )
            } else if let Some(n) = lower
                .strip_prefix("surveil ")
                .and_then(equipped_trigger::count)
            {
                (
                    json!({"effect":"surveil","amount":fixed(n)}),
                    Instruction::Surveil {
                        amount: i64::from(n).into(),
                        source_block: block,
                    },
                )
            } else if matches!(lower.as_str(), "draw a card" | "you draw a card") {
                (
                    json!({"effect":"draw","who":{"player":"you"},"amount":fixed(1)}),
                    Instruction::draw(PlayerExpr::you(context), 1.into(), block),
                )
            } else {
                let n = lower
                    .strip_prefix("you gain ")
                    .or_else(|| lower.strip_prefix("gain "))?
                    .strip_suffix(" life")
                    .and_then(equipped_trigger::count)?;
                (
                    json!({"effect":"gain-life","who":{"player":"you"},"amount":fixed(n)}),
                    Instruction::act(
                        Verb::GainLife {
                            player: PlayerExpr::you(context),
                            amount: Amount::fixed(i64::from(n)),
                        },
                        block,
                    ),
                )
            };
            steps.push(ast);
            instructions.push(inst);
        }
        effects.push(json!({"steps":steps}));
    }
    if reminder.is_some_and(|r| {
        !reminder_valid(r, &instructions) && !predefined_tokens::reminder(r, &instructions)
    }) {
        return None;
    }
    Some((instructions, json!(effects)))
}

fn cost(text: &str, noun: &str) -> Option<(ActivationCost, Value)> {
    if text == "{T}" {
        return Some((ActivationCost::TapSource, json!([{"cost":"tap-self"}])));
    }
    let (text, sacrifice) = text
        .strip_suffix(&format!(", Sacrifice this {noun}"))
        .map_or((text, false), |s| (s, true));
    let (mut text, tap) = text
        .strip_suffix(", {T}")
        .map_or((text, false), |s| (s, true));
    if !sacrifice {
        return None;
    }
    let mut mana = ManaCost {
        generic: Quantity::zero(),
        colored: vec![],
    };
    let mut symbols = Vec::new();
    while !text.is_empty() {
        let (symbol, rest) = text.strip_prefix('{')?.split_once('}')?;
        match symbol {
            "W" => mana.colored.push(Color::W),
            "U" => mana.colored.push(Color::U),
            "B" => mana.colored.push(Color::B),
            "R" => mana.colored.push(Color::R),
            "G" => mana.colored.push(Color::G),
            _ => {
                let n = symbol.parse::<u32>().ok()?;
                if n.to_string() != symbol || !symbols.is_empty() {
                    return None;
                }
                mana.generic = i64::from(n).into();
            }
        }
        symbols.push(symbol);
        text = rest;
    }
    if symbols.is_empty() {
        return None;
    }
    let mut ast = vec![json!({"cost":"mana","symbols":symbols})];
    if tap {
        ast.push(json!({"cost":"tap-self"}))
    }
    ast.push(json!({"cost":"sacrifice","what":{"ref":"this","noun":noun}}));
    Some((
        ActivationCost::Composite {
            costs: PermanentActivationCosts {
                mana: Some(mana),
                tap,
                sacrifice,
                remove_source_plus_one_counters: None,
            },
        },
        json!(ast),
    ))
}

pub fn printed(line: &str, raw: &Value, block: u32, kind: CardKind) -> Option<BoundAbility> {
    if !candidate(line, kind) {
        return None;
    }
    let noun = subject(kind)?;
    let program = |instructions| SpellProgram {
        timing: SpellTiming::Instant,
        functioning_zone: FunctioningZone::Stack,
        resolution: SpellResolution::Instructions,
        optional_additional_costs: vec![],
        additional_sacrifice: None,
        targets: vec![],
        modes: vec![],
        instructions,
    };
    let (event, body, condition) =
        if let Some(b) = line.strip_prefix(&format!("When this {noun} enters, ")) {
            ("enters", b, TriggerCondition::SelfEntersBattlefield)
        } else if let Some(b) = line.strip_prefix("When this creature dies, ") {
            ("dies", b, TriggerCondition::SelfDies)
        } else if let Some(b) = line.strip_prefix("Whenever this creature attacks, ") {
            (
                "attacks",
                b,
                TriggerCondition::CreatureAttacked {
                    attacker: AttackSubject::Source {},
                },
            )
        } else {
            let (text, body) = line.split_once(": ")?;
            let (cost, costs) = cost(text, noun)?;
            let (instructions, effects) = effects(body, block, Context::Activated)?;
            return (*raw == json!({"kind":"activated","costs":costs,"effects":effects}))
                .then_some(BoundAbility::Activation(ActivatedAbility {
                    functioning_zone: FunctioningZone::Battlefield,
                    cost,
                    program: program(instructions),
                }));
        };
    let (instructions, effects) = effects(body, block, Context::Triggered)?;
    (*raw==json!({"kind":"triggered","trigger":{"trigger":event,"what":{"ref":"this","noun":noun}},"effects":effects}))
        .then_some(BoundAbility::Trigger(TriggeredAbility { functioning_zone:FunctioningZone::Battlefield,
            condition,intervening_if:None,death_filter:None,program:program(instructions) }))
}

fn has_scalar(a: &SpellProgram) -> bool {
    a.instructions.iter().any(|i| {
        matches!(
            i,
            Instruction::Scry { .. }
                | Instruction::Surveil { .. }
                | kernel!(CreateTokens {
                    token: kir::TokenSpec::Predefined { .. },
                    ..
                })
        )
    }) || matches!(
        a.instructions.as_slice(),
        [kernel!(GainLife { .. }), draw] if draw.uncounted_draw()
    )
}

pub(crate) fn activation(a: &ActivatedAbility) -> bool {
    (matches!(a.cost, ActivationCost::Composite { .. }) && has_scalar(&a.program))
        || (a.cost == ActivationCost::TapSource
            && a.program.instructions.iter().any(|i| {
                matches!(
                    i,
                    kernel!(CreateTokens {
                        token: kir::TokenSpec::Predefined { .. },
                        ..
                    })
                )
            }))
}

pub(crate) fn trigger(a: &TriggeredAbility, kind: CardKind) -> bool {
    subject(kind).is_some()
        && has_scalar(&a.program)
        && a.program.instructions.iter().any(|i| {
            matches!(
                i,
                Instruction::Scry { .. }
                    | Instruction::Surveil { .. }
                    | kernel!(CreateTokens {
                        token: kir::TokenSpec::Predefined { .. },
                        ..
                    })
            )
        })
        && (a.condition == TriggerCondition::SelfEntersBattlefield
            || a.condition == TriggerCondition::SelfDies
            || a.condition
                == TriggerCondition::CreatureAttacked {
                    attacker: AttackSubject::Source {},
                })
}

pub(crate) fn program(p: &SpellProgram) -> bool {
    p.timing == SpellTiming::Instant
        && p.functioning_zone == FunctioningZone::Stack
        && p.resolution == SpellResolution::Instructions
        && p.targets.is_empty()
        && p.modes.is_empty()
        && p.additional_sacrifice.is_none()
        && p.optional_additional_costs.is_empty()
        && has_scalar(p)
        && p.instructions.iter().all(|i| {
            matches!(
                i,
                Instruction::Scry { .. }
                    | Instruction::Surveil { .. }
                    | kernel!(CreateTokens {
                        token: kir::TokenSpec::Predefined { .. },
                        ..
                    })
                    | kernel!(Draw {
                        player: you!(),
                        amount: fixed!(_)
                    })
                    | kernel!(GainLife { player: you!(), .. })
            )
        })
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let kind = d.characteristics.kind;
    let actual = d.triggered.iter().filter(|a| trigger(a, kind)).count()
        + d.activated.iter().filter(|a| activation(a)).count();
    if actual == 0
        && ![
            "enters, scry ",
            " Treasure token",
            " Food token",
            " Clue token",
            "enters, surveil ",
            "attacks, scry ",
            "attacks, surveil ",
            ", Sacrifice this ",
            r"\u",
        ]
        .iter()
        .any(|m| d.provenance.source_json.contains(m))
    {
        return Ok(());
    }
    let fail = "requires exact printed scalar trigger or activation";
    let src: Value = strict_read(&d.provenance.source_json)?;
    let text = src["oracle_text"].as_str().ok_or(fail)?;
    let expected: Vec<_> = text
        .lines()
        .enumerate()
        .filter(|(_, line)| candidate(line, kind))
        .collect();
    if actual == 0 && expected.is_empty() {
        return Ok(());
    }
    let ast: Vec<Value> = strict_read(&d.provenance.abilities_json)?;
    if actual != expected.len()
        || ast.len() != text.lines().count()
        || src["layout"] != "normal"
        || src["oracle_id"] != d.oracle_id
        || src["name"] != d.name
        || src["id"] != d.provenance.printing_id
    {
        return Err(fail.into());
    }
    for (i, line) in expected {
        let n = u32::try_from(i).map_err(|_| fail)?;
        let ordinal = ast[..i]
            .iter()
            .filter(|a| a["kind"] == ast[i]["kind"])
            .count();
        let exact = match printed(line, &ast[i], n, kind) {
            Some(BoundAbility::Trigger(a)) => {
                d.triggered.get(ordinal) == Some(&a)
                    && d.source_labels
                        .as_ref()
                        .is_none_or(|l| l.triggered.get(ordinal) == Some(&n))
            }
            Some(BoundAbility::Activation(a)) => {
                // Earlier mana abilities have their own IR list.
                let paid_index = ast[..i]
                    .iter()
                    .filter(|a| {
                        a["kind"] == "activated"
                            && a["effects"][0]["steps"][0]["effect"] != "add-mana"
                    })
                    .count();
                d.activated.get(paid_index) == Some(&a)
                    && d.source_labels
                        .as_ref()
                        .is_none_or(|l| l.activated.get(paid_index) == Some(&n))
            }
            None => false,
        };
        if !exact {
            return Err(fail.into());
        }
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    let has_look = json!({"contains":{"anyOf":[{"properties":{"op":{"enum":["scry","surveil"]}}},instruction_schema::predefined_tokens()]}});
    let inst = json!({"minItems":1,"items":{"anyOf":[{"properties":{"op":{"enum":["scry","surveil"]}}},instruction_schema::predefined_tokens(),instruction_schema::uncounted_draw(),instruction_schema::kernel_verb("gain_life")]}});
    schema["$defs"]["ScalarAbilityProgram"] = json!({"type":"object","required":["instructions","targets"],"properties":{
        "timing":{"const":"instant"},"functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
        "targets":{"maxItems":0},"modes":{"maxItems":0},"additional_sacrifice":{"type":"null"},
        "optional_additional_costs":{"maxItems":0},"instructions":{"allOf":[inst,{"anyOf":[has_look,{
            "minItems":2,"maxItems":2,"prefixItems":[
                instruction_schema::kernel_verb("gain_life"),
                instruction_schema::single_draw()
            ],"items":false}]}]}
    }});
    schema["$defs"]["ScalarAbilityTrigger"] = json!({"required":["condition","program"],"properties":{
        "condition":{"enum":[TriggerCondition::SelfEntersBattlefield,TriggerCondition::SelfDies,TriggerCondition::CreatureAttacked{attacker:AttackSubject::Source{}}]},
        "functioning_zone":{"const":"battlefield"},"intervening_if":{"type":"null"},"death_filter":{"type":"null"},
        "program":{"$ref":SCHEMA}
    }});
    schema["$defs"]["ScalarAbilityActivation"] = json!({"required":["cost","program"],"properties":{
        "cost":{"anyOf":[{"const":"tap_source"},{"type":"object","required":["composite"],"properties":{"composite":{"properties":{"costs":{"properties":{"sacrifice":{"const":true}}}}}}}]},
        "functioning_zone":{"const":"battlefield"},"program":{"$ref":SCHEMA}
    }});
}
