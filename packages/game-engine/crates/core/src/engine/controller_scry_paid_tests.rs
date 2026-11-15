//! Private complete-source paid witnesses; no pool/consumer admission.
//! Archive SHA256 d705222b280a71cb5a5362f657d530b22402ca51687bac3227c1d6db403d0282.
//! Physical records / source indexes: Loran 4059/4057, Serum 13007/13005,
//! Opt 17039/17037, Preordain 25951/25949. Exact record digests below.
//! CR SHA256 8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca.
//! CR20260925 701.22a-d, 109.5, 400.7, 608.2b/c, 800.4a:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

const LORAN: &str =
    include_str!("../../../../tests/fixtures/1a9048d6-f3fa-4f3b-9baa-35318177bc6e.json");
const SERUM: &str =
    include_str!("../../../../tests/fixtures/56956afd-db53-4542-816b-490c8b0bbcf7.json");
const OPT: &str =
    include_str!("../../../../tests/fixtures/713332c1-5bd8-400f-bfff-c1ca0697a043.json");
const PREORDAIN: &str =
    include_str!("../../../../tests/fixtures/ac641490-ca14-48d7-8cc4-b69ce984befa.json");
const MYR: &str =
    include_str!("../../../../tests/fixtures/66e8f7f8-3a6d-46ba-837c-b9713ddf7f40.json");
const SEAT: &str =
    include_str!("../../../../tests/fixtures/39451b4d-cd7a-40da-b457-cb51b609173f.json");

fn prepared(rows: &[Row]) -> Prepared {
    let mut p = prepared_with_sources(rows, &[LORAN, SERUM, OPT, PREORDAIN, MYR, SEAT]);
    // Configure diverse, deliberately non-physical library order before creation.
    // There is no synthetic program, paid receipt or runtime history injection.
    let names = [
        "Island",
        "Grizzly Bears",
        "Mountain",
        "Bonesplitter",
        "Plains",
        "Forest",
    ];
    for seat in 0..4 {
        let cards: Vec<_> = p
            .config
            .cards
            .iter_mut()
            .filter(|c| {
                c.zone
                    == (Zone::Library {
                        owner: PlayerId(seat),
                    })
            })
            .collect();
        for (i, c) in cards.into_iter().enumerate() {
            c.oracle_id = p
                .catalog
                .definitions
                .iter()
                .find(|d| d.name == names[i % names.len()])
                .unwrap()
                .oracle_id
                .clone();
        }
    }
    let mut libraries = p.config.initial_libraries();
    for library in libraries.values_mut() {
        library.rotate_left(3);
        library.reverse();
    }
    p.config.initial.libraries = Some(libraries);
    p.profile_digest = digest(&p.config).unwrap();
    p
}

fn library(g: &Game, seat: u8) -> Vec<ObjectId> {
    g.state.rules.libraries[&PlayerId(seat)]
        .iter()
        .map(|id| g.object_id(*id).unwrap())
        .collect()
}

fn announce_paid(g: &mut Game, q: u64, id: u32, targets: Vec<TargetEntity>) {
    let caster = g.state.rules.cards[&CardId(id)].owner;
    until_player(g, q, caster);
    let cost = g
        .definition(CardId(id))
        .unwrap()
        .characteristics
        .mana_cost
        .clone();
    // Stabilize each accepted activation before reading pool/taps again.
    loop {
        let r = request(g, q);
        assert_eq!(r.player(), caster);
        if g.payment_pool(caster, &cost).is_ok() {
            break;
        }
        let color = cost
            .colored
            .iter()
            .copied()
            .find(|c| {
                let required = cost.colored.iter().filter(|x| *x == c).count();
                g.state.rules.players[caster.0 as usize]
                    .mana
                    .get(c)
                    .cloned()
                    .unwrap_or_default()
                    < Quantity::from(required as i64)
            })
            .unwrap_or_else(|| *cost.colored.last().unwrap());
        let name = match color {
            Color::W => "Plains",
            Color::U => "Island",
            Color::G => "Forest",
            _ => panic!("witness mana color"),
        };
        let source = g
            .state
            .rules
            .objects
            .values()
            .find(|o| {
                o.controller == caster
                    && o.zone == Zone::Battlefield
                    && !o.tapped
                    && g.definition(o.id.card).unwrap().name == name
            })
            .unwrap()
            .id;
        g.respond(
            caster,
            r.decision_id(),
            Response::ActivateMana { object: source },
        )
        .unwrap();
    }
    recover(g);
    announce(g, q, id, targets);
    let stack = g.state.rules.stack.last().unwrap();
    assert_eq!(stack.source.card, CardId(id));
    assert_eq!(stack.controller, caster);
    assert_eq!(stack.paid.as_ref(), Some(&cost));
    assert!(
        g.state.rules.players[caster.0 as usize]
            .mana
            .values()
            .all(|n| *n == Quantity::zero())
    );
    recover(g);
}

