//! Stack-local protection and its supported compositions.
use super::*;
use serde_json::{Value, json};

const TARGET_ID: &str = "line:1:1";

// Preserve the wire capabilities of the existing protected board damage program.
fn legacy_damage_spell(d: &Definition) -> bool {
    d.characteristics.kind == CardKind::Instant
        && d.spell.as_ref().is_some_and(|p| {
            p.targets.is_empty()
                && p.modes.is_empty()
                && matches!(
                    p.instructions.as_slice(),
                    [damage] if damage.set_damage().is_some_and(|d| d.each_player
                        && d.amount.as_fixed().is_some()
                        && d.source_block == 1
                        && d.sets == [&ObjectSet::Creatures.kernel(Context::Spell)])
                )
        })
}

pub(super) fn capability(d: &Definition) -> Capability {
    if legacy_damage_spell(d) {
        Capability::BoardWipesV1
    } else {
        Capability::SelfSpellProtectionV1
    }
}

fn target() -> TargetSpec {
    TargetSpec {
        slot: 0,
        selection_id: TARGET_ID.into(),
        chooser: Chooser::Controller,
        stage: EvaluationStage::Announcement,
        domain: TargetDomain::BattlefieldPermanent,
        filter: Some(PermanentFilter {
            power: None,
            types: vec![],
            non_types: vec![CardKind::Land],
            non_colors: vec![],
            opponent: false,
            tapped: false,
        }),
    }
}
