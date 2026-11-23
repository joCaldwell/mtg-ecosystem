//! Whole-procedure lowerers the shared rule lowerers call. Each consumes one
//! complete typed AST shape (a sentence, a cost and its effect, a static or a
//! trigger condition) and emits its dedicated instruction. None reads a
//! diagnostic binding graph, card text or identity: the record's Archive R4
//! resolution is certified before any block is lowered. Each moves to Kernel
//! IR, or is deleted with its legacy `Instruction` variant, in Phase 2C-2.
use super::*;
use serde_json::{Value, json};

pub(crate) mod controller_library {
    use super::*;
    pub(crate) fn fixed_count(amount: &serde_json::Value) -> Result<Quantity, Failure> {
        let count: Count = serde_json::from_value(amount.clone()).map_err(|_| {
            unsupported(
                "library count",
                "requires complete fixed nonnegative amount",
            )
        })?;
        if count.amount != "fixed" {
            return Err(unsupported(
                "library count",
                "requires fixed nonnegative amount",
            ));
        }
        Ok(Quantity::from(i64::from(count.value)))
    }

    pub(crate) fn instruction(
        amount: &serde_json::Value,
        source_block: u32,
    ) -> Result<Instruction, Failure> {
        Ok(Instruction::Scry {
            amount: fixed_count(amount)?,
            source_block,
        })
    }

    pub(crate) fn surveil_instruction(
        amount: &serde_json::Value,
        source_block: u32,
    ) -> Result<Instruction, Failure> {
        Ok(Instruction::Surveil {
            amount: fixed_count(amount)?,
            source_block,
        })
    }

    pub(crate) fn artifact_creature_target(what: &serde_json::Value) -> bool {
        lower_permanent_filter(&what["filter"]).is_ok_and(|f| {
            f.types.len() == 2
                && f.types.contains(&CardKind::Creature)
                && f.types.contains(&CardKind::Artifact)
                && f.non_types.is_empty()
                && f.non_colors.is_empty()
                && !f.opponent
                && !f.tapped
        })
    }

    pub(crate) fn bind_grant_target(
        p: &mut SpellProgram,
        slots: &mut BTreeMap<String, u32>,
        what: &serde_json::Value,
    ) -> Result<u32, Failure> {
        if !artifact_creature_target(what) {
            return Err(unsupported(
                "targets",
                "requires unqualified artifact-or-creature selector",
            ));
        }
        let slot = bind_permanent_target(p, slots, what)?;
        // Alternatives are commutative. Canonicalize ONLY this new closed family;
        // preserve every historical permanent-removal filter's serialized order.
        p.targets[slot as usize]
            .filter
            .as_mut()
            .ok_or_else(|| unsupported("targets", "missing permanent filter"))?
            .types
            .sort();
        Ok(slot)
    }

    // Match both directions: an edited AST must not erase a printed lookup,
    // and an injected lookup cannot borrow a different printed effect.
    pub(crate) fn lower_cast_opponent_damage(
        condition: &TriggerCondition,
        effects: &[Sentence],
        block: u32,
        program: &mut SpellProgram,
    ) -> Result<Option<Instruction>, Failure> {
        // The complete printed block, operand and scoped graph were checked by
        // validate_fixed_cast_effects before lowering any abilities.
        let TriggerCondition::SpellCast {
            caster: CasterRelation::Controller,
            spell: TypePredicate::SpellColor { .. },
        } = condition
        else {
            return Ok(None);
        };
        let [
            Sentence {
                bindings,
                steps,
                condition: None,
                condition_kind: None,
            },
        ] = effects
        else {
            return Ok(None);
        };
        let [
            Step::Damage {
                optional: None,
                amount,
                to: DamageDestination::Player { reference },
                ..
            },
        ] = steps.as_slice()
        else {
            return Ok(None);
        };
        if !bindings.is_empty() || reference["player"] != "target-opponent" {
            return Ok(None);
        }
        let selection = reference["selectionId"]
            .as_str()
            .ok_or_else(|| unsupported("damage.target", "missing bound opponent occurrence"))?;
        Ok(Some(Instruction::damage_target(
            bind_target(
                program,
                &mut BTreeMap::new(),
                selection,
                TargetDomain::OpponentPlayer,
            )?,
            kir::Amount::Fixed {
                value: fixed_positive_value(amount)?,
            },
            block,
        )))
    }
}
pub(crate) mod fixed_mill {
    use super::*;
    pub(crate) fn target_selection(who: &serde_json::Value) -> Option<&str> {
        let object = who.as_object()?;
        if object.len() != 2 || who["player"] != "target-player" {
            return None;
        }
        who["selectionId"].as_str().filter(|s| !s.is_empty())
    }

