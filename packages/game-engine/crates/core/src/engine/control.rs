//! Layer 2 for complete, independent creature-control Auras. Pinned CR 109.5,
//! 110.2, 302.6, 506.4, 613.1b/7a/7e and 800.4a-c.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl Game {
    fn supported_control_grant(&self, effect: &ControlEffect) -> bool {
        self.is_live(effect.controller)
            && match effect.source {
                ControlSource::Persistent => true,
                ControlSource::Static { object } => self
                    .current(object)
                    .is_ok_and(|source| source.zone == Zone::Battlefield),
            }
    }

    /// Derive the complete post-mutation view before changing any cached value.
    /// Synthetic grants retain their existing authority; they have no timestamp
    /// in the Aura clock, so composing the two orders is explicitly unsupported.
    fn derived_controllers(&self) -> Result<BTreeMap<ObjectId, PlayerId>, String> {
        let mut controllers = BTreeMap::new();
        let mut granted = BTreeSet::new();
        for object in self.state.rules.objects.values() {
            if object.zone != Zone::Battlefield {
                continue;
            }
            let grant = self
                .state
                .rules
                .control_effects
                .iter()
                .filter(|effect| effect.object == object.id && self.supported_control_grant(effect))
                .max_by_key(|effect| effect.id);
            if grant.is_some() {
                granted.insert(object.id);
            }
            controllers.insert(
                object.id,
                grant.map_or(object.default_controller, |effect| effect.controller),
            );
        }

        let mut candidates: BTreeMap<ObjectId, (u64, PlayerId)> = BTreeMap::new();
        let mut timestamps = BTreeSet::new();
        for source in self.state.rules.objects.values() {
            if source.zone != Zone::Battlefield {
                continue;
            }
            let definition = self.definition(source.id.card)?;
            if !definition
                .static_abilities
                .iter()
                .any(|ability| ability.effect == StaticEffect::ControlEnchantedCreature)
            {
                continue;
            }
            if !mtg_contracts::aura_control_source(definition) {
                return Err("unsupported dependent Aura control source".into());
            }
            let Some(target) = source.attached_to else {
                continue;
            };
            // A stale pre-SBA attachment is reachable, but supplies no grant.
            // Never follow a physical card to its successor incarnation.
            if !self.is_battlefield_creature(target) {
                continue;
            }
            let timestamp = source
                .attachment_timestamp
                .filter(|timestamp| {
                    *timestamp > 0 && *timestamp < self.state.rules.next_attachment_timestamp
                })
                .ok_or("missing or invalid Aura control timestamp")?;
            if !timestamps.insert(timestamp) {
                return Err("unsupported simultaneous Aura control timestamps".into());
            }
            let controller = controllers[&source.id];
            // A source with no living controller is about to be exiled under
            // 800.4a/c. It cannot transiently grant control during that cleanup.
            if !self.is_live(controller) {
                continue;
            }
            if granted.contains(&target) {
                return Err("unsupported composed Aura and synthetic control grants".into());
            }
            let candidate = candidates.entry(target).or_insert((timestamp, controller));
            if timestamp > candidate.0 {
                *candidate = (timestamp, controller);
            }
        }
        for (object, (_, controller)) in candidates {
            controllers.insert(object, controller);
        }
        Ok(controllers)
    }

    /// Only layer-2 consequences: no stabilization, trigger placement, priority
    /// change or continuation replacement. The caller owns the atomic boundary.
    pub(super) fn reconcile_controllers(&mut self) -> Result<(), String> {
        let controllers = self.derived_controllers()?;
        let mut changed = vec![];
        let mut exiled = vec![];
        for (object, controller) in controllers {
            let previous = self.current(object)?.controller;
            if !self.is_live(controller) {
                exiled.push(object);
            } else if previous != controller {
                changed.push((object, previous, controller));
            }
        }
        if changed.is_empty() && exiled.is_empty() {
            return Ok(());
        }
        let authority = self.control_before()?;
        // Publish the complete layer-2 view before deriving any layer-6 source
        // facts. Its meaning cannot depend on iteration order among objects.
        for (object, _, controller) in &changed {
            let cached = self.state.rules.objects.get_mut(&object.card).unwrap();
            cached.controller = *controller;
            cached.attack_ready = false;
            self.remove_from_combat(*object);
        }
        for (object, previous, controller) in changed {
            let fact = self.object_before(object)?;
            self.retain_damage_control_facts(&fact);
            self.record_event(Event::ControlChanged {
                object,
                previous,
                controller,
            })?;
        }
        let prior = exiled
            .iter()
            .map(|object| self.object_before(*object))
            .collect::<Result<Vec<_>, _>>()?;
        for object in exiled {
            // A dedicated movement batch avoids commit -> apply_group ->
            // reconciliation recursion, while retaining consumed authority,
            // ordinary zone history, exact incarnation and bounded LKI.
            self.exile_without_controller(object, authority.clone())?;
        }
        for fact in prior {
            self.retain_source_facts(&fact);
        }
        Ok(())
    }

    pub(super) fn validate_control_state(&self) -> Result<(), String> {
        for (object, controller) in self.derived_controllers()? {
            if !self.is_live(controller) || self.current(object)?.controller != controller {
                return Err("snapshot cached controller invariant".into());
            }
        }
        Ok(())
    }
}
