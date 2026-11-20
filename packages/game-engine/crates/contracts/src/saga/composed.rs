//! Fixed chapter operands. Printed clauses and complete AST remain the authority.
use super::*;

#[derive(Clone, Debug)]
pub enum FixedChapter {
    Token {
        color_words: Vec<String>,
        colors: Vec<Color>,
        subtype: String,
        power: u32,
        toughness: u32,
    },
    Counters {
        amount: u32,
    },
    Trample {
        power: u32,
        toughness: u32,
    },
}

fn word_count(word: &str) -> Option<u32> {
    Some(match word {
        "a" | "one" => 1,
        "two" => 2,
        "three" => 3,
        "four" => 4,
        "five" => 5,
        _ => return None,
    })
}

impl FixedChapter {
    pub(super) fn parse(text: &str) -> Option<Self> {
        if let Some(text) = text
            .strip_prefix("Create a ")
            .and_then(|s| s.strip_suffix(" creature token."))
        {
            let (stats, colors_and_type) = text.split_once(' ')?;
            let (power, toughness) = stats.split_once('/')?;
            let (colors, subtype) = colors_and_type.rsplit_once(' ')?;
            let color_words = colors.split(" and ").map(str::to_owned).collect::<Vec<_>>();
            let colors = color_words
                .iter()
                .map(|word| {
                    Some(match word.as_str() {
                        "white" => Color::W,
                        "blue" => Color::U,
                        "black" => Color::B,
                        "red" => Color::R,
                        "green" => Color::G,
                        _ => return None,
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            if colors.len() > 2
                || subtype.is_empty()
                || !subtype.bytes().all(|c| c.is_ascii_alphabetic())
            {
                return None;
            }
            return Some(Self::Token {
                color_words,
                colors,
                subtype: subtype.into(),
                power: number(power).ok()?,
                toughness: number(toughness).ok()?,
            });
        }
        if let Some(text) = text.strip_prefix("Put ") {
            let (word, tail) = text.split_once(" +1/+1 counter")?;
            let amount = word_count(word)?;
            let expected = if amount == 1 {
                " on target creature you control."
            } else {
                "s on target creature you control."
            };
            return (tail == expected).then_some(Self::Counters { amount });
        }
        let stats = text
            .strip_prefix("Target creature you control gets +")?
            .strip_suffix(" and gains trample until end of turn.")?;
        let (power, toughness) = stats.split_once("/+")?;
        Some(Self::Trample {
            power: number(power).ok()?,
            toughness: number(toughness).ok()?,
        })
    }

    pub(super) fn selection_id(&self, block: u32) -> Option<String> {
        let position = match self {
            Self::Token { .. } => return None,
            Self::Counters { .. } => 11,
            Self::Trample { .. } => 2,
        };
        Some(format!("line:{block}:{position}"))
    }

    pub(super) fn instructions(&self, source_block: u32) -> Vec<Instruction> {
        let quantity = |n| Quantity::from(i64::from(n));
        vec![match self {
            Self::Token {
                colors,
                subtype,
                power,
                toughness,
                ..
            } => Instruction::CreateCreatureTokens {
                token: CreatureToken {
                    additional_types: vec![],
                    colors: colors.clone(),
                    subtypes: vec![subtype.clone()],
                    power: quantity(*power),
                    toughness: quantity(*toughness),
                    keywords: vec![],
                },
                count: 1,
                source_block,
            },
            Self::Counters { amount } => Instruction::PutPlusOneCounters {
                on: CounterRecipient::Target { target_slot: 0 },
                amount: quantity(*amount),
                source_block,
            },
            Self::Trample { power, toughness } => Instruction::GrantKeywords {
                target_slot: 0,
                modifier: Some(CreatureStatModifier {
                    power: quantity(*power),
                    toughness: quantity(*toughness),
                }),
                keywords: vec![KeywordAbility::Trample],
                duration: Duration::UntilEndOfTurn,
                source_block,
            },
        }]
    }

    pub(super) fn steps(&self, block: u32) -> Vec<Value> {
        let fixed = |n: u32| json!({"amount":"fixed","value":n});
        let signed = |n| json!({"sign":1,"amount":fixed(n)});
        let target = || {
            json!({"ref":"target","filter":{"types":["creature"],"control":"you"},
            "count":fixed(1),"selectionId":self.selection_id(block).unwrap()})
        };
        match self {
            Self::Token {
                color_words,
                subtype,
                power,
                toughness,
                ..
            } => vec![json!({
                "effect":"create-token","count":fixed(1),"token":{"colors":color_words,
                    "supertypes":[],"types":["creature"],"subtypes":[subtype],"keywords":[],
                    "power":fixed(*power),"toughness":fixed(*toughness)}
            })],
            Self::Counters { amount } => vec![json!({
                "effect":"put-counters","on":target(),"counter":{"type":"+1/+1"},"count":fixed(*amount)
            })],
            Self::Trample { power, toughness } => vec![
                json!({"effect":"pump","what":target(),"power":signed(*power),"toughness":signed(*toughness),
                    "duration":{"duration":"end-of-turn"}}),
                json!({"effect":"gain-abilities","what":target(),
                    "abilities":[{"gained":"keyword","keyword":{"keyword":"trample"}}],
                    "duration":{"duration":"end-of-turn"}}),
            ],
        }
    }
}
