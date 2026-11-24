//! Integer encoding of the viewer's published knowledge. Hidden zone access
//! follows CR 400.2 and 401.2 (pinned20260925):
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
pub mod action;
pub mod catalog;
use mtg_contracts::{Catalog, Color, HybridPayment, ManaProduction, Quantity};
use mtg_view::*;
use std::collections::HashMap;

pub fn int(q: &Quantity) -> i32 {
    if let Some(n) = q.to_u32() {
        return i32::try_from(n).unwrap_or(i32::MAX);
    }
    if q.is_negative() {
        q.to_string().parse().unwrap_or(i32::MIN)
    } else {
        i32::MAX
    }
}
pub fn color(c: Color) -> usize {
    match c {
        Color::W => 0,
        Color::U => 1,
        Color::B => 2,
        Color::R => 3,
        Color::G => 4,
        Color::C => 5,
    }
}
fn card(c: &Catalog, id: &str) -> Result<i32, String> {
    if id.is_empty() {
        return Ok(0);
    }
    c.definitions
        .binary_search_by(|d| d.oracle_id.as_str().cmp(id))
        .map(|i| i as i32 + 1)
        .map_err(|_| format!("missing visible catalog identity {id}"))
}
pub fn number_bound(c: &Catalog, v: &PermittedView) -> i32 {
    let mut total = 0i32;
    for p in &v.players {
        if p.player == v.viewer {
            for q in p.mana.values() {
                total = total.saturating_add(int(q));
            }
        }
    }
    for o in &v.objects {
        if o.controller != v.viewer || o.zone != Zone::Battlefield || o.phased_out || o.tapped {
            continue;
        }
        if let Some(d) = c.definition_by_oracle_id(&o.oracle_id) {
            let amount = d
                .mana_abilities
                .iter()
                .map(|a| match &a.production {
                    ManaProduction::Fixed { symbols } => symbols.len() as i32,
                    ManaProduction::Choose { amount, .. } => int(amount),
                    _ => 1,
                })
                .max()
                .unwrap_or(i32::from(
                    d.characteristics.kind == mtg_contracts::CardKind::Land,
                ));
            total = total.saturating_add(amount);
        }
    }
    total
}
#[derive(Clone, PartialEq, Eq)]
pub struct Row {
    pub globals: [i32; 48],
    pub players: [[i32; 24]; 4],
    pub ent_int: [[i32; 10]; 256],
    pub ent_num: [[i32; 24]; 256],
    pub ent_mask: [u8; 256],
    pub cand: [[i32; 6]; 128],
    pub ptr_mask: [u8; 131],
    pub num_mask: [u8; 64],
    pub head: u8,
    pub seat: u8,
    pub reward: [i32; 4],
    pub done: u8,
    pub info: [i32; 8],
}
impl Default for Row {
    fn default() -> Self {
        Self {
            globals: [0; 48],
            players: [[0; 24]; 4],
            ent_int: [[-1; 10]; 256],
            ent_num: [[0; 24]; 256],
            ent_mask: [0; 256],
            cand: [[-1; 6]; 128],
            ptr_mask: [0; 131],
            num_mask: [0; 64],
            head: 0,
            seat: 0,
            reward: [0; 4],
            done: 0,
            info: [0; 8],
        }
    }
}
fn zone(z: Zone) -> i32 {
    match z {
        Zone::Battlefield => 0,
        Zone::Hand { .. } => 1,
        Zone::Graveyard { .. } => 2,
        Zone::Library { .. } => 3,
        Zone::Command { .. } => 4,
        Zone::Exile => 5,
        Zone::Stack => 6,
    }
}
pub fn encode(
    c: &Catalog,
    o: &Observation,
    micro: usize,
    choices: &action::Choices,
    funding: &[u8],
    mask_unfundable: bool,
) -> Result<Row, String> {
    let v = &o.view;
    let d = o.decision.as_ref().ok_or("expected viewer decision")?;
    let relative = |p: PlayerId| i32::from((p.0 + 4 - v.viewer.0) % 4);
    if d.candidates.len() > 128
        || funding.len() != d.candidates.len()
        || v.players.len() > 4
        || v.objects.len() + v.stack.len() + v.known_library_positions.len() > 256
    {
        return Err("observation exceeds tensor capacity or funding length mismatch".into());
    }
    let mut r = Row {
        head: choices.numeric as u8,
        seat: v.viewer.0,
        ptr_mask: choices.pointer,
        num_mask: choices.number,
        ..Default::default()
    };
    let g = &mut r.globals;
    g[..12].copy_from_slice(&[
        v.turn.number.min(i32::MAX as u64) as i32,
        v.turn.step as i32,
        v.active.map_or(-1, relative),
        v.priority.map_or(-1, relative),
        v.stack.len() as i32,
        v.live_turn_order.len() as i32,
        match d.shape {
            Shape::Pick { .. } => 0,
            Shape::Order { .. } => 1,
            Shape::Distribute { .. } => 2,
            Shape::Number { .. } => 3,
            Shape::Pay { .. } => 4,
        },
        i32::from(d.reason.code()),
        micro as i32,
        choices.slot as i32,
        choices.picked as i32,
        choices.remaining,
    ]);
    if let Shape::Pay { cost, pool, .. } = &d.shape {
        for &c in &cost.colored {
            g[12 + color(c)] += 1;
        }
        g[18] = int(&cost.generic);
        for (&c, q) in &pool.unrestricted {
            g[19 + color(c)] = int(q);
        }
    } else if let Some(p) = v.players.iter().find(|p| p.player == v.viewer) {
        for (&c, q) in &p.mana {
            g[19 + color(c)] = int(q);
        }
    }
    g[26] = v.setup.as_ref().map_or(0, |s| match s.phase {
        VisibleSetupPhase::Declare => 1,
        VisibleSetupPhase::Bottom => 2,
    });
    for p in &v.players {
        let row = &mut r.players[relative(p.player) as usize];
        row[..4].copy_from_slice(&[
            int(&p.life),
            p.hand_count as i32,
            p.library_count as i32,
            p.library_shuffles.min(i32::MAX as u64) as i32,
        ]);
        for (&c, q) in &p.mana {
            row[4 + color(c)] = int(q);
        }
        row[10] = p.status as i32;
        row[11] = p.life_frozen_until_next_turn as i32;
        for cmd in &v.commanders {
            let src = relative(cmd.history.owner) as usize;
            if let Some(q) = cmd.history.command_casts.get(&p.player) {
                row[12 + src] = row[12 + src].saturating_add(int(q));
            }
            if let Some(q) = cmd.history.combat_damage.get(&p.player) {
                row[16 + src] = row[16 + src].saturating_add(int(q));
            }
        }
    }
    let objects: HashMap<_, _> = v
        .objects
        .iter()
        .enumerate()
        .map(|(i, o)| (o.handle.0.as_str(), i as i32))
        .collect();
    let object = |h: &ObjectHandle| objects.get(h.0.as_str()).copied().unwrap_or(-1);
    for (i, o) in v.objects.iter().enumerate() {
        r.ent_mask[i] = 1;
        let row = &mut r.ent_int[i];
        row[..6].copy_from_slice(&[
            card(c, &o.oracle_id)?,
            zone(o.zone),
            relative(o.owner),
            relative(o.controller),
            o.attached_to.as_ref().map_or(-1, object),
            o.exiled_by.as_ref().map_or(-1, object),
        ]);
        let n = &mut r.ent_num[i];
        n[..6].copy_from_slice(&[
            o.tapped as i32,
            o.attack_ready as i32,
            o.entered_this_turn as i32,
            o.phased_out as i32,
            o.face_down as i32,
            o.token.is_some() as i32,
        ]);
        if let Some(ch) = &o.characteristics {
            n[6] = ch.power.as_ref().map_or(0, int);
            n[7] = ch.toughness.as_ref().map_or(0, int);
        }
        n[8] = int(&o.marked_damage);
        n[9] = int(&o.plus_one_counters);
        for k in &o.keywords {
            let bit = *k as usize;
            n[11 + bit / 32] |= 1 << (bit % 32);
        }
        for k in &o.restrictions {
            n[13] |= 1 << (*k as u32);
        }
        n[15] = -1;
        n[18] = -1;
    }
    for a in &v.combat {
        let i = object(&a.attacker);
        if i >= 0 {
            r.ent_int[i as usize][7] = relative(a.defender);
        }
        for b in &a.blockers {
            let j = object(b);
            if j >= 0 {
                r.ent_int[j as usize][6] = i;
            }
        }
    }
    let base = v.objects.len();
    let stack: HashMap<_, _> = v
        .stack
        .iter()
        .enumerate()
        .filter_map(|(i, s)| s.handle.as_ref().map(|h| (h.0.as_str(), (base + i) as i32)))
        .collect();
    for (j, s) in v.stack.iter().enumerate() {
        let i = base + j;
        r.ent_mask[i] = 1;
        let row = &mut r.ent_int[i];
        let source = s
            .spell
            .as_ref()
            .and_then(|h| objects.get(h.0.as_str()))
            .map(|&x| &v.objects[x as usize]);
        let id = source
            .map(|o| o.oracle_id.as_str())
            .or_else(|| s.ability.as_ref().map(|a| a.oracle_id.as_str()))
            .or_else(|| s.triggered.as_ref().map(|t| t.oracle_id.as_str()))
            .unwrap_or("");
        row[..4].copy_from_slice(&[
            card(c, id)?,
            6,
            source.map_or(-1, |o| relative(o.owner)),
            relative(s.controller),
        ]);
        let targets = s
            .targets
            .iter()
            .map(|t| t.current.as_ref().map_or(-1, object))
            .chain(s.entity_targets.iter().map(|t| match t {
                VisibleEntityTarget::Object { object: t } => t.current.as_ref().map_or(-1, object),
                _ => -1,
            }))
            .chain(s.stack_targets.iter().map(|t| {
                t.current
                    .as_ref()
                    .and_then(|h| stack.get(h.0.as_str()).copied())
                    .unwrap_or(-1)
            }));
        for (field, value) in row[8..].iter_mut().zip(targets) {
            *field = value
        }
        let n = &mut r.ent_num[i];
        n[14] = s.x.as_ref().map_or(0, int);
        n[15] = -1;
        n[16] = s.cast_complete as i32;
        n[17] = s.spell_copy as i32;
        n[18] = -1;
    }
    for (j, k) in v.known_library_positions.iter().enumerate() {
        let i = base + v.stack.len() + j;
        r.ent_mask[i] = 1;
        let p = relative(k.library_owner);
        r.ent_int[i][..4].copy_from_slice(&[card(c, &k.oracle_id)?, 3, p, p]);
        r.ent_num[i][15] = -1;
        r.ent_num[i][18] = k.position as i32;
    }
    for (i, k) in d.candidates.iter().enumerate() {
        let (kind, entity, seat, a, b) = match k {
            Candidate::Object { object: h } => (0, object(h), -1, 0, 0),
            Candidate::Player { player } => (1, -1, relative(*player), 0, 0),
            Candidate::Trigger { .. } => (2, -1, -1, 0, 0),
            Candidate::DamageEffect { .. } => (3, -1, -1, 0, 0),
            Candidate::Color { color: c } => (4, -1, -1, color(*c) as i32, 0),
            Candidate::ManaAbility { ability } => match ability {
                ManaAbilityIdentity::Printed { index } => (5, -1, -1, 1, *index as i32),
                ManaAbilityIdentity::Intrinsic { color: c } => (5, -1, -1, 0, color(*c) as i32),
            },
            Candidate::AdditionalCost { mana, .. } => {
                (6, -1, -1, int(&mana.generic), mana.colored.len() as i32)
            }
            Candidate::HybridPayment { payment } => (
                7,
                -1,
                -1,
                match payment {
                    HybridPayment::Mana(c) => color(*c) as i32,
                    HybridPayment::PayTwoLife => 6,
                },
                0,
            ),
            Candidate::Flag { flag } => (8, -1, -1, *flag as i32, 0),
            Candidate::Action { action } => match action {
                Action::Pass => (9, -1, -1, 0, 0),
                Action::PlayLand { object: h } => (10, object(h), -1, 0, 0),
                Action::Cast { card: h } => (11, object(h), -1, 0, 0),
                Action::ActivateMana { object: h } => (12, object(h), -1, 0, 0),
                Action::ActivateAbility { object: h, ability } => {
                    (13, object(h), -1, *ability as i32, 0)
                }
            },
        };
        r.cand[i] = [kind, entity, seat, a, b, i32::from(funding[i])];
        if mask_unfundable && funding[i] == 0 {
            r.ptr_mask[i] = 0;
        }
    }
    Ok(r)
}
