//! Ordered choices for printed two-color hybrid, monocolored hybrid and
//! single-color Phyrexian symbols.
//! CR 107.4e-f, 119.4, 202.3d/f, 601.2b and 601.2f–h.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub enum HybridPayment {
    #[serde(rename = "pay_two_life")]
    PayTwoLife,
    /// CR 107.4e: the `{2}` half of a monocolored hybrid symbol `{2/C}`.
    #[serde(rename = "pay_two_generic")]
    PayTwoGeneric,
    #[serde(untagged)]
    Mana(Color),
}

impl HybridPayment {
    pub fn color(self) -> Option<Color> {
        match self {
            Self::Mana(color) => Some(color),
            Self::PayTwoLife | Self::PayTwoGeneric => None,
        }
    }

    /// CR 601.2f: add this announced payment's mana to a total cost; a
    /// 2-life payment adds none.
    pub fn add_to(self, cost: &mut ManaCost) {
        match self {
            Self::Mana(color) => cost.colored.push(color),
            Self::PayTwoGeneric => cost.generic.add(&Quantity::from(2)),
            Self::PayTwoLife => {}
        }
    }
}

impl ColorHybridSymbol {
    /// CR 202.3f-g: a hybrid symbol counts its largest component and a
    /// Phyrexian symbol counts one.
    pub fn mana_value(&self) -> Quantity {
        Quantity::from(if self.colors.contains(&HybridPayment::PayTwoGeneric) {
            2
        } else {
            1
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ColorHybridSymbol {
    pub colors: [HybridPayment; 2],
}

pub fn color_hybrid_cost(text: &str) -> Option<(ManaCost, Vec<ColorHybridSymbol>)> {
    ordinary_cost(text).filter(|(_, hybrid)| !hybrid.is_empty())
}

pub(super) fn ordinary_cost(text: &str) -> Option<(ManaCost, Vec<ColorHybridSymbol>)> {
    fn color(symbol: &str) -> Option<Color> {
        match symbol {
            "W" => Some(Color::W),
            "U" => Some(Color::U),
            "B" => Some(Color::B),
            "R" => Some(Color::R),
            "G" => Some(Color::G),
            "C" => Some(Color::C),
            _ => None,
        }
    }
    let mut rest = text;
    let mut first = true;
    let mut base = ManaCost {
        generic: Quantity::zero(),
        colored: vec![],
    };
    let mut hybrid = vec![];
    while !rest.is_empty() {
        let (symbol, tail) = rest.strip_prefix('{')?.split_once('}')?;
        if let Some((a, b)) = symbol.split_once('/') {
            // CR 107.4e: `{2/C}` is paid with one mana of its color or {2}.
            if a == "2" {
                let b = color(b).filter(|b| *b != Color::C)?;
                hybrid.push(ColorHybridSymbol {
                    colors: [HybridPayment::Mana(b), HybridPayment::PayTwoGeneric],
                });
                rest = tail;
                first = false;
                continue;
            }
            let a = color(a)?;
            if a == Color::C {
                return None;
            }
            let second = if b == "P" {
                HybridPayment::PayTwoLife
            } else {
                let b = color(b)?;
                if a == b || b == Color::C {
                    return None;
                }
                HybridPayment::Mana(b)
            };
            hybrid.push(ColorHybridSymbol {
                colors: [HybridPayment::Mana(a), second],
            });
        } else if let Some(c) = color(symbol) {
            base.colored.push(c);
        } else if first && !symbol.is_empty() && symbol.bytes().all(|b| b.is_ascii_digit()) {
            base.generic = symbol.parse().ok()?;
            if base.generic.to_string() != symbol {
                return None;
            }
        } else {
            return None;
        }
        rest = tail;
        first = false;
    }
    Some((base, hybrid))
}
