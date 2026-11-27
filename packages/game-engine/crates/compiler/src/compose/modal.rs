//! CR 601.2b–c, 602.2b, 603.3c, 700.2a–d, 608.2c: choose distinct printed
//! modes before announcing targets. Each choice runs its modes in their
//! printed order. Spells, activated and triggered abilities share one
//! declaration; each mode lowers through its block's ordinary lowerer.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OptionBody {
    label: Option<String>,
    effects: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Declaration {
    effect: String,
    count: Value,
    options: Vec<OptionBody>,
}

fn unsupported_modal(detail: &str) -> Failure {
    unsupported("effects.steps", format!("verb:modal:{detail}"))
}

pub(super) fn declared(effects: &[Value]) -> bool {
    effects.iter().any(|sentence| {
        sentence["steps"]
            .as_array()
            .is_some_and(|steps| steps.iter().any(|s| s["effect"] == "modal"))
    })
}

/// CR 605.1a: an untargeted activated ability that could add mana is a mana
/// ability, which never uses the stack. A modal activation with a mode that
/// adds mana is outside the modal and mana tables alike.
pub(super) fn reject_mana_modes(effects: &[Value]) -> Result<(), Failure> {
    fn adds_mana(value: &Value) -> bool {
        match value {
            Value::Object(map) => {
                map.get("effect").and_then(Value::as_str) == Some("add-mana")
                    || map.values().any(adds_mana)
            }
            Value::Array(items) => items.iter().any(adds_mana),
            _ => false,
        }
    }
    if effects.iter().any(adds_mana) {
        return Err(unsupported_modal("mana_ability"));
    }
    Ok(())
}

/// Where the printed "choose" phrase starts: a spell's whole first line, a
/// triggered ability's first line after its trigger condition (", choose")
/// or an activated ability's after its cost (": Choose").
#[derive(Clone, Copy)]
pub(super) enum Header {
    Spell,
    Triggered,
    Activated,
}

/// The printed declaration: its count, its labels and each option's effects,
/// with every option certified as exactly its printed bullet.
struct Printed {
    min: u32,
    max: u32,
    labels: Vec<String>,
    options: Vec<Vec<Value>>,
}

fn printed(
    effects: &[Value],
    text: &str,
    header: Header,
    site: &control::Site<'_>,
) -> Result<Printed, Failure> {
    let [sentence] = effects else {
        return Err(unsupported_modal("siblings"));
    };
    let Some(steps) = sentence["steps"]
        .as_array()
        .filter(|_| sentence.as_object().is_some_and(|s| s.len() == 1))
    else {
        return Err(unsupported_modal("sentence"));
    };
    let [step] = steps.as_slice() else {
        return Err(unsupported_modal("siblings"));
    };
    let Declaration {
        effect,
        count,
        options,
    } = serde_json::from_value(step.clone()).map_err(|_| unsupported_modal("declaration"))?;
    let len = options.len();
    if effect != "modal" || !(2..=ModalSpell::MAX_MODES).contains(&len) {
        return Err(unsupported_modal("option_count"));
    }
    let mut lines = text.lines();
    let first = lines.next().unwrap_or_default();
    // CR 603.3c, 602.2b: an ability's header precedes the same phrase.
    let phrase = match header {
        Header::Spell => Some(first),
        Header::Triggered => first
            .split_once(", choose ")
            .filter(|(condition, _)| !condition.is_empty())
            .map(|(_, rest)| rest),
        Header::Activated => first
            .split_once(": Choose ")
            .filter(|(cost, _)| !cost.is_empty())
            .map(|(_, rest)| rest),
    };
    let phrase = match (header, phrase) {
        (Header::Spell, Some(p)) => p.strip_prefix("Choose "),
        (_, p) => p,
    };
    let (expected, min, max) = match phrase {
        Some("one —") => (json!({"exactly": 1}), 1, 1),
        Some("up to one —") => (json!({"min": 0, "max": 1}), 0, 1),
        Some("any number —") => (json!({"atLeast": 0}), 0, len as u32),
        Some("two —") => (json!({"exactly": 2}), 2, 2),
        Some("one or both —") if len == 2 => (json!({"min": 1, "max": 2}), 1, 2),
        Some("one or more —") => (json!({"atLeast": 1}), 1, len as u32),
        _ => return Err(unsupported_modal("printed_count")),
    };
    if count != expected {
        return Err(unsupported_modal("count_disagrees_with_print"));
    }
    let labels = lines
        .map(|line| line.strip_prefix("• ").filter(|label| !label.is_empty()))
        .collect::<Option<Vec<_>>>()
        .filter(|labels| labels.len() == len)
        .ok_or_else(|| unsupported_modal("printed_options"))?;
    let segments = site.resolution.segments(site.index)?;
    for (index, (bullet, option)) in labels.iter().zip(&options).enumerate() {
        // CR 207.2c–d: ability words and flavor words have no rules meaning.
        // Check the archive label against the same certified printed bullet.
        if let Some(label) = &option.label
            && !bullet.split_once(" — ").is_some_and(|(prefix, body)| {
                prefix == label && !prefix.trim().is_empty() && !body.trim().is_empty()
            })
        {
            return Err(unsupported_modal("printed_option_label"));
        }
        let path = format!("/effects/0/steps/0/options/{index}");
        let mut covered = segments
            .iter()
            .filter(|(fields, ..)| fields.as_slice() == [path.as_str()]);
        if !covered
            .next()
            .is_some_and(|(_, start, end)| text.get(*start..*end) == Some(*bullet))
            || covered.next().is_some()
        {
            return Err(unsupported_modal("uncertified_option"));
        }
    }
    Ok(Printed {
        min,
        max,
        labels: labels.into_iter().map(str::to_owned).collect(),
        options: options.into_iter().map(|o| o.effects).collect(),
    })
}

