//! P6.6 completion decisions use only permitted observations and public IR.
use super::*;
use mtg_core::{PermittedView, StackHandle, VisibleObject};

impl Strategy<'_> {
    pub(super) fn four_colors(&self) -> bool {
        self.catalog.schema_version == 16 && self.catalog.definitions.len() == 106
    }
    pub(super) fn opposing_creature<'a>(&self, v: &'a PermittedView) -> Option<&'a VisibleObject> {
        v.objects
            .iter()
            .filter(|o| {
                o.zone == Zone::Battlefield
                    && o.controller != v.viewer
                    && o.characteristics
                        .as_ref()
                        .is_some_and(|c| c.kind == CardKind::Creature)
            })
            .max_by_key(|o| {
                (
                    o.characteristics
                        .as_ref()
                        .and_then(|c| c.power.clone())
                        .unwrap_or_default(),
                    v.commanders.iter().any(|c| c.oracle_id == o.oracle_id),
                    o.handle.0.clone(),
                )
            })
    }
    pub(super) fn opposing_stack(
        &self,
        v: &PermittedView,
        ability: bool,
        candidates: Option<&[StackHandle]>,
    ) -> Option<StackHandle> {
        let entries = v.stack.iter().filter(|s| {
            s.controller != v.viewer
                && s.cast_complete
                && s.handle
                    .as_ref()
                    .is_some_and(|h| candidates.is_none_or(|cs| cs.contains(h)))
        });
        if ability {
            entries
                .filter(|s| s.ability.is_some() || s.triggered.is_some())
                .find_map(|s| s.handle.clone())
        } else {
            entries
                .filter_map(|s| {
                    let h = s.spell.as_ref()?;
                    let o = v.objects.iter().find(|o| &o.handle == h)?;
                    let d = definition(self.catalog, &o.oracle_id).ok()?;
                    let mut cost = d.characteristics.mana_cost.generic.clone();
                    cost.add(&Quantity::from(
                        d.characteristics.mana_cost.colored.len() as i64
                    ));
                    Some((cost, s.handle.clone()?))
                })
                .max_by(|a, b| a.0.cmp(&b.0).then(a.1.0.cmp(&b.1.0)))
                .map(|(_, h)| h)
        }
    }
    pub(super) fn four_colors_priority(
        &mut self,
        v: &PermittedView,
    ) -> Result<Option<PlayerResponse>, String> {
        for name in ["Counterspell", "Stifle"] {
            if self.opposing_stack(v, name == "Stifle", None).is_none() {
                continue;
            }
            if let Some(o) = v.objects.iter().find(|o| {
                o.zone == Zone::Hand { owner: v.viewer }
                    && definition(self.catalog, &o.oracle_id).is_ok_and(|d| d.name == name)
            }) {
                let cost = &definition(self.catalog, &o.oracle_id)?
                    .characteristics
                    .mana_cost;
                if let Some(plan) = mana_policy::plan(self.catalog, v, cost, None) {
                    self.mana_plan = plan;
                    self.beneficial = false;
                    return Ok(Some(PlayerResponse::Cast {
                        card: o.handle.clone(),
                    }));
                }
            }
        }
        // Repeatable draw abilities are used in late own main phases, only
        // while a real draw remains. Tap readiness comes from the public row.
        if v.active != Some(v.viewer)
            || !v.stack.is_empty()
            || v.turn.step != TurnStep::PostcombatMain
            || v.turn.number < 20
        {
            return Ok(None);
        }
        if !v
            .players
            .iter()
            .find(|p| p.player == v.viewer)
            .is_some_and(|p| p.library_count > 1)
        {
            return Ok(None);
        }
        for o in &v.objects {
            if o.zone != Zone::Battlefield || o.controller != v.viewer || o.token.is_some() {
                continue;
            }
            let d = definition(self.catalog, &o.oracle_id)?;
            if !matches!(
                d.name.as_str(),
                "Merfolk Looter" | "Azure Mage" | "Hapless Researcher" | "Prodigal Sorcerer"
            ) {
                continue;
            }
            for (ability, a) in d.activated.iter().enumerate() {
                let cost = match &a.cost {
                    mtg_contracts::ActivationCost::TapSource => {
                        if o.tapped || !o.attack_ready {
                            continue;
                        }
                        ManaCost {
                            generic: Quantity::zero(),
                            colored: vec![],
                        }
                    }
                    mtg_contracts::ActivationCost::SacrificeSource => ManaCost {
                        generic: Quantity::zero(),
                        colored: vec![],
                    },
                    mtg_contracts::ActivationCost::Mana { cost } => cost.clone(),
                    _ => continue,
                };
                if let Some(plan) = mana_policy::plan(self.catalog, v, &cost, Some(&o.handle)) {
                    self.mana_plan = plan;
                    self.beneficial = false;
                    return Ok(Some(PlayerResponse::ActivateAbility {
                        object: o.handle.clone(),
                        ability: ability as u32,
                    }));
                }
            }
        }
        Ok(None)
    }
    pub(super) fn four_colors_entities(
        &self,
        v: &PermittedView,
        candidates: &[PlayerTargetEntity],
    ) -> Result<PlayerTargetEntity, String> {
        if let Some(o) = self.opposing_creature(v)
            && o.characteristics
                .as_ref()
                .and_then(|c| c.power.as_ref())
                .is_some_and(|p| *p > Quantity::zero())
            && candidates.contains(&PlayerTargetEntity::Object {
                object: o.handle.clone(),
            })
        {
            return Ok(PlayerTargetEntity::Object {
                object: o.handle.clone(),
            });
        }
        candidates
            .iter()
            .filter_map(|e| match e {
                PlayerTargetEntity::Player { player } if *player != v.viewer => v
                    .players
                    .iter()
                    .find(|p| p.player == *player)
                    .map(|p| (p.life.clone(), p.player)),
                _ => None,
            })
            .min_by_key(|(life, p)| (life.clone(), *p))
            .map(|(_, player)| PlayerTargetEntity::Player { player })
            .ok_or("no opposing damage target".into())
    }
}

