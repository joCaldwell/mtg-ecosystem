//! Paid X announcement, ordered draws and life loss under CR 107.3a,
//! 119.5, 121.2, 601.2b–c/f/h and 608.2c.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn paid_draw_setup() -> Prepared {
    let mut rows = vec![("Sign in Blood", 0, 0, true)];
    lands(&mut rows, "Swamp", 0, 4);
    if let Some(path) = std::env::var_os("MTG_ANNOUNCED_X_DRAW_ARCHIVE_PROBE") {
        let archive = std::fs::read_to_string(path).expect("read frozen archive probe");
        let pact = archive
            .lines()
            .find(|line| {
                mtg_contracts::sha256(line.as_bytes())
                    == "58a8241cd35acc6b584acbaf88ca1feaf7ac7a71aa401bb3c555b84ab372c0d2"
            })
            .expect("pinned complete Damnable Pact row");
        rows[0].0 = "Damnable Pact";
        return prepared_with_sources(&rows, &[pact]);
    }

    // Keep the payment and recovery witness runnable without a local archive.
    let mut prepared = prepared_with_sources(&rows, &[]);
    let catalog = Arc::make_mut(&mut prepared.catalog);
    let mut spell = catalog
        .definitions
        .iter()
        .find(|d| d.name == "Sign in Blood")
        .unwrap()
        .clone();
    spell.name = "Synthetic X Draw and Loss".into();
    spell.oracle_id = "synthetic-x-draw-and-loss".into();
    spell.source_labels = None;
    spell.x_mana_cost = true;
    let recipient = LifeRecipient::Target { target_slot: 0 };
    spell.spell.as_mut().unwrap().instructions = vec![
        Instruction::act(
            Verb::Draw {
                player: recipient.kernel(Context::Spell).unwrap(),
                amount: mtg_contracts::kir::Amount::announced_x(),
            },
            0,
        ),
        Instruction::announced_x_life(recipient.kernel(Context::Spell).unwrap(), false, 0),
    ];
    spell.capabilities = spell.requirements();
    prepared.config.cards[0].oracle_id = spell.oracle_id.clone();
    catalog.definitions.push(spell);
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    prepared.catalog_digest = digest(catalog).unwrap();
    prepared.profile_digest = digest(&prepared.config).unwrap();
    prepared
}

#[test]
fn announced_x_draw_paid_pact_preserves_recipient_amount_and_order() {
    let recipient = PlayerId(1);
    let prepared = paid_draw_setup();
    for x in [0_i64, 2] {
        let mut game = Game::with_trace(prepared.clone(), TraceMode::Detailed);
        let q = 1;
        let library = game.state.rules.libraries[&recipient].clone();
        answer(&mut game, q, Response::Cast { card: CardId(0) });
        answer(&mut game, q, Response::AnnounceX { value: x.into() });
        answer(
            &mut game,
            q,
            Response::PlayerTargets {
                players: vec![recipient],
            },
        );
        fund(&mut game, q, "Swamp", (2 + x) as usize);
        answer(&mut game, q, Response::FinishMana { undo_mana: false });
        answer(
            &mut game,
            q,
            Response::SpendMana {
                spending: BTreeMap::from([(Color::B, (2 + x).into())]),
            },
        );
        assert!(matches!(request(&mut game, q), Request::Priority { .. }));
        let entry = game.top().unwrap();
        assert_eq!(entry.x, Some(x.into()));
        let paid = entry.paid.as_ref().unwrap();
        assert_eq!(paid.generic, x.into());
        assert_eq!(paid.colored, [Color::B, Color::B]);
        assert_eq!(game.mana(PlayerId(0), Color::B), Some(0.into()));

        let mut recovered_draw = false;
        resolve(&mut game, q, |g| {
            if let Frame::Resolve { frame } = &g.state.rules.frame
                && frame
                    .draw_progress
                    .as_ref()
                    .is_some_and(|p| p.completed == 1.into())
                && !recovered_draw
            {
                assert_eq!(g.state.rules.players[1].life, 40.into());
                recover(g);
                recovered_draw = true;
            }
        });
        assert_eq!(recovered_draw, x > 1);
        assert_eq!(
            game.state.rules.libraries[&recipient],
            library[x as usize..]
        );
        assert_eq!(game.action_hand(recipient).len(), x as usize);
        assert_eq!(game.state.rules.players[1].life, (40 - x).into());
        assert_eq!(game.state.rules.players[0].life, 40.into());
        let cursors: Vec<_> = batches(&game)
            .into_iter()
            .filter_map(|batch| match &batch.cause {
                BatchCause::Instruction { cursor, .. } => Some(*cursor),
                _ => None,
            })
            .collect();
        assert_eq!(cursors, if x == 0 { vec![] } else { vec![0, 0, 1] });
        recover(&game);
    }
}