    pub(crate) fn instruction(
        program: &mut SpellProgram,
        slots: &mut BTreeMap<String, u32>,
        who: &serde_json::Value,
        amount: &serde_json::Value,
        context: Context,
        source_block: u32,
    ) -> Result<Instruction, Failure> {
        let player = if *who == serde_json::json!({"player":"you"}) {
            PlayerExpr::you(context)
        } else {
            let id = target_selection(who).ok_or_else(|| {
                unsupported(
                    "mill.who",
                    "requires exact controller or target-player actor",
                )
            })?;
            PlayerExpr::target(bind_target(program, slots, id, TargetDomain::AnyPlayer)?)
        };
        Ok(Instruction::mill(
            player,
            Amount::Fixed {
                value: controller_library::fixed_count(amount)?,
            },
            source_block,
        ))
    }
}
pub(crate) mod hand_return {
    use super::*;
    pub(crate) fn coalesce(effects: &[Sentence]) -> Result<Option<(Quantity, Quantity)>, Failure> {
        if !effects.iter().flat_map(|s|&s.steps).any(|s|matches!(s,Step::MoveZone{what,order:Some(_),..} if what["filter"]["zone"]["zone"]=="hand")){return Ok(None);}
        let fail = || {
            unsupported(
                "hand return",
                "requires whole controller draw and ordered hand return",
            )
        };
        let [
            Sentence {
                bindings,
                steps,
                condition: None,
                condition_kind: None,
            },
        ] = effects
        else {
            return Err(fail());
        };
        let [
            Step::Draw {
                who,
                amount: DrawAmount::Fixed { value },
                optional: false,
            },
            Step::MoveZone {
                what,
                to,
                tapped: None,
                order: Some(order),
            },
        ] = steps.as_slice()
        else {
            return Err(fail());
        };
        if !bindings.is_empty()
            || who.player != "you"
            || who.selection_id.is_some()
            || who.of.is_some()
            || *to != json!({"zone":"library","owner":"your","position":"top"})
            || order != "any"
            || *what
                != json!({"ref":"filter","filter":{"cls":"card","zone":{"zone":"hand","owner":"your"}},"count":what["count"]})
        {
            return Err(fail());
        }
        Ok(Some((
            (*value as i64).into(),
            controller_library::fixed_count(&what["count"])?,
        )))
    }
}
pub(crate) mod library_order {
    use super::*;
    pub(crate) fn coalesce(effects: &[Sentence]) -> Result<Option<Quantity>, Failure> {
        if !effects.iter().flat_map(|s| &s.steps).any(|s| {
            matches!(
                s,
                Step::Shuffle {
                    optional: Some(true),
                    ..
                }
            )
        }) {
            return Ok(None);
        }
        let fail = || {
            unsupported(
                "library order",
                "requires complete controller ordering and optional shuffle",
            )
        };
        let [
            Sentence {
                condition: None,
                condition_kind: None,
                bindings: a,
                steps: ordered,
            },
            Sentence {
                condition: None,
                condition_kind: None,
                bindings: b,
                steps: shuffled,
            },
        ] = effects
        else {
            return Err(fail());
        };
        let [
            Step::LookAt { who, what },
            Step::MoveZone {
                what: moved,
                to,
                tapped: None,
                order: Some(order),
            },
        ] = ordered.as_slice()
        else {
            return Err(fail());
        };
        let [
            Step::Shuffle {
                who: shuffler,
                library,
                optional: Some(true),
            },
        ] = shuffled.as_slice()
        else {
            return Err(fail());
        };
        if !a.is_empty()
            || !b.is_empty()
            || *who != json!({"player":"you"})
            || *what != json!({"top":what["top"],"of":{"zone":"library","owner":"your"}})
            || *moved != json!({"ref":"them"})
            || *to != json!({"zone":"library","owner":"your","position":"top"})
            || order != "any"
            || shuffler.player != "you"
            || shuffler.selection_id.is_some()
            || shuffler.of.is_some()
            || *library != json!({"zone":"library","owner":{"player":"you"}})
        {
            return Err(fail());
        }
        Ok(Some(controller_library::fixed_count(&what["top"])?))
    }
}
pub(crate) mod library_selection {
    use super::*;
    pub(crate) fn coalesce(
        effects: &[Sentence],
    ) -> Result<Option<(Quantity, LibraryRemainder)>, Failure> {
        if !effects
            .iter()
            .flat_map(|s| &s.steps)
            .any(|s| matches!(s, Step::LookAt { .. }))
        {
            return Ok(None);
        }
        let fail = || {
            unsupported(
                "library selection",
                "requires complete look/pick-one/remainder source",
            )
        };
        let [
            Sentence {
                condition: None,
                condition_kind: None,
                bindings: a,
                steps: look,
            },
            Sentence {
                condition: None,
                condition_kind: None,
                bindings: b,
                steps: rest,
            },
        ] = effects
        else {
            return Err(fail());
        };
        let [Step::LookAt { who, what }] = look.as_slice() else {
            return Err(fail());
        };
        let [Step::Simultaneous { steps }] = rest.as_slice() else {
            return Err(fail());
        };
        if !a.is_empty()
            || !b.is_empty()
            || *who != json!({"player":"you"})
            || *what != json!({"top":what["top"],"of":{"zone":"library","owner":"your"}})
        {
            return Err(fail());
        }
        let amount = controller_library::fixed_count(&what["top"])?;
        let [hand, remainder] = steps.as_slice() else {
            return Err(fail());
        };
        if *hand
            != json!({"effect":"move-zone","what":{"ref":"from-pool","pool":{"ref":"them"},"count":{"amount":"fixed","value":1}},"to":{"zone":"hand","owner":"your"}})
        {
            return Err(fail());
        }
        let grave = json!({"effect":"move-zone","what":{"ref":"rest"},"to":{"zone":"graveyard","owner":"your"}});
        let bottom = json!({"effect":"move-zone","what":{"ref":"rest"},"to":{"zone":"library","owner":"your","position":"bottom"}});
        let mut ordered = bottom.clone();
        ordered["order"] = json!("any");
        let destination = if *remainder == grave {
            LibraryRemainder::Graveyard
        } else if *remainder == ordered || (*remainder == bottom && amount <= 2.into()) {
            LibraryRemainder::Bottom
        } else {
            return Err(fail());
        };
        Ok(Some((amount, destination)))
    }
}
pub(crate) mod typed_land_search {
    use super::*;
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Find {
        #[serde(rename = "ref")]
        pub(crate) reference: String,
        pub(crate) filter: Value,
        pub(crate) count: Count,
        #[serde(rename = "upTo")]
        pub(crate) up_to: Option<bool>,
    }