fn scry_request(g: &mut Game, q: u64) -> (Request, Vec<ObjectId>) {
    for _ in 0..1000 {
        let r = request(g, q);
        if let Request::Scry {
            ref candidates,
            player,
            ..
        } = r
        {
            assert_eq!(player, PlayerId(0));
            assert!(
                matches!(&g.state.rules.frame, Frame::ResolutionChoice { choice: ResolutionChoice::Scry { player, looked, .. }, .. } if *player == PlayerId(0) && looked == candidates)
            );
            let looked = candidates.clone();
            recover(g);
            return (r, looked);
        }
        assert!(
            !g.state.rules.stack.is_empty(),
            "paid spell must reach scry"
        );
        answer(g, q, pass(&r));
    }
    panic!("bounded paid scry")
}

fn finish(g: &mut Game, q: u64) {
    for _ in 0..1000 {
        let r = request(g, q);
        if g.state.rules.stack.is_empty() && matches!(r, Request::Priority { .. }) {
            recover(g);
            return;
        }
        assert!(!matches!(r, Request::Scry { .. }), "no extra scry decision");
        answer(g, q, pass(&r));
    }
    panic!("bounded paid completion")
}

fn choose(g: &mut Game, r: &Request, top: Vec<ObjectId>, bottom: Vec<ObjectId>) {
    g.respond(r.player(), r.decision_id(), Response::Scry { top, bottom })
        .unwrap();
    assert!(g.state.rules.pending.is_some());
    recover(g);
}

#[test]
fn paid_controller_scry_complete_sources_costs_order_and_provenance() {
    for (raw, name, index, record, color, blocks, scry_cursor, amount) in [
        (
            LORAN,
            "Loran's Escape",
            4057,
            "5a1488a883a71b471003fc2c2909ff25e45fd25c9c77a8636732ff9885516571",
            Color::W,
            [0, 1],
            1,
            1,
        ),
        (
            SERUM,
            "Serum Visions",
            13005,
            "a95b4ac8017fac4d632079fa696d3e89f8545277600fa9512a9ae66391334f90",
            Color::U,
            [0, 1],
            1,
            2,
        ),
        (
            OPT,
            "Opt",
            17037,
            "3627b05641eea07780e6fa8fc2691352a5dcfdf26ce1374be7b5739aace979f7",
            Color::U,
            [0, 1],
            0,
            1,
        ),
        (
            PREORDAIN,
            "Preordain",
            25949,
            "0185b470d802635525fe9e6070363be7450be55c73669940ef537a78fdac45f7",
            Color::U,
            [0, 0],
            0,
            2,
        ),
    ] {
        let source: serde_json::Value = serde_json::from_str(raw).unwrap();
        assert_eq!(mtg_contracts::sha256(raw.trim().as_bytes()), record);
        assert_eq!(source["source"]["layout"], "normal");
        assert_eq!(source["source"]["legalities"]["commander"], "legal");
        assert_eq!(source["binding"]["fullyBound"], true);
        let mut rows = vec![(name, 0, 0, true), ("Grizzly Bears", 1, 1, false)];
        lands(
            &mut rows,
            if color == Color::W {
                "Plains"
            } else {
                "Island"
            },
            0,
            1,
        );
        let p = prepared(&rows);
        let d = p
            .catalog
            .definitions
            .iter()
            .find(|d| d.name == name)
            .unwrap();
        assert_eq!(d.provenance.source_index, index);
        assert_eq!(
            d.provenance.source_sha256,
            source["sourceSha256"].as_str().unwrap()
        );
        assert_eq!(
            d.provenance.ast_sha256,
            source["astSha256"].as_str().unwrap()
        );
        assert_eq!(
            d.provenance.binding_sha256,
            source["bindingSha256"].as_str().unwrap()
        );
        assert_eq!(d.characteristics.mana_cost.generic, Quantity::zero());
        assert_eq!(d.characteristics.mana_cost.colored, [color]);
        assert!(d.capabilities.contains(&Capability::ControllerScryV1));
        let program = d.spell.as_ref().unwrap();
        assert_eq!(program.instructions.len(), 2);
        assert_eq!(
            program
                .instructions
                .iter()
                .map(Instruction::source_block)
                .collect::<Vec<_>>(),
            blocks
        );
        assert!(
            matches!(&program.instructions[scry_cursor], Instruction::Scry { amount: n, .. } if *n == Quantity::from(amount))
        );
        if name == "Loran's Escape" {
            assert!(
                matches!(&program.instructions[0], Instruction::GrantKeywords { target_slot: 0, modifier: None, keywords, duration: Duration::UntilEndOfTurn, .. } if keywords == &[KeywordAbility::Hexproof, KeywordAbility::Indestructible])
            );
        } else {
            assert!(matches!(
                &program.instructions[1 - scry_cursor],
                Instruction::DrawCards { .. }
            ));
        }
        both(p, |g, q| {
            let original = library(g, 0);
            let other = library(g, 1);
            announce_paid(
                g,
                q,
                0,
                if name == "Loran's Escape" {
                    target(g, 1)
                } else {
                    vec![]
                },
            );
            assert_eq!(
                g.state.rules.objects.values().filter(|o| o.tapped).count(),
                1
            );
            let invocation = g.state.rules.stack.last().unwrap().clone();
            let (r, looked) = scry_request(g, q);
            let offset = usize::from(name == "Serum Visions");
            assert_eq!(looked, original[offset..offset + amount as usize]);
            assert_eq!(library(g, 0), original[offset..]);
            if offset == 1 {
                assert_eq!(
                    g.current(object(g, original[0].card.0)).unwrap().zone,
                    Zone::Hand { owner: PlayerId(0) }
                );
            }
            choose(g, &r, vec![], looked.clone());
            finish(g, q);
            let mut expected = original[offset + amount as usize..].to_vec();
            expected.extend(&looked);
            if name == "Opt" || name == "Preordain" {
                let drawn = expected.remove(0);
                let successor = object(g, drawn.card.0);
                assert_ne!(successor, drawn);
                assert_eq!(
                    g.current(successor).unwrap().zone,
                    Zone::Hand { owner: PlayerId(0) }
                );
            }
            assert_eq!(library(g, 0), expected);
            assert_eq!(library(g, 1), other);
            if g.trace.mode == TraceMode::Detailed {
                let effects: Vec<_> = batches(g).into_iter().filter(|b| matches!(b.cause, BatchCause::Instruction { stack_id, .. } if stack_id == invocation.id)).collect();
                let (position, batch) = effects
                    .iter()
                    .enumerate()
                    .find(|(_, b)| {
                        b.occurrences
                            .iter()
                            .any(|o| matches!(o.actual, EventOutcome::Scried { .. }))
                    })
                    .unwrap();
                assert_eq!(position, scry_cursor);
                assert!(
                    matches!(batch.cause, BatchCause::Instruction { source, player: PlayerId(0), cursor, source_block, .. } if source == invocation.source && cursor == scry_cursor as u32 && source_block == blocks[scry_cursor])
                );
                assert_eq!(batch.occurrences.len(), 1);
                assert_eq!(
                    batch.occurrences[0].proposed,
                    ProposedEvent::Scry {
                        player: PlayerId(0),
                        amount: amount.into(),
                        looked: looked.clone(),
                        top: vec![],
                        bottom: looked.clone()
                    }
                );
                assert_eq!(
                    batch.occurrences[0].actual,
                    EventOutcome::Scried {
                        player: PlayerId(0),
                        looked: looked.clone(),
                        top: vec![],
                        bottom: looked
                    }
                );
            }
        });
    }
}

