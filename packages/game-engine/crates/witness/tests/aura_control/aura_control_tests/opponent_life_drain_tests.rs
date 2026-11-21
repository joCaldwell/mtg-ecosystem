//! CR 107.3a, 119.3/119.9, 608.2c/h, 704.4. Frozen archive d705222b280a71cb.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

fn paid_drain_setup() -> Prepared {
    let mut rows = vec![("Sign in Blood", 0, 0, true)];
    lands(&mut rows, "Swamp", 0, 4);
    let mut prepared = if let Some(path) = std::env::var_os("MTG_OPPONENT_LIFE_DRAIN_ARCHIVE_PROBE")
    {
        let archive = std::fs::read_to_string(path).expect("read frozen archive probe");
        let source = archive
            .lines()
            .find(|line| {
                line.contains("Exsanguinate")
                    && mtg_contracts::sha256(line.as_bytes())
                        == "a77306b60e81ef512604cd681331a9cd37ea6d86d89f1ed45915f4aa4e9b5e5c"
            })
            .expect("pinned Exsanguinate row (physical line 19469)");
        rows[0].0 = "Exsanguinate";
        prepared_with_sources(&rows, &[source])
    } else {
        let mut prepared = prepared(&rows);
        let catalog = Arc::make_mut(&mut prepared.catalog);
        let mut spell = catalog
            .definitions
            .iter()
            .find(|d| d.name == "Sign in Blood")
            .unwrap()
            .clone();
        spell.oracle_id = "synthetic-opponent-life-drain".into();
        spell.name = "Synthetic Opponent Life Drain".into();
        spell.source_labels = None;
        spell.x_mana_cost = true;
        let program = spell.spell.as_mut().unwrap();
        program.targets.clear();
        program.instructions = vec![Instruction::DrainOpponents {
            amount: OpponentDrainAmount::AnnouncedX,
            source_block: 0,
        }];
        spell.capabilities = spell.requirements();
        prepared.config.cards[0].oracle_id = spell.oracle_id.clone();
        catalog.definitions.push(spell);
        catalog
            .definitions
            .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        catalog.validate().unwrap();
        prepared.catalog_digest = digest(catalog).unwrap();
        prepared
    };
    prepared.config.initial.life[1] = 1.into();
    prepared.profile_digest = digest(&prepared.config).unwrap();
    prepared
}

#[test]
fn opponent_life_drain_paid_x_uses_simultaneous_actual_losses() {
    let prepared = paid_drain_setup();
    let caster = PlayerId(0);
    for x in [0_i64, 2] {
        let mut game = Game::with_trace(prepared.clone(), TraceMode::Detailed);
        let q = 1;
        answer(&mut game, q, Response::Cast { card: CardId(0) });
        answer(&mut game, q, Response::AnnounceX { value: x.into() });
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
        let cause = BatchCause::Instruction {
            stack_id: entry.id,
            player: caster,
            source: entry.source,
            cursor: 0,
            source_block: 0,
        };
        game.concede(PlayerId(3)).unwrap();
        let mut resolved = false;
        resolve(&mut game, q, |g| {
            if let Frame::Resolve { frame } = &g.state.rules.frame
                && frame.cursor == 1
            {
                assert!(g.is_live(PlayerId(1)));
                assert_eq!(g.state.rules.players[1].life, (1 - x).into());
                assert_eq!(g.state.rules.players[0].life, (40 + 2 * x).into());
                if x > 0 {
                    recover(g);
                }
                resolved = true;
            }
        });
        assert!(resolved);
        assert_eq!(game.is_live(PlayerId(1)), x == 0);
        let changes: Vec<_> = batches(&game)
            .into_iter()
            .filter(|b| matches!(b.cause, BatchCause::Instruction { .. }))
            .collect();
        if x == 0 {
            assert!(changes.is_empty());
            continue;
        }
        let [losses, gain] = changes.as_slice() else {
            panic!("expected loss and gain batches")
        };
        assert_eq!(losses.cause, cause);
        assert_eq!(gain.cause, cause);
        assert_eq!(losses.batch_id + 1, gain.batch_id);
        assert!(matches!(losses.group, EventGroup::Simultaneous { .. }));
        assert_eq!(losses.occurrences.len(), 2);
        for (occurrence, (seat, life)) in losses.occurrences.iter().zip([(1, 1_i64), (2, 40)]) {
            assert_eq!(
                occurrence.actual,
                EventOutcome::LifeLost {
                    player: PlayerId(seat),
                    amount: x.into(),
                    previous: life.into(),
                    current: (life - x).into(),
                }
            );
        }
        assert_eq!(gain.occurrences.len(), 1);
        assert_eq!(
            gain.occurrences[0].actual,
            EventOutcome::LifeGained {
                player: caster,
                amount: 4.into(),
                previous: 40.into(),
                current: 44.into(),
            }
        );
    }
}