    fn selector(filter: &Value) -> Option<Vec<BasicLandType>> {
        let types = [
            BasicLandType::Plains,
            BasicLandType::Island,
            BasicLandType::Swamp,
            BasicLandType::Mountain,
            BasicLandType::Forest,
        ];
        for t in types {
            if *filter == json!({"cls":"card","subtypes":[t.subtype()]}) {
                return Some(vec![t]);
            }
        }
        let leaves = filter.get("allOf")?.as_array()?;
        let [alternatives] = leaves.as_slice() else {
            return None;
        };
        let alternatives = alternatives.get("anyOf")?.as_array()?;
        if *filter != json!({"cls":"card","allOf":[{"anyOf":alternatives}]})
            || alternatives.is_empty()
        {
            return None;
        }
        let mut result = Vec::new();
        for leaf in alternatives {
            let t = types
                .into_iter()
                .find(|t| *leaf == json!({"subtypes":[t.subtype()]}))?;
            if result.contains(&t) {
                return None;
            }
            result.push(t);
        }
        result.sort();
        Some(result)
    }

    pub(crate) fn lower(steps: &[Step], source_block: u32) -> Result<Option<Instruction>, Failure> {
        let [
            Step::Search { who, zones, find },
            Step::MoveZone {
                what,
                to,
                tapped,
                order: None,
            },
            Step::Shuffle {
                who: shuffler,
                library,
                optional: None,
            },
        ] = steps
        else {
            return Ok(None);
        };
        // Basic/split search keeps its own complete-source recognizer unchanged.
        if find["filter"].get("supertypes").is_some() {
            return Ok(None);
        }
        let bad = || {
            unsupported(
                "effects.search",
                "unsupported complete typed land search/move/shuffle sequence",
            )
        };
        let original = find;
        let find: Find = serde_json::from_value(find.clone()).map_err(|_| bad())?;
        let any_basic_land_types = selector(&find.filter).ok_or_else(bad)?;
        let singular = find.count.value == 1;
        let mut expected = json!({"ref":"filter","filter":find.filter,"count":{"amount":"fixed","value":find.count.value}});
        if !singular {
            expected["upTo"] = json!(true);
        }
        let you = PlayerSelector {
            player: "you".into(),
            selection_id: None,
            of: None,
        };
        let bound = if singular {
            *what == json!({"ref":"it"}) || *what == json!({"ref":"that","filter":{"cls":"card"}})
        } else {
            *what == json!({"ref":"them"})
        };
        if *original != expected
            || *who != you
            || *shuffler != you
            || *zones != json!({"selection":"all","of":[{"zone":"library","owner":"your"}]})
            || find.reference != "filter"
            || find.count.amount != "fixed"
            || find.count.value == 0
            || (singular && find.up_to.is_some())
            || (!singular && find.up_to != Some(true))
            || !bound
            || *to != json!({"zone":"battlefield"})
            || *tapped == Some(false)
            || *library != json!({"zone":"library","owner":{"player":"you"}})
        {
            return Err(bad());
        }
        Ok(Some(Instruction::SearchTypedLands {
            search: TypedLandSearch {
                any_basic_land_types,
                maximum: Quantity::from(i64::from(find.count.value)),
                force_tapped: *tapped == Some(true),
            },
            source_block,
        }))
    }
}
pub(crate) mod graveyard_battlefield_return {
    use super::*;
    pub(crate) fn creature_target(what: &Value) -> Result<&str, Failure> {
        let id = what["selectionId"].as_str().unwrap_or("");
        if *what
            != json!({"ref":"target","filter":{"types":["creature"],"cls":"card","zone":{"zone":"graveyard","owner":"your"}},
                       "count":{"amount":"fixed","value":1},"selectionId":id})
        {
            return Err(unsupported(
                "effects.return",
                "unsupported graveyard target",
            ));
        }
        Ok(id)
    }
}
pub(crate) mod source_dies_nonland_destroy {
    use super::*;
    fn selection() -> Value {
        json!({"ref":"all","filter":{"nonTypes":["land"],"cls":"permanent"}})
    }

