//! Paid modes, targets and X effects under CR 107.3a, 120.8, 601.2b-c/f/h,
//! 608.2b-c and 701.17. Frozen archive d705222b280a71cb.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const INVOKE: CardId = CardId(0);
const RAY: CardId = CardId(1);
const DRAIN: CardId = CardId(2);

fn scenario(names: [&'static str; 3], sources: &[&str]) -> Prepared {
    let mut rows: Vec<_> = names.into_iter().map(|name| (name, 0, 0, true)).collect();
    rows.push(("Grizzly Bears", 1, 1, false));
    lands(&mut rows, "Island", 0, 2);
    lands(&mut rows, "Mountain", 0, 1);
    lands(&mut rows, "Swamp", 0, 1);
    lands(&mut rows, "Forest", 0, 2);
    let mut prepared = prepared_with_sources(&rows, sources);
    // Leave the mill target one library card so the following gain must still use paid X.
    let opponent = PlayerId(1);
    for card in prepared
        .config
        .cards
        .iter_mut()
        .filter(|card| card.zone == (Zone::Library { owner: opponent }))
        .skip(1)
    {
        card.zone = Zone::Graveyard { owner: opponent };
    }
    prepared.profile_digest = digest(&prepared.config).unwrap();
    prepared
}

fn paid_effects_setup() -> Prepared {
    if let Some(path) = std::env::var_os("MTG_ANNOUNCED_X_EFFECTS_ARCHIVE_PROBE") {
        let archive = std::fs::read_to_string(path).expect("read frozen archive probe");
        let sources = [
            "1db96cd11e32a251f2257922a118d4c470ed97a2c30490795e51b61b87bd0e39",
            "5449565d2013894cd3dc2ad411e69ec10c0e45d056afb03b6958defb4af88816",
            "05e435067025ee15684fce7f06c895baebf17661d664aba11fba37113636bec5",
        ]
        .map(|hash| {
            archive
                .lines()
                .find(|line| mtg_contracts::sha256(line.as_bytes()) == hash)
                .expect("pinned complete source row")
        });
        return scenario(
            ["Invoke the Firemind", "Heat Ray", "Psychic Drain"],
            &sources,
        );
    }

    let mut prepared = scenario(["Healing Salve", "Lightning Bolt", "Sign in Blood"], &[]);
    let catalog = Arc::make_mut(&mut prepared.catalog);
    let damage = Instruction::DealDamageAnnouncedX {
        target_slot: 0,
        source_block: 0,
    };
    for (card, colored) in [
        (INVOKE, vec![Color::U, Color::U, Color::R]),
        (RAY, vec![Color::R]),
        (DRAIN, vec![Color::U, Color::B]),
    ] {
        let setup = &mut prepared.config.cards[card.0 as usize];
        let mut spell = catalog
            .definitions
            .iter()
            .find(|d| d.oracle_id == setup.oracle_id)
            .unwrap()
            .clone();
        spell.name = format!("Synthetic X {}", card.0);
        spell.oracle_id = format!("synthetic-x-effect-{}", card.0);
        spell.source_labels = None;
        spell.x_mana_cost = true;
        spell.characteristics.colors = colored
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        spell.characteristics.color_identity = spell.characteristics.colors.clone();
        spell.characteristics.mana_cost.colored = colored;
        let program = spell.spell.as_mut().unwrap();
        match card {
            INVOKE => {
                spell.characteristics.kind = CardKind::Sorcery;
                program.timing = SpellTiming::MainPhase;
                for mode in &mut program.modes {
                    mode.program.timing = SpellTiming::MainPhase;
                }
                let draw = &mut program.modes[0].program;
                draw.targets.clear();
                draw.instructions = vec![Instruction::act(
                    Verb::Draw {
                        player: PlayerExpr::you(Context::Spell),
                        amount: kir::Amount::announced_x(),
                    },
                    0,
                )];
                program.modes[1].program.instructions = vec![damage.clone()];
            }
            RAY => {
                program.targets[0].domain = TargetDomain::BattlefieldCreature;
                program.instructions = vec![damage.clone()];
            }
            DRAIN => {
                program.instructions = vec![
                    Instruction::MillAnnouncedX {
                        target_slot: 0,
                        source_block: 0,
                    },
                    Instruction::announced_x_life(
                        mtg_contracts::kir::PlayerExpr::you(Context::Spell),
                        true,
                        0,
                    ),
                ];
            }
            _ => unreachable!(),
        }
        spell.capabilities = spell.requirements();
        setup.oracle_id = spell.oracle_id.clone();
        catalog.definitions.push(spell);
    }
    catalog
        .definitions
        .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
    catalog.validate().unwrap();
    prepared.catalog_digest = digest(catalog).unwrap();
    prepared.profile_digest = digest(&prepared.config).unwrap();
    prepared
}

#[test]
fn announced_x_effects_paid_modes_damage_creature_and_mill_then_gain() {
    let prepared = paid_effects_setup();
    let caster = PlayerId(0);
    let opponent = PlayerId(1);
    let q = 1;
    for (card, mode, x) in [
        (INVOKE, Some(0), 2_i64),
        (INVOKE, Some(1), 2),
        (INVOKE, Some(1), 0),
        (RAY, None, 1),
        (DRAIN, None, 2),
    ] {
        let mut game = Game::with_trace(prepared.clone(), TraceMode::Detailed);
        let creature = object(&game, 3);
        let library = game.state.rules.libraries.clone();
        answer(&mut game, q, Response::Cast { card });
        if let Some(mode) = mode {
            assert!(
                matches!(request(&mut game, q), Request::Modes { candidates, .. } if candidates == [0,1])
            );
            recover(&game);
            answer(&mut game, q, Response::Modes { mode });
        }
        assert!(matches!(request(&mut game, q), Request::AnnounceX { .. }));
        assert_eq!(game.top().unwrap().mode, mode);
        assert!(game.top().unwrap().x.is_none());
        if mode == Some(1) {
            recover(&game);
        }
        answer(&mut game, q, Response::AnnounceX { value: x.into() });
        match (card, mode) {
            (INVOKE, Some(0)) => (),
            (INVOKE, Some(1)) => answer(
                &mut game,
                q,
                Response::EntityTargets {
                    entities: vec![TargetEntity::Player { player: opponent }],
                },
            ),
            (RAY, _) => {
                assert!(
                    matches!(request(&mut game, q), Request::Targets { candidates, .. } if candidates == [creature])
                );
                answer(
                    &mut game,
                    q,
                    Response::Targets {
                        objects: vec![creature],
                    },
                );
            }
            (DRAIN, _) => answer(
                &mut game,
                q,
                Response::PlayerTargets {
                    players: vec![opponent],
                },
            ),
            _ => unreachable!(),
        }
        let mut spending = BTreeMap::<Color, Quantity>::new();
        let pips = match card {
            INVOKE => &[
                ("Island", Color::U),
                ("Island", Color::U),
                ("Mountain", Color::R),
            ][..],
            RAY => &[("Mountain", Color::R)][..],
            DRAIN => &[("Island", Color::U), ("Swamp", Color::B)][..],
            _ => unreachable!(),
        };
        for &(land, color) in pips {
            fund(&mut game, q, land, 1);
            spending
                .entry(color)
                .or_insert_with(Quantity::zero)
                .add(&1.into());
        }
        if x > 0 {
            fund(&mut game, q, "Forest", x as usize);
            spending.insert(Color::G, x.into());
        }
        answer(&mut game, q, Response::FinishMana { undo_mana: false });
        answer(&mut game, q, Response::SpendMana { spending });
        assert!(matches!(request(&mut game, q), Request::Priority { .. }));
        let entry = game.top().unwrap();
        assert_eq!(entry.x, Some(x.into()));
        assert_eq!(entry.paid.as_ref().unwrap().generic, x.into());

        let mut prepared_damage = false;
        resolve(&mut game, q, |g| {
            if matches!(g.state.rules.frame, Frame::DamageEffects { .. }) && !prepared_damage {
                recover(g);
                prepared_damage = true;
            }
        });
        assert_eq!(prepared_damage, x > 0 && (mode == Some(1) || card == RAY));
        let drawn = if mode == Some(0) { x as usize } else { 0 };
        let milled = usize::from(card == DRAIN);
        assert_eq!(
            game.state.rules.libraries[&caster],
            library[&caster][drawn..]
        );
        assert_eq!(
            game.state.rules.libraries[&opponent],
            library[&opponent][milled..]
        );
        for id in &library[&caster][..drawn] {
            assert_eq!(
                game.state.rules.objects[id].zone,
                Zone::Hand { owner: caster }
            );
        }
        for id in &library[&opponent][..milled] {
            assert_eq!(
                game.state.rules.objects[id].zone,
                Zone::Graveyard { owner: opponent }
            );
        }
        assert_eq!(
            game.state.rules.players[0].life,
            (if card == DRAIN { 40 + x } else { 40 }).into()
        );
        assert_eq!(
            game.state.rules.players[1].life,
            (if mode == Some(1) { 40 - x } else { 40 }).into()
        );
        assert_eq!(
            game.current(creature).unwrap().marked_damage,
            (if card == RAY { x } else { 0 }).into()
        );
        let cursors: Vec<_> = batches(&game)
            .into_iter()
            .filter_map(|batch| match &batch.cause {
                BatchCause::Instruction { cursor, .. } => Some(*cursor),
                _ => None,
            })
            .collect();
        if card == DRAIN {
            assert_eq!(cursors, [0, 1]);
        }
        if x == 0 {
            assert!(cursors.is_empty());
        }
    }
}
