//! CR 714.2b-c: every printed Roman numeral denotes its own triggered ability.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(super) fn chapters(
    card: &Card<'_>,
    numbers: Vec<i64>,
    word: &Option<String>,
    effects: &[serde_json::Value],
    text: &str,
    site: &control::Site<'_>,
) -> Result<Vec<TriggeredAbility>, Failure> {
    if card.kind != CardKind::Enchantment || card.subtypes != ["Saga"] {
        return Err(unsupported("compose.block", "chapter:source"));
    }
    const ROMAN: [&str; 10] = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];
    let mismatch = || unsupported("compose.block", "chapter:printed_number_mismatch");
    let (header, body) = text.split_once(" — ").ok_or_else(mismatch)?;
    let printed = header
        .split(", ")
        .map(|n| {
            ROMAN
                .iter()
                .position(|s| s.eq_ignore_ascii_case(n))
                .map(|i| (i + 1) as i64)
                .ok_or_else(mismatch)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if printed != numbers || printed.windows(2).any(|w| w[0] >= w[1]) {
        return Err(mismatch());
    }
    ability_word(word, body)?;
    if modal::declared(effects) {
        return Err(unsupported("compose.block", "verb:modal:chapter"));
    }

    let peeled = control::peel(effects)?;
    let lower = |sentences: &[serde_json::Value]| {
        let mut program = SpellProgram::instructions(SpellTiming::Instant);
        append_effects(
            &mut program,
            parts(sentences)?,
            site.index,
            &mut BTreeMap::new(),
            Context::Triggered,
            |typed| {
                validate_damage_sources(
                    typed,
                    DamageSourceBinding::Invocation {
                        first_it_is_self: false,
                    },
                )
            },
            PlayerBinding::Unbound,
        )?;
        Ok::<_, Failure>(program)
    };
    let mut program = lower(&peeled.sentences)?;
    if program.instructions.is_empty() {
        return Err(unsupported("compose.block", "chapter:empty_body"));
    }
    control::apply(&peeled, site, &mut program.instructions, 0, |prefix| {
        Ok(lower(prefix)?.instructions.len())
    })?;
    control::admitted(
        &peeled,
        &program.instructions,
        &control_env(Context::Triggered, &program.targets, None),
    )?;

    Ok(numbers
        .into_iter()
        .map(|number| TriggeredAbility {
            functioning_zone: FunctioningZone::Battlefield,
            condition: TriggerCondition::on(EventPattern::SagaChapter {
                number: number as u32,
            }),
            program: program.clone(),
            modal: None,
        })
        .collect())
}
