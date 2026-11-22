//! Fixed artifact-set payment, CR 601.2b/c/h, 602.2b, 700.2, 701.21a.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub struct BoundArtifactMode {
    pub ability: ActivatedAbility,
    pub ast: Value,
}

fn positive(s: &str) -> Option<u32> {
    let n = s.parse::<u32>().ok()?;
    (n > 0 && n.to_string() == s).then_some(n)
}

fn count(s: &str) -> Option<u32> {
    positive(s).or_else(|| {
        [
            "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
        ]
        .iter()
        .position(|x| *x == s)
        .map(|i| i as u32 + 2)
    })
}

fn program() -> SpellProgram {
    SpellProgram {
        timing: SpellTiming::Instant,
        resolution: SpellResolution::Instructions,
        functioning_zone: FunctioningZone::Stack,
        targets: vec![],
        instructions: vec![],
        modes: vec![],
        additional_sacrifice: None,
        optional_additional_costs: vec![],
    }
}

fn signed(s: &str) -> Option<(i64, Value)> {
    let (sign, n) = if let Some(n) = s.strip_prefix('+') {
        (1, n)
    } else {
        (-1, s.strip_prefix('-')?)
    };
    let n = positive(n)?;
    Some((
        sign * i64::from(n),
        json!({"sign":sign,"amount":{"amount":"fixed","value":n}}),
    ))
}

