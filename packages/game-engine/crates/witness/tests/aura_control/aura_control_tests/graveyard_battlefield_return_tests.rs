//! CR 400.7, 603.3/6a, 608.2b/c and 704.4.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

fn setup() -> Prepared {
    let mut rows = vec![
        ("Raise Dead", 0, 0, true),
        ("Elvish Visionary", 0, 0, false),
        ("Grizzly Bears", 1, 1, false),
    ];
    lands(&mut rows, "Plains", 0, 5);
    let mut prepared = if let Some(path) =
        std::env::var_os("MTG_GRAVEYARD_BATTLEFIELD_RETURN_ARCHIVE_PROBE")
    {
        let probe = std::fs::read_to_string(path).expect("read frozen archive probe");
        let row = probe
            .lines()
            .find(|line| {
                mtg_contracts::sha256(line.as_bytes())
                    == "e88c54cbd14928a7bad58ee3a85fc66042674eddb1e36ccfd35a5f3e68cbcce6"
            })
            .expect("complete Emerge from the Cocoon row 38542");
        rows[0].0 = "Emerge from the Cocoon";
        prepared_with_sources(&rows, &[row])
    } else {
        let mut prepared = prepared(&rows);
        let catalog = Arc::make_mut(&mut prepared.catalog);
        let spell = catalog
            .definitions
            .iter_mut()
            .find(|d| d.name == "Raise Dead")
            .unwrap();
        spell.name = "Synthetic graveyard creature return".into();
        spell.oracle_id = "synthetic-graveyard-battlefield-return".into();
        spell.source_labels = None;
        spell.characteristics.mana_cost = ManaCost {
            generic: 4.into(),
            colored: vec![Color::W],
        };
        spell.characteristics.colors = vec![Color::W];
        spell.characteristics.color_identity = vec![Color::W];
        let mut source: Value = serde_json::from_str(&spell.provenance.source_json).unwrap();
        source["name"] = json!(spell.name);
        source["oracle_id"] = json!(spell.oracle_id);
        source["mana_cost"] = json!("{4}{W}");
        source["oracle_text"] = json!(
            "Return target creature card from your graveyard to the battlefield. You gain 3 life."
        );
        let target = json!({
            "ref": "target",
            "filter": {"types": ["creature"], "cls": "card",
                       "zone": {"zone": "graveyard", "owner": "your"}},
            "count": {"amount": "fixed", "value": 1},
            "selectionId": "line:0:1"
        });
        let abilities = json!([{"kind": "spell", "effects": [
            {"steps": [{"effect": "move-zone", "what": target,
                        "to": {"zone": "battlefield"}}]},
            {"steps": [{"effect": "gain-life", "who": {"player": "you"},
                        "amount": {"amount": "fixed", "value": 3}}]}
        ]}]);
        spell.provenance.source_json = source.to_string();
        spell.provenance.source_sha256 =
            mtg_contracts::sha256(spell.provenance.source_json.as_bytes());
        spell.provenance.abilities_json = abilities.to_string();
        spell.provenance.ast_sha256 = mtg_contracts::sha256(format!("[{abilities}]").as_bytes());
        spell.spell.as_mut().unwrap().instructions = vec![
            Instruction::ReturnOwnGraveyardCreatureToBattlefield {
                target_slot: 0,
                source_block: 0,
            },
            Instruction::act(
                Verb::GainLife {
                    player: PlayerExpr::you(Context::Spell),
                    amount: Amount::fixed(3),
                },
                0,
            ),
        ];
        spell.capabilities = spell.requirements();
        prepared.config.cards[0].oracle_id = spell.oracle_id.clone();
        catalog
            .definitions
            .sort_by(|a, b| a.oracle_id.cmp(&b.oracle_id));
        catalog.validate().unwrap();
        prepared.catalog_digest = digest(catalog).unwrap();
        prepared
    };
    for card in &mut prepared.config.cards[1..=2] {
        card.zone = Zone::Graveyard { owner: card.owner };
    }
    prepared.profile_digest = digest(&prepared.config).unwrap();
    prepared
}

#[test]
fn graveyard_battlefield_return_paid_entry_then_life_before_trigger_resolution() {
    let mut game = Game::with_trace(setup(), TraceMode::Detailed);
    let caster = PlayerId(0);
    let graveyard = Zone::Graveyard { owner: caster };
    let old = object(&game, 1);
    let draw = game.state.rules.libraries[&caster][0];
    answer(&mut game, 1, Response::Cast { card: CardId(0) });
    let Request::Targets {
        player, candidates, ..
    } = request(&mut game, 1)
    else {
        panic!("caster chooses a graveyard creature");
    };
    assert_eq!(player, caster);
    assert_eq!(candidates, [old]);
    answer(&mut game, 1, Response::Targets { objects: vec![old] });
    fund(&mut game, 1, "Plains", 5);
    answer(&mut game, 1, Response::FinishMana { undo_mana: false });
    answer(
        &mut game,
        1,
        Response::SpendMana {
            spending: BTreeMap::from([(Color::W, 5.into())]),
        },
    );
    assert!(matches!(request(&mut game, 1), Request::Priority { .. }));
    let spell = game.top().unwrap();
    assert_eq!(
        spell.paid.as_ref(),
        Some(&ManaCost {
            generic: 4.into(),
            colored: vec![Color::W]
        })
    );
    let spell_id = spell.id;
    let mut saw_move = false;
    resolve(&mut game, 1, |g| {
        if let Frame::Resolve { frame } = &g.state.rules.frame
            && frame.stack_id == spell_id
            && frame.cursor == 1
            && !saw_move
        {
            let [InstructionOutcome::Moved { movement }] = frame.outcomes.as_slice() else {
                panic!("return recorded its actual move");
            };
            let entrant = g.current(object(g, 1)).unwrap();
            assert_eq!(movement.previous, old);
            assert_eq!(movement.successor, entrant.id);
            assert_eq!(movement.from, graveyard);
            assert_eq!(movement.destination, Zone::Battlefield);
            assert!(g.current(old).is_err());
            assert_eq!(entrant.controller, caster);
            assert!(!entrant.tapped);
            let [trigger] = g.state.rules.pending_triggers.as_slice() else {
                panic!("Visionary's self-entry trigger is waiting");
            };
            assert_eq!(trigger.source, entrant.id);
            assert_eq!(g.top().unwrap().id, spell_id);
            assert_eq!(g.state.rules.players[0].life, 40.into());
            recover(g);
            saw_move = true;
        }
    });
    assert!(saw_move);
    assert_eq!(game.state.rules.players[0].life, 43.into());
    assert!(
        game.state.rules.players[1..]
            .iter()
            .all(|p| p.life == 40.into())
    );
    assert_eq!(game.zone(object(&game, 0)), Some(graveyard));
    assert_eq!(
        game.zone(game.object_id(draw).unwrap()),
        Some(Zone::Hand { owner: caster })
    );
    assert_eq!(
        game.events()
            .iter()
            .filter_map(|event| match event {
                Event::SpellCast { stack_id } => Some(*stack_id),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [spell_id]
    );
}