    pub(crate) fn instruction(
        condition: &TriggerCondition,
        effects: &[Sentence],
        intervening: Option<&SourceCondition>,
        block: u32,
    ) -> Option<Instruction> {
        let [
            Sentence {
                condition: None,
                condition_kind: None,
                bindings,
                steps,
            },
        ] = effects
        else {
            return None;
        };
        if *condition == TriggerCondition::SelfDies
            && intervening.is_none()
            && bindings.is_empty()
            && matches!(steps.as_slice(),
            [Step::Destroy { what, optional: None, regeneration_forbidden: Some(true) }]
                if *what == selection())
        {
            Some(sweep(block))
        } else {
            None
        }
    }

    /// CR 701.8, 701.19c: destroy every nonland permanent; none regenerates.
    fn sweep(block: u32) -> Instruction {
        Instruction::act(
            Verb::Destroy {
                what: ObjectSet::NonlandPermanents.kernel(Context::Triggered),
                regeneration_forbidden: true,
            },
            block,
        )
    }
}
pub(crate) mod conditional_lands {
    use super::*;

    pub(crate) fn lower(effect: &SourceStaticEffect) -> Result<Option<StaticAbility>, Failure> {
        if let Some(reveal) = reveal_land_entry::lower(effect)? {
            return Ok(Some(reveal));
        }
        if let SourceStaticEffect::EntryChoice {
            what,
            who,
            costs,
            declined,
        } = effect
        {
            let bad = || {
                unsupported(
                    "static.entry",
                    "unsupported complete optional-life land entry",
                )
            };
            let [SourceEntryCost::PayLife { amount }] = costs.as_slice() else {
                return Err(bad());
            };
            // serde_json's exact native integer range is the SOURCE decoder limit.
            // Never lower a float (including rounded huge JSON numbers) to Quantity.
            // IR quantities remain arbitrary precision, independently of this limit.
            let value = amount["value"].as_u64().ok_or_else(bad)?;
            if *what != json!({"ref":"this","noun":"land"})
                || *who != json!({"player":"you"})
                || *amount != json!({"amount":"fixed","value":value})
                || *declined != json!({"tapped":true})
            {
                return Err(bad());
            }
            return Ok(Some(StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::EntersTappedUnlessPayLife {
                    amount: value.to_string().parse().map_err(|_| bad())?,
                },
            }));
        }
        let SourceStaticEffect::EntersTapped {
            what,
            condition_kind,
            condition,
        } = effect
        else {
            return Ok(None);
        };
        if condition_kind.is_none() && condition.is_none() {
            return Ok(None);
        }
        if let Some(entry) = checkland_entry::lower(effect)? {
            return Ok(Some(entry));
        }
        if let Some(entry) = other_land_entry::lower(effect)? {
            return Ok(Some(entry));
        }
        if let Some(entry) = opponent_count_entry::lower(effect)? {
            return Ok(Some(entry));
        }
        let bad = || {
            unsupported(
                "static.entry",
                "unsupported complete conditional land entry",
            )
        };
        let condition = condition.as_ref().ok_or_else(bad)?;
        let operand = &condition["comparison"]["value"]["value"];
        let minimum: Quantity = match operand {
            Value::Number(n) => n.to_string().parse().ok(),
            _ => None,
        }
        .filter(|q: &Quantity| !q.is_negative())
        .ok_or_else(bad)?;
        if *what != json!({"ref":"this","noun":"land"})
            || condition_kind.as_ref() != Some(&json!("unless"))
            || *condition
                != json!({"condition":"control-count","who":{"player":"you"},
            "of":{"supertypes":["basic"],"types":["land"]},
            "comparison":{"op":"ge","value":{"amount":"fixed","value":operand}}})
        {
            return Err(bad());
        }
        Ok(Some(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::EntersTappedUnlessControllerBasicLands { minimum },
        }))
    }
}
pub(crate) mod cast_origin {
    use super::*;
    fn hand() -> Value {
        json!({"zone":"hand","owner":"your"})
    }

    fn unsupported_shape() -> Failure {
        unsupported(
            "trigger.cast.origin",
            "requires the complete controller cast-origin draw source and reference bindings",
        )
    }

