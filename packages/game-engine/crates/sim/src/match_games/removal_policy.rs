//! P6.8 completion policy. Every choice uses only permitted observation and IR.
use super::*;
use mtg_contracts::{Definition, PermanentFilter, TargetDomain};
use mtg_core::{PermittedView, StackHandle, VisibleObject};
impl Strategy<'_> {
    fn removal_candidate(&self, v: &PermittedView, o: &VisibleObject, f: &PermanentFilter) -> bool {
        o.zone == Zone::Battlefield
            && self.targetable(v, o)
            && (!f.opponent || o.controller != v.viewer)
            && o.characteristics.as_ref().is_some_and(|c| {
                (f.types.is_empty() || f.types.contains(&c.kind))
                    && !f.non_types.contains(&c.kind)
                    && !c.colors.iter().any(|c| f.non_colors.contains(c))
            })
    }
    fn removal_best(
        &self,
        v: &PermittedView,
        d: &Definition,
        candidates: Option<&[ObjectHandle]>,
    ) -> Option<ObjectHandle> {
        let p = if let Some(a) = d.triggered.first() {
            &a.program
        } else if let Some(a) = d.activated.first() {
            &a.program
        } else {
            d.spell.as_ref()?
        };
        let t = p.targets.first()?;
        if t.domain != TargetDomain::BattlefieldPermanent {
            return None;
        }
        let f = t.filter.as_ref()?;
        let creature = f.types == [CardKind::Creature];
        v.objects
            .iter()
            .filter(|o| {
                if !self.removal_candidate(v, o, f)
                    || candidates.is_some_and(|cs| !cs.contains(&o.handle))
                {
                    return false;
                }
                let own_pacifism = definition(self.catalog, &o.oracle_id)
                    .is_ok_and(|d| d.name == "Pacifism")
                    && o.attached_to.as_ref().is_some_and(|h| {
                        v.objects
                            .iter()
                            .any(|c| &c.handle == h && c.controller == v.viewer)
                    });
                (o.controller != v.viewer || own_pacifism)
                    && (!matches!(d.name.as_str(), "Beast Within" | "Generous Gift")
                        || o.characteristics
                            .as_ref()
                            .is_some_and(|c| c.kind != CardKind::Land))
            })
            .max_by_key(|o| {
                let c = o.characteristics.as_ref().unwrap();
                let own_pacifism = definition(self.catalog, &o.oracle_id)
                    .is_ok_and(|d| d.name == "Pacifism")
                    && o.attached_to.as_ref().is_some_and(|h| {
                        v.objects
                            .iter()
                            .any(|c| &c.handle == h && c.controller == v.viewer)
                    });
                let mut mv = c.mana_cost.generic.clone();
                mv.add(&Quantity::from(c.mana_cost.colored.len() as i64));
                (
                    own_pacifism,
                    if creature {
                        c.power.clone().unwrap_or_default()
                    } else {
                        mv
                    },
                    o.handle.0.clone(),
                )
            })
            .map(|o| o.handle.clone())
    }
    pub(super) fn swan_target(
        &self,
        v: &PermittedView,
        candidates: Option<&[StackHandle]>,
    ) -> Option<StackHandle> {
        v.stack
            .iter()
            .filter(|s| s.controller != v.viewer && s.cast_complete)
            .find_map(|s| {
                let h = s.handle.as_ref()?;
                if candidates.is_some_and(|cs| !cs.contains(h)) {
                    return None;
                }
                let o = v
                    .objects
                    .iter()
                    .find(|o| Some(&o.handle) == s.spell.as_ref())?;
                definition(self.catalog, &o.oracle_id)
                    .ok()
                    .filter(|d| {
                        matches!(
                            d.characteristics.kind,
                            CardKind::Enchantment | CardKind::Instant | CardKind::Sorcery
                        )
                    })
                    .map(|_| h.clone())
            })
    }
    pub(super) fn optional_removal(&self, v: &PermittedView) -> bool {
        let Some(target) = v
            .stack
            .last()
            .and_then(|s| s.targets.first())
            .and_then(|t| t.current.as_ref())
        else {
            return false;
        };
        v.objects
            .iter()
            .find(|o| &o.handle == target)
            .is_some_and(|o| {
                o.controller != v.viewer
                    || (definition(self.catalog, &o.oracle_id).is_ok_and(|d| d.name == "Pacifism")
                        && o.attached_to.as_ref().is_some_and(|h| {
                            v.objects
                                .iter()
                                .any(|c| &c.handle == h && c.controller == v.viewer)
                        }))
            })
    }
    pub(super) fn removal_spell_ready(&self, v: &PermittedView, d: &Definition) -> bool {
        d.name != "Swan Song" && self.removal_best(v, d, None).is_some()
    }
    pub(super) fn removal_priority(
        &mut self,
        v: &PermittedView,
    ) -> Result<Option<PlayerResponse>, String> {
        if self.swan_target(v, None).is_some()
            && let Some(o) = v.objects.iter().find(|o| {
                o.zone == (Zone::Hand { owner: v.viewer })
                    && definition(self.catalog, &o.oracle_id).is_ok_and(|d| d.name == "Swan Song")
            })
        {
            let d = definition(self.catalog, &o.oracle_id)?;
            if let Some(plan) =
                mana_policy::plan(self.catalog, v, &d.characteristics.mana_cost, None)
            {
                self.mana_plan = plan;
                self.beneficial = false;
                return Ok(Some(PlayerResponse::Cast {
                    card: o.handle.clone(),
                }));
            }
        }
        for o in v.objects.iter().filter(|o| {
            o.zone == Zone::Battlefield && o.controller == v.viewer && o.token.is_none()
        }) {
            let d = definition(self.catalog, &o.oracle_id)?;
            if d.name != "Viridian Zealot" || self.removal_best(v, d, None).is_none() {
                continue;
            }
            let mtg_contracts::ActivationCost::Composite { costs } = &d.activated[0].cost else {
                return Err("Zealot cost invariant".into());
            };
            if let Some(plan) = mana_policy::plan(
                self.catalog,
                v,
                costs.mana.as_ref().ok_or("Zealot mana cost")?,
                Some(&o.handle),
            ) {
                self.mana_plan = plan;
                return Ok(Some(PlayerResponse::ActivateAbility {
                    object: o.handle.clone(),
                    ability: 0,
                }));
            }
        }
        Ok(None)
    }
    pub(super) fn removal_target(
        &self,
        v: &PermittedView,
        candidates: &[ObjectHandle],
    ) -> Result<Option<ObjectHandle>, String> {
        let Some(s) = v.stack.last() else {
            return Ok(None);
        };
        let id = if let Some(t) = &s.triggered {
            Some(t.oracle_id.as_str())
        } else if let Some(a) = &s.ability {
            Some(a.oracle_id.as_str())
        } else {
            s.spell
                .as_ref()
                .and_then(|h| v.objects.iter().find(|o| &o.handle == h))
                .map(|o| o.oracle_id.as_str())
        };
        let Some(id) = id else { return Ok(None) };
        let d = definition(self.catalog, id)?;
        let p = if let Some(t) = &s.triggered {
            &d.triggered[t.ability as usize].program
        } else if let Some(a) = &s.ability {
            &d.activated[a.ability as usize].program
        } else {
            d.spell.as_ref().ok_or("removal spell")?
        };
        if !p
            .targets
            .first()
            .is_some_and(|t| t.domain == TargetDomain::BattlefieldPermanent)
        {
            return Ok(None);
        }
        // Mandatory triggered targets may include our own permanents; if no
        // preferred enemy remains, select a legal target and decide "may" later.
        Ok(self
            .removal_best(v, d, Some(candidates))
            .or_else(|| candidates.first().cloned()))
    }
    pub(super) fn is_swan_announcement(&self, v: &PermittedView) -> bool {
        v.stack
            .last()
            .and_then(|s| s.spell.as_ref())
            .and_then(|h| v.objects.iter().find(|o| &o.handle == h))
            .is_some_and(|o| {
                definition(self.catalog, &o.oracle_id).is_ok_and(|d| d.name == "Swan Song")
            })
    }
}

