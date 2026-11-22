//! Closed ordinary scalar sequencing. CR 109.5, 305.2, 608.2b/c,
//! 701.17, 701.22, 701.25.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(crate) const SCHEMA: &str = "#/$defs/OrdinaryScalarSequence";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Operation {
    Scry,
    Surveil,
    Draw,
    Mill,
    TargetMill,
    Gain,
    Lose,
    Land,
    Destroy,
    Exile,
    Bounce,
    Discard,
}
impl Operation {
    fn is_library(self) -> bool {
        matches!(
            self,
            Self::Scry | Self::Surveil | Self::Mill | Self::TargetMill | Self::Land
        )
    }
}

#[derive(Clone)]
struct Clause {
    operation: Operation,
    count: i64,
    filter: Option<PermanentFilter>,
    archive_filter: Option<Value>,
}

fn fixed(n: i64) -> Value {
    json!({"amount":"fixed","value":n})
}

fn number(word: &str) -> Option<i64> {
    if matches!(word, "a" | "an") {
        return Some(1);
    }
    if let Ok(n) = word.parse::<u32>()
        && n > 0
        && n.to_string() == word
    {
        return Some(n.into());
    }
    (1..100).find(|&n| controller_library::matches_fixed_amount(word, n))
}

fn counted_noun(text: &str, singular: &str, plural: &str) -> Option<i64> {
    let (amount, noun) = text.split_once(' ')?;
    let n = number(amount)?;
    (noun == if n == 1 { singular } else { plural }).then_some(n)
}

fn permanent_filter(text: &str) -> Option<(PermanentFilter, Value)> {
    use CardKind::*;
    let (types, non_types, ast) = match text {
        "creature" => (vec![Creature], vec![], json!({"types":["creature"]})),
        "artifact" => (vec![Artifact], vec![], json!({"types":["artifact"]})),
        "land" => (vec![Land], vec![], json!({"types":["land"]})),
        "enchantment" => (vec![Enchantment], vec![], json!({"types":["enchantment"]})),
        "permanent" => (vec![], vec![], json!({"cls":"permanent"})),
        "nonland permanent" => (
            vec![],
            vec![Land],
            json!({"cls":"permanent","nonTypes":["land"]}),
        ),
        "nonblack creature" => {
            return Some((
                PermanentFilter {
                    types: vec![Creature],
                    non_types: vec![],
                    non_colors: vec![Color::B],
                    opponent: false,
                    tapped: false,
                    power: None,
                },
                json!({"types":["creature"],"nonColors":["black"]}),
            ));
        }
        "artifact or enchantment" => (
            vec![Artifact, Enchantment],
            vec![],
            json!({"allOf":[{"anyOf":[{"types":["artifact"]},{"types":["enchantment"]}]}]}),
        ),
        _ => return None,
    };
    Some((
        PermanentFilter {
            types,
            non_types,
            non_colors: vec![],
            opponent: false,
            tapped: false,
            power: None,
        },
        ast,
    ))
}