    pub(crate) fn lower_condition(
        caster: CasterRelation,
        spell: TypePredicate,
        zone: &Value,
    ) -> Result<TriggerCondition, Failure> {
        if caster != CasterRelation::Controller
            || spell != (TypePredicate::Any {})
            || *zone != hand()
        {
            return Err(unsupported_shape());
        }
        Ok(TriggerCondition::SpellCastWithOrigin {
            caster,
            spell,
            origin: CastOriginPredicate::Outside {
                zone: CastOriginZone::ControllerHand,
            },
        })
    }
}
pub(crate) mod controlled_land_entry {
    use super::*;
    pub(crate) fn lower(
        trigger: &SourceTrigger,
        effects: &[Sentence],
        intervening: Option<&SourceCondition>,
        block: usize,
    ) -> Result<Option<TriggeredAbility>, Failure> {
        let SourceTrigger::Enters {
            what: TriggerObject::Filter { filter, count },
        } = trigger
        else {
            return Ok(None);
        };
        if filter.types != ["land"] {
            return Ok(None);
        }
        let bad = || {
            unsupported(
                "abilities.trigger",
                "requires the complete controller land entry trigger and effect",
            )
        };
        if block != 0
            || intervening.is_some()
            || filter.control.as_deref() != Some("you")
            || filter.other
            || filter.non_token
            || !filter.non_subtypes.is_empty()
            || filter.properties.is_some()
            || !fixed_one(count)
        {
            return Err(bad());
        }
        let [
            Sentence {
                condition: None,
                condition_kind: None,
                bindings,
                steps,
            },
        ] = effects
        else {
            return Err(bad());
        };
        let [
            Step::GainLife {
                who,
                amount,
                for_each: None,
            },
            rest @ ..,
        ] = steps.as_slice()
        else {
            return Err(bad());
        };
        let you =
            |p: &PlayerSelector| p.player == "you" && p.of.is_none() && p.selection_id.is_none();
        if !bindings.is_empty() || !you(who) {
            return Err(bad());
        }
        let draw = match rest {
            [] => false,
            [
                Step::Draw {
                    who,
                    amount,
                    optional: false,
                },
            ] if you(who) && amount.fixed_one() => true,
            _ => return Err(bad()),
        };
        let amount = serde_json::from_value(amount.clone()).map_err(|_| bad())?;
        Ok(Some(controlled_land_entry_trigger(
            fixed_positive(&amount)?,
            draw,
        )))
    }
}
pub(crate) mod controller_land_types {
    use super::*;
    pub(crate) fn lower(effect: &SourceStaticEffect) -> Option<StaticAbility> {
        let SourceStaticEffect::Modify { what, modifiers } = effect else {
            return None;
        };
        for (ast, grant) in [
            (
                controller_land_type_ast(),
                StaticEffect::GrantControllerLandsAllBasicTypes,
            ),
            (
                controller_nonland_colors_ast(),
                StaticEffect::ControllerNonlandsAllColors,
            ),
        ] {
            let expected = &ast[0]["effect"];
            if *what == expected["what"] && expected["modifiers"].as_array() == Some(modifiers) {
                return Some(StaticAbility {
                    functioning_zone: FunctioningZone::Battlefield,
                    effect: grant,
                });
            }
        }
        None
    }
}
pub(crate) mod land_entry_gain_life {
    use super::*;
    pub(crate) fn lower(
        trigger: &SourceTrigger,
        effects: &[Sentence],
        intervening: Option<&SourceCondition>,
        block: usize,
    ) -> Result<Option<TriggeredAbility>, Failure> {
        if !matches!(trigger, SourceTrigger::Enters {what: TriggerObject::This {noun}} if noun == "land")
        {
            return Ok(None);
        }
        let [
            Sentence {
                bindings,
                steps,
                condition: None,
                condition_kind: None,
            },
        ] = effects
        else {
            return Ok(None);
        };
        let [
            Step::GainLife {
                who,
                amount,
                for_each: None,
            },
        ] = steps.as_slice()
        else {
            return Ok(None);
        };
        if intervening.is_some()
            || !bindings.is_empty()
            || who.player != "you"
            || who.of.is_some()
            || who.selection_id.is_some()
        {
            return Err(unsupported(
                "abilities.trigger",
                "entry life gain must be mandatory for the trigger controller",
            ));
        }
        let amount = fixed_positive(
            &serde_json::from_value(amount.clone())
                .map_err(|_| unsupported("amount", "entry life gain requires a fixed amount"))?,
        )?;
        Ok(Some(TriggeredAbility {
            functioning_zone: FunctioningZone::Battlefield,
            condition: TriggerCondition::SelfEntersBattlefield,
            intervening_if: None,
            death_filter: None,
            program: SpellProgram {
                functioning_zone: FunctioningZone::Stack,
                timing: SpellTiming::Instant,
                resolution: SpellResolution::Instructions,
                optional_additional_costs: vec![],
                additional_sacrifice: None,
                targets: vec![],
                instructions: vec![Instruction::act(
                    Verb::GainLife {
                        player: PlayerExpr::you(Context::Triggered),
                        amount: Amount::Fixed { value: amount },
                    },
                    source_block(block)?,
                )],
                modes: vec![],
            },
        }))
    }
}
pub(crate) mod unlimited_hand_size {
    use super::*;
    fn hand_size_rule(who: &Value, change: Value) -> Value {
        json!({"who":who,"rule":"maximum-hand-size","change":change})
    }

    fn fixed_change(operation: &str, amount: i64) -> Value {
        json!({"operation":operation,"amount":{"amount":"fixed","value":amount}})
    }

