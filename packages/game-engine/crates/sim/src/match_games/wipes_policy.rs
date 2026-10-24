//! Frozen P6.10 completion policy; only observations and the public catalog.
use super::*;
use mtg_contracts::{Definition, KeywordAbility, StaticEffect};
use mtg_core::{PermittedView, VisibleObject};
impl Strategy<'_> {
    pub(super) fn lineage(&self, minimum: &str) -> bool {
        mtg_core::MATCH_PROFILES
            .iter()
            .find(|p| p.ir_version == self.catalog.schema_version)
            .is_some_and(|p| mtg_core::profile_at_least(p.name, minimum))
    }
    pub(super) fn wipes(&self) -> bool {
        self.lineage(mtg_core::WIPES_PROFILE)
    }
    fn wipe_creatures<'a>(&self, v: &'a PermittedView) -> Vec<&'a VisibleObject> {
        v.objects
            .iter()
            .filter(|o| {
                o.zone == Zone::Battlefield
                    && o.characteristics
                        .as_ref()
                        .is_some_and(|c| c.kind == CardKind::Creature)
            })
            .collect()
    }
    fn damage_multiplier(&self, v: &PermittedView) -> Quantity {
        let mut q = Quantity::from(1);
        for o in &v.objects {
            if o.zone == Zone::Battlefield
                && definition(self.catalog, &o.oracle_id).is_ok_and(|d| {
                    d.static_abilities
                        .iter()
                        .any(|a| a.effect == StaticEffect::DoubleDamageToPermanentOrPlayer)
                })
            {
                let a = q.clone();
                q.add(&a);
            }
        }
        q
    }
    pub(super) fn earthquake_x(&self, v: &PermittedView) -> Option<Quantity> {
        let multiplier = self.damage_multiplier(v);
        let life = &v.players.iter().find(|p| p.player == v.viewer)?.life;
        let d = self
            .catalog
            .definitions
            .iter()
            .find(|d| d.name == "Earthquake")?;
        let creatures = self.wipe_creatures(v);
        let mut best = 0;
        let mut chosen = None;
        // Finite payment proof: 400 two-mana sources + 48 inherited/recursion returns
        // + eight mass returns of at most 400 sources, each producing two.
        for n in 1..=7296 {
            let x = Quantity::from(n);
            let mut dealt = Quantity::zero();
            for _ in 0..n {
                dealt.add(&multiplier);
            }
            if &dealt >= life {
                break;
            }
            let mut cost = d.characteristics.mana_cost.clone();
            cost.generic.add(&x);
            if mana_policy::plan(self.catalog, v, &cost, None).is_none() {
                break;
            }
            let count = creatures
                .iter()
                .filter(|o| {
                    o.controller != v.viewer
                        && !o.keywords.contains(&KeywordAbility::Flying)
                        && o.characteristics
                            .as_ref()
                            .and_then(|c| c.toughness.as_ref())
                            .is_some_and(|t| {
                                let mut lethal = dealt.clone();
                                lethal.add(&o.marked_damage);
                                &lethal >= t
                            })
                })
                .count();
            let wins = v
                .players
                .iter()
                .filter(|p| p.player != v.viewer && v.live_turn_order.contains(&p.player))
                .all(|p| p.life <= dealt);
            if count > best || (chosen.is_none() && wins) {
                best = count;
                chosen = Some(x);
            }
        }
        chosen
    }
    pub(super) fn wipe_cost(&self, v: &PermittedView, d: &Definition) -> Option<ManaCost> {
        let mut cost = d.characteristics.mana_cost.clone();
        if d.name == "Earthquake" {
            cost.generic.add(&self.earthquake_x(v)?);
        }
        if d.static_abilities
            .iter()
            .any(|a| a.effect == StaticEffect::ReduceGenericByCreatureCount)
        {
            cost.generic
                .subtract(&Quantity::from(self.wipe_creatures(v).len() as i64));
            if cost.generic.is_negative() {
                cost.generic = Quantity::zero();
            }
        }
        Some(cost)
    }
    pub(super) fn wipe_ready(&self, v: &PermittedView, d: &Definition) -> bool {
        let cs = self.wipe_creatures(v);
        let ours = cs.iter().filter(|o| o.controller == v.viewer).count();
        match d.name.as_str() {
            "Earthquake" => self.earthquake_x(v).is_some(),
            "Massacre Wurm" => true,
            "Innocent Blood" => {
                (ours == 0
                    || cs
                        .iter()
                        .any(|o| o.controller == v.viewer && o.token.is_some()))
                    && cs.iter().any(|o| o.controller != v.viewer)
            }
            "Tranquility" => {
                let mut ours = Quantity::zero();
                let mut theirs = Quantity::zero();
                for o in &v.objects {
                    if o.zone == Zone::Battlefield
                        && o.characteristics
                            .as_ref()
                            .is_some_and(|c| c.kind == CardKind::Enchantment)
                    {
                        let c = o.characteristics.as_ref().unwrap();
                        let mut mv = c.mana_cost.generic.clone();
                        mv.add(&Quantity::from(c.mana_cost.colored.len() as i64));
                        if o.controller == v.viewer {
                            ours.add(&mv);
                        } else {
                            theirs.add(&mv);
                        }
                    }
                }
                theirs > ours
            }
            "Aetherize" => {
                if v.turn.step != TurnStep::DeclareBlockers {
                    return false;
                }
                let attacks = v
                    .combat
                    .iter()
                    .filter(|a| a.defender == v.viewer)
                    .collect::<Vec<_>>();
                let mut damage = Quantity::zero();
                for a in &attacks {
                    if let Some(o) = v.objects.iter().find(|o| o.handle == a.attacker)
                        && let Some(p) = o.characteristics.as_ref().and_then(|c| c.power.as_ref())
                    {
                        damage.add(p);
                        if o.keywords.contains(&KeywordAbility::DoubleStrike) {
                            damage.add(p);
                        }
                    }
                }
                for o in &v.objects {
                    if o.zone == Zone::Battlefield
                        && definition(self.catalog, &o.oracle_id).is_ok_and(|d| {
                            d.static_abilities
                                .iter()
                                .any(|a| a.effect == StaticEffect::DoubleDamageToPermanentOrPlayer)
                        })
                    {
                        let n = damage.clone();
                        damage.add(&n);
                    }
                }
                attacks.len() >= 2
                    || v.players
                        .iter()
                        .find(|p| p.player == v.viewer)
                        .is_some_and(|p| damage >= p.life)
            }
            "Fumigate" | "Planar Cleansing" | "Final Judgment" | "Infest" | "Pyroclasm"
            | "Anger of the Gods" | "Chain Reaction" | "Blasphemous Act" | "Evacuation"
            | "Volcanic Fallout" => {
                let mut damage = match d.name.as_str() {
                    "Infest" | "Pyroclasm" | "Volcanic Fallout" => Some(Quantity::from(2)),
                    "Anger of the Gods" => Some(Quantity::from(3)),
                    "Chain Reaction" => Some(Quantity::from(cs.len() as i64)),
                    "Blasphemous Act" => Some(Quantity::from(13)),
                    _ => None,
                };
                if d.name != "Infest" {
                    for o in &v.objects {
                        if o.zone == Zone::Battlefield
                            && definition(self.catalog, &o.oracle_id).is_ok_and(|d| {
                                d.static_abilities.iter().any(|a| {
                                    a.effect == StaticEffect::DoubleDamageToPermanentOrPlayer
                                })
                            })
                            && let Some(q) = &mut damage
                        {
                            let a = q.clone();
                            q.add(&a);
                        }
                    }
                }
                if d.name == "Volcanic Fallout"
                    && v.players
                        .iter()
                        .find(|p| p.player == v.viewer)
                        .is_some_and(|p| damage.as_ref().is_some_and(|q| q >= &p.life))
                {
                    return false;
                }
                let removes = |o: &&VisibleObject| {
                    damage.as_ref().is_none_or(|q| {
                        o.characteristics
                            .as_ref()
                            .and_then(|c| c.toughness.as_ref())
                            .is_some_and(|t| {
                                let mut n = q.clone();
                                if d.name != "Infest" {
                                    n.add(&o.marked_damage);
                                }
                                &n >= t
                            })
                    })
                };
                cs.iter()
                    .filter(|o| o.controller != v.viewer && removes(o))
                    .count()
                    >= cs
                        .iter()
                        .filter(|o| o.controller == v.viewer && removes(o))
                        .count()
                        + 2
            }
            _ => true,
        }
    }
    pub(super) fn wipes_priority(
        &mut self,
        v: &PermittedView,
    ) -> Result<Option<PlayerResponse>, String> {
        for o in &v.objects {
            if o.zone != (Zone::Hand { owner: v.viewer }) {
                continue;
            }
            let d = definition(self.catalog, &o.oracle_id)?;
            if !matches!(
                d.name.as_str(),
                "Aetherize" | "Evacuation" | "Volcanic Fallout"
            ) || !self.wipe_ready(v, d)
            {
                continue;
            }
            if let Some(plan) = self
                .wipe_cost(v, d)
                .and_then(|c| mana_policy::plan(self.catalog, v, &c, None))
            {
                self.mana_plan = plan;
                return Ok(Some(PlayerResponse::Cast {
                    card: o.handle.clone(),
                }));
            }
        }
        Ok(None)
    }
}

