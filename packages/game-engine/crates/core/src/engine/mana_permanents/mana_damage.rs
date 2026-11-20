//! Immediate damage from the disjoint mana family and its payment reversal.
use super::*;

impl Game {
    pub(super) fn begin_mana_damage(
        &mut self,
        a: ManaActivation,
        amount: Quantity,
    ) -> Result<(), String> {
        // CR 605.3b: mana is already produced. The damage is an effect of
        // this paid invocation, so the continuation owns the rest of payment.
        let damage = PreparedDamage {
            source_appearance: None,
            source_handles: self.capture_trigger_source_handles(a.source)?,
            id: DamageId(0),
            source: self.object_before(a.source)?,
            controller: a.player,
            recipient: DamageRecipient::Player { player: a.player },
            original_amount: amount.clone(),
            amount,
            applied: vec![],
        };
        self.begin_damage(
            DamageKind::Noncombat,
            vec![damage],
            a.cause(),
            DamageContinuation::Mana {
                activation: Box::new(a),
                departed: false,
            },
        )?;
        // A plain occurrence can commit in the same staged step. Replacement
        // applications retain their ordinary budgeting and choice boundaries.
        if self.damage_effect_group()?.is_none() {
            self.damage_effect_step()?;
        }
        Ok(())
    }

    pub(super) fn mana_damage_resume_available(&self, a: &ManaActivation) -> bool {
        self.is_live(a.player)
            && match self.underlying_mana_frame(&a.resume) {
                Frame::Priority => self.state.rules.priority == a.player,
                Frame::ResolutionPayment { payment, .. } => {
                    payment.payer == a.player && payment.stage == ResolutionPaymentStage::Mana
                }
                Frame::Mana { cast } => self
                    .top()
                    .is_ok_and(|top| top.id == cast.stack_id && top.controller == a.player),
                _ => false,
            }
    }

    pub(in crate::engine) fn validate_mana_damage(
        &self,
        tx: &DamageTransaction,
        a: &ManaActivation,
        departed: bool,
    ) -> Result<(), String> {
        let definition = self.copiable_definition(a.source)?;
        let branch = a.branch.ok_or("missing paid mana branch")?;
        let receipt = a.paid.as_ref().ok_or("unpaid mana damage")?;
        let [damage] = tx.entries.as_slice() else {
            return Err("invalid mana damage group".into());
        };
        if !disjoint_pain_mana_family(definition)
            || branch.index != 1
            || a.stage != ManaStage::Payment
            || !a.receipts.is_empty()
            || definition.mana_abilities.get(branch.index as usize) != Some(&a.ability)
            || !self
                .mana_colors(a.player, &a.ability.production)?
                .contains(&branch.color)
            || a.ability.self_damage.as_ref() != Some(&damage.original_amount)
            || tx.cause != a.cause()
            || !tx.allocation_groups.is_empty()
            || damage.recipient != (DamageRecipient::Player { player: a.player })
            || damage.controller != a.player
            || damage.source.object != a.source
            || damage.source.zone != Zone::Battlefield
            || damage.source.controller != a.player
            || !damage.source.tapped
            || receipt.player != a.player
            || receipt.branch != Some(branch)
            || receipt.source.object != a.source
            || receipt.source.zone != Zone::Battlefield
            || receipt.source.owner != damage.source.owner
            || receipt.source.controller != a.player
            || receipt.source.tapped
            || receipt.produced != BTreeMap::from([(branch.color, Quantity::from(1))])
            || !receipt.spent.is_empty()
            || receipt.ability_use.is_some()
            || receipt.sacrificed
            || receipt.damage.is_some()
        {
            return Err("unbound mana damage transaction".into());
        }
        if self.mana_damage_resume_available(a) {
            if !self.mana_resume_player(&a.resume, a.player) {
                return Err("invalid mana damage continuation".into());
            }
            self.validate_nested_mana_frame(&a.resume)?;
        } else if !departed {
            return Err("missing suspended mana payment".into());
        }
        Ok(())
    }