/// Each legal set of distinct modes, its modes' targets and instructions in
/// printed order (CR 700.2d, 608.2c). The archive numbers occurrences
/// across all modes. Executable selections number only their chosen targets.
fn declaration(
    modes: Vec<SpellProgram>,
    declared: Printed,
    timing: SpellTiming,
    site: &control::Site<'_>,
    combine: impl Fn(u8, &Printed) -> Result<SpellProgram, Failure>,
) -> Result<ModalSpell, Failure> {
    let mut archive_targets = Vec::new();
    for mode in &modes {
        if mode.instructions.is_empty() {
            return Err(unsupported_modal("mode_shape"));
        }
        // The archive has one occurrence for a counted group (CR 115.3).
        let occurrences = if mode.target_group().is_some() {
            &mode.targets[..1]
        } else {
            &mode.targets
        };
        for target in occurrences {
            archive_targets.push(TargetSpec {
                slot: archive_targets.len() as u32,
                ..target.clone()
            });
        }
    }
    site.resolution
        .target_slots(site.index, site.printed, &archive_targets)?;
    let mut selections = BTreeMap::new();
    for mask in ModalSpell::masks(modes.len(), declared.min, declared.max) {
        let chosen = || {
            modes
                .iter()
                .enumerate()
                .filter(|(index, _)| mask & (1 << index) != 0)
        };
        let selection = if chosen()
            .filter(|(_, mode)| !mode.targets.is_empty())
            .count()
            > 1
        {
            if chosen().any(|(_, mode)| {
                mode.optional_target
                    || mode.target_group().is_some()
                    || mode.variable_target_count().is_some()
            }) {
                return Err(unsupported_modal("mixed_target_groups"));
            }
            combine(mask, &declared)?
        } else {
            let mut selection = SpellProgram::instructions(timing);
            for (_, mode) in chosen() {
                selection.targets.extend_from_slice(&mode.targets);
                selection.instructions.extend_from_slice(&mode.instructions);
                selection.optional_target |= mode.optional_target;
            }
            selection
        };
        selections.insert(mask, selection);
    }
    Ok(ModalSpell {
        source_block: source_block(site.index)?,
        min: declared.min,
        max: declared.max,
        labels: declared.labels,
        selections,
    })
}

pub(super) fn lower(
    effects: &[Value],
    timing: SpellTiming,
    text: &str,
    site: &control::Site<'_>,
) -> Result<ModalSpell, Failure> {
    let declared = printed(effects, text, Header::Spell, site)?;
    let append = |mode: &mut SpellProgram, option: &[Value]| {
        append_effects(
            mode,
            parts(option)?,
            site.index,
            &mut BTreeMap::new(),
            Context::Spell,
            |typed| validate_damage_sources(typed, DamageSourceBinding::Spell),
            PlayerBinding::Unbound,
        )
    };
    let mut modes = Vec::with_capacity(declared.options.len());
    for option in &declared.options {
        let mut mode = SpellProgram::instructions(timing);
        append(&mut mode, option)?;
        modes.push(mode);
    }
    declaration(modes, declared, timing, site, |mask, declared| {
        let mut selection = SpellProgram::instructions(timing);
        for (index, option) in declared.options.iter().enumerate() {
            if mask & (1 << index) != 0 {
                append(&mut selection, option)?;
            }
        }
        Ok(selection)
    })
}

/// A modal activated or triggered ability. Every mode lowers as a complete
/// ability of the same block through `lower_mode`; the modes must agree on
/// everything but their programs (event, intervening condition, cost, zone
/// and restriction), which the returned base ability keeps with an empty
/// program of the same timing.
pub(super) fn lower_ability<A: Clone + PartialEq>(
    effects: &[Value],
    text: &str,
    header: Header,
    site: &control::Site<'_>,
    mut lower_mode: impl FnMut(&[Value]) -> Result<A, Failure>,
    program: impl Fn(&mut A) -> &mut SpellProgram,
) -> Result<(A, ModalSpell), Failure> {
    let declared = printed(effects, text, header, site)?;
    let mut base: Option<A> = None;
    let mut modes = Vec::with_capacity(declared.options.len());
    for option in &declared.options {
        let mut ability = lower_mode(option)?;
        let mode = std::mem::replace(
            program(&mut ability),
            SpellProgram::instructions(SpellTiming::Instant),
        );
        *program(&mut ability) = SpellProgram::instructions(mode.timing);
        match &base {
            None => base = Some(ability),
            Some(first) if *first == ability => {}
            Some(_) => return Err(unsupported_modal("mode_dependent_ability")),
        }
        modes.push(mode);
    }
    let mut base = base.ok_or_else(|| unsupported_modal("option_count"))?;
    let timing = program(&mut base).timing;
    if modes.iter().any(|mode| {
        mode.timing != timing
            || mode.resolution != SpellResolution::Instructions
            || mode.functioning_zone != FunctioningZone::Stack
            || mode.optional_target
            || mode.additional_sacrifice.is_some()
            || !mode.optional_additional_costs.is_empty()
    }) {
        return Err(unsupported_modal("mode_program_shape"));
    }
    let modal = declaration(modes, declared, timing, site, |_, _| {
        Err(unsupported_modal("multiple_targets_in_ability_selection"))
    })?;
    Ok((base, modal))
}

#[cfg(test)]
#[path = "modal_tests.rs"]
mod tests;
