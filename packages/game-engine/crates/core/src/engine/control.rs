//! Layer 2 control from kernel control grants. Pinned CR 109.5, 110.2,
//! 302.6, 506.4, 611.3, 613.1b/7a/7e/8a-c and 800.4a-c.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

impl RulesTxn {
    pub(super) fn control_source_remains(&self, source: ObjectId) -> bool {
        // CR 702.26f: a duration tracking the source ends when the source
        // phases out even though phasing preserves the incarnation.
        self.current(source)
            .is_ok_and(|object| object.present_on_battlefield())
            && self.is_live(self.state.rules.cards[&source.card].owner)
    }

    fn supported_control_grant(&self, effect: &ControlEffect) -> bool {
        self.is_live(effect.controller)
            && match effect.source {
                ControlSource::Persistent => true,
                ControlSource::Static { object } => self.control_battlefield(object),
            }
    }

    /// CR 800.4a: all frozen losing owners' objects are absent from the virtual
    /// battlefield even during the existing sequential physical removal wave.
    fn control_battlefield(&self, object: ObjectId) -> bool {
        self.current(object).is_ok_and(|o| {
            o.present_on_battlefield() && self.is_live(self.state.rules.cards[&object.card].owner)
        })
    }

    /// Derive the complete post-mutation view before changing any cached value.
    pub(crate) fn derived_controllers(&self) -> Result<BTreeMap<ObjectId, PlayerId>, String> {
        let mut resolved = BTreeMap::new();
        for effect in &self.state.rules.effects {
            if let ResolvedEffectKind::GainControl {
                targets,
                controller,
                ..
            } = &effect.kind
                && self.is_live(*controller)
            {
                for object in targets {
                    // CR 611.2c, 613.7b, 613.8: fixed recipients and an
                    // unconditional assignment in creation order.
                    resolved.insert(*object, *controller);
                }
            }
        }
        if !resolved.is_empty() && !self.state.rules.control_effects.is_empty() {
            return Err("resolved and synthetic control grants share no timestamp domain".into());
        }
        let mut controllers = BTreeMap::new();
        for object in self.state.rules.objects.values() {
            if !self.control_battlefield(object.id) {
                continue;
            }
            let grant = self
                .state
                .rules
                .control_effects
                .iter()
                .filter(|effect| effect.object == object.id && self.supported_control_grant(effect))
                .max_by_key(|effect| effect.id);
            controllers.insert(
                object.id,
                resolved.get(&object.id).copied().unwrap_or_else(|| {
                    grant.map_or(object.default_controller, |effect| effect.controller)
                }),
            );
        }
        // A static grant whose exact source needs control-related exile would
        // require removing that grant and reevaluating. Preserve the explicit
        // boundary rather than publish an ID-ordered or stale interpretation.
        if self.state.rules.control_effects.iter().any(|effect| {
            self.supported_control_grant(effect)
                && controllers.contains_key(&effect.object)
                && matches!(effect.source, ControlSource::Static { object }
                    if controllers.get(&object).is_some_and(|controller| !self.is_live(*controller)))
        }) {
            return Err("unsupported dependent synthetic control-source exile".into());
        }
        Ok(controllers)
    }

    /// Only layer-2 consequences: no stabilization, trigger placement, priority
    /// change or continuation replacement. The caller owns the atomic boundary.
    pub(crate) fn reconcile_controllers(&mut self) -> Result<(), String> {
        // CR 800.4c: each further wave begins only after at least one
        // permanent was exiled. That move can end a source-bound grant.
        while self.reconcile_control_wave()? {}
        // CR 506.4: a permanent that stops being a creature, such as a God
        // whose devotion falls (CR 700.5), is removed from combat.
        let gone: Vec<ObjectId> = self
            .state
            .rules
            .combat
            .iter()
            .flat_map(|a| std::iter::once(a.attacker).chain(a.blockers.iter().copied()))
            .chain(self.state.rules.blocking_creatures.iter().copied())
            .filter(|o| {
                self.current(*o)
                    .is_ok_and(|object| object.present_on_battlefield())
                    && !self.is_battlefield_creature(*o)
            })
            .collect();
        for object in gone {
            self.remove_from_combat(object);
        }
        Ok(())
    }

    fn reconcile_control_wave(&mut self) -> Result<bool, String> {
        let ended = self
            .state
            .rules
            .effects
            .iter()
            .filter_map(|effect| {
                let ResolvedEffectKind::GainControl {
                    targets,
                    controller,
                    until,
                } = &effect.kind
                else {
                    return None;
                };
                // A phase-out preserves its incarnation (CR 702.26d).
                (!self.is_live(*controller)
                    || (*until == mtg_contracts::kir::Until::SourceLeaves
                        && !self.control_source_remains(effect.origin.source))
                    || !targets.iter().any(|&id| {
                        self.current(id)
                            .is_ok_and(|object| object.zone == Zone::Battlefield)
                            && self.is_live(self.state.rules.cards[&id.card].owner)
                    }))
                .then_some(effect.id)
            })
            .collect::<BTreeSet<_>>();
        if !ended.is_empty() {
            self.rules_mut()
                .effects
                .retain(|effect| !ended.contains(&effect.id));
        }
        let controllers = self.derived_controllers()?;
        self.grant_hideaway_looks(&controllers);
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
            return Ok(false);
        }
        let authority = self.control_before()?;
        // Publish the complete layer-2 view before deriving any layer-6 source
        // facts. Its meaning cannot depend on iteration order among objects.
        for (object, _, controller) in &changed {
            let upkeeps = self.state.rules.players[controller.0 as usize].upkeeps;
            let cached = self.rules_mut().objects.get_mut(&object.card).unwrap();
            cached.controller = *controller;
            cached.attack_ready = false;
            cached.control_upkeeps = Some(upkeeps);
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
        let exiled_any = !exiled.is_empty();
        for object in exiled {
            // A dedicated movement batch avoids commit -> apply_group ->
            // reconciliation recursion, while retaining consumed authority,
            // ordinary zone history, exact incarnation and bounded LKI.
            self.exile_without_controller(object, authority.clone())?;
        }
        for fact in prior {
            self.retain_source_facts(&fact);
        }
        Ok(exiled_any)
    }

    pub(crate) fn validate_control_state(&self) -> Result<(), String> {
        for (object, controller) in self.derived_controllers()? {
            if !self.is_live(controller) || self.current(object)?.controller != controller {
                return Err("snapshot cached controller invariant".into());
            }
        }
        Ok(())
    }
}