fn clause(text: &str) -> Option<Clause> {
    use Operation::*;
    let (operation, count) = if let Some(n) = text
        .strip_prefix("scry ")
        .or_else(|| text.strip_prefix("Scry "))
    {
        (Scry, number(n)?)
    } else if let Some(n) = text
        .strip_prefix("surveil ")
        .or_else(|| text.strip_prefix("Surveil "))
    {
        (Surveil, number(n)?)
    } else if let Some(n) = text
        .strip_prefix("Draw ")
        .or_else(|| text.strip_prefix("draw "))
        .or_else(|| text.strip_prefix("You draw "))
    {
        (Draw, counted_noun(n, "card", "cards")?)
    } else if let Some(n) = text.strip_prefix("Target player mills ") {
        (TargetMill, counted_noun(n, "card", "cards")?)
    } else if let Some(n) = text
        .strip_prefix("Mill ")
        .or_else(|| text.strip_prefix("mill "))
        .or_else(|| text.strip_prefix("You mill "))
    {
        (Mill, counted_noun(n, "card", "cards")?)
    } else if let Some(n) = text
        .strip_prefix("You gain ")
        .or_else(|| text.strip_prefix("gain "))
    {
        (Gain, number(n.strip_suffix(" life")?)?)
    } else if let Some(n) = text
        .strip_prefix("You lose ")
        .or_else(|| text.strip_prefix("lose "))
    {
        (Lose, number(n.strip_suffix(" life")?)?)
    } else if let Some(n) = text
        .strip_prefix("You may play ")
        .or_else(|| text.strip_prefix("you may play "))
    {
        (
            Land,
            counted_noun(
                n.strip_suffix(" this turn")?,
                "additional land",
                "additional lands",
            )?,
        )
    } else if let Some(n) = text
        .strip_prefix("Discard ")
        .or_else(|| text.strip_prefix("discard "))
        .or_else(|| text.strip_prefix("You discard "))
    {
        (Discard, counted_noun(n, "card", "cards")?)
    } else {
        let (op, target) = if let Some(t) = text.strip_prefix("Destroy target ") {
            (Destroy, t)
        } else if let Some(t) = text.strip_prefix("Exile target ") {
            (Exile, t)
        } else {
            (
                Bounce,
                text.strip_prefix("Return target ")?
                    .strip_suffix(" to its owner's hand")?,
            )
        };
        let (filter, archive_filter) = permanent_filter(target)?;
        return Some(Clause {
            operation: op,
            count: 0,
            filter: Some(filter),
            archive_filter: Some(archive_filter),
        });
    };
    Some(Clause {
        operation,
        count,
        filter: None,
        archive_filter: None,
    })
}

fn reminder(text: &str, clauses: &[Vec<Clause>]) -> bool {
    // CR 207.2a. Check only known explanatory forms against their actual
    // operation and amount. Unrecognized parentheticals are unconsumed source.
    clauses.iter().flatten().any(|c| {
        let (verb, tail) = match c.operation {
            Operation::Scry => ("scry", "on the bottom and the rest on top in any order."),
            Operation::Surveil => (
                "surveil",
                "into your graveyard and the rest on top of your library in any order.",
            ),
            _ => return false,
        };
        let Some(text) = text.strip_suffix(')') else {
            return false;
        };
        let text = if let Some(t) = text.strip_prefix("Look at the top ") {
            t
        } else if let Some(t) =
            text.strip_prefix(&format!("To {verb} {}, look at the top ", c.count))
        {
            t
        } else {
            return false;
        };
        if c.count == 1
            && ((c.operation == Operation::Scry
                && text == "card of your library. You may put that card on the bottom.")
                || (c.operation == Operation::Surveil
                    && matches!(
                        text,
                        "card of your library. You may put it into your graveyard."
                            | "card of your library. You may put that card into your graveyard."
                    )))
        {
            return true;
        }
        let Some((n, rest)) =
            text.split_once(" cards of your library, then put any number of them ")
        else {
            return false;
        };
        number(n) == Some(c.count) && rest == tail
    })
}

fn printed(text: &str) -> Option<Vec<Vec<Vec<Clause>>>> {
    let blocks = text
        .lines()
        .map(|line| {
            let (line, parenthetical) = line
                .split_once(" (")
                .map_or((line, None), |(line, r)| (line, Some(r)));
            let effects = line
                .strip_suffix('.')?
                .split(". ")
                .map(|sentence| {
                    sentence
                        .split(", ")
                        .map(|part| clause(part.strip_prefix("then ").unwrap_or(part)))
                        .collect::<Option<Vec<_>>>()
                })
                .collect::<Option<Vec<_>>>()?;
            if parenthetical.is_some_and(|r| !reminder(r, &effects)) {
                return None;
            }
            Some(effects)
        })
        .collect::<Option<Vec<_>>>()?;
    let mut ops = blocks
        .iter()
        .flatten()
        .flatten()
        .map(|c| (c.operation, c.count));
    let first = ops.next()?;
    let rest: Vec<_> = ops.collect();
    if rest.is_empty()
        || !(std::iter::once(&first)
            .chain(&rest)
            .any(|(op, _)| op.is_library())
            || first.0 == Operation::Discard && rest.iter().any(|(op, _)| *op == Operation::Draw))
    {
        return None;
    }
    // Existing source encodings and catalog bytes stay stable.
    if let [second] = rest.as_slice() {
        let old_look = |op| {
            matches!(
                op,
                Operation::Scry | Operation::Surveil | Operation::Mill | Operation::TargetMill
            )
        };
        if (old_look(first.0) && *second == (Operation::Draw, 1))
            || (first == (Operation::Draw, 1) && old_look(second.0))
            || (first.0 == Operation::Draw && second.0 == Operation::Land && blocks.len() == 1)
        {
            return None;
        }
    }
    Some(blocks)
}

