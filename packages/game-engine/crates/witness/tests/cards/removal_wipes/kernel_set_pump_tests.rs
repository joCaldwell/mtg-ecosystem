//! A kernel end-of-turn pump over a general battlefield filter: the effect is
//! captured, checked and recovered through the same battlefield query destroy
//! and move use. Synthetic typed control, NOT a printed-card claim.
//! CR 608.2h, 611.2c, 613.4c:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use mtg_contracts::kir::{Atom, Pred, SetExpr};

use mtg_witness::temporary::{announce, pass};

#[test]
fn kernel_pump_over_a_general_filter_captures_executes_and_recovers() {
    let mut p = prepared(&[
        ("Infest", true),
        ("Swamp", false),
        ("Swamp", false),
        ("Swamp", false),
        ("Typhoid Rats", false),
        ("Grizzly Bears", false),
    ]);
    let mut c = (*p.catalog).clone();
    let d = c
        .definitions
        .iter_mut()
        .find(|d| d.name == "Infest")
        .unwrap();
    // "Black creatures get +1/+1": a filter no closed object set names.
    let black = SetExpr::battlefield(Pred(vec![
        Atom::Type {
            kind: CardKind::Creature,
        },
        Atom::Color { color: Color::B },
    ]));
    let program = d.spell.as_mut().unwrap();
    let block = program.instructions[0].source_block();
    program.instructions = vec![Instruction::pump(black, 1.into(), 1.into(), block)];
    d.name = "Synthetic typed black-creature pump control".into();
    d.capabilities = d.requirements();
    c.validate().unwrap();
    p.catalog_digest = digest(&c).unwrap();
    p.catalog = Arc::new(c);
    p.profile_digest = digest(&p.config).unwrap();

    let mut g = Game::with_trace(p, TraceMode::Detailed);
    let (rats, bears) = (Driver::object(&g, 4), Driver::object(&g, 5));
    announce(&mut g, 1, 0, vec![]);
    for _ in 0..2000 {
        if g.state.rules.stack.is_empty() && !g.state.rules.effects.is_empty() {
            break;
        }
        let r = Driver::request(&mut g, 1);
        g.respond(r.player, r.id, pass(&r)).unwrap();
    }
    let [effect] = g.state.rules.effects.as_slice() else {
        panic!("one set pump effect")
    };
    let ResolvedEffectKind::PumpSet {
        targets,
        power,
        toughness,
    } = &effect.kind
    else {
        panic!("set pump")
    };
    assert_eq!(targets, &[rats]);
    assert_eq!((power, toughness), (&1.into(), &1.into()));
    let stats = |o| {
        let c = g.characteristics(o).unwrap();
        (c.power.unwrap(), c.toughness.unwrap())
    };
    assert_eq!(stats(rats), (2.into(), 2.into()));
    assert_eq!(stats(bears), (2.into(), 2.into()));
    g.validate_effect_state().unwrap();
    // Snapshot restore and journal replay re-derive the same captured set.
}

/// The source-lowered R4 witness catalog with the rows in play.
fn prepared(rows: &[(&str, bool)]) -> Prepared {
    let c = FixtureCatalog::Witness.get().clone();
    let mut board = Board::new(c, "equipped-indestructible-private").step(TurnStep::PrecombatMain);
    for &(name, hand) in rows {
        board = board.permanent_or_hand(name, 0, 0, hand);
    }
    board.libraries("Forest", 6).prepare()
}
