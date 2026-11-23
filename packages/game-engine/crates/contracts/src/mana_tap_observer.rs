//! A stack trigger observes the tap cost of a mana ability (CR 106.12,
//! 603.2–3 and 605.1b). Its damage instruction is not a mana ability.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "filter", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManaTapFilter {
    Land,
    NonbasicLand,
    LandSubtype { subtype: BasicLandType },
}

impl ManaTapFilter {
    pub fn matches(self, characteristics: &Characteristics) -> bool {
        characteristics.has_type(CardKind::Land)
            && match self {
                Self::Land => true,
                Self::NonbasicLand => !characteristics.basic,
                Self::LandSubtype { subtype } => characteristics
                    .subtypes
                    .iter()
                    .any(|s| s == subtype.subtype()),
            }
    }
}

pub fn printed(text: &str) -> Option<(Value, TriggeredAbility)> {
    let (land, effect) = text
        .strip_prefix("Whenever a player taps ")?
        .split_once(" for mana, ")?;
    let damage = effect
        .strip_prefix("this enchantment deals ")?
        .strip_suffix(" damage to that player.")?
        .parse::<u32>()
        .ok()?;
    if damage == 0 {
        return None;
    }
    let (condition, filter) = match land {
        "a land" => (ManaTapFilter::Land, json!({"types":["land"]})),
        "a nonbasic land" => (
            ManaTapFilter::NonbasicLand,
            json!({"nonSupertypes":["basic"],"types":["land"]}),
        ),
        "an Island" => (
            ManaTapFilter::LandSubtype {
                subtype: BasicLandType::Island,
            },
            json!({"subtypes":["Island"]}),
        ),
        _ => return None,
    };
    let fixed = |value| json!({"amount":"fixed","value":value});
    let ast = json!({"kind":"triggered","trigger":{"trigger":"taps-for-mana",
        "what":{"ref":"filter","filter":filter,"count":fixed(1)},"who":{"player":"a-player"}},
        "effects":[{"steps":[{"effect":"damage","source":{"ref":"this","noun":"enchantment"},"amount":fixed(damage),
            "to":{"to":"player","ref":{"player":"that-player"}}}]}]});
    Some((
        ast,
        TriggeredAbility {
            condition: TriggerCondition::ManaTapped { filter: condition },
            functioning_zone: FunctioningZone::Battlefield,
            intervening_if: None,
            death_filter: None,
            program: SpellProgram {
                functioning_zone: FunctioningZone::Stack,
                timing: SpellTiming::Instant,
                resolution: SpellResolution::Instructions,
                optional_additional_costs: vec![],
                additional_sacrifice: None,
                targets: vec![],
                modes: vec![],
                instructions: vec![Instruction::damage(
                    kir::DamageTo::Ref {
                        who: kir::Ref::EventPlayer {
                            role: kir::EventPlayerRole::ActivatingPlayer,
                        },
                    },
                    kir::Amount::Fixed {
                        value: i64::from(damage).into(),
                    },
                    0,
                )],
            },
        },
    ))
}

fn uses(program: &SpellProgram) -> bool {
    program.instructions.iter().any(|i| {
        matches!(
            i,
            kernel!(Damage {
                to: kir::DamageTo::Ref {
                    who: kir::Ref::EventPlayer {
                        role: kir::EventPlayerRole::ActivatingPlayer
                    }
                },
                amount: fixed!(_),
                ..
            })
        )
    }) || program.modes.iter().any(|m| uses(&m.program))
}