    pub(crate) fn lower_rule(rule: &Value) -> Result<StaticAbility, Failure> {
        let rule_error = || unsupported("static.player-rule", "unsupported complete player rule");
        let who = &rule["who"];
        let scope = if *who == json!({"player":"you"}) {
            HandSizeScope::Controller
        } else if *who == json!({"player":"each-opponent"}) {
            HandSizeScope::Opponents
        } else {
            return Err(rule_error());
        };
        let effect = if scope == HandSizeScope::Controller
            && *rule == hand_size_rule(who, json!({"operation":"none"}))
        {
            StaticEffect::UnlimitedControllerHandSize
        } else {
            let amount = rule["change"]["amount"]["value"]
                .as_i64()
                .filter(|value| *value > 0)
                .ok_or_else(rule_error)?;
            if *rule == hand_size_rule(who, fixed_change("reduce", amount)) {
                StaticEffect::ReduceMaximumHandSize {
                    scope,
                    amount: Quantity::from(amount),
                }
            } else if scope == HandSizeScope::Controller
                && *rule == hand_size_rule(who, fixed_change("increase", amount))
            {
                StaticEffect::IncreaseMaximumHandSize {
                    amount: Quantity::from(amount),
                }
            } else {
                return Err(rule_error());
            }
        };
        Ok(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect,
        })
    }
}
pub(crate) mod spell_cascade_grants {
    use super::*;
    pub(crate) fn lower(
        effect: &SourceStaticEffect,
        block: u32,
    ) -> Result<Option<StaticAbility>, Failure> {
        if let SourceStaticEffect::CastSpellAbilities { scope, abilities } = effect {
            let ordinary = json!({"action":"cast","spells":{"ref":"each","filter":{"cls":"spell"}},
            "caster":{"player":"you"},"occurrence":{"ordinal":1,"period":"turn"}});
            let mut exile = ordinary.clone();
            exile["from"] = json!({"zone":"exile"});
            let eligibility = if *scope == ordinary {
                SpellCascadeEligibility::FirstCastEachTurn {}
            } else if *scope == exile {
                SpellCascadeEligibility::FirstCastFromExileEachTurn {}
            } else {
                return Err(unsupported(
                    "static.spell-grant",
                    "unsupported complete cast scope",
                ));
            };
            if *abilities != vec![json!({"gained":"keyword","keyword":{"keyword":"cascade"}})] {
                return Err(unsupported(
                    "static.spell-grant",
                    "unsupported complete gained ability",
                ));
            }
            return Ok(Some(StaticAbility {
                functioning_zone: FunctioningZone::Battlefield,
                effect: StaticEffect::GrantSpellCascade {
                    eligibility,
                    program: canonical_cascade_program(block),
                },
            }));
        }
        let SourceStaticEffect::Modify { what, modifiers } = effect else {
            return Ok(None);
        };
        if what["filter"]["cls"] != "spell" {
            return Ok(None);
        }
        let operand = &what["filter"]["properties"][0]["comparison"]["value"]["value"];
        let number = match operand {
            Value::Number(n) => Some(n),
            _ => None,
        };
        let minimum: Quantity = number
            .and_then(|n| n.to_string().parse().ok())
            .filter(|q: &Quantity| !q.is_negative())
            .ok_or_else(|| {
                unsupported(
                    "static.spell-grant",
                    "expected exact nonnegative fixed quantity",
                )
            })?;
        if *what
            != json!({"ref":"each","filter":{"cls":"spell","cast":{"by":{"player":"you"}},
        "properties":[{"property":"mana-value","comparison":{"op":"ge","value":{"amount":"fixed","value":operand}}}]}})
            || *modifiers
                != vec![
                    json!({"modifier":"abilities","abilities":[{"gained":"keyword","keyword":{"keyword":"cascade"}}]}),
                ]
        {
            return Err(unsupported(
                "static.spell-grant",
                "unsupported complete selector or grant",
            ));
        }
        Ok(Some(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::GrantSpellCascade {
                eligibility: SpellCascadeEligibility::ManaValueAtLeast { minimum },
                program: canonical_cascade_program(block),
            },
        }))
    }
}
pub(crate) mod static_permanent_indestructible {
    use super::*;
    fn selector(effect: &SourceStaticEffect) -> Option<&Value> {
        let SourceStaticEffect::Modify { what, modifiers } = effect else {
            return None;
        };
        if *what != json!({"ref":"each","filter":{"other":true,"cls":"permanent","control":"you"}})
            || modifiers.as_slice()
                != [
                    json!({"modifier":"abilities","abilities":[{"gained":"keyword","keyword":{"keyword":"indestructible"}}]}),
                ]
        {
            return None;
        }
        Some(what)
    }

    pub(crate) fn lower(effect: &SourceStaticEffect) -> Option<StaticAbility> {
        selector(effect)?;
        Some(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::GrantOtherControlledPermanentsIndestructible,
        })
    }
}
pub(crate) mod successor_tuck {
    use super::*;
    use std::num::NonZeroU32;
    fn unsupported_shape() -> Failure {
        unsupported(
            "trigger.successor",
            "requires the complete controller graveyard-to-library successor program",
        )
    }

