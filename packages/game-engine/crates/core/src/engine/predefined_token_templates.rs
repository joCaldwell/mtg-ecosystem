//! Functional artifact templates use the normal paid activation machinery.
//! CR 111.10a–b,f, 602.2 and 605.3.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::PredefinedToken;
use mtg_contracts::kir::{Amount, Context, PlayerExpr, Verb};

pub(super) fn definition(source: &Definition, id: CardId, kind: PredefinedToken) -> Definition {
    let mut d = source.clone();
    d.oracle_id = format!("token:{}", id.0);
    d.name = format!("{} Token", kind.name());
    d.characteristics = Characteristics {
        kind: CardKind::Artifact,
        hybrid_mana: vec![],
        additional_types: vec![],
        subtypes: vec![kind.name().into()],
        basic: false,
        legendary: false,
        snow: false,
        mana_cost: ManaCost {
            generic: Quantity::zero(),
            colored: vec![],
        },
        colors: vec![],
        color_identity: vec![],
        power: None,
        toughness: None,
    };
    d.intrinsic_zone = None;
    d.spell = None;
    d.activated.clear();
    d.mana_abilities.clear();
    d.triggered.clear();
    d.static_abilities.clear();
    d.keywords.clear();
    d.capabilities.clear();
    d.source_labels = None;
    d.x_mana_cost = false;
    d.flashback = None;
    let treasure = kind == PredefinedToken::Treasure;
    let costs = PermanentActivationCosts {
        remove_source_plus_one_counters: None,
        tap: kind != PredefinedToken::Clue,
        mana: (!treasure).then(|| ManaCost {
            generic: 2.into(),
            colored: vec![],
        }),
        sacrifice: true,
        pay_life: None,
    };
    if treasure {
        d.mana_abilities.push(ManaAbility {
            costs,
            production: ManaProduction::AnyColor,
            riders: vec![],
            source_block: 0,
            restriction: None,
        });
    } else {
        let instruction = match kind {
            PredefinedToken::Clue => {
                Instruction::draw(PlayerExpr::you(Context::Activated), 1.into(), 0)
            }
            PredefinedToken::Food => Instruction::act(
                Verb::GainLife {
                    player: PlayerExpr::you(Context::Activated),
                    amount: Amount::fixed(3),
                },
                0,
            ),
            PredefinedToken::Treasure => unreachable!(),
        };
        d.activated.push(ActivatedAbility {
            functioning_zone: FunctioningZone::Battlefield,
            cost: ActivationCost::Composite { costs },
            program: SpellProgram {
                timing: SpellTiming::Instant,
                functioning_zone: FunctioningZone::Stack,
                resolution: SpellResolution::Instructions,
                optional_additional_costs: vec![],
                additional_sacrifice: None,
                targets: vec![],
                instructions: vec![instruction],
            },
        });
    }
    d
}

pub(super) fn can_create(source: &Definition, kind: PredefinedToken) -> bool {
    let program = |p: &SpellProgram| {
        p.instructions
            .iter()
            .any(|i| matches!(i,kernel!(CreateTokens { token: mtg_contracts::kir::TokenSpec::Predefined { kind: k }, .. }) if *k==kind))
    };
    // CR 111.10: a resolving spell may be the creating recipe as well.
    source.triggered.iter().any(|a| program(&a.program))
        || source.activated.iter().any(|a| program(&a.program))
        || source.spell.as_ref().is_some_and(program)
}