pub fn printed_candidate(text: &str) -> bool {
    [
        "scry ",
        "Scry ",
        "surveil ",
        "Surveil ",
        "ill ",
        "ills ",
        "additional land",
        "iscard ",
    ]
    .iter()
    .any(|marker| text.contains(marker))
        && printed(text).is_some()
}

fn selection(step: &Value, block: u32) -> Option<&str> {
    let id = step["selectionId"].as_str()?;
    let prefix = format!("line:{block}:");
    let index = id.strip_prefix(&prefix)?.parse::<u32>().ok()?;
    (id == format!("{prefix}{index}")).then_some(id)
}

fn push_target(
    p: &mut SpellProgram,
    id: &str,
    domain: TargetDomain,
    filter: Option<PermanentFilter>,
) -> Option<()> {
    if !p.targets.is_empty() {
        return None;
    }
    p.targets.push(TargetSpec {
        slot: 0,
        selection_id: id.into(),
        chooser: Chooser::Controller,
        stage: EvaluationStage::Announcement,
        domain,
        filter,
    });
    Some(())
}

fn instruction(
    p: &mut SpellProgram,
    cl: &Clause,
    raw: &Value,
    source_block: u32,
) -> Option<Instruction> {
    use Operation::*;
    let count = Quantity::from(cl.count);
    let amount = fixed(cl.count);
    let you = json!({"player":"you"});
    let pair = match cl.operation {
        Scry => (
            json!({"effect":"scry","amount":amount}),
            Instruction::Scry {
                amount: count,
                source_block,
            },
        ),
        Surveil => (
            json!({"effect":"surveil","amount":amount}),
            Instruction::Surveil {
                amount: count,
                source_block,
            },
        ),
        Draw => (
            json!({"effect":"draw","who":you,"amount":amount}),
            Instruction::draw(PlayerExpr::you(Context::Spell), count, source_block),
        ),
        Mill | TargetMill => {
            let (who, player) = if cl.operation == TargetMill {
                let id = selection(&raw["who"], source_block)?;
                push_target(p, id, TargetDomain::AnyPlayer, None)?;
                (
                    json!({"player":"target-player","selectionId":id}),
                    PlayerExpr::target(0),
                )
            } else {
                (you, PlayerExpr::you(Context::Spell))
            };
            (
                json!({"effect":"mill","who":who,"amount":amount}),
                Instruction::mill(player, Amount::Fixed { value: count }, source_block),
            )
        }
        Gain => (
            json!({"effect":"gain-life","who":you,"amount":amount}),
            Instruction::act(
                Verb::GainLife {
                    player: PlayerExpr::you(Context::Spell),
                    amount: Amount::Fixed { value: count },
                },
                source_block,
            ),
        ),
        Lose => (
            json!({"effect":"lose-life","who":you,"amount":amount}),
            Instruction::act(
                Verb::LoseLife {
                    player: PlayerExpr::you(Context::Spell),
                    amount: Amount::Fixed { value: count },
                },
                source_block,
            ),
        ),
        Land => (
            json!({"effect":"additional-land-plays","allowance":{"who":you,"additional":amount},"duration":{"duration":"this-turn"}}),
            Instruction::GrantLandPlaysThisTurn {
                count,
                source_block,
            },
        ),
        Discard => (
            json!({"effect":"discard","who":you,"what":{"ref":"filter","filter":{"cls":"card"},"count":amount}}),
            Instruction::discard(
                PlayerExpr::you(Context::Spell),
                Amount::Fixed { value: count },
                false,
                source_block,
            ),
        ),
        Destroy | Exile | Bounce => {
            let id = selection(&raw["what"], source_block)?;
            let filtered = cl.filter.clone()?;
            let creature = filtered.types == [CardKind::Creature] && filtered.non_colors.is_empty();
            let domain = if creature && cl.operation != Exile {
                TargetDomain::BattlefieldCreature
            } else {
                TargetDomain::BattlefieldPermanent
            };
            push_target(
                p,
                id,
                domain,
                (domain == TargetDomain::BattlefieldPermanent).then_some(filtered),
            )?;
            let what = json!({"ref":"target","filter":cl.archive_filter,"count":fixed(1),"selectionId":id});
            match cl.operation {
                Destroy => (
                    json!({"effect":"destroy","what":what}),
                    Instruction::destroy_target(0, false, source_block),
                ),
                Exile => (
                    json!({"effect":"exile","what":what}),
                    Instruction::move_target(0, kir::Destination::Exile, source_block),
                ),
                Bounce => (
                    json!({"effect":"return-to-hand","what":what,"to":"owner"}),
                    Instruction::move_target(0, kir::Destination::OwnerHand, source_block),
                ),
                _ => unreachable!(),
            }
        }
    };
    (*raw == pair.0).then_some(pair.1)
}