/// Count committed outcomes from a Detailed replay, never policy inputs.
pub(super) fn interactions(
    catalog: &Catalog,
    physical: &[mtg_core::CardSetup],
    g: &Game,
) -> Result<serde_json::Value, String> {
    let (mut spells, mut abilities, mut bounces, mut tucks, mut doubled) =
        (0u64, 0u64, 0u64, 0u64, 0u64);
    let mut before = Quantity::zero();
    let mut after = Quantity::zero();
    let mut casts = BTreeMap::from([
        ("W".to_string(), 0u64),
        ("B".into(), 0),
        ("G".into(), 0),
        ("U".into(), 0),
        ("R".into(), 0),
    ]);
    let mut proposed = BTreeMap::new();
    for event in g.events() {
        match event {
            mtg_core::Event::SpellProposed { card, stack_id } => {
                proposed.insert(*stack_id, *card);
            }
            mtg_core::Event::SpellCast { stack_id } => {
                if let Some(c) = proposed
                    .get(stack_id)
                    .and_then(|id| physical.iter().find(|c| c.id == *id))
                    && c.id.0 % 100 == 0
                {
                    for color in &definition(catalog, &c.oracle_id)?
                        .characteristics
                        .color_identity
                    {
                        let color = format!("{color:?}");
                        *casts.entry(color).or_default() += 1;
                    }
                }
            }
            mtg_core::Event::Committed { actual, .. } => match actual {
                mtg_core::EventOutcome::StackObjectCountered { spell, .. } => {
                    if *spell {
                        spells += 1;
                    } else {
                        abilities += 1;
                    }
                }
                mtg_core::EventOutcome::Moved { movement }
                    if movement.from == Zone::Battlefield
                        && matches!(movement.destination, Zone::Hand { .. }) =>
                {
                    bounces += 1
                }
                mtg_core::EventOutcome::LibraryTopMoved { .. } => tucks += 1,
                mtg_core::EventOutcome::DamageEffectApplied { application }
                    if matches!(application.effect, mtg_core::EffectKey::Static { .. })
                        && application.after > application.before =>
                {
                    doubled += 1;
                    before.add(&application.before);
                    after.add(&application.after);
                }
                _ => {}
            },
            _ => {}
        }
    }
    Ok(
        serde_json::json!({"spells_countered":spells,"abilities_countered":abilities,"battlefield_to_hand_moves":bounces,"battlefield_to_library_moves":tucks,"furnace_damage_occurrences_doubled":doubled,"furnace_damage_before":before,"furnace_damage_after":after,"commander_card_casts_by_color":casts}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reactive_policy_uses_opposing_cost_and_power_and_respects_payment() {
        let c = mtg_compiler::four_colors_fixture_catalog().unwrap();
        let player = PlayerId(0);
        let card = |name: &str, owner: u8, zone: Zone| {
            let d = c.definitions.iter().find(|d| d.name == name).unwrap();
            VisibleObject {
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
        };
        let mut v = PermittedView {
            setup: None,
            effects: vec![],
            combat: vec![],
            viewer: player,
            active: Some(PlayerId(1)),
            turn_owner: PlayerId(1),
            turn: mtg_core::TurnState {
                number: 10,
                step: TurnStep::PrecombatMain,
                lands_played: 1,
            },
            live_turn_order: (0..4).map(PlayerId).collect(),
            commanders: vec![],
            priority: Some(player),
            players: (0..4)
                .map(|i| mtg_core::VisiblePlayer {
                    library_shuffles: 0,
                    player: PlayerId(i),
                    status: mtg_core::PlayerStatus::Live,
                    life: Quantity::from(40 - i64::from(i)),
                    mana: BTreeMap::new(),
                    hand_count: 1,
                    library_count: 20,
                })
                .collect(),
            objects: vec![
                card("Counterspell", 0, Zone::Hand { owner: player }),
                card("Island", 0, Zone::Battlefield),
                card("Sol Ring", 0, Zone::Stack),
                card("Rorix Bladewing", 1, Zone::Stack),
                card("Goliath Sphinx", 2, Zone::Stack),
                card("Storm Crow", 1, Zone::Battlefield),
                card("Caelorna, Coral Tyrant", 2, Zone::Battlefield),
            ],
            stack: vec![],
        };
        for (name, owner) in [
            ("Sol Ring", 0),
            ("Rorix Bladewing", 1),
            ("Goliath Sphinx", 2),
        ] {
            v.stack.push(mtg_core::VisibleStackEntry {
                mode: None,
                triggered: None,
                entity_targets: vec![],
                handle: Some(StackHandle(name.into())),
                stack_targets: vec![],
                spell: Some(ObjectHandle(name.into())),
                ability: None,
                player_targets: vec![],
                controller: PlayerId(owner),
                cast_complete: true,
                targets: vec![],
            });
        }
        let mut s = Strategy {
            catalog: &c,
            policy: Policy::Develop,
            mulligans: [0; 4],
            beneficial: false,
            mana_plan: Default::default(),
            mana_color: None,
        };
        assert_eq!(
            s.opposing_stack(&v, false, None),
            Some(StackHandle("Goliath Sphinx".into()))
        );
        assert!(s.four_colors_priority(&v).unwrap().is_none()); // Only one Island.
        let mut second = card("Island", 0, Zone::Battlefield);
        second.handle = ObjectHandle("second Island".into());
        v.objects.push(second);
        assert_eq!(
            s.four_colors_priority(&v).unwrap(),
            Some(PlayerResponse::Cast {
                card: ObjectHandle("Counterspell".into())
            })
        );
        let entities = vec![
            PlayerTargetEntity::Player {
                player: PlayerId(1),
            },
            PlayerTargetEntity::Player {
                player: PlayerId(3),
            },
            PlayerTargetEntity::Object {
                object: ObjectHandle("Storm Crow".into()),
            },
        ];
        assert_eq!(s.four_colors_entities(&v, &entities).unwrap(), entities[2]);
        v.objects.retain(|o| o.handle.0 != "Storm Crow");
        assert_eq!(s.four_colors_entities(&v, &entities).unwrap(), entities[1]);
        v.stack[0].spell = None;
        v.stack[0].controller = PlayerId(1);
        v.stack[0].ability = Some(mtg_core::VisibleAbility {
            oracle_id: definition(&c, &v.objects[0].oracle_id)
                .unwrap()
                .oracle_id
                .clone(),
            ability: 0,
            handle: StackHandle("Sol Ring".into()),
            source: mtg_core::VisibleTarget {
                announced: ObjectHandle("source".into()),
                current: None,
            },
        });
        assert_eq!(
            s.opposing_stack(&v, true, None),
            Some(StackHandle("Sol Ring".into()))
        );
    }
}