#[test]
fn paid_controller_scry_two_all_ordered_partitions_preserve_tail_and_incarnations() {
    for (order, top_count) in [
        ([0, 1], 0),
        ([1, 0], 0),
        ([0, 1], 1),
        ([1, 0], 1),
        ([0, 1], 2),
        ([1, 0], 2),
    ] {
        let rows = [("Serum Visions", 0, 0, true), ("Island", 0, 0, false)];
        both(prepared(&rows), |g, q| {
            announce_paid(g, q, 0, vec![]);
            let (r, looked) = scry_request(g, q);
            assert_eq!(looked.len(), 2);
            let before = library(g, 0);
            let objects: Vec<_> = before
                .iter()
                .map(|id| g.current(*id).unwrap().clone())
                .collect();
            let shuffles = g.visible_player(PlayerId(0)).unwrap().library_shuffles;
            let ordered = [looked[order[0]], looked[order[1]]];
            let top = ordered[..top_count].to_vec();
            let bottom = ordered[top_count..].to_vec();
            let expected: Vec<_> = top
                .iter()
                .chain(&before[2..])
                .chain(&bottom)
                .copied()
                .collect();
            choose(g, &r, top.clone(), bottom.clone());
            finish(g, q);
            assert_eq!(library(g, 0), expected);
            for old in &objects {
                assert_eq!(g.current(old.id).unwrap(), old);
            }
            assert_eq!(
                g.visible_player(PlayerId(0)).unwrap().library_shuffles,
                shuffles
            );
            if g.trace.mode == TraceMode::Detailed {
                let scries: Vec<_> = batches(g)
                    .into_iter()
                    .flat_map(|b| &b.occurrences)
                    .filter(|o| matches!(o.actual, EventOutcome::Scried { .. }))
                    .collect();
                assert_eq!(scries.len(), 1);
                assert_eq!(
                    scries[0].actual,
                    EventOutcome::Scried {
                        player: PlayerId(0),
                        looked,
                        top,
                        bottom
                    }
                );
            }
        });
    }
}