    pub(in crate::engine) fn finish_mana_damage(
        &mut self,
        mut a: ManaActivation,
        applications: Vec<DamageApplication>,
        actual: EventOutcome,
        first_trigger: u64,
        departed: bool,
    ) -> Result<(), String> {
        let EventOutcome::DamageCommitted {
            results,
            life_gains,
            ..
        } = actual
        else {
            return Err("missing actual mana damage outcome".into());
        };
        let mut life = vec![];
        for result in results {
            let DamageRecipient::Player { player } = result.recipient else {
                return Err("mana damage affected a nonplayer".into());
            };
            life.push(ManaDamageLife {
                player,
                before: result.previous,
                after: result.current,
            });
        }
        for gain in life_gains {
            life.push(ManaDamageLife {
                player: gain.player,
                before: gain.previous,
                after: gain.current,
            });
        }
        let mut shields = vec![];
        for application in applications {
            if let (EffectKey::Resolved { id }, Some(before), Some(after)) = (
                application.effect,
                application.budget_before,
                application.budget_after,
            ) {
                shields.push(ManaDamageShield {
                    effect: id,
                    before,
                    after,
                });
            }
        }
        let triggers = self
            .state
            .rules
            .pending_triggers
            .iter()
            .filter(|trigger| trigger.id.0 >= first_trigger)
            .map(|trigger| (trigger.id, trigger.controller))
            .collect();
        a.paid.as_mut().ok_or("missing mana damage receipt")?.damage =
            Some(Box::new(ManaDamageUndo {
                life,
                shields,
                triggers,
            }));
        if !self.mana_damage_resume_available(&a) {
            // A departure may remove the payer or the suspended announcement.
            // Its legal paid activation stands, with any invalid damage cancelled
            // by the ordinary transaction. Leave their last-known state alone.
            if let frame @ Frame::ResolutionPayment { .. } =
                spell_cascade_grants::retained_effect_frame(&a.resume)
            {
                self.state.rules.frame = frame.clone();
                return self.revalidate_resolution_payment_departure();
            }
            self.state.rules.passes.clear();
            if let Frame::Mana { cast } = spell_cascade_grants::retained_effect_frame(&a.resume)
                && let CastCompletion::Resolution {
                    progress: CastOwner::LinkedPlay { progress, .. },
                    ..
                } = &cast.completion
                && self.is_live(progress.frame.spell.controller)
            {
                self.state.rules.frame = Frame::PlayHideawayLinked {
                    progress: progress.clone(),
                };
                return Ok(());
            }
            self.state.rules.frame = Frame::Priority;
            return self.stabilize();
        }
        self.finish_mana_ability(a)?;
        if departed {
            self.revalidate_mana_departure()?;
        }
        Ok(())
    }

    pub(super) fn reverse_mana_damage(
        &mut self,
        payer: PlayerId,
        damage: &ManaDamageUndo,
    ) -> Result<(), String> {
        for &(id, controller) in &damage.triggers {
            if let Some(index) = self
                .state
                .rules
                .pending_triggers
                .iter()
                .position(|t| t.id == id)
            {
                if self.state.rules.pending_triggers[index].controller != controller {
                    return Err("mana damage trigger changed".into());
                }
                self.state.rules.pending_triggers.remove(index);
            } else if self.is_live(controller) {
                return Err("missing pending mana damage trigger".into());
            }
        }
        for shield in damage.shields.iter().rev() {
            let effect = self
                .state
                .rules
                .effects
                .iter_mut()
                .find(|e| e.id == shield.effect)
                .ok_or("missing mana damage prevention effect")?;
            let ResolvedEffectKind::PreventNextDamage { target, remaining } = &mut effect.kind
            else {
                return Err("mana damage prevention effect changed kind".into());
            };
            if *target != (TargetEntity::Player { player: payer }) || *remaining != shield.after {
                return Err("mana damage prevention effect changed".into());
            }
            *remaining = shield.before.clone();
        }
        // Damage is written before lifelink, including when both affect the
        // same player. Reverse this ordered chain without touching departures.
        for entry in damage.life.iter().rev() {
            if self.is_live(entry.player) {
                let life = &mut self.state.rules.players[entry.player.0 as usize].life;
                if *life != entry.after {
                    return Err("mana damage life changed".into());
                }
                *life = entry.before.clone();
            }
        }
        Ok(())
    }
}
