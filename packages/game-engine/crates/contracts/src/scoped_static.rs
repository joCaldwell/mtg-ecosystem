//! Continuous controller/subtype filters in layers 6 and 7c. CR 205.3m,
//! 208.3, 611.3a/b, 613.1f and 613.4c.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Subtype {
    Elf,
    Goblin,
    Golem,
    Merfolk,
    Sliver,
    Soldier,
    Vampire,
    Warrior,
    Zombie,
}

impl Subtype {
    const ALL: [Self; 9] = [
        Self::Elf,
        Self::Goblin,
        Self::Golem,
        Self::Merfolk,
        Self::Sliver,
        Self::Soldier,
        Self::Vampire,
        Self::Warrior,
        Self::Zombie,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Elf => "Elf",
            Self::Goblin => "Goblin",
            Self::Golem => "Golem",
            Self::Merfolk => "Merfolk",
            Self::Sliver => "Sliver",
            Self::Soldier => "Soldier",
            Self::Vampire => "Vampire",
            Self::Warrior => "Warrior",
            Self::Zombie => "Zombie",
        }
    }

    fn plural(self) -> String {
        match self {
            Self::Elf => "Elves".into(),
            Self::Merfolk => "Merfolk".into(),
            _ => format!("{}s", self.name()),
        }
    }
}

pub fn keyword_rule(keyword: KeywordAbility) -> Option<&'static str> {
    Some(match keyword {
        KeywordAbility::Deathtouch => "702.2",
        KeywordAbility::DoubleStrike => "702.4",
        KeywordAbility::FirstStrike => "702.7",
        KeywordAbility::Flying => "702.9",
        KeywordAbility::Haste => "702.10",
        KeywordAbility::Lifelink => "702.15",
        KeywordAbility::Reach => "702.17",
        KeywordAbility::Trample => "702.19",
        KeywordAbility::Vigilance => "702.20",
        _ => return None,
    })
}

fn additive_stat(text: &str) -> Option<u32> {
    let text = text.strip_prefix('+')?;
    let x = text.parse::<u32>().ok()?;
    (x.to_string() == text).then_some(x)
}

pub fn printed(line: &str) -> Option<(StaticAbility, Value)> {
    let (subject, tail) = line.strip_suffix('.')?.split_once(" you control ")?;
    let (subject, other) = subject
        .strip_prefix("Other ")
        .map_or((subject, false), |s| (s, true));
    let (subtype, creatures_only) = if let Some(named) = subject.strip_suffix(" creatures") {
        (Subtype::ALL.into_iter().find(|s| s.name() == named)?, true)
    } else {
        (
            Subtype::ALL.into_iter().find(|s| s.plural() == subject)?,
            false,
        )
    };
    let (power, toughness, keywords, modifier) = if let Some(name) = tail.strip_prefix("have ") {
        let keyword: KeywordAbility = serde_json::from_value(json!(name)).ok()?;
        keyword_rule(keyword)?;
        (
            0,
            0,
            vec![keyword],
            json!({"modifier":"abilities","abilities":[{"gained":"keyword","keyword":{"keyword":name}}]}),
        )
    } else if subtype == Subtype::Golem {
        // Other already reviewed additive stats retain their existing encoding.
        let (p, t) = tail.strip_prefix("get ")?.split_once('/')?;
        let (p, t) = (additive_stat(p)?, additive_stat(t)?);
        if p == 0 && t == 0 {
            return None;
        }
        let signed = |n| json!({"sign":1,"amount":{"amount":"fixed","value":n}});
        (
            p,
            t,
            vec![],
            json!({"modifier":"pt","power":signed(p),"toughness":signed(t)}),
        )
    } else {
        return None;
    };
    let mut filter = json!({"subtypes":[subtype.name()],"control":"you"});
    if creatures_only {
        filter["types"] = json!(["creature"]);
    }
    if other {
        filter["other"] = json!(true);
    }
    let a = StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::ModifyCreatures {
            scope: CreatureScope::ControlledSubtype {
                subtype,
                creatures_only,
                other,
            },
            power: i64::from(power).into(),
            toughness: i64::from(toughness).into(),
            keywords,
            restrictions: vec![],
        },
    };
    Some((
        a,
        json!({"kind":"static","effect":{"static":"modify","what":{"ref":"each","filter":filter},"modifiers":[modifier]}}),
    ))
}
