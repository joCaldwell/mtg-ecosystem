//! CR 601.2b–c, 700.2a–d, 608.2c: choose distinct printed modes before
//! announcing targets. Each choice runs its modes in their printed order.
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

pub(super) fn lower(
    effects: &[Value],
    timing: SpellTiming,
    text: &str,
    site: &control::Site<'_>,
) -> Result<ModalSpell, Failure> {
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
    let (expected, min, max) = match lines.next() {
        Some("Choose one —") => (json!({"exactly": 1}), 1, 1),
        Some("Choose two —") => (json!({"exactly": 2}), 2, 2),
        Some("Choose one or both —") if len == 2 => (json!({"min": 1, "max": 2}), 1, 2),
        Some("Choose one or more —") => (json!({"atLeast": 1}), 1, len as u32),
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

    let mut modes = Vec::with_capacity(len);
    let mut archive_targets = Vec::new();
    for option in options {
        let mut mode = SpellProgram::instructions(timing);
        let mut slots = BTreeMap::new();
        append_effects(
            &mut mode,
            parts(&option.effects)?,
            site.index,
            &mut slots,
            Context::Spell,
            |typed| validate_damage_sources(typed, DamageSourceBinding::Spell),
        )?;
        if mode.targets.len() > 1 || mode.instructions.is_empty() {
            return Err(unsupported_modal("mode_shape"));
        }
        for target in &mode.targets {
            archive_targets.push(TargetSpec {
                slot: archive_targets.len() as u32,
                ..target.clone()
            });
        }
        modes.push(mode);
    }
    site.resolution
        .target_slots(site.index, site.printed, &archive_targets)?;

    let mut selections = BTreeMap::new();
    for mask in ModalSpell::masks(len, min, max) {
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
        min,
        max,
        labels: labels.into_iter().map(str::to_owned).collect(),
        selections,
    })
}

#[cfg(test)]
#[path = "modal_tests.rs"]
mod tests;
