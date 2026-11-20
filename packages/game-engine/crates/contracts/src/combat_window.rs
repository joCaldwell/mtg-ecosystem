//! Targeted combat restrictions through cleanup. CR 509.1b, 611.2c, 613.11 and 514.2.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub fn restriction(text: &str) -> Option<(CreatureRestriction, &'static str)> {
    Some(match text {
        "Target creature can't be blocked this turn." => {
            (CreatureRestriction::CantBeBlocked, "be-blocked")
        }
        "Target creature can't block this turn." => (CreatureRestriction::CantBlock, "block"),
        _ => return None,
    })
}

pub fn split_activation(line: &str) -> Option<(&str, u32)> {
    let (cost, body) = line.split_once(": ")?;
    let n = cost
        .strip_prefix('{')?
        .strip_suffix("}, {T}")?
        .parse::<u32>()
        .ok()?;
    (cost == format!("{{{n}}}, {{T}}") && n > 0 && restriction(body).is_some()).then_some((body, n))
}

fn target_step(
    line: &str,
    ast: &Value,
    block: u32,
    offset: u32,
) -> Option<(TargetSpec, Value, Instruction)> {
    let (restriction, action) = restriction(line)?;
    let selection_id = format!("line:{block}:{offset}");
    let step = json!({"effect":"cant","what":{"ref":"target","filter":{"types":["creature"]},
        "count":{"amount":"fixed","value":1},"selectionId":selection_id},"action":action,"duration":{"duration":"this-turn"}});
    if *ast != step {
        return None;
    }
    let target = TargetSpec {
        slot: 0,
        selection_id,
        chooser: Chooser::Controller,
        stage: EvaluationStage::Announcement,
        domain: TargetDomain::BattlefieldCreature,
        filter: None,
    };
    Some((
        target,
        step,
        Instruction::GrantCreatureRestriction {
            target_slot: 0,
            restriction,
            source_block: block,
        },
    ))
}

fn program(kind: CardKind, target: TargetSpec, instructions: Vec<Instruction>) -> SpellProgram {
    SpellProgram {
        functioning_zone: FunctioningZone::Stack,
        resolution: SpellResolution::Instructions,
        timing: if kind == CardKind::Sorcery {
            SpellTiming::MainPhase
        } else {
            SpellTiming::Instant
        },
        targets: vec![target],
        modes: vec![],
        optional_additional_costs: vec![],
        additional_sacrifice: None,
        instructions,
    }
}

pub fn spell(text: &str, ast: &Value, kind: CardKind) -> Option<SpellProgram> {
    if !matches!(kind, CardKind::Instant | CardKind::Sorcery) {
        return None;
    }
    let mut lines = text.lines();
    let (target, step, inst) = target_step(lines.next()?, &ast[0]["effects"][0]["steps"][0], 0, 0)?;
    let mut blocks = vec![json!({"kind":"spell","effects":[{"steps":[step]}]})];
    let mut instructions = vec![inst];
    if let Some(line) = lines.next() {
        if line != "Draw a card." || lines.next().is_some() {
            return None;
        }
        blocks.push(json!({"kind":"spell","effects":[{"steps":[{
            "effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}
        }]}]}));
        instructions.push(Instruction::DrawCards {
            recipient: None,
            count: 1.into(),
            source_block: 1,
        });
    }
    (*ast == json!(blocks)).then_some(program(kind, target, instructions))
}

pub fn activation(line: &str, ast: &Value, block: u32) -> Option<ActivatedAbility> {
    let (body, n) = split_activation(line)?;
    let (target, step, inst) = target_step(body, &ast["effects"][0]["steps"][0], block, 4)?;
    let expected = json!({"kind":"activated","costs":[{"cost":"mana","symbols":[n.to_string()]},{"cost":"tap-self"}],
        "effects":[{"steps":[step]}]});
    (*ast == expected).then_some(ActivatedAbility {
        functioning_zone: FunctioningZone::Battlefield,
        cost: ActivationCost::Composite {
            costs: PermanentActivationCosts {
                mana: Some(ManaCost {
                    generic: i64::from(n).into(),
                    colored: vec![],
                }),
                tap: true,
                sacrifice: false,
                remove_source_plus_one_counters: None,
            },
        },
        program: program(CardKind::Instant, target, vec![inst]),
    })
}

fn uses(p: &SpellProgram) -> bool {
    activation_program(p) || p.modes.iter().any(|m| uses(&m.program))
}