    pub(crate) fn lower(
        effects: &[Sentence],
        intervening: Option<&SourceCondition>,
        from: &str,
        to: &Value,
        block: u32,
    ) -> Result<TriggeredAbility, Failure> {
        let [
            Sentence {
                bindings,
                steps,
                condition: None,
                condition_kind: None,
            },
        ] = effects
        else {
            return Err(unsupported_shape());
        };
        let [
            Step::MoveZone {
                what,
                to: library,
                order: None,
                tapped: None,
            },
        ] = steps.as_slice()
        else {
            return Err(unsupported_shape());
        };
        let depth = library["depth"]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .and_then(NonZeroU32::new)
            .ok_or_else(unsupported_shape)?;
        if intervening.is_some()
            || !bindings.is_empty()
            || from != "battlefield"
            || *to != graveyard()
            || *what != json!({"ref":"it"})
            || *library != destination(depth)
        {
            return Err(unsupported_shape());
        }
        Ok(TriggeredAbility {
            death_filter: None,
            functioning_zone: FunctioningZone::Battlefield,
            condition: TriggerCondition::SelfMoved {
                from: SourceMoveOrigin::Battlefield,
                to: TriggerSuccessorZone::TriggerControllerGraveyard,
            },
            intervening_if: None,
            program: SpellProgram {
                optional_additional_costs: vec![],
                additional_sacrifice: None,
                timing: SpellTiming::Instant,
                resolution: SpellResolution::Instructions,
                functioning_zone: FunctioningZone::Stack,
                targets: vec![],
                instructions: vec![Instruction::MoveTriggeredSuccessor {
                    expected_zone: TriggerSuccessorZone::TriggerControllerGraveyard,
                    destination: TriggerSuccessorZone::TriggerControllerLibrary,
                    position: LibraryPosition::NthFromTop(depth),
                    source_block: block,
                }],
                modes: vec![],
            },
        })
    }

    fn graveyard() -> Value {
        json!({"zone":"graveyard","owner":"your"})
    }

    fn destination(depth: NonZeroU32) -> Value {
        json!({"zone":"library","owner":"your","position":"top","depth":depth})
    }
}
pub(crate) mod land_entry_scry {
    use super::*;
    pub(crate) fn matches(
        trigger: &SourceTrigger,
        effects: &[Sentence],
        intervening: Option<&SourceCondition>,
        block: usize,
    ) -> bool {
        matches!(trigger, SourceTrigger::Enters { what: TriggerObject::This { noun } } if noun == "land")
            && block == 1
            && intervening.is_none()
            && matches!(effects, [Sentence { condition: None, condition_kind: None, bindings, steps }]
            if bindings.is_empty() && matches!(steps.as_slice(), [Step::Scry { amount }]
                if *amount == json!({"amount":"fixed","value":1})))
    }
}
pub(crate) mod power_entry_draw {
    use super::*;
    fn shape_error() -> Failure {
        unsupported(
            "abilities.power-entry-draw",
            "requires complete power-filtered entry draw source",
        )
    }

    pub(crate) fn minimum_power(
        filter: &TriggerFilter,
        count: &Count,
        entering: bool,
    ) -> Result<Quantity, Failure> {
        let Some([property]) = &filter.properties else {
            return Err(shape_error());
        };
        if !fixed_one(count)
            || filter.types != ["creature"]
            || filter.control.as_deref() != entering.then_some("you")
            || (!entering && filter.other)
            || filter.non_token
            || !filter.non_subtypes.is_empty()
            || property.property != "power"
            || property.comparison.op != "ge"
            || property.comparison.value.amount != "fixed"
        {
            return Err(shape_error());
        }
        Ok(i64::from(property.comparison.value.value).into())
    }
}
pub(crate) mod reveal_land_entry {
    use super::*;
    const BAD: &str = "requires a complete optional two-type hand reveal entry";