pub fn lower(text: &str, abilities: &Value, timing: SpellTiming) -> Option<SpellProgram> {
    let printed = printed(text)?;
    let abilities = abilities.as_array()?;
    if printed.len() != abilities.len() {
        return None;
    }
    let mut p = SpellProgram {
        timing,
        functioning_zone: FunctioningZone::Stack,
        resolution: SpellResolution::Instructions,
        optional_additional_costs: vec![],
        additional_sacrifice: None,
        targets: vec![],
        instructions: vec![],
        modes: vec![],
    };
    for (block, (sentences, ability)) in printed.iter().zip(abilities).enumerate() {
        let source_block = u32::try_from(block).ok()?;
        let effects = ability["effects"].as_array()?;
        if ability.as_object()?.len() != 2
            || ability["kind"] != "spell"
            || effects.len() != sentences.len()
        {
            return None;
        }
        for (clauses, sentence) in sentences.iter().zip(effects) {
            let steps = sentence["steps"].as_array()?;
            if sentence.as_object()?.len() != 1 || steps.len() != clauses.len() {
                return None;
            }
            for (cl, step) in clauses.iter().zip(steps) {
                let i = instruction(&mut p, cl, step, source_block)?;
                p.instructions.push(i);
            }
        }
    }
    program(&p).then_some(p)
}

pub(crate) fn program(p: &SpellProgram) -> bool {
    if p.functioning_zone != FunctioningZone::Stack
        || p.resolution != SpellResolution::Instructions
        || !p.modes.is_empty()
        || !p.optional_additional_costs.is_empty()
        || p.additional_sacrifice.is_some()
        || p.instructions.len() < 2
        || p.targets.len() > 1
    {
        return false;
    }
    if controller_scry_program(p)
        || controller_surveil_program(p)
        || fixed_mill_program(p)
        || hand_return::controller_draw_land_allowance_program(p)
    {
        return false;
    }
    let mut library = matches!(
        p.instructions.first(),
        Some(kernel!(Discard {
            player: you!(),
            amount: fixed!(_),
            random: false
        }))
    ) && p.instructions.iter().any(|i| i.fixed_draw().is_some());
    for i in &p.instructions {
        match i {
            Instruction::Scry { .. }
            | Instruction::Surveil { .. }
            | kernel!(Mill {
                amount: fixed!(_),
                ..
            })
            | Instruction::GrantLandPlaysThisTurn { .. } => library = true,
            kernel!(Draw {
                player: you!(),
                amount: fixed!(_)
            })
            | kernel!(GainLife { player: you!(), .. })
            | kernel!(LoseLife { player: you!(), .. })
            | kernel!(Discard {
                player: you!(),
                amount: fixed!(_),
                random: false
            })
            | kernel!(Destroy {
                what: target_object!(_),
                regeneration_forbidden: false
            })
            | kernel!(Move {
                what: target_object!(_),
                to: kir::Destination::Exile
            })
            | kernel!(Move {
                what: target_object!(_),
                to: kir::Destination::OwnerHand
            }) => {}
            _ => return false,
        }
    }
    library
}