/// Independent trusted evidence adapter. Counts the actual Detailed replay
/// outcomes (the enum tag is "outcome", never "event"). Policies do not use it.
pub(super) fn interactions(
    catalog: &Catalog,
    e: &Evidence,
    physical: &[mtg_core::CardSetup],
    g: &Game,
) -> Result<serde_json::Value, String> {
    let mut destroyed = BTreeMap::<String, u64>::from([
        ("artifact".into(), 0),
        ("enchantment".into(), 0),
        ("creature".into(), 0),
        ("land".into(), 0),
    ]);
    let mut exiles = 0u64;
    let mut other_tokens = 0u64;
    let mut swans = 0u64;
    for stored in g.trace().records.iter() {
        let mtg_core::EvidenceRecord::Batch { batch } = &stored.record else {
            continue;
        };
        let invocation = match &batch.cause {
            mtg_core::BatchCause::Instruction { source, player, .. } => Some((*source, *player)),
            _ => None,
        };
        for occurrence in &batch.occurrences {
            let actual = serde_json::to_value(&occurrence.actual).map_err(|e| e.to_string())?;
            match actual["outcome"].as_str() {
                Some("moved") | Some("state_based_moved") => {
                    let movement: mtg_core::MoveOutcome =
                        serde_json::from_value(actual["movement"].clone())
                            .map_err(|e| e.to_string())?;
                    let destruction = matches!(
                        occurrence.proposed,
                        mtg_core::ProposedEvent::DestroyPermanent { .. }
                            | mtg_core::ProposedEvent::DestroyCreature { .. }
                    ) || matches!(
                        occurrence.actual,
                        mtg_core::EventOutcome::StateBasedMoved {
                            reason: mtg_core::CreatureLossReason::LethalDamage
                                | mtg_core::CreatureLossReason::Deathtouch,
                            ..
                        }
                    );
                    if destruction {
                        let kind = if let Some(c) =
                            physical.iter().find(|c| c.id == movement.previous.card)
                        {
                            definition(catalog, &c.oracle_id)?.characteristics.kind
                        } else {
                            CardKind::Creature
                        };
                        let key = match kind {
                            CardKind::Artifact => "artifact",
                            CardKind::Enchantment => "enchantment",
                            CardKind::Land => "land",
                            CardKind::Creature => "creature",
                            _ => return Err("destroyed nonpermanent".into()),
                        };
                        *destroyed.get_mut(key).ok_or("destruction type")? += 1;
                    }
                    if movement.from == Zone::Battlefield && movement.destination == Zone::Exile {
                        exiles += 1;
                    }
                }
                Some("tokens_created") => {
                    if let Some((_, caster)) = invocation {
                        let mtg_core::EventOutcome::TokensCreated {
                            controller,
                            objects,
                            ..
                        } = &occurrence.actual
                        else {
                            return Err("token outcome tag mismatch".into());
                        };
                        if *controller != caster {
                            other_tokens += objects.len() as u64;
                        }
                    }
                }
                Some("stack_object_countered") => {
                    if let Some((source, _)) = invocation
                        && physical
                            .iter()
                            .find(|c| c.id == source.card)
                            .is_some_and(|c| {
                                definition(catalog, &c.oracle_id)
                                    .is_ok_and(|d| d.name == "Swan Song")
                            })
                    {
                        swans += 1;
                    }
                }
                _ => {}
            }
        }
    }
    // Replay target decisions to count otherwise-domain-legal candidates removed
    // by hexproof/shroud. This includes the new permanent filters, all old
    // creature targets and damage targets. Counts are per selection scan.
    let mut replay = Game::with_trace(
        Prepared::admit_pod(catalog.clone(), e.config.clone()).map_err(|e| e.to_string())?,
        TraceMode::Off,
    );
    if !e.journal.concessions.is_empty() {
        return Err("removal diagnostic requires natural games".into());
    }
    let mut hexproof = 0u64;
    let mut shroud = 0u64;
    for input in &e.journal.decisions {
        let Advance::AwaitingDecision(r) = replay.advance(Budget::unlimited()) else {
            return Err("removal diagnostic decision".into());
        };
        if let mtg_core::Request::Targets { occurrences, .. }
        | mtg_core::Request::EntityTargets { occurrences, .. } = &r
        {
            let t = occurrences.first().ok_or("empty target occurrence")?;
            let QueryResult::Complete(QueryValue::Observation(o)) =
                replay.observe(r.player(), Budget::unlimited())
            else {
                return Err("removal diagnostic observation".into());
            };
            for object in &o.view.objects {
                let Some(c) = &object.characteristics else {
                    continue;
                };
                if object.zone != Zone::Battlefield {
                    continue;
                }
                let eligible = match t.domain {
                    TargetDomain::BattlefieldPermanent => t.filter.as_ref().is_some_and(|f| {
                        (f.types.is_empty() || f.types.contains(&c.kind))
                            && !f.non_types.contains(&c.kind)
                            && !c.colors.iter().any(|c| f.non_colors.contains(c))
                            && (!f.opponent || object.controller != r.player())
                    }),
                    TargetDomain::BattlefieldCreature | TargetDomain::AnyDamageTarget => {
                        c.kind == CardKind::Creature
                    }
                    TargetDomain::ControlledBattlefieldCreature => {
                        c.kind == CardKind::Creature && object.controller == r.player()
                    }
                    _ => false,
                };
                if !eligible {
                    continue;
                }
                if object
                    .keywords
                    .contains(&mtg_contracts::KeywordAbility::Shroud)
                {
                    shroud += 1;
                } else if object.controller != r.player()
                    && object
                        .keywords
                        .contains(&mtg_contracts::KeywordAbility::Hexproof)
                {
                    hexproof += 1;
                }
            }
        }
        replay.respond(input.player, input.decision_id, input.response.clone())?;
    }
    if !matches!(
        replay.advance(Budget::unlimited()),
        Advance::Finished { .. }
    ) || replay.state_digest()? != e.final_state_sha256
    {
        return Err("removal diagnostic final replay mismatch".into());
    }
    Ok(
        serde_json::json!({"destructions_by_permanent_type":destroyed,"exiles":exiles,"tokens_created_for_other_player":other_tokens,"swan_song_counters":swans,"targets_excluded_by_hexproof":hexproof,"targets_excluded_by_shroud":shroud,"destruction_unit":"committed destroy instructions and lethal/deathtouch SBAs; Aura put-into-graveyard SBAs are excluded","exclusion_unit":"objects per target-decision candidate scan; shroud takes precedence"}),
    )
}
