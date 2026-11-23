//! CR 207.2d, 400.7, 508.3a, 611.2c: an attack trigger modifies its own
//! battlefield incarnation until cleanup. Flavor labels carry no rules meaning.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const BODY: &str = "Whenever this creature attacks, it gets ";
const END: &str = " until end of turn.";
const INVALID: &str = "requires one complete printed fixed self-attack pump";

#[derive(Debug, PartialEq, Eq)]
pub struct SourceAttackPump {
    pub keyword_blocks: Vec<(usize, Vec<KeywordAbility>)>,
    pub block: usize,
    pub ability: TriggeredAbility,
}

fn printed_trigger(line: &str, block: u32) -> Option<(TriggeredAbility, Value)> {
    let (label, body) = match line.split_once(" — ") {
        Some((label, body)) if !label.is_empty() && label.trim() == label => (Some(label), body),
        Some(_) => return None,
        None => (None, line),
    };
    let stats = body
        .strip_prefix(BODY)?
        .strip_suffix(END)?
        .replace('−', "-");
    let (power, toughness) = stats.split_once('/')?;
    let (power, ast_power) = fixed_creature_activation::signed_stat(power)?;
    let (toughness, ast_toughness) = fixed_creature_activation::signed_stat(toughness)?;
    let mut ast = json!({
        "kind":"triggered","trigger":{"trigger":"attacks","what":{"ref":"this","noun":"creature"}},
        "effects":[{"steps":[{"effect":"pump","what":{"ref":"it"},
            "power":ast_power,"toughness":ast_toughness,"duration":{"duration":"end-of-turn"}}]}]
    });
    if let Some(label) = label {
        ast["abilityWord"] = json!(label);
    }
    Some((
        TriggeredAbility {
            death_filter: None,
            functioning_zone: FunctioningZone::Battlefield,
            condition: TriggerCondition::CreatureAttacked {
                attacker: AttackSubject::Source {},
            },
            intervening_if: None,
            program: SpellProgram {
                additional_sacrifice: None,
                optional_additional_costs: vec![],
                timing: SpellTiming::Instant,
                resolution: SpellResolution::Instructions,
                functioning_zone: FunctioningZone::Stack,
                targets: vec![],
                instructions: vec![Instruction::pump(
                    kir::SetExpr::source(),
                    power,
                    toughness,
                    block,
                )],
                modes: vec![],
            },
        },
        ast,
    ))
}

const INTRINSIC: [KeywordAbility; 15] = {
    use KeywordAbility::*;
    [
        Flying,
        Reach,
        Trample,
        Vigilance,
        Defender,
        FirstStrike,
        DoubleStrike,
        Flash,
        Haste,
        Menace,
        Lifelink,
        Deathtouch,
        Hexproof,
        Shroud,
        Indestructible,
    ]
};

/// Bind every printed source line to its complete archived AST. The only
/// permitted siblings are plain, independently supported intrinsic keywords.
pub fn source(text: &str, raw_ast: &str) -> Result<Option<SourceAttackPump>, String> {
    if !text.contains(BODY) {
        return Ok(None);
    }
    let pump = text.lines().enumerate().find_map(|(index, line)| {
        let source_block = u32::try_from(index).ok()?;
        printed_trigger(line, source_block).map(|spec| (index, spec))
    });
    let Some((block, (ability, expected))) = pump else {
        return Ok(None);
    };
    let ast: Vec<Value> = strict_read(raw_ast)?;
    let err = || INVALID.to_string();
    if ast.len() != text.lines().count() || ast[block] != expected {
        return Err(err());
    }
    let mut keyword_blocks = Vec::new();
    let mut seen = Vec::new();
    for (index, (printed, ast)) in text.lines().zip(ast).enumerate() {
        if index == block {
            continue;
        }
        let printed = basic_landwalk::without_reminder(printed).ok_or_else(err)?;
        let mut keywords = Vec::new();
        for word in printed.split([',', ';']) {
            let word = word.trim().to_ascii_lowercase();
            let keyword: KeywordAbility = serde_json::from_value(json!(word)).map_err(|_| err())?;
            if !INTRINSIC.contains(&keyword) || seen.contains(&keyword) {
                return Err(err());
            }
            seen.push(keyword);
            keywords.push(keyword);
        }
        let words: Vec<_> = keywords
            .iter()
            .map(|keyword| json!({"keyword":keyword}))
            .collect();
        if ast != json!({"kind":"keywords","keywords":words}) {
            return Err(err());
        }
        keyword_blocks.push((index, keywords));
    }
    Ok(Some(SourceAttackPump {
        keyword_blocks,
        block,
        ability,
    }))
}