impl BoundArtifactMode {
    pub fn parse(text: &str, block: u32, first_line: usize) -> Option<Self> {
        let mut lines = text.lines();
        let header = lines.next()?.strip_suffix(" artifacts: Choose one —")?;
        let (mana, n) = if let Some((prefix, n)) = header.split_once(", Sacrifice ") {
            let symbol = prefix.strip_prefix('{')?.strip_suffix('}')?;
            let n_mana = positive(symbol)?;
            (
                Some((
                    symbol,
                    ManaCost {
                        generic: i64::from(n_mana).into(),
                        colored: vec![],
                    },
                )),
                n,
            )
        } else {
            (None, header.strip_prefix("Sacrifice ")?)
        };
        let n = count(n)?;
        if n < 2 {
            return None;
        }
        let mut costs = vec![];
        if let Some((symbol, _)) = &mana {
            costs.push(json!({"cost":"mana","symbols":[symbol]}));
        }
        costs.push(json!({"cost":"sacrifice","what":{"ref":"filter","filter":{"types":["artifact"]},"count":{"amount":"fixed","value":n}}}));
        let mut root = program();
        let mut options = vec![];
        for (i, line) in lines.enumerate() {
            let text = line.strip_prefix("• ")?;
            let mut child = program();
            let (ast, instruction) = if text == "Draw a card." {
                (
                    json!({"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}),
                    Instruction::draw(PlayerExpr::you(Context::Activated), 1.into(), block),
                )
            } else if let Some(n) = text
                .strip_prefix("You gain ")
                .and_then(|s| s.strip_suffix(" life."))
            {
                let n = positive(n)?;
                (
                    json!({"effect":"gain-life","who":{"player":"you"},"amount":{"amount":"fixed","value":n}}),
                    Instruction::act(
                        Verb::GainLife {
                            player: PlayerExpr::you(Context::Activated),
                            amount: Amount::fixed(i64::from(n)),
                        },
                        block,
                    ),
                )
            } else if let Some(rest) = text.strip_prefix("~ deals ") {
                let (amount, recipient) = rest.split_once(" damage to ")?;
                let n = positive(amount)?;
                let line = first_line.checked_add(i)?.checked_add(1)?;
                let (domain, selection, to) = match recipient {
                    "any target." => {
                        let id = format!("line:{line}:7");
                        (
                            TargetDomain::AnyDamageTarget,
                            id.clone(),
                            json!({"to":"any-target","selectionId":id}),
                        )
                    }
                    "target player or planeswalker." => {
                        let id = format!("line:{line}:6");
                        (
                            TargetDomain::PlayerOrPlaneswalker,
                            id.clone(),
                            json!({"to":"object","ref":{"ref":"target",
                            "filter":{"orPlayer":true,"types":["planeswalker"]},"count":{"amount":"fixed","value":1},"selectionId":id}}),
                        )
                    }
                    _ => return None,
                };
                child.targets.push(TargetSpec {
                    slot: 0,
                    selection_id: selection,
                    domain,
                    chooser: Chooser::Controller,
                    stage: EvaluationStage::Announcement,
                    filter: None,
                });
                (
                    json!({"effect":"damage","source":{"ref":"self"},"amount":{"amount":"fixed","value":n},"to":to}),
                    Instruction::damage_target(
                        0,
                        kir::Amount::Fixed {
                            value: i64::from(n).into(),
                        },
                        block,
                    ),
                )
            } else {
                let (p, t) = text
                    .strip_prefix("Target creature gets ")?
                    .strip_suffix(" until end of turn.")?
                    .split_once('/')?;
                let (power, p) = signed(p)?;
                let (toughness, t) = signed(t)?;
                let selection = format!("line:{}:1", first_line.checked_add(i)?.checked_add(1)?);
                child.targets.push(TargetSpec {
                    slot: 0,
                    selection_id: selection.clone(),
                    domain: TargetDomain::BattlefieldCreature,
                    chooser: Chooser::Controller,
                    stage: EvaluationStage::Announcement,
                    filter: None,
                });
                (
                    json!({"effect":"pump","what":{"ref":"target","filter":{"types":["creature"]},"count":{"amount":"fixed","value":1},"selectionId":selection},
                    "power":p,"toughness":t,"duration":{"duration":"end-of-turn"}}),
                    Instruction::ModifyPowerToughness {
                        target_slot: 0,
                        power: power.into(),
                        toughness: toughness.into(),
                        duration: Duration::UntilEndOfTurn,
                        source_block: block,
                    },
                )
            };
            child.instructions.push(instruction);
            root.modes.push(SpellMode { program: child });
            options.push(json!({"effects":[{"steps":[ast]}]}));
        }
        if options.len() < 2 {
            return None;
        }
        Some(Self {
            ast: json!({"kind":"activated","costs":costs,"effects":[{"steps":[{"effect":"modal","count":{"exactly":1},"options":options}]}]}),
            ability: ActivatedAbility {
                functioning_zone: FunctioningZone::Battlefield,
                program: root,
                cost: ActivationCost::SacrificePermanent {
                    count: n,
                    filter: SacrificeFilter {
                        types: vec![CardKind::Artifact],
                        subtypes: vec![],
                    },
                    mana: mana.map(|(_, m)| m),
                },
            },
        })
    }
}

// Keep exhaustive. Introducing planeswalker characteristics requires explicitly
// classifying them here before a catalog can use player-or-planeswalker damage.
pub(super) fn requires_planeswalker_target_support(kind: CardKind) -> bool {
    match kind {
        CardKind::Creature
        | CardKind::Artifact
        | CardKind::Land
        | CardKind::Enchantment
        | CardKind::Instant
        | CardKind::Sorcery => false,
    }
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    fn narrow(p: &SpellProgram) -> bool {
        p.targets
            .iter()
            .any(|t| t.domain == TargetDomain::PlayerOrPlaneswalker)
            || p.modes.iter().any(|m| narrow(&m.program))
    }
    if d.spell.as_ref().is_some_and(narrow) || d.triggered.iter().any(|a| narrow(&a.program)) {
        return Err("player-or-planeswalker damage requires a bound counted artifact mode".into());
    }
    let uses = |a: &ActivatedAbility| {
        narrow(&a.program)
            || matches!(
                a.cost,
                ActivationCost::SacrificePermanent { count: 2.., .. }
            ) && a.program.modes.iter().any(|m| {
                m.program.instructions.iter().any(|i| {
                    matches!(
                        i,
                        kernel!(GainLife { .. }) | Instruction::ModifyPowerToughness { .. }
                    )
                })
            })
    };
    let raw = &d.provenance.source_json;
    if !d.activated.iter().any(uses)
        && !raw.contains(" artifacts: Choose one")
        && !raw.contains(r"\u")
    {
        return Ok(());
    }
    let source: Value = strict_read(raw)?;
    let text = source["oracle_text"].as_str().unwrap_or("");
    let mut lines = text.lines().enumerate().peekable();
    let mut blocks = vec![];
    while let Some((start, line)) = lines.next() {
        let mut body = line.to_string();
        while lines.peek().is_some_and(|(_, next)| next.starts_with("• ")) {
            body.push('\n');
            body.push_str(lines.next().unwrap().1);
        }
        blocks.push((start, body));
    }
    let error = || "counted artifact modes require their source payment and choices".to_string();
    let mut verified = vec![];
    for (block, (start, body)) in blocks.iter().enumerate() {
        let b = u32::try_from(block).map_err(|_| error())?;
        let mut normalized = Vec::new();
        for line in body.lines() {
            if let Some((actor, rest)) = line
                .strip_prefix("• ")
                .and_then(|s| s.split_once(" deals "))
            {
                if actor == d.name
                    || d.name
                        .split_once(',')
                        .is_some_and(|(short, _)| actor == short)
                {
                    normalized.push(format!("• ~ deals {rest}"));
                    continue;
                }
                // Raw text cannot use the parser-only self placeholder.
                normalized.push(String::new());
            } else {
                normalized.push(line.to_owned());
            }
        }
        let normalized = normalized.join("\n");
        let Some(bound) = BoundArtifactMode::parse(&normalized, b, *start) else {
            continue;
        };
        let ast: Vec<Value> = strict_read(&d.provenance.abilities_json)?;
        if source["name"] != d.name
            || source["id"] != d.provenance.printing_id
            || source["oracle_id"] != d.oracle_id
            || ast.len() != blocks.len()
            || ast[block] != bound.ast
        {
            return Err(error());
        }
        // Mana activations use their own label table. The executed block on
        // every child fixes placement without counting unrelated activations.
        let mut matches = d
            .activated
            .iter()
            .enumerate()
            .filter(|(_, a)| **a == bound.ability);
        let (index, _) = matches.next().ok_or_else(error)?;
        if matches.next().is_some()
            || d.source_labels.as_ref().is_some_and(|l| {
                l.activated.get(index) != Some(&b) || l.blocks.get(block) != Some(&normalized)
            })
        {
            return Err(error());
        }
        verified.push(index);
    }
    if d.activated
        .iter()
        .enumerate()
        .any(|(i, a)| uses(a) && !verified.contains(&i))
    {
        return Err(error());
    }
    Ok(())
}