pub(super) fn interactions(
    catalog: &Catalog,
    physical: &[mtg_core::CardSetup],
    g: &Game,
) -> Result<serde_json::Value, String> {
    use mtg_core::{
        BatchCause, Event, EventOutcome, EvidenceRecord, ProposedEvent, ResolvedEffectKind,
    };
    let name = |card: mtg_core::CardId| {
        physical
            .iter()
            .find(|c| c.id == card)
            .and_then(|c| definition(catalog, &c.oracle_id).ok())
            .map(|d| d.name.as_str())
    };
    let creature = |card: mtg_core::CardId| {
        physical.iter().find(|c| c.id == card).is_none_or(|c| {
            definition(catalog, &c.oracle_id)
                .is_ok_and(|d| d.characteristics.kind == CardKind::Creature)
        })
    };
    let mut casts = BTreeMap::<String, u64>::new();
    let mut removed = BTreeMap::<String, u64>::new();
    let wipe = |n: &str| {
        catalog
            .definitions
            .iter()
            .find(|d| d.name == n)
            .is_some_and(|d| {
                d.capabilities
                    .contains(&mtg_contracts::Capability::BoardWipesV1)
                    || d.spell.as_ref().is_some_and(|p| {
                        p.instructions.iter().any(|i| {
                            matches!(i, mtg_contracts::Instruction::DestroyAllCreatures { .. })
                        })
                    })
            })
    };
    for d in &catalog.definitions {
        if wipe(&d.name) {
            casts.insert(d.name.clone(), 0);
            removed.insert(d.name.clone(), 0);
        }
    }
    let mut proposed = BTreeMap::new();
    let mut x = vec![];
    let mut anger = std::collections::BTreeSet::<mtg_core::ObjectId>::new();
    let mut anger_exiles = 0;
    let mut wurm = Quantity::zero();
    let mut counters = 0;
    let mut sba_targets = BTreeMap::new();
    for record in g.trace().records.iter() {
        match &record.record {
            EvidenceRecord::Event { event } => match event.as_ref() {
                Event::SpellProposed { card, stack_id } => {
                    proposed.insert(*stack_id, *card);
                    sba_targets.clear();
                }
                Event::SpellCast { stack_id } => {
                    if let Some(n) = proposed.get(stack_id).and_then(|c| name(*c))
                        && wipe(n)
                    {
                        *casts.entry(n.into()).or_default() += 1;
                    }
                }
                Event::XAnnounced { value, .. } => x.push(value.clone()),
                _ => {}
            },
            EvidenceRecord::Batch { batch } => {
                let source = if let BatchCause::Instruction { source, .. } = batch.cause {
                    name(source.card)
                } else {
                    None
                };
                for o in &batch.occurrences {
                    // Detailed outcomes are internally tagged with "outcome".
                    let v = serde_json::to_value(&o.actual).map_err(|e| e.to_string())?;
                    match v["outcome"].as_str() {
                        Some("effect_created") => {
                            if let EventOutcome::EffectCreated { effect } = &o.actual {
                                match &effect.kind {
                                    ResolvedEffectKind::ExileDamaged { targets } => {
                                        anger.extend(targets)
                                    }
                                    ResolvedEffectKind::PumpSet { targets, .. } => {
                                        if let Some(n) = source {
                                            for t in targets {
                                                sba_targets.insert(*t, n.to_string());
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Some("damage_committed") => {
                            if let EventOutcome::DamageCommitted { actual, .. } = &o.actual
                                && let Some(n) = source
                                && wipe(n)
                            {
                                for d in actual {
                                    if let mtg_core::DamageRecipient::Creature { object } =
                                        d.recipient
                                        && d.amount > Quantity::zero()
                                    {
                                        sba_targets.insert(object, n.to_string());
                                    }
                                }
                            }
                        }
                        Some("life_lost") if source == Some("Massacre Wurm") => {
                            if let EventOutcome::LifeLost { amount, .. } = &o.actual {
                                wurm.add(amount);
                            }
                        }
                        Some("moved" | "state_based_moved" | "sacrificed") => {
                            let m = match &o.actual {
                                EventOutcome::Moved { movement }
                                | EventOutcome::StateBasedMoved { movement, .. }
                                | EventOutcome::Sacrificed { movement, .. } => movement,
                                _ => unreachable!(),
                            };
                            if m.from == Zone::Battlefield && creature(m.previous.card) {
                                let n = source
                                    .filter(|n| wipe(n))
                                    .map(str::to_owned)
                                    .or_else(|| sba_targets.remove(&m.previous));
                                if let Some(n) = n {
                                    *removed.entry(n).or_default() += 1;
                                }
                                if m.destination == Zone::Exile && anger.remove(&m.previous) {
                                    anger_exiles += 1;
                                }
                            }
                        }
                        Some("effects_cleaned") => anger.clear(),
                        _ => {}
                    }
                    if let ProposedEvent::CounterStackObject { target, .. } = o.proposed
                        && proposed.get(&target).and_then(|c| name(*c)) == Some("Volcanic Fallout")
                    {
                        counters += 1;
                    }
                }
                if batch.cause == BatchCause::StateBased {
                    sba_targets.clear();
                }
            }
        }
    }
    Ok(
        serde_json::json!({"wipes_cast":casts,"creatures_removed":removed,"anger_exiles":anger_exiles,"x_values":x,"massacre_wurm_life_loss":wurm,"volcanic_fallout_counter_attempts":counters}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn view(c: &Catalog) -> PermittedView {
        let p = Prepared::admit_pod(
            c.clone(),
            MatchConfig::for_profile(mtg_core::WIPES_PROFILE, [3; 32], PlayerId(3)).unwrap(),
        )
        .unwrap();
        let mut g = Game::new(p);
        g.advance(Budget::unlimited());
        let QueryResult::Complete(QueryValue::Observation(o)) =
            g.observe(PlayerId(3), Budget::unlimited())
        else {
            panic!("view")
        };
        let mut v = o.view;
        v.objects.clear();
        v.stack.clear();
        v.commanders.clear();
        v
    }
    fn card(c: &Catalog, name: &str, owner: u8, zone: Zone) -> VisibleObject {
        let d = c.definitions.iter().find(|d| d.name == name).unwrap();
        VisibleObject {
            attached_to: None,
            attachments: vec![],
            keywords: d.keywords.clone(),
            restrictions: vec![],
            token: None,
            plus_one_counters: 0.into(),
            characteristics: Some(d.characteristics.clone()),
            marked_damage: 0.into(),
            attack_ready: true,
            handle: ObjectHandle(name.into()),
            oracle_id: d.oracle_id.clone(),
            owner: PlayerId(owner),
            controller: PlayerId(owner),
            zone,
            tapped: false,
        }
    }
    #[test]
    fn wipes_counter_policy_orders_announced_x_mana_value() {
        let c = mtg_compiler::wipes_fixture_catalog().unwrap();
        let mut v = view(&c);
        for (name, x, mv) in [
            ("Earthquake", Some(3.into()), Some(4.into())),
            ("Pyroclasm", None, None),
        ] {
            v.objects.push(card(&c, name, 1, Zone::Stack));
            v.stack.push(mtg_core::VisibleStackEntry {
                x,
                mana_value: mv,
                mode: None,
                triggered: None,
                entity_targets: vec![],
                handle: Some(mtg_core::StackHandle(name.into())),
                stack_targets: vec![],
                spell: Some(ObjectHandle(name.into())),
                ability: None,
                player_targets: vec![],
                controller: PlayerId(1),
                cast_complete: true,
                targets: vec![],
            });
        }
        let s = Strategy {
            catalog: &c,
            policy: Policy::Develop,
            mulligans: [0; 4],
            beneficial: false,
            mana_plan: Default::default(),
            mana_color: None,
        };
        assert!(s.sacrifice() && s.attachments() && s.four_colors());
        assert_eq!(
            s.opposing_stack(&v, false, None),
            Some(mtg_core::StackHandle("Earthquake".into()))
        );
        v.stack[0].x = Some(0.into());
        v.stack[0].mana_value = Some(1.into());
        assert_eq!(
            s.opposing_stack(&v, false, None),
            Some(mtg_core::StackHandle("Pyroclasm".into()))
        );
    }
    #[test]
    fn wipes_policy_thresholds_and_smallest_safe_earthquake_x() {
        let c = mtg_compiler::wipes_fixture_catalog().unwrap();
        let mut v = view(&c);
        v.players
            .iter_mut()
            .find(|p| p.player == v.viewer)
            .unwrap()
            .mana
            .insert(Color::R, 8.into());
        v.players
            .iter_mut()
            .find(|p| p.player == v.viewer)
            .unwrap()
            .life = 6.into();
        v.objects = vec![
            card(&c, "Furnace of Rath", 3, Zone::Battlefield),
            card(&c, "Alpine Watchdog", 0, Zone::Battlefield),
            card(&c, "Serra Angel", 0, Zone::Battlefield),
            card(&c, "Giant Spider", 1, Zone::Battlefield),
            card(&c, "Storm Crow", 2, Zone::Battlefield),
        ];
        let s = Strategy {
            catalog: &c,
            policy: Policy::Develop,
            mulligans: [0; 4],
            beneficial: false,
            mana_plan: Default::default(),
            mana_color: None,
        };
        assert_eq!(s.earthquake_x(&v), Some(2.into())); // 4 damage, two ground kills; flyers excluded.
        let d = |name: &str| c.definitions.iter().find(|d| d.name == name).unwrap();
        assert!(s.wipe_ready(&v, d("Pyroclasm")));
        assert!(s.wipe_ready(&v, d("Innocent Blood")));
        assert!(s.wipe_ready(&v, d("Massacre Wurm")));
        v.players
            .iter_mut()
            .find(|p| p.player == v.viewer)
            .unwrap()
            .life = 2.into();
        assert_eq!(s.earthquake_x(&v), None);
        assert!(!s.wipe_ready(&v, d("Volcanic Fallout")));
        v.objects = vec![
            card(&c, "Bastion of Remembrance", 3, Zone::Battlefield),
            card(&c, "Glorious Anthem", 0, Zone::Battlefield),
        ];
        assert!(!s.wipe_ready(&v, d("Tranquility")));
        v.objects
            .push(card(&c, "Spidersilk Armor", 1, Zone::Battlefield));
        assert!(s.wipe_ready(&v, d("Tranquility")));
    }
}