pub(crate) fn validate(d: &Definition) -> Result<(), String> {
    let nested = |p: &SpellProgram| p.modes.iter().any(|m| program(&m.program));
    if d.spell.as_ref().is_some_and(nested)
        || d.activated.iter().any(|a| {
            !ability_sequence::activation(a) && (program(&a.program) || nested(&a.program))
        })
        || d.triggered.iter().any(|a| {
            !ability_sequence::trigger(a, d.characteristics.kind)
                && (program(&a.program) || nested(&a.program))
        })
    {
        return Err("ordinary scalar sequences may only be the main spell program".into());
    }
    let uses = d.spell.as_ref().is_some_and(program);
    let raw = &d.provenance.source_json;
    if !uses
        && ![
            "Scry ",
            "Surveil ",
            "scry ",
            "surveil ",
            "ill ",
            "ills ",
            "additional land",
            "iscard ",
            r"\u",
        ]
        .iter()
        .any(|s| raw.contains(s))
    {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let text = source["oracle_text"].as_str().unwrap_or_default();
    if !uses && !printed_candidate(text) {
        return Ok(());
    }
    let fail = "scalar sequence requires the complete ordinary printed and bound source";
    let c = &d.characteristics;
    let (kind, timing) = match c.kind {
        CardKind::Instant => ("Instant", SpellTiming::Instant),
        CardKind::Sorcery => ("Sorcery", SpellTiming::MainPhase),
        _ => return Err(fail.into()),
    };
    if !uses
        || d.spell != lower(text, &strict_read(&d.provenance.abilities_json)?, timing)
        || source["layout"] != "normal"
        || source["type_line"] != kind
        || source["oracle_id"] != d.oracle_id
        || source["name"] != d.name
        || source["id"] != d.provenance.printing_id
        || source["mana_cost"] != c.mana_cost.printed_fixed_cost()
        || source["colors"] != json!(c.colors)
        || source["color_identity"] != json!(c.color_identity)
        || !c.hybrid_mana.is_empty()
        || d.x_mana_cost
        || d.intrinsic_zone.is_some()
        || c.basic
        || c.legendary
        || c.snow
        || c.power.is_some()
        || c.toughness.is_some()
        || !c.additional_types.is_empty()
        || !c.subtypes.is_empty()
        || !d.keywords.is_empty()
        || !d.activated.is_empty()
        || !d.triggered.is_empty()
        || !d.static_abilities.is_empty()
        || !d.mana_abilities.is_empty()
    {
        return Err(fail.into());
    }
    Ok(())
}

pub(crate) fn extend_schema(schema: &mut Value) {
    let op = |name: &str| json!({"required":["op"],"properties":{"op":{"const":name}}});
    let library = json!({"anyOf":[
        {"required":["op"],"properties":{"op":{"enum":["scry","surveil","grant_land_plays_this_turn"]}}},
        instruction_schema::fixed_mill()
    ]});
    let draw = instruction_schema::kernel_verb("draw");
    let has_instruction = |op: &Value| json!({"properties":{"instructions":{"contains":op}}});
    let targets = |domain: &str, filter: Value| {
        let mut item = json!({"properties":{"domain":{"const":domain},"filter":filter}});
        if domain == "battlefield_permanent" {
            item["required"] = json!(["filter"]);
        }
        json!({"properties":{"targets":{
            "minItems":1,"maxItems":1,"items":item
        }}})
    };
    let mill_target = instruction_schema::target_mill(Some(0));
    let exile = instruction_schema::target_move(Some(0), "exile");
    let destroy_or_bounce = json!({"anyOf":[
        instruction_schema::target_destroy(Some(0), Some(false)),
        instruction_schema::target_move(Some(0), "owner_hand")
    ]});
    let needs_target = json!({"anyOf":[mill_target,exile,destroy_or_bounce]});
    let one_targeted_step = json!({"properties":{"instructions":{"contains":needs_target,"minContains":1,"maxContains":1}}});
    schema["$defs"]["OrdinaryScalarSequence"] = json!({
        "type":"object","required":["instructions","targets"],
        "properties":{"functioning_zone":{"const":"stack"},"resolution":{"const":"instructions"},
            "additional_sacrifice":{"type":"null"},"optional_additional_costs":{"maxItems":0},"modes":{"maxItems":0},
            "targets":{"maxItems":1,"items":{"properties":{"slot":{"const":0},"selection_id":{"minLength":1},
                "chooser":{"const":"controller"},"stage":{"const":"announcement"},
                "domain":{"enum":["any_player","battlefield_creature","battlefield_permanent"]}}}},
            "instructions":{"minItems":2,"items":{"anyOf":[
                op("scry"),op("surveil"),instruction_schema::controller_mill(),instruction_schema::target_mill(Some(0)),op("grant_land_plays_this_turn"),
                instruction_schema::any_counted_draw(),
                instruction_schema::target_move(Some(0), "exile"),instruction_schema::target_move(Some(0), "owner_hand"),
                instruction_schema::single_draw(),
                instruction_schema::controller_discard(json!({"type":"string"})),
                instruction_schema::kernel_verb("gain_life"),
                instruction_schema::kernel_verb("lose_life"),
                instruction_schema::target_destroy(Some(0), Some(false))
            ]},"anyOf":[{"contains":library},{"prefixItems":[instruction_schema::controller_discard(json!({"type":"string"}))],"contains":draw}]
            }
        },
        "allOf":[
            {"if":has_instruction(&needs_target),"then":one_targeted_step,"else":{"properties":{"targets":{"maxItems":0}}}},
            {"if":has_instruction(&mill_target),"then":targets("any_player",json!({"type":"null"}))},
            {"if":has_instruction(&exile),"then":targets("battlefield_permanent",json!({"type":"object"}))},
            {"if":has_instruction(&destroy_or_bounce),"then":{"anyOf":[targets("battlefield_creature",json!({"type":"null"})),targets("battlefield_permanent",json!({"type":"object"}))]}},
            {"properties":{"instructions":{"items":{"properties":{
                "target_slot":{"const":0},"amount":{"pattern":"^[1-9][0-9]*$"},"count":{"pattern":"^[1-9][0-9]*$"}
            }}}}}
        ],
        "not":{"anyOf":[{"$ref":"#/$defs/ControllerScryProgram"},{"$ref":"#/$defs/ControllerSurveilProgram"},
            {"$ref":"#/$defs/FixedMillProgram"},{"$ref":"#/$defs/ControllerDrawLandAllowanceProgram"}]}
    });
    let free = "#/$defs/OrdinaryScalarSequenceFreeProgram";
    schema["$defs"]["OrdinaryScalarSequenceFreeProgram"] = json!({
        "not":{"$ref":SCHEMA},
        "properties":{"modes":{"items":{"properties":{"program":{"$ref":free}}}}}
    });
    schema["$defs"]["SpellProgram"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "properties":{"modes":{"items":{"properties":{"program":{"$ref":free}}}}}
        }));
    let capability = |cap| json!({"contains":{"const":cap}});
    let def = schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap();
    def.push(json!({"properties":{
        "activated":{"items":{"properties":{"program":{"$ref":free}}}},
        "triggered":{"items":{"properties":{"program":{"$ref":free}}}}
    }}));
    def.push(json!({"if":{"required":["spell"],"properties":{"spell":{"$ref":SCHEMA}}},
        "then":{"properties":{
            "capabilities":capability(Capability::ComposedOrdinarySpellV1),
            "characteristics":{"properties":{"kind":{"enum":["instant","sorcery"]},"hybrid_mana":{"maxItems":0}}},
            "keywords":{"maxItems":0},"activated":{"maxItems":0},"triggered":{"maxItems":0},
            "mana_abilities":{"maxItems":0},"static_abilities":{"maxItems":0},"x_mana_cost":{"const":false}
        }}
    }));
    for (name, cap) in [
        (
            "grant_land_plays_this_turn",
            Capability::ControllerDrawLandAllowanceV1,
        ),
        ("scry", Capability::ControllerScryV1),
        ("surveil", Capability::ControllerSurveilV1),
    ]
    .map(|(name, cap)| (op(name), cap))
    .into_iter()
    .chain([
        (
            instruction_schema::any_counted_draw(),
            Capability::ControllerCountedDrawV1,
        ),
        (
            instruction_schema::fixed_mill(),
            Capability::FixedMillProgramV1,
        ),
        (
            instruction_schema::kernel_verb("gain_life"),
            Capability::LifeGainV1,
        ),
        (
            instruction_schema::kernel_verb("lose_life"),
            Capability::LifeLossV1,
        ),
    ]) {
        def.push(json!({"if":{"required":["spell"],"properties":{"spell":{"$ref":SCHEMA}}},
            "then":{"if":{"properties":{"spell":{"properties":{"instructions":{"contains":name}}}}},
                "then":{"properties":{"capabilities":capability(cap)}},
                "else":{"properties":{"capabilities":{"not":capability(cap)}}}
            }
        }));
    }
}