pub(super) fn activation_program(p: &SpellProgram) -> bool {
    p.instructions
        .iter()
        .any(|i| matches!(i, Instruction::GrantCreatureRestriction { .. }))
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    let err = "requires exact printed temporary creature restriction";
    let has_spell = d.spell.as_ref().is_some_and(uses);
    let found = d.activated.iter().filter(|a| uses(&a.program)).count();
    if d.triggered.iter().any(|a|uses(&a.program)) || d.static_abilities.iter().any(|a| {
        matches!(&a.effect,StaticEffect::Bestow{program,..}|StaticEffect::GrantSpellCascade{program,..} if uses(program))
    }) { return Err(err.into()); }
    let raw = &d.provenance.source_json;
    if !has_spell && found == 0 && !raw.contains(" this turn.") && !raw.contains(r"\u") {
        return Ok(());
    }
    let row: Value = strict_read(raw)?;
    let text = row["oracle_text"].as_str().unwrap_or_default();
    let kind = d.characteristics.kind;
    if !has_spell
        && found == 0
        && !(matches!(kind, CardKind::Instant | CardKind::Sorcery)
            && text
                .lines()
                .next()
                .is_some_and(|l| restriction(l).is_some()))
        && !text.lines().any(|l| split_activation(l).is_some())
    {
        return Ok(());
    }
    let ast: Value = strict_read(&d.provenance.abilities_json)?;
    let expected = spell(text, &ast, kind);
    let mut printed = text.lines();
    let printed_spell = matches!(kind, CardKind::Instant | CardKind::Sorcery)
        && printed
            .next()
            .is_some_and(|line| restriction(line).is_some())
        && matches!(printed.next(), None | Some("Draw a card."))
        && printed.next().is_none();
    if has_spell || printed_spell {
        if expected.is_none()
            || d.spell != expected
            || found != 0
            || !d.activated.is_empty()
            || !d.triggered.is_empty()
            || !d.mana_abilities.is_empty()
            || !d.static_abilities.is_empty()
            || !d.keywords.is_empty()
            || d.intrinsic_zone.is_some()
            || d.x_mana_cost
            || row["type_line"]
                != if kind == CardKind::Instant {
                    "Instant"
                } else {
                    "Sorcery"
                }
            || !d.characteristics.hybrid_mana.is_empty()
            || row["mana_cost"] != d.characteristics.mana_cost.printed_fixed_cost()
            || row["colors"] != json!(d.characteristics.colors)
            || row["color_identity"] != json!(d.characteristics.color_identity)
        {
            return Err(err.into());
        }
    }
    let blocks = ast.as_array().ok_or(err)?;
    if blocks.len() != text.lines().count() {
        return Err(err.into());
    }
    let mut count = 0;
    for (block, line) in text.lines().enumerate() {
        if split_activation(line).is_none() {
            continue;
        }
        if kind != CardKind::Land {
            return Err(err.into());
        }
        let expected = activation(
            line,
            blocks.get(block).ok_or(err)?,
            u32::try_from(block).map_err(|_| err)?,
        )
        .ok_or(err)?;
        if d.activated.get(count) != Some(&expected)
            || d.source_labels
                .as_ref()
                .is_some_and(|l| l.activated.get(count) != Some(&(block as u32)))
        {
            return Err(err.into());
        }
        count += 1;
    }
    if count > 0 {
        let mut mana = vec![];
        for (index, line) in text.lines().enumerate() {
            if split_activation(line).is_some() {
                continue;
            }
            let value = line
                .strip_prefix("{T}: Add {")
                .and_then(|s| s.strip_suffix("}."))
                .ok_or(err)?;
            let color: Color = strict_read(&canonical(&json!(value))?)?;
            if *blocks.get(index).ok_or(err)?
                != json!({"kind":"activated","costs":[{"cost":"tap-self"}],"effects":[{"steps":[{
                    "effect":"add-mana","who":{"player":"you"},"mana":{"mana":"fixed","symbols":[color]}
                }]}]})
            {
                return Err(err.into());
            }
            mana.push(ManaAbility {
                costs: PermanentActivationCosts {
                    mana: None,
                    tap: true,
                    sacrifice: false,
                    remove_source_plus_one_counters: None,
                },
                production: ManaProduction::Fixed {
                    symbols: vec![color],
                },
                source_block: u32::try_from(index).map_err(|_| err)?,
                restriction: None,
                self_damage: None,
            });
        }
        if mana.len() != 1
            || d.mana_abilities != mana
            || !d.triggered.is_empty()
            || !d.static_abilities.is_empty()
            || !d.keywords.is_empty()
        {
            return Err(err.into());
        }
    }
    if count != found
        || (count > 0 && (count != d.activated.len() || row["type_line"] != "Land"))
        || row["layout"] != "normal"
        || row["name"] != d.name
        || row["oracle_id"] != d.oracle_id
        || row["id"] != d.provenance.printing_id
    {
        return Err(err.into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    let opcode =
        json!({"required":["op"],"properties":{"op":{"const":"grant_creature_restriction"}}});
    let free = json!({"$ref":"#/$defs/CombatRestrictionFreeProgram"});
    schema["$defs"]["CombatRestrictionFreeProgram"] = json!({"properties":{"instructions":{"items":{"not":opcode}},
        "modes":{"items":{"properties":{"program":free}}}}});
    let uses = json!({"required":["program"],"properties":{"program":{"not":free}}});
    let cap = json!({"contains":{"const":Capability::TemporaryCombatRestrictionV1}});
    schema["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{"triggered":{"items":{"properties":{"program":free}}}},
        "if":{"anyOf":[{"required":["spell"],"properties":{"spell":{"type":"object","not":free}}},
            {"required":["activated"],"properties":{"activated":{"contains":uses}}}]},
        "then":{"properties":{"capabilities":cap}},
        "else":{"properties":{"capabilities":{"not":cap}}}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let row: Value = strict_read(include_str!(
        "../../../tests/fixtures/081764b4-ef96-44aa-836c-05353efd215c.json"
    ))
    .unwrap();
    let mut d = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-effects-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.characteristics.kind == CardKind::Instant)
    .unwrap();
    let ast = composition_fixtures::stamp(&mut d, &row);
    d.characteristics.colors = vec![Color::U];
    d.characteristics.color_identity = vec![Color::U];
    d.characteristics.mana_cost = ManaCost {
        generic: Quantity::zero(),
        colored: vec![Color::U],
    };
    d.spell = spell(
        row["source"]["oracle_text"].as_str().unwrap(),
        &ast,
        CardKind::Instant,
    );
    d.capabilities = d.requirements();
    d
}
