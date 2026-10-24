//! Observation-only completion machinery for the frozen sacrifice pool.
use super::*;
use mtg_contracts::{ActivationCost, Definition, SacrificeFilter};
use mtg_core::{PermittedView, VisibleObject};
impl Strategy<'_> {
    pub(super) fn sacrifice(&self) -> bool {
        self.catalog.schema_version == 19 && self.catalog.definitions.len() == 153
    }
    fn sacrifice_permanents<'a>(
        &self,
        v: &'a PermittedView,
        f: &SacrificeFilter,
    ) -> Vec<&'a VisibleObject> {
        v.objects
            .iter()
            .filter(|o| {
                o.zone == Zone::Battlefield
                    && o.controller == v.viewer
                    && o.characteristics.as_ref().is_some_and(|c| {
                        (f.types.is_empty() || f.types.contains(&c.kind))
                            && f.subtypes.iter().all(|s| c.subtypes.contains(s))
                    })
            })
            .collect()
    }
    fn sacrifice_rank(&self, v: &PermittedView, o: &VisibleObject) -> (bool, bool, bool, Quantity) {
        (
            v.commanders
                .iter()
                .any(|c| c.oracle_id == o.oracle_id && c.history.owner == v.viewer),
            o.token.is_none(),
            !o.characteristics
                .as_ref()
                .is_some_and(|c| c.kind == CardKind::Creature),
            o.characteristics
                .as_ref()
                .and_then(|c| c.power.clone())
                .unwrap_or_default(),
        )
    }
    pub(super) fn sacrifice_choice(
        &self,
        v: &PermittedView,
        candidates: &[ObjectHandle],
    ) -> Result<ObjectHandle, String> {
        v.objects
            .iter()
            .filter(|o| candidates.contains(&o.handle))
            .min_by_key(|o| self.sacrifice_rank(v, o))
            .map(|o| o.handle.clone())
            .ok_or("empty sacrifice candidates".into())
    }
    fn sacrifice_victim<'a>(
        &self,
        v: &'a PermittedView,
        f: &SacrificeFilter,
    ) -> Option<&'a VisibleObject> {
        self.sacrifice_permanents(v, f)
            .into_iter()
            .min_by_key(|o| self.sacrifice_rank(v, o))
    }
    fn sacrifice_lethal(&self, v: &PermittedView, amount: &Quantity) -> bool {
        v.players.iter().any(|p| {
            p.player != v.viewer && p.status == mtg_core::PlayerStatus::Live && p.life <= *amount
        }) || v.objects.iter().any(|o| {
            o.zone == Zone::Battlefield
                && o.controller != v.viewer
                && self.targetable(v, o)
                && o.characteristics.as_ref().is_some_and(|c| {
                    c.kind == CardKind::Creature
                        && c.toughness.as_ref().is_some_and(|t| t <= amount)
                })
        })
    }
    pub(super) fn sacrifice_spell_ready(&self, v: &PermittedView, d: &Definition) -> bool {
        let Some(f) = d
            .spell
            .as_ref()
            .and_then(|p| p.additional_sacrifice.as_ref())
        else {
            return true;
        };
        let Some(victim) = self.sacrifice_victim(v, f) else {
            return false;
        };
        let power = victim
            .characteristics
            .as_ref()
            .and_then(|c| c.power.clone())
            .unwrap_or_default();
        if d.name == "Fling" {
            self.sacrifice_lethal(v, &power)
        } else {
            victim.token.is_some()
                || (victim
                    .characteristics
                    .as_ref()
                    .is_some_and(|c| c.kind == CardKind::Creature)
                    && power <= 1.into())
        }
    }
    pub(super) fn sacrifice_priority(
        &mut self,
        v: &PermittedView,
    ) -> Result<Option<PlayerResponse>, String> {
        let f = SacrificeFilter {
            types: vec![CardKind::Creature],
            subtypes: vec![],
        };
        for o in v.objects.iter().filter(|o| {
            o.zone == Zone::Battlefield && o.controller == v.viewer && o.token.is_none()
        }) {
            let d = definition(self.catalog, &o.oracle_id)?;
            let Some(a) = d.activated.first() else {
                continue;
            };
            let usable = match d.name.as_str() {
                "Carrion Feeder" | "Bloodthrone Vampire" => self
                    .sacrifice_victim(v, &f)
                    .is_some_and(|t| t.token.is_some()),
                "Goblin Bombardment" | "Siege-Gang Commander" => {
                    let ActivationCost::SacrificePermanent { filter, .. } = &a.cost else {
                        continue;
                    };
                    !self.sacrifice_permanents(v, filter).is_empty()
                        && self.sacrifice_lethal(
                            v,
                            &if d.name == "Siege-Gang Commander" {
                                2.into()
                            } else {
                                1.into()
                            },
                        )
                }
                "Mogg Fanatic" => self.sacrifice_lethal(v, &1.into()),
                "Sakura-Tribe Elder" => {
                    (v.active == Some(v.viewer) && v.turn.step == TurnStep::End)
                        || v.stack.iter().any(|s| {
                            let id=s.ability.as_ref().map(|a|a.oracle_id.as_str())
                                .or_else(||s.triggered.as_ref().map(|a|a.oracle_id.as_str()))
                                .or_else(||s.spell.as_ref().and_then(|h|v.objects.iter().find(|o|&o.handle==h)).map(|o|o.oracle_id.as_str()));
                            let Some(d)=id.and_then(|id|definition(self.catalog,id).ok()) else {return false;};
                            let p=if let Some(a)=&s.ability {d.activated.get(a.ability as usize).map(|a|&a.program)}
                                else if let Some(a)=&s.triggered {d.triggered.get(a.ability as usize).map(|a|&a.program)}else {d.spell.as_ref()};
                            let targeted=s.targets.iter().any(|t|t.current.as_ref()==Some(&o.handle))
                                || s.entity_targets.iter().any(|t|matches!(t,mtg_core::VisibleEntityTarget::Object {object} if object.current.as_ref()==Some(&o.handle)));
                            p.is_some_and(|p|p.instructions.iter().any(|i|match i {
                                mtg_contracts::Instruction::DestroyAllCreatures {..}=>true,
                                mtg_contracts::Instruction::DestroyPermanent {..}|mtg_contracts::Instruction::DestroyTargetCreature {..}=>targeted,
                                mtg_contracts::Instruction::DealDamage {amount,..}
                                    |mtg_contracts::Instruction::SacrificeDamage {amount:mtg_contracts::SacrificeDamageAmount::Fixed {value:amount},..}=> {
                                        let mut damage=amount.clone();
                                        for _ in 0..v.objects.iter().filter(|o|o.zone==Zone::Battlefield && definition(self.catalog,&o.oracle_id).is_ok_and(|d|d.static_abilities.iter().any(|a|matches!(a.effect,mtg_contracts::StaticEffect::DoubleDamageToPermanentOrPlayer)))).count() {let copy=damage.clone();damage.add(&copy);}
                                        damage.add(&o.marked_damage);
                                        targeted && o.characteristics.as_ref().and_then(|c|c.toughness.as_ref()).is_some_and(|n|*n<=damage)
                                    }
                                _=>false,
                            }))
                        })
                }
                _ => false,
            };
            if !usable {
                continue;
            }
            let cost = match &a.cost {
                ActivationCost::SacrificePermanent { mana, .. } => {
                    mana.clone().unwrap_or(ManaCost {
                        generic: 0.into(),
                        colored: vec![],
                    })
                }
                ActivationCost::SacrificeSource => ManaCost {
                    generic: 0.into(),
                    colored: vec![],
                },
                _ => continue,
            };
            if let Some(plan) = mana_policy::plan(self.catalog, v, &cost, None) {
                self.mana_plan = plan;
                return Ok(Some(PlayerResponse::ActivateAbility {
                    object: o.handle.clone(),
                    ability: 0,
                }));
            }
        }
        // Additional-cost instants may be cast in an opponent's priority window.
        for o in v
            .objects
            .iter()
            .filter(|o| o.zone == (Zone::Hand { owner: v.viewer }))
        {
            let d = definition(self.catalog, &o.oracle_id)?;
            if d.characteristics.kind != CardKind::Instant
                || d.spell
                    .as_ref()
                    .is_none_or(|p| p.additional_sacrifice.is_none())
                || !self.sacrifice_spell_ready(v, d)
            {
                continue;
            }
            if let Some(plan) =
                mana_policy::plan(self.catalog, v, &d.characteristics.mana_cost, None)
            {
                self.mana_plan = plan;
                return Ok(Some(PlayerResponse::Cast {
                    card: o.handle.clone(),
                }));
            }
        }
        Ok(None)
    }
    pub(super) fn sacrifice_damage_target(
        &self,
        v: &PermittedView,
        candidates: &[PlayerTargetEntity],
    ) -> Option<PlayerTargetEntity> {
        let top = v.stack.last()?;
        let id = top
            .ability
            .as_ref()
            .map(|a| a.oracle_id.as_str())
            .or_else(|| top.triggered.as_ref().map(|a| a.oracle_id.as_str()))
            .or_else(|| {
                top.spell
                    .as_ref()
                    .and_then(|h| v.objects.iter().find(|o| &o.handle == h))
                    .map(|o| o.oracle_id.as_str())
            })?;
        let d = definition(self.catalog, id).ok()?;
        let amount = match d.name.as_str() {
            "Fling" => self
                .sacrifice_victim(v, d.spell.as_ref()?.additional_sacrifice.as_ref()?)?
                .characteristics
                .as_ref()?
                .power
                .clone()?,
            "Siege-Gang Commander" => 2.into(),
            "Mogg Fanatic" | "Goblin Arsonist" | "Goblin Bombardment" => 1.into(),
            _ => return None,
        };
        candidates.iter().find(|t|matches!(t,PlayerTargetEntity::Player {player} if *player!=v.viewer && v.players.iter().any(|p|p.player==*player && p.life<=amount)))
            .or_else(||candidates.iter().find(|t|matches!(t,PlayerTargetEntity::Object {object:handle} if v.objects.iter().any(|o|o.handle==*handle && o.controller!=v.viewer && o.characteristics.as_ref().and_then(|c|c.toughness.as_ref()).is_some_and(|n|n<=&amount)))))
            .or_else(||candidates.iter().find(|t|matches!(t,PlayerTargetEntity::Player {player} if *player!=v.viewer))).cloned()
    }
}
pub(super) fn interactions(
    catalog: &Catalog,
    physical: &[mtg_core::CardSetup],
    g: &Game,
) -> Result<serde_json::Value, String> {
    let name = |id: mtg_core::CardId| {
        physical
            .iter()
            .find(|c| c.id == id)
            .and_then(|c| definition(catalog, &c.oracle_id).ok())
            .map(|d| d.name.as_str())
    };
    let mut costs = 0u64;
    let mut forced = 0u64;
    let mut life = 0u64;
    let mut damage = 0u64;
    let mut fling = Quantity::zero();
    let mut tokens = 0u64;
    let mut dies = BTreeMap::<String, u64>::new();
    for e in g.events() {
        if let mtg_core::Event::TriggerCollected {
            source, ability, ..
        } = e
            && let Some(n) = name(source.card)
            && definition(
                catalog,
                &physical
                    .iter()
                    .find(|c| c.id == source.card)
                    .ok_or("trigger source")?
                    .oracle_id,
            )?
            .triggered
            .get(*ability as usize)
            .is_some_and(|a| {
                matches!(
                    a.condition,
                    mtg_contracts::TriggerCondition::AnyCreatureDies
                        | mtg_contracts::TriggerCondition::SelfDies
                )
            })
        {
            *dies.entry(n.into()).or_default() += 1;
        }
    }
    for e in g.trace().records.iter() {
        let mtg_core::EvidenceRecord::Batch { batch } = &e.record else {
            continue;
        };
        let source = match batch.cause {
            mtg_core::BatchCause::Instruction { source, .. } => name(source.card),
            _ => None,
        };
        for o in &batch.occurrences {
            let v = serde_json::to_value(&o.actual).map_err(|e| e.to_string())?;
            match v["outcome"].as_str() {
                Some("sacrificed") => {
                    if matches!(batch.cause, mtg_core::BatchCause::Instruction { .. }) {
                        forced += 1
                    } else {
                        costs += 1
                    }
                }
                Some("life_lost")
                    if matches!(
                        source,
                        Some("Zulaport Cutthroat" | "Bastion of Remembrance")
                    ) =>
                {
                    life += 1
                }
                Some("damage_committed") => {
                    if let mtg_core::EventOutcome::DamageCommitted { actual, .. } = &o.actual {
                        if source == Some("Vindictive Vampire") {
                            damage += actual.len() as u64;
                        }
                        if source == Some("Fling") {
                            for d in actual {
                                fling.add(&d.amount);
                            }
                        }
                    }
                }
                Some("tokens_created")
                    if matches!(source, Some("Doomed Dissenter" | "Requiem Angel")) =>
                {
                    if let mtg_core::EventOutcome::TokensCreated { objects, .. } = &o.actual {
                        tokens += objects.len() as u64;
                    }
                }
                _ => {}
            }
        }
    }
    Ok(
        serde_json::json!({"cost_sacrifices":costs,"forced_sacrifices":forced,"dies_triggers":dies,"each_opponent_life_loss_events":life,"each_opponent_damage_events":damage,"fling_damage":fling,"tokens_created_on_death":tokens}),
    )
}
