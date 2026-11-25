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
    for (index, label) in labels.iter().enumerate() {
        let path = format!("/effects/0/steps/0/options/{index}");
        let mut covered = segments
            .iter()
            .filter(|(fields, ..)| fields.as_slice() == [path.as_str()]);
        if !covered
            .next()
            .is_some_and(|(_, start, end)| text.get(*start..*end) == Some(*label))
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
/// printed order (CR 700.2d, 608.2c). One selection announces one target at
/// most, and the archive's target occurrences are the modes' in order.
fn declaration(
    modes: Vec<SpellProgram>,
    declared: Printed,
    timing: SpellTiming,
    site: &control::Site<'_>,
) -> Result<ModalSpell, Failure> {
    let mut archive_targets = Vec::new();
    for mode in &modes {
        if mode.targets.len() > 1 || mode.instructions.is_empty() {
            return Err(unsupported_modal("mode_shape"));
        }
        for target in &mode.targets {
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
        let mut selection = SpellProgram::instructions(timing);
        for (index, mode) in modes.iter().enumerate() {
            if mask & (1 << index) != 0 {
                selection.targets.extend_from_slice(&mode.targets);
                selection.instructions.extend_from_slice(&mode.instructions);
            }
        }
        if selection.targets.len() > 1 {
            return Err(unsupported_modal("multiple_targets_in_one_selection"));
        }
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
    let mut modes = Vec::with_capacity(declared.options.len());
    for option in &declared.options {
        let mut mode = SpellProgram::instructions(timing);
        let mut slots = BTreeMap::new();
        append_effects(
            &mut mode,
            parts(option)?,
            site.index,
            &mut slots,
            Context::Spell,
            |typed| validate_damage_sources(typed, DamageSourceBinding::Spell),
        )?;
        modes.push(mode);
    }
    declaration(modes, declared, timing, site)
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
    let modal = declaration(modes, declared, timing, site)?;
    Ok((base, modal))
}

#[cfg(test)]
#[path = "modal_tests.rs"]
mod tests;