    pub(crate) fn lower(effect: &SourceStaticEffect) -> Result<Option<StaticAbility>, Failure> {
        let SourceStaticEffect::EntryChoice {
            what,
            who,
            costs,
            declined,
        } = effect
        else {
            return Ok(None);
        };
        if !costs
            .iter()
            .any(|c| matches!(c, SourceEntryCost::Reveal { .. }))
        {
            return Ok(None);
        }
        let bad = || unsupported("static.entry", BAD);
        let [SourceEntryCost::Reveal { what: card }] = costs.as_slice() else {
            return Err(bad());
        };
        let Some([first, second]) = card["filter"]["allOf"][0]["anyOf"]
            .as_array()
            .map(Vec::as_slice)
        else {
            return Err(bad());
        };
        let first = first["subtypes"][0]
            .as_str()
            .and_then(type_for_subtype)
            .ok_or_else(bad)?;
        let second = second["subtypes"][0]
            .as_str()
            .and_then(type_for_subtype)
            .ok_or_else(bad)?;
        if first == second
            || *card != query([first, second])
            || *what != json!({"ref":"this","noun":"land"})
            || *who != json!({"player":"you"})
            || *declined != json!({"tapped":true})
        {
            return Err(bad());
        }
        Ok(Some(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::EntersTappedUnlessRevealLandTypes {
                subtypes: [first, second],
            },
        }))
    }

    fn type_for_subtype(name: &str) -> Option<BasicLandType> {
        [
            BasicLandType::Plains,
            BasicLandType::Island,
            BasicLandType::Swamp,
            BasicLandType::Mountain,
            BasicLandType::Forest,
        ]
        .into_iter()
        .find(|kind| kind.subtype() == name)
    }

    fn query([first, second]: [BasicLandType; 2]) -> Value {
        json!({"ref":"filter","filter":{"allOf":[{"anyOf":[
            {"subtypes":[first.subtype()]},{"subtypes":[second.subtype()]}
        ]}],"cls":"card","zone":{"zone":"hand","owner":"your"}},"count":{"amount":"fixed","value":1}})
    }
}
pub(crate) mod checkland_entry {
    use super::*;
    pub(crate) fn lower(effect: &SourceStaticEffect) -> Result<Option<StaticAbility>, Failure> {
        let SourceStaticEffect::EntersTapped {
            what,
            condition_kind,
            condition: Some(condition),
        } = effect
        else {
            return Ok(None);
        };
        if condition["condition"] != "control" {
            return Ok(None);
        }

        let bad = || unsupported("static.entry", "unsupported complete checkland entry");
        let Some([first, second]) = condition["what"]["filter"]["anyOf"]
            .as_array()
            .map(Vec::as_slice)
        else {
            return Err(bad());
        };
        let first = first["subtypes"][0].as_str().ok_or_else(bad)?;
        let second = second["subtypes"][0].as_str().ok_or_else(bad)?;
        let subtypes = [
            BasicLandType::from_subtype(first).ok_or_else(bad)?,
            BasicLandType::from_subtype(second).ok_or_else(bad)?,
        ];
        if first == second
            || *what != json!({"ref":"this","noun":"land"})
            || condition_kind.as_ref() != Some(&json!("unless"))
            || *condition
                != json!({
                    "condition":"control","who":{"player":"you"},"what":query([first, second])
                })
        {
            return Err(bad());
        }

        Ok(Some(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::EntersTappedUnlessControllerLandTypes { subtypes },
        }))
    }

    fn query([first, second]: [&str; 2]) -> Value {
        json!({"ref":"filter","filter":{"anyOf":[{"subtypes":[first]},{"subtypes":[second]}]},
        "count":{"amount":"fixed","value":1}})
    }
}
pub(crate) mod other_land_entry {
    use super::*;
    pub(crate) fn lower(effect: &SourceStaticEffect) -> Result<Option<StaticAbility>, Failure> {
        let SourceStaticEffect::EntersTapped {
            what,
            condition_kind,
            condition: Some(condition),
        } = effect
        else {
            return Ok(None);
        };
        if condition["of"]["other"] != true {
            return Ok(None);
        }
        let bad = || {
            unsupported(
                "static.entry",
                "unsupported complete other-land entry condition",
            )
        };
        let operator = condition["comparison"]["op"].as_str().ok_or_else(bad)?;
        let comparison = match operator {
            "ge" => LandCountComparison::AtLeast,
            "le" => LandCountComparison::AtMost,
            _ => return Err(bad()),
        };
        let threshold = condition["comparison"]["value"]["value"]
            .as_i64()
            .filter(|n| *n >= 0)
            .ok_or_else(bad)?;
        if *what != json!({"ref":"this","noun":"land"})
            || condition_kind.as_ref() != Some(&json!("unless"))
            || *condition
                != json!({
                    "condition":"control-count","who":{"player":"you"},"of":{"other":true,"types":["land"]},
                    "comparison":{"op":operator,"value":{"amount":"fixed","value":threshold}}
                })
        {
            return Err(bad());
        }
        Ok(Some(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::EntersTappedUnlessControllerOtherLands {
                comparison,
                threshold: threshold.into(),
            },
        }))
    }
}
pub(crate) mod opponent_count_entry {
    use super::*;
    const BAD: &str = "requires a complete opponent-count entry and tap-only dual mana source";

    pub(crate) fn lower(effect: &SourceStaticEffect) -> Result<Option<StaticAbility>, Failure> {
        let SourceStaticEffect::EntersTapped {
            what,
            condition_kind,
            condition: Some(condition),
        } = effect
        else {
            return Ok(None);
        };
        if condition["condition"] != "opponent-count" {
            return Ok(None);
        }
        let bad = || unsupported("static.entry", BAD);
        let minimum = condition["comparison"]["value"]["value"]
            .as_i64()
            .filter(|n| *n > 0)
            .ok_or_else(bad)?;
        if *what != json!({"ref":"this","noun":"land"})
            || condition_kind.as_ref() != Some(&json!("unless"))
            || *condition != predicate(minimum)
        {
            return Err(bad());
        }
        Ok(Some(StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::EntersTappedUnlessOpponentCount {
                minimum: minimum.into(),
            },
        }))
    }

    fn predicate(minimum: i64) -> Value {
        json!({"condition":"opponent-count","who":{"player":"you"},
        "comparison":{"op":"ge","value":{"amount":"fixed","value":minimum}}})
    }
}
