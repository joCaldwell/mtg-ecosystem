//! Predefined artifact tokens, CR 111.10a–b,f.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PredefinedToken {
    Treasure,
    Food,
    Clue,
}
impl PredefinedToken {
    const ALL: [Self; 3] = [Self::Treasure, Self::Food, Self::Clue];
    pub fn name(self) -> &'static str {
        match self {
            Self::Treasure => "Treasure",
            Self::Food => "Food",
            Self::Clue => "Clue",
        }
    }
    fn reminder(self) -> &'static str {
        match self {
            Self::Treasure => {
                r#"It's an artifact with "{T}, Sacrifice this token: Add one mana of any color.""#
            }
            Self::Food => {
                r#"It's an artifact with "{2}, {T}, Sacrifice this token: You gain 3 life.""#
            }
            Self::Clue => r#"It's an artifact with "{2}, Sacrifice this token: Draw a card.""#,
        }
    }
    pub fn appears(text: &str) -> bool {
        Self::ALL
            .into_iter()
            .any(|k| text.contains(&format!(" {} token", k.name())))
    }
}

pub fn printed(
    clause: &str,
    context: kir::Context,
    source_block: u32,
) -> Option<(Value, Instruction)> {
    let clause = clause
        .strip_prefix("Create ")
        .or_else(|| clause.strip_prefix("create "))?;
    let (count, object) = clause.split_once(' ')?;
    let count = equipped_trigger::count(count)?;
    let noun = object.strip_suffix(if count == 1 { " token" } else { " tokens" })?;
    let kind = PredefinedToken::ALL
        .into_iter()
        .find(|k| k.name() == noun)?;
    Some((
        json!({"effect":"create-token","count":{"amount":"fixed","value":count},"token":{"predefined":kind.name()}}),
        Instruction::create_tokens(
            context,
            count,
            kir::TokenSpec::Predefined { kind },
            source_block,
        ),
    ))
}

pub(crate) fn reminder(text: &str, instructions: &[Instruction]) -> bool {
    instructions.iter().any(|i| {
        matches!(i,kernel!(CreateTokens { token: kir::TokenSpec::Predefined { kind }, .. })
        if text.strip_suffix(')')==Some(kind.reminder()))
    })
}

fn has_tokens(p: &SpellProgram) -> bool {
    p.instructions.iter().any(|i| {
        matches!(
            i,
            kernel!(CreateTokens {
                token: kir::TokenSpec::Predefined { .. },
                ..
            })
        )
    }) || p.modes.iter().any(|m| has_tokens(&m.program))
}

pub(super) fn validate(d: &Definition) -> Result<(), String> {
    if d.spell.as_ref().is_some_and(has_tokens)
        || d.activated.iter().any(|a|has_tokens(&a.program)&&!ability_sequence::activation(a))
        || d.triggered.iter().any(|a|has_tokens(&a.program)&&!ability_sequence::trigger(a,d.characteristics.kind)&&!equipped_trigger::is_equipped(a))
        || d.static_abilities.iter().any(|a|matches!(&a.effect,StaticEffect::Bestow{program,..}|StaticEffect::GrantSpellCascade{program,..} if has_tokens(program)))
    {
        Err("predefined tokens require an exact supported ability".into())
    } else { Ok(()) }
}

pub(super) fn extend_schema(s: &mut Value) {
    let op = instruction_schema::predefined_tokens();
    let free = "#/$defs/PredefinedTokenFreeProgram";
    s["$defs"]["PredefinedTokenFreeProgram"] = json!({"properties":{
        "instructions":{"items":{"not":op}},"modes":{"items":{"properties":{"program":{"$ref":free}}}}
    }});
    let on_trigger = json!({"properties":{"triggered":{"contains":{"required":["program"],"properties":{"program":{"not":{"$ref":free}}}}}},"required":["triggered"]});
    let on_activation = json!({"properties":{"activated":{"contains":{"required":["program"],"properties":{"program":{"not":{"$ref":free}}}}}},"required":["activated"]});
    let checked = |ability: Value| json!({"if":{"required":["program"],"properties":{"program":{"not":{"$ref":free}}}},"then":ability});
    let cap = json!({"contains":{"const":Capability::PredefinedArtifactTokensV1}});
    s["$defs"]["Definition"]["allOf"].as_array_mut().unwrap().push(json!({
        "properties":{"spell":{"$ref":free},
            "activated":{"items":checked(json!({"$ref":"#/$defs/ScalarAbilityActivation"}))},
            "triggered":{"items":checked(json!({"anyOf":[{"$ref":"#/$defs/ScalarAbilityTrigger"},{"properties":{
                "condition":{"enum":[TriggerCondition::EquippedCombatDamage,TriggerCondition::EquippedDies]}
            }}]}))},
            "static_abilities":{"items":{"properties":{"effect":{"properties":{
            "bestow":{"properties":{"program":{"$ref":free}}},"grant_spell_cascade":{"properties":{"program":{"$ref":free}}}
        }}}}}},
        "if":{"anyOf":[on_trigger,on_activation]},"then":{"properties":{"capabilities":cap}},
        "else":{"properties":{"capabilities":{"not":cap}}}
    }));
}

#[cfg(test)]
pub(super) fn witness() -> Definition {
    let row: Value = strict_read(include_str!(
        "../../../tests/fixtures/c4efae03-99de-4439-925b-504602623bfd.json"
    ))
    .unwrap();
    let source = &row["source"];
    let ast = &row["faces"][0]["result"]["abilities"];
    let mut d = Catalog::read(include_str!(
        "../../../tests/fixtures/expected-wipes-ir.json"
    ))
    .unwrap()
    .definitions
    .into_iter()
    .find(|d| d.name == "Grizzly Bears")
    .unwrap();
    d.name = source["name"].as_str().unwrap().into();
    d.oracle_id = source["oracle_id"].as_str().unwrap().into();
    d.provenance.printing_id = source["id"].as_str().unwrap().into();
    d.provenance.source_index = row["sourceIndex"].as_u64().unwrap().try_into().unwrap();
    d.provenance.source_json = canonical(source).unwrap();
    d.provenance.source_sha256 = sha256(d.provenance.source_json.as_bytes());
    d.provenance.abilities_json = canonical(ast).unwrap();
    d.provenance.ast_sha256 = sha256(format!("[{}]", d.provenance.abilities_json).as_bytes());
    d.provenance.binding_sha256 = row["bindingSha256"].as_str().unwrap().into();
    d.characteristics.subtypes = vec!["Human".into(), "Pirate".into()];
    d.characteristics.mana_cost = ManaCost {
        generic: 2.into(),
        colored: vec![Color::U],
    };
    d.characteristics.colors = vec![Color::U];
    d.characteristics.color_identity = vec![Color::U];
    d.characteristics.power = Some(1.into());
    d.characteristics.toughness = Some(4.into());
    d.source_labels = None;
    d.triggered = vec![
        ability_sequence::printed(
            row["faces"][0]["result"]["lines"][0]["text"]
                .as_str()
                .unwrap(),
            &ast[0],
            0,
            CardKind::Creature,
        )
        .and_then(|a| match a {
            ability_sequence::BoundAbility::Trigger(a) => Some(a),
            _ => None,
        })
        .unwrap(),
    ];
    d.capabilities = d.requirements();
    d
}

#[cfg(test)]
#[test]
fn slice4_capability_predefined_tokens_has_a_complete_witness() {
    let d = witness();
    assert!(
        d.capabilities
            .contains(&Capability::PredefinedArtifactTokensV1)
    );
    d.validate().unwrap();
}
