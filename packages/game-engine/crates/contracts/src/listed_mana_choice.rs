//! CR 106.1b, 109.5, 605.1a and 605.3b. Choose one listed color during
//! the permanent's mana ability. The tap is the sole activation cost.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

pub(super) fn present(definition: &Definition) -> bool {
    matches!(
        definition.mana_abilities.as_slice(),
        [ManaAbility {
            production: ManaProduction::Choice { .. },
            ..
        }]
    )
}

pub(super) fn validate(definition: &Definition) -> Result<(), String> {
    let [
        ManaAbility {
            production: ManaProduction::Choice { colors },
            costs,
            restriction,
            self_damage,
            ..
        },
    ] = definition.mana_abilities.as_slice()
    else {
        return Ok(());
    };
    if !costs.tap
        || costs.sacrifice
        || costs.mana.is_some()
        || costs.remove_source_plus_one_counters.is_some()
        || restriction.is_some()
        || self_damage.is_some()
        || !(2..=5).contains(&colors.len())
        || colors.contains(&Color::C)
        || colors.iter().collect::<BTreeSet<_>>().len() != colors.len()
    {
        return Err("invalid tap-only listed mana choice".into());
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    let production = json!({
        "properties":{"mana":{"const":"choice"}}
    });
    let capability = json!({"contains":{"const":Capability::ListedManaChoiceV1}});
    schema["$defs"]["Definition"]["allOf"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "if":{"required":["mana_abilities"],"properties":{"mana_abilities":{
                "minItems":1,"maxItems":1,"items":{"properties":{"production":production}}
            }}},
            "then":{"properties":{
                "capabilities":capability,
                "mana_abilities":{"items":{"properties":{
                    "self_damage":{"type":"null"},"restriction":{"type":"null"},
                    "costs":{"properties":{
                        "tap":{"const":true},"sacrifice":{"const":false},
                        "mana":{"type":"null"},"remove_source_plus_one_counters":{"type":"null"}
                    }},
                    "production":{"properties":{"colors":{
                        "minItems":2,"maxItems":5,"uniqueItems":true,
                        "items":{"enum":["W","U","B","R","G"]}
                    }}}
                }}}
            }},
            "else":{"properties":{"capabilities":{"not":capability}}}
        }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let mut definition = super::pain_mana::witness();
    definition.mana_abilities.truncate(1);
    definition.mana_abilities[0].production = ManaProduction::Choice {
        colors: vec![Color::W, Color::U],
    };
    definition.capabilities = definition.requirements();
    definition
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listed_mana_choice_rejects_other_costs_and_repeated_colors() {
        let valid = witness();
        valid.validate().unwrap();

        let mut duplicate = valid.clone();
        duplicate.mana_abilities[0].production = ManaProduction::Choice {
            colors: vec![Color::U, Color::U],
        };
        assert!(duplicate.validate().is_err());

        let mut extra_cost = valid;
        extra_cost.mana_abilities[0].costs.mana = Some(ManaCost {
            generic: 1.into(),
            colored: vec![],
        });
        extra_cost.capabilities = extra_cost.requirements();
        assert!(extra_cost.validate().is_err());
    }
}
