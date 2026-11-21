//! Ordinary chapter operands that reuse the existing removal, damage and mana kernels.
use super::*;

#[derive(Clone, Debug)]
pub enum OrdinaryChapter {
    DamageOpponentCreature(u32),
    DestroyOpponentArtifact,
    FixedMana { color: Color, count: u32 },
}

impl OrdinaryChapter {
    pub(super) fn parse(text: &str) -> Option<Self> {
        if let Some(amount) = text
            .strip_prefix("This Saga deals ")
            .and_then(|s| s.strip_suffix(" damage to target creature an opponent controls."))
        {
            return Some(Self::DamageOpponentCreature(number(amount).ok()?));
        }
        if text == "Destroy target artifact an opponent controls." {
            return Some(Self::DestroyOpponentArtifact);
        }
        let mut symbols = text.strip_prefix("Add ")?.strip_suffix('.')?;
        let (symbol, _) = symbols.strip_prefix('{')?.split_once('}')?;
        let color = match symbol {
            "W" => Color::W,
            "U" => Color::U,
            "B" => Color::B,
            "R" => Color::R,
            "G" => Color::G,
            "C" => Color::C,
            _ => return None,
        };
        let unit = format!("{{{symbol}}}");
        let mut count = 0;
        while let Some(rest) = symbols.strip_prefix(&unit) {
            symbols = rest;
            count += 1;
        }
        (symbols.is_empty() && (1..=8).contains(&count)).then_some(Self::FixedMana { color, count })
    }

    fn target(&self, block: u32) -> Option<(CardKind, String)> {
        let (kind, position) = match self {
            Self::DamageOpponentCreature(_) => (CardKind::Creature, 8),
            Self::DestroyOpponentArtifact => (CardKind::Artifact, 3),
            Self::FixedMana { .. } => return None,
        };
        Some((kind, format!("line:{block}:{position}")))
    }

    pub(super) fn step(&self, block: u32) -> Value {
        let target = || {
            let (kind, selection) = self.target(block).unwrap();
            json!({"ref":"target","filter":{"types":[kind],"control":"opponent"},
                "count":{"amount":"fixed","value":1},"selectionId":selection})
        };
        match *self {
            Self::DamageOpponentCreature(amount) => json!({
                "effect":"damage","source":{"ref":"this","noun":"saga"},
                "amount":{"amount":"fixed","value":amount},
                "to":{"to":"object","ref":target()}
            }),
            Self::DestroyOpponentArtifact => json!({"effect":"destroy","what":target()}),
            Self::FixedMana { color, count } => json!({
                "effect":"add-mana","who":{"player":"you"},
                "mana":{"mana":"fixed","symbols":vec![color; count as usize]}
            }),
        }
    }

    pub(super) fn program(&self, source_block: u32) -> SpellProgram {
        let instruction = match *self {
            Self::DamageOpponentCreature(amount) => Instruction::DealDamage {
                target_slot: 0,
                amount: Quantity::from(i64::from(amount)),
                source_block,
            },
            Self::DestroyOpponentArtifact => Instruction::DestroyPermanent {
                target_slot: 0,
                optional: false,
                regeneration_forbidden: false,
                source_block,
            },
            Self::FixedMana { color, count } => Instruction::AddResolutionMana {
                color,
                quantity: ManaResolutionQuantity::Fixed {
                    amount: Quantity::from(i64::from(count)),
                },
                source_block,
            },
        };
        let mut result = program(vec![instruction]);
        if let Some((kind, selection_id)) = self.target(source_block) {
            result.targets.push(TargetSpec {
                slot: 0,
                selection_id,
                domain: TargetDomain::BattlefieldPermanent,
                stage: EvaluationStage::Announcement,
                chooser: Chooser::Controller,
                filter: Some(PermanentFilter {
                    types: vec![kind],
                    opponent: true,
                    non_types: vec![],
                    non_colors: vec![],
                    power: None,
                    tapped: false,
                }),
            });
        }
        result
    }
}

/// Exactly the filtered recipient of a source-validated damage chapter.
pub fn damage_target(target: &TargetSpec) -> bool {
    target.domain == TargetDomain::BattlefieldPermanent
        && target.filter.as_ref().is_some_and(|filter| {
            filter.types == [CardKind::Creature]
                && filter.opponent
                && filter.non_types.is_empty()
                && filter.non_colors.is_empty()
                && filter.power.is_none()
                && !filter.tapped
        })
}

pub(super) fn mana_shape() -> Value {
    json!({"required":["op","quantity"],"properties":{
        "op":{"const":"add_resolution_mana"},
        "quantity":{"required":["fixed"],"properties":{"fixed":{"properties":{
            "amount":{"enum":["1","2","3","4","5","6","7","8"]}
        }}}}
    }})
}

pub(super) fn removal_target(kind: CardKind) -> Value {
    json!({"required":["slot","domain","chooser","stage","selection_id","filter"],"properties":{
        "slot":{"const":0},"domain":{"const":"battlefield_permanent"},
        "selection_id":{"minLength":1},"chooser":{"const":"controller"},"stage":{"const":"announcement"},
        "filter":{"required":["types","opponent"],"properties":{
            "types":{"const":[kind]},"opponent":{"const":true},
            "non_types":{"maxItems":0},"non_colors":{"maxItems":0},
            "power":{"type":"null"},"tapped":{"const":false}
        }}
    }})
}
