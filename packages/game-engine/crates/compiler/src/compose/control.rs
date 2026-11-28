//! Wave W5 control: printed choices and conditions over complete steps.
//!
//! A printed sentence may wrap ordinary steps in control: "you may"
//! (an `optional` step or an `optional-group`), "unless [a player] pays"
//! and an "if"/"unless" condition. Control is peeled off the AST sentence;
//! the remaining core steps lower through the unchanged verb lowerers; the
//! instructions each wrapped step range produced are then nested under one
//! Kernel IR control node, in printed order:
//!
//! * CR 608.2d: "may" is [`Node::Optional`]. Its chooser must be the
//!   ability's controller: a departed non-controlling chooser would need the
//!   CR 800.4g reassignment the continuation does not perform. A target's
//!   controller may choose to search and shuffle their own library: that
//!   library leaves with them (CR 800.4a), so no reassigned choice acts.
//!   Its body is made of verbs whose feasibility the continuation decides
//!   before offering the choice (CR 701.26a: tapping a tapped permanent is
//!   impossible).
//! * CR 118.12a: "unless [player] pays" is [`Node::UnlessPays`] for
//!   fixed mana, life or a single artifact discard.
//! * CR 118.12, 608.2c: "[player] may pay [cost]. If they do/don't, ..."
//!   is [`Node::IfPays`] / [`Node::UnlessPays`] over the next sentence.
//!   A controller's optional sacrifice of one creature uses the same route.
//! * CR 614.15: an "instead" step that changes only the amount of the
//!   original effect is one [`Node::If`] with both branches.
//! * CR 608.2c: "if"/"unless" is [`Node::If`] over a board condition the
//!   continuation evaluates on resolution: counts of permanents a player
//!   controls (CR 109.5), exact turn history for graveyard casts and
//!   activations (CR 601.2i, 602.2b), and Boolean combinations.
//!
//! A wrapped step whose instruction range is empty, a wrap that overlaps
//! another, a chooser or payer the resolution does not bind to a kernel
//! player, and any condition outside these rows reject under
//! `compose.control:<detail>`. Dependent clauses outside the admitted
//! resolution costs reject under `compose.control:you_do`. A destroy step
//! keeps its own optional row
//! (`Cell::OptionalTargetDestroy`).
//!
//! Pinned Comprehensive Rules 2026-09-25:
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use kir::{Cmp, Cond, Node, Payment, Ref};
use serde_json::{Map, Value};

fn reject<T>(detail: &str) -> Result<T, Failure> {
    Err(unsupported("compose.control", detail))
}

/// CR 111.2, 121.2, 608.2h: a single instruction to create identical
/// tokens or draw cards for each member reads one count. Token creation
/// is one event, and the evaluator makes individual attempts for draws.
/// Other instruction-wise bodies can need bound members and a transaction.
pub(super) fn counted_instruction(sentence: &Value) -> Value {
    let mut lowered = sentence.clone();
    let Some(steps) = lowered.get_mut("steps").and_then(Value::as_array_mut) else {
        return lowered;
    };
    for step in steps {
        if let Some(action) = counted_step(step) {
            *step = action;
        }
    }
    lowered
}

pub(crate) fn counted_step(step: &Value) -> Option<Value> {
    if step.as_object()?.len() != 4
        || step["effect"] != "for-each"
        || step["processing"] != "instruction-wise"
        || step["each"]["ref"] != "each"
        || step["each"].as_object()?.len() != 2
        || !step["each"]["filter"].is_object()
    {
        return None;
    }
    let [body] = step["effects"].as_array()?.as_slice() else {
        return None;
    };
    let [action] = body["steps"].as_array()?.as_slice() else {
        return None;
    };
    if body.as_object()?.len() != 1
        || !matches!(action["effect"].as_str(), Some("draw" | "create-token"))
        || action.get("forEach").is_some()
        || action.get("optional").is_some()
        || action
            .get("who")
            .is_some_and(|who| who != &serde_json::json!({"player": "you"}))
    {
        return None;
    }
    let mut action = action.clone();
    action["forEach"] = step["each"]["filter"].clone();
    Some(action)
}

/// One printed control construct over core steps `[start, end)` of core
/// sentence `sentence`. `depth` orders wraps over the same range: a
/// sentence condition encloses a group, which encloses a step choice.
#[derive(Clone, Debug)]
struct Wrap {
    sentence: usize,
    start: usize,
    end: usize,
    depth: u8,
    kind: Kind,
}

#[derive(Clone, Debug)]
enum Kind {
    /// The chooser's printed player path, or the imperative "you".
    Optional { who: Option<String> },
    /// `path` is the printed sentence; its condition is read on lowering.
    Condition { path: String, unless: bool },
    /// `path` is the printed optional `pay-cost` step a following "if you
    /// do" (`unless` false) or "if you don't" (`unless` true) reads.
    Pays { path: String, unless: bool },
    /// The step's original instruction precedes its replacement.
    Instead { path: String },
}

/// A block's sentences with their control peeled off.
pub(super) struct Peeled {
    /// The core sentences: printed steps, groups flattened, without control.
    pub sentences: Vec<Value>,
    wraps: Vec<Wrap>,
}

impl Peeled {
    pub(super) fn is_plain(&self) -> bool {
        self.wraps.is_empty()
    }
}

/// Peel every control construct off a block's printed sentences.
pub(super) fn peel(effects: &[Value]) -> Result<Peeled, Failure> {
    if super::revealed::candidate(effects) {
        return Ok(Peeled {
            sentences: effects.to_vec(),
            wraps: vec![],
        });
    }
    let mut sentences = Vec::with_capacity(effects.len());
    let mut wraps = Vec::new();
    // The printed optional payment an "if you do/don't" sentence reads.
    let mut payment: Option<String> = None;
    for (index, sentence) in effects.iter().enumerate() {
        if let Some(path) = paid_sentence(sentence, effects.get(index + 1)) {
            payment = Some(format!("/effects/{index}/{path}"));
            continue;
        }
        let core_index = sentences.len();
        let Some(fields) = sentence.as_object() else {
            return Ok(Peeled {
                sentences: effects.to_vec(),
                wraps: vec![],
            });
        };
        let mut core: Map<String, Value> = fields.clone();
        let mut steps = Vec::new();
        if let Some(printed) = fields.get("steps").and_then(Value::as_array) {
            for (j, step) in printed.iter().enumerate() {
                peel_step(
                    step,
                    &format!("/effects/{index}/steps/{j}"),
                    core_index,
                    &mut steps,
                    &mut wraps,
                )?;
            }
            core.insert("steps".into(), Value::Array(steps.clone()));
        }
        // CR 608.2c: where the condition is printed does not change it.
        if let Some(path) = payment.take() {
            let unless = fields["condition"]["condition"] == "you-dont";
            for key in ["condition", "conditionKind"] {
                core.remove(key);
            }
            wraps.push(Wrap {
                sentence: core_index,
                start: 0,
                end: steps.len(),
                depth: 0,
                kind: Kind::Pays { path, unless },
            });
        } else if fields.contains_key("condition") {
            let unless = match fields.get("conditionKind").and_then(Value::as_str) {
                Some("if") => false,
                Some("unless") => true,
                _ => return reject("condition_kind"),
            };
            if !matches!(
                fields.get("conditionPosition"),
                None | Some(Value::String(_))
            ) {
                return reject("condition_position");
            }
            for key in ["condition", "conditionKind", "conditionPosition"] {
                core.remove(key);
            }
            if steps.is_empty() {
                return reject("empty_condition");
            }
            wraps.push(Wrap {
                sentence: core_index,
                start: 0,
                end: steps.len(),
                depth: 0,
                kind: Kind::Condition {
                    path: format!("/effects/{index}"),
                    unless,
                },
            });
        } else if fields.contains_key("conditionKind") || fields.contains_key("conditionPosition") {
            return reject("condition_kind");
        }
        sentences.push(Value::Object(core));
    }
    Ok(Peeled { sentences, wraps })
}

/// CR 118.12, 608.2c: a sentence that is only "[player] may pay [cost]",
/// followed by an "If [they] do" or "If [they] don't" sentence, is the
/// payment that sentence performs on. Returns the payment step's path within
/// the sentence.
fn paid_sentence(sentence: &Value, next: Option<&Value>) -> Option<&'static str> {
    let fields = sentence.as_object()?;
    let [step] = fields.get("steps")?.as_array()?.as_slice() else {
        return None;
    };
    let step_fields = step.as_object()?;
    let next = next?.as_object()?;
    (fields.len() == 1
        && step["optional"] == true
        && (step["effect"] == "pay-cost" && step_fields.contains_key("costs")
            || step["effect"] == "sacrifice" && step_fields.contains_key("what"))
        && step_fields.keys().all(|k| match k.as_str() {
            "effect" | "who" | "optional" | "decidedBy" => true,
            "costs" => step["effect"] == "pay-cost",
            "what" => step["effect"] == "sacrifice",
            _ => false,
        })
        && step_fields.contains_key("who")
        && next.get("conditionKind") == Some(&Value::from("if"))
        && !next.contains_key("conditionPosition")
        && matches!(
            next.get("condition").and_then(Value::as_object),
            Some(c) if c.len() == 1
                && matches!(c.get("condition").and_then(Value::as_str), Some("you-do" | "you-dont"))
        ))
    .then_some("steps/0")
}

/// Append one printed step's core steps, recording its choice.
fn peel_step(
    step: &Value,
    path: &str,
    sentence: usize,
    steps: &mut Vec<Value>,
    wraps: &mut Vec<Wrap>,
) -> Result<(), Failure> {
    let start = steps.len();
    match step["effect"].as_str() {
        // CR 608.2d: "[player] may [steps]" as one choice.
        Some("optional-group") => {
            let fields = step.as_object().map(Map::len);
            let (Some(inner), true) = (step["steps"].as_array(), fields == Some(3)) else {
                return reject("optional_group_shape");
            };
            if step.get("who").is_none() || inner.is_empty() {
                return reject("optional_group_shape");
            }
            // Pushed before its children: among wraps over the same steps
            // the earlier one is the printed ancestor and encloses the rest.
            let at = wraps.len();
            wraps.push(Wrap {
                sentence,
                start,
                end: start,
                depth: 1,
                kind: Kind::Optional {
                    who: Some(format!("{path}/who")),
                },
            });
            for (k, child) in inner.iter().enumerate() {
                peel_step(child, &format!("{path}/steps/{k}"), sentence, steps, wraps)?;
            }
            wraps[at].end = steps.len();
        }
        // CR 614.15: select the event before applying either alternative.
        Some("self-replacement") => {
            if step["original"]["effect"] == "add-mana" {
                return Err(unsupported(
                    "compose.mana-replacement",
                    "conditional production requires a checked mana evaluator",
                ));
            }
            let Some([replacement]) = step["instead"].as_array().map(Vec::as_slice) else {
                return reject("self_replacement_shape");
            };
            let original = &step["original"];
            // Reuse the original announcement and restrict this row to
            // replacements that vary only the amount of the same action.
            let mut equivalent = replacement.clone();
            let (Some(amount), Some(fields)) = (original.get("amount"), equivalent.as_object_mut())
            else {
                return reject("self_replacement_shape");
            };
            if fields.insert("amount".into(), amount.clone()).is_none()
                || step.as_object().map(Map::len) != Some(5)
                || step["targetPolicy"] != "retain-original"
                || equivalent != *original
                || original.get("optional").is_some()
            {
                return reject("self_replacement_shape");
            }
            steps.extend([original.clone(), replacement.clone()]);
            wraps.push(Wrap {
                sentence,
                start,
                end: steps.len(),
                depth: 2,
                kind: Kind::Instead {
                    path: format!("{path}/condition"),
                },
            });
        }
        // Destroy and the evaluated verbs check their own optional rows.
        Some(effect)
            if !matches!(
                effect,
                "destroy" | "draw" | "gain-life" | "lose-life" | "discard" | "mill"
            ) && step.get("optional") == Some(&Value::Bool(true)) =>
        {
            let mut core = step.clone();
            let fields = core.as_object_mut().expect("step object");
            fields.remove("optional");
            let who = fields
                .remove("decidedBy")
                .map(|_| format!("{path}/decidedBy"));
            steps.push(core);
            wraps.push(Wrap {
                sentence,
                start,
                end: steps.len(),
                depth: 2,
                kind: Kind::Optional { who },
            });
        }
        _ => steps.push(step.clone()),
    }
    Ok(())
}

/// What lowering a block's peeled control needs from its block.
pub(super) struct Site<'a> {
    pub resolution: &'a resolution::Resolution,
    pub index: usize,
    /// The printed (pre-canonical) block AST.
    pub printed: &'a Value,
    pub context: Context,
}

impl Site<'_> {
    fn player(&self, path: &str) -> Result<PlayerExpr, Failure> {
        self.resolution
            .control_player(self.index, self.printed, path, self.context)?
            .ok_or_else(|| unsupported("compose.control", "unbound_player"))
    }

    fn sentence(&self, path: &str) -> Result<&Value, Failure> {
        self.printed
            .pointer(path)
            .ok_or_else(|| invalid("compose.control", "missing sentence"))
    }

    fn node(
        &self,
        kind: &Kind,
        body: Node,
        siblings: &[Node],
        uses_result: &mut bool,
    ) -> Result<Node, Failure> {
        if let Kind::Instead { path } = kind {
            let Node::Seq(nodes) = body else {
                return reject("self_replacement_instructions");
            };
            let Ok([original, replacement]) = <[Node; 2]>::try_from(nodes) else {
                return reject("self_replacement_instructions");
            };
            if !matches!((&original, &replacement), (Node::Do(_), Node::Do(_))) {
                return reject("self_replacement_instructions");
            }
            return Ok(Node::If {
                condition: self.condition(path, self.sentence(path)?)?,
                then: Box::new(replacement),
                otherwise: Some(Box::new(original)),
            });
        }
        let node = Box::new(body);
        Ok(match kind {
            Kind::Instead { .. } => unreachable!("handled before wrapping"),
            Kind::Optional { who } => {
                let chooser = match who {
                    Some(path) => self.player(path)?,
                    None => PlayerExpr::you(self.context),
                };
                // CR 608.2d, 402.3: "you may put/return [cards] from your
                // hand or graveyard" is the one choice of those cards, where
                // choosing none declines. A separate yes/no question would
                // tell other seats whether a hidden card matched.
                if let Node::Do(Verb::Search { player, search }) = &*node
                    && search.from != kir::SelectionZone::Library
                    && !search.random
                    && chooser == *player
                {
                    let mut search = search.clone();
                    search.optional = true;
                    return Ok(Node::Do(Verb::Search {
                        player: player.clone(),
                        search,
                    }));
                }
                // CR 800.4a, 800.4g: a departed chooser who would search
                // or shuffle only their own library has none left, so any
                // reassigned choice performs nothing.
                if !chooser.is_you() && !own_library(&node, &chooser) {
                    return reject("optional_chooser");
                }
                if !decided(&node) {
                    return reject("optional_feasibility");
                }
                Node::Optional { chooser, node }
            }
            Kind::Pays { path, unless } => {
                let step = self.sentence(path)?;
                let payer = self.player(&format!("{path}/who"))?;
                let chooser = match step.get("decidedBy") {
                    Some(_) => self.player(&format!("{path}/decidedBy"))?,
                    None => PlayerExpr::you(self.context),
                };
                if chooser != payer {
                    return reject("payment_shape");
                }
                let payment = if step["effect"] == "sacrifice" {
                    if !single_card(&step["what"], serde_json::json!({"types": ["creature"]})) {
                        return reject("payment_sacrifice");
                    }
                    Payment::Sacrifice {
                        count: 1,
                        filter: kir::Pred::creatures(),
                    }
                } else {
                    self.implicit_payer(&format!("{path}/costs/0/payer"), &payer)?;
                    payment(&step["costs"])?
                };
                if *unless {
                    Node::UnlessPays {
                        payer,
                        payment,
                        node,
                    }
                } else {
                    Node::IfPays {
                        payer,
                        payment,
                        node,
                    }
                }
            }
            Kind::Condition { path, unless } => {
                let sentence = self.sentence(path)?;
                let condition = &sentence["condition"];
                // CR 118.12a: "unless [player] pays [cost]".
                if *unless && condition["condition"] == "pays" {
                    return Ok(Node::UnlessPays {
                        payer: self.payer(path, condition, &node)?,
                        payment: payment(&condition["cost"])?,
                        node,
                    });
                }
                let condition = if condition["condition"] == "count"
                    && condition
                        .as_object()
                        .is_some_and(|fields| fields.len() == 3)
                {
                    let filter = typed::<ast::ObjectFilter>(&condition["of"], "condition")?;
                    let Some((family, filter)) = crate::rules::card_result::card_result(&filter)?
                    else {
                        return reject("count_condition");
                    };
                    let producer = crate::rules::card_result::producer(siblings, family)?;
                    let step = u32::try_from(producer)
                        .map_err(|_| invalid("compose.control", "step index overflow"))?;
                    let (cmp, value) = comparison(&condition["comparison"])?;
                    *uses_result = true;
                    Cond::Compare {
                        left: kir::Amount::Count {
                            set: SetExpr::Result {
                                step,
                                filter,
                                destination: None,
                            },
                        },
                        cmp,
                        right: kir::Amount::Fixed { value },
                    }
                } else {
                    self.condition(&format!("{path}/condition"), condition)?
                };
                Node::If {
                    condition: if *unless {
                        Cond::Not {
                            condition: Box::new(condition),
                        }
                    } else {
                        condition
                    },
                    then: node,
                    otherwise: None,
                }
            }
        })
    }

    /// The payer the condition names; the cost's implicit payer, when the
    /// resolution records one, is the same player.
    fn payer(&self, path: &str, condition: &Value, node: &Node) -> Result<PlayerExpr, Failure> {
        let fields = condition.as_object().map(Map::len);
        if fields != Some(3) || condition["cost"].as_array().map(Vec::len) != Some(1) {
            return reject("payment_shape");
        }
        let who = format!("{path}/condition/who");
        let implicit = format!("{path}/condition/cost/0/payer");
        // R4 certifies this payer as an evaluated selection of the
        // controller of a separately bound target. Keep both references
        // tied to the counter's own target (CR 118.12a, 608.2h).
        if let Node::Do(Verb::Counter {
            what: object @ Ref::Target { slot },
        }) = node
            && condition["who"].as_object().map(Map::len) == Some(2)
            && condition["who"]["player"] == "controller"
            && self
                .resolution
                .binds_target_player_selection(self.index, &who, &implicit, *slot)?
        {
            return Ok(PlayerExpr::ControllerOf { object: *object });
        }
        let payer = self.player(&who)?;
        self.implicit_payer(&implicit, &payer)?;
        Ok(payer)
    }

    /// A cost's implicit payer, when the resolution binds one, is exactly
    /// the explicit payer; any other binding contradicts it.
    fn implicit_payer(&self, path: &str, payer: &PlayerExpr) -> Result<(), Failure> {
        if self.resolution.binds(self.index, path)?
            && self
                .resolution
                .control_player(self.index, self.printed, path, self.context)?
                .as_ref()
                != Some(payer)
        {
            return reject("payer_disagrees");
        }
        Ok(())
    }

    /// CR 608.2c, 109.5: a board condition the continuation reads.
    fn condition(&self, path: &str, condition: &Value) -> Result<Cond, Failure> {
        let fields = condition.as_object().map(Map::len).unwrap_or(0);
        match condition["condition"].as_str() {
            Some("not") if fields == 2 => Ok(Cond::Not {
                condition: Box::new(self.condition(&format!("{path}/of"), &condition["of"])?),
            }),
            Some("all-of") if fields == 2 => {
                let parts = condition["conditions"]
                    .as_array()
                    .filter(|parts| parts.len() >= 2)
                    .ok_or_else(|| unsupported("compose.control", "all_of_shape"))?;
                Ok(Cond::All {
                    conditions: parts
                        .iter()
                        .enumerate()
                        .map(|(i, part)| self.condition(&format!("{path}/conditions/{i}"), part))
                        .collect::<Result<_, _>>()?,
                })
            }
            Some("any-of") if fields == 2 => {
                let parts = condition["conditions"]
                    .as_array()
                    .filter(|parts| parts.len() >= 2)
                    .ok_or_else(|| unsupported("compose.control", "any_of_shape"))?;
                Ok(Cond::Not {
                    condition: Box::new(Cond::All {
                        conditions: parts
                            .iter()
                            .enumerate()
                            .map(|(i, part)| {
                                Ok(Cond::Not {
                                    condition: Box::new(
                                        self.condition(&format!("{path}/conditions/{i}"), part)?,
                                    ),
                                })
                            })
                            .collect::<Result<_, Failure>>()?,
                    }),
                })
            }
            Some("cast-count" | "player-action-history")
                if fields
                    == (if condition["condition"] == "cast-count" {
                        6
                    } else {
                        5
                    })
                    && condition["during"] == "this-turn"
                    && condition["who"] == serde_json::json!({"player":"you"})
                    && self.player(&format!("{path}/who"))?.is_you() =>
            {
                let graveyard = serde_json::json!({"zone":"graveyard","owner":"any"});
                let fact = match condition["condition"].as_str() {
                    Some("cast-count")
                        if condition["what"] == serde_json::json!({"cls":"spell"})
                            && condition["from"] == graveyard =>
                    {
                        kir::TurnFact::SpellsCastFromGraveyard
                    }
                    Some("player-action-history")
                        if condition["action"]
                            == serde_json::json!({
                                "action":"activate",
                                "what":{"cls":"ability","abilitySource":{
                                    "ref":"filter","filter":{"cls":"card","zone":graveyard},
                                    "count":{"amount":"fixed","value":1}
                                }}
                            }) =>
                    {
                        kir::TurnFact::AbilitiesActivatedFromGraveyard
                    }
                    _ => return reject("graveyard_history_shape"),
                };
                let (cmp, value) = comparison(&condition["comparison"])?;
                Ok(Cond::TurnHistory { fact, cmp, value })
            }
            // CR 608.2h: "if it was a creature card" reads the announced
            // graveyard incarnation, even after the preceding exile.
            Some("is-filter")
                if *condition
                    == serde_json::json!({
                        "condition":"is-filter", "time":"previous", "what":{"ref":"it"},
                        "filter":{"types":["creature"],"cls":"card"}
                    }) =>
            {
                let slot = self
                    .resolution
                    .target_slot_at(self.index, &format!("{path}/what"))?
                    .ok_or_else(|| unsupported("compose.control", "unbound_previous_card"))?;
                Ok(Cond::Satisfies {
                    object: Ref::Target { slot },
                    filter: kir::Pred(vec![kir::Atom::Type {
                        kind: CardKind::Creature,
                    }]),
                })
            }
            // "If you control a [filter]": at least one such permanent.
            Some("control") if fields == 3 => {
                let what = &condition["what"];
                if what.as_object().map(Map::len) != Some(3)
                    || what["ref"] != "filter"
                    || what["count"] != serde_json::json!({"amount": "fixed", "value": 1})
                {
                    return reject("control_shape");
                }
                Ok(Cond::Exists {
                    set: self.controlled(&format!("{path}/who"), &what["filter"])?,
                })
            }
            // "If you control N or more [filter]".
            Some("control-count") if fields == 4 => {
                let set = self.controlled(&format!("{path}/who"), &condition["of"])?;
                let (cmp, value) = comparison(&condition["comparison"])?;
                Ok(Cond::Compare {
                    left: kir::Amount::Count { set },
                    cmp,
                    right: kir::Amount::Fixed { value },
                })
            }
            // CR 702.33d: "if this spell was kicked" reads the spell's own
            // declared kicker cost.
            Some("kicked")
                if fields == 3
                    && self.context == Context::Spell
                    && condition["what"] == serde_json::json!({"ref": "this", "noun": "spell"})
                    && condition["comparison"]
                        == serde_json::json!({"op": "ge", "value": {"amount": "fixed", "value": 1}}) =>
            {
                Ok(Cond::CostPaid {
                    identity: super::keywords::KICKER.into(),
                })
            }
            Some("you-do" | "you-dont") => reject("you_do"),
            Some(other) => Err(unsupported("compose.control", format!("condition:{other}"))),
            None => reject("condition_shape"),
        }
    }

    /// The permanents the player at `who` controls matching `filter`.
    fn controlled(&self, who: &str, filter: &Value) -> Result<SetExpr, Failure> {
        let player = self.player(who)?;
        if !player.is_you() {
            return reject("controller");
        }
        if filter.get("control").is_some() || filter.get("zone").is_some() {
            return reject("control_filter");
        }
        let filter: ast::ObjectFilter = typed(filter, "filter")?;
        let mut pred = lower_filter(&filter, self.context)?;
        pred.0.push(kir::Atom::ControlledBy { player });
        Ok(SetExpr::battlefield(pred))
    }
}

/// CR 608.2d, 701.26a: the verbs an optional choice may wrap. A player
/// cannot choose to perform an impossible action; the continuation decides
/// before offering the choice whether a target or source verb can be
/// performed (`core` `kir::control`). A draw, life change or mill can
/// always be performed (CR 121.3, 119.3, 701.17b). Transforming the source
/// is decided the same way (CR 701.27c-f).
fn decided(node: &Node) -> bool {
    use kir::Verb;
    match node {
        Node::Seq(nodes) => nodes.iter().enumerate().all(|(index, node)| {
            decided(node)
                || matches!(node, Node::Do(Verb::Move {
                what: SetExpr::LiveResult { step, .. }, ..
            }) if (*step as usize) < index && nodes.get(*step as usize).is_some_and(decided))
        }),
        Node::Do(Verb::CreateTokens {
            controller, count, ..
        }) => controller.is_you() && count.as_fixed().is_some(),
        Node::Do(
            Verb::Draw { .. } | Verb::GainLife { .. } | Verb::LoseLife { .. } | Verb::Mill { .. },
        ) => true,
        // CR 701.23g, 701.24a, 701.22a, 701.25a: a live player can always
        // search, shuffle, scry or surveil, even an empty library.
        Node::Do(
            Verb::Search { .. } | Verb::Shuffle { .. } | Verb::Scry { .. } | Verb::Surveil { .. },
        ) => true,
        Node::Do(Verb::Tap { what } | Verb::Untap { what } | Verb::Move { what, .. }) => matches!(
            what,
            SetExpr::One {
                object: kir::Ref::Target { .. }
            }
        ),
        // CR 701.27c-f: the continuation checks the source can still
        // transform before offering the choice.
        Node::Do(Verb::Transform {
            what: kir::Ref::Source,
        }) => true,
        Node::Do(Verb::PutCounters { on, .. }) => {
            matches!(
                on,
                SetExpr::One {
                    object: kir::Ref::Source | kir::Ref::Target { .. }
                } | SetExpr::Matching {
                    zone: kir::ZoneExpr::Battlefield,
                    ..
                }
            )
        }
        _ => false,
    }
}

/// A body whose every verb searches or shuffles `player`'s own library.
fn own_library(node: &Node, player: &PlayerExpr) -> bool {
    use kir::Verb;
    match node {
        Node::Seq(nodes) => nodes.iter().all(|node| own_library(node, player)),
        Node::Do(Verb::Search { player: p, .. } | Verb::Shuffle { player: p }) => p == player,
        _ => false,
    }
}

fn single_card(what: &Value, filter: Value) -> bool {
    *what
        == serde_json::json!({
            "ref": "filter", "filter": filter, "count": {"amount": "fixed", "value": 1}
        })
}

/// CR 118.12a: the fixed cost of one resolution payment.
fn payment(cost: &Value) -> Result<Payment, Failure> {
    let Some([cost]) = cost.as_array().map(Vec::as_slice) else {
        return reject("payment_shape");
    };
    match cost["cost"].as_str() {
        Some("mana") if cost.as_object().map(Map::len) == Some(2) => {
            let symbols = cost["symbols"]
                .as_array()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| unsupported("compose.control", "payment_shape"))?;
            let mut printed = String::new();
            for symbol in symbols {
                let symbol = symbol
                    .as_str()
                    .filter(|symbol| !symbol.contains(['{', '}']))
                    .ok_or_else(|| unsupported("compose.control", "payment:mana_symbol"))?;
                printed.push('{');
                printed.push_str(symbol);
                printed.push('}');
            }
            let cost = crate::ingest::mana_cost(&printed)
                .map_err(|_| unsupported("compose.control", "payment:mana_symbol"))?;
            if cost.colored.is_empty() {
                Ok(Payment::Generic {
                    amount: kir::Amount::Fixed {
                        value: cost.generic,
                    },
                })
            } else {
                Ok(Payment::Mana { mana: cost })
            }
        }
        Some("pay-life") if cost.as_object().map(Map::len) == Some(2) => {
            let amount: ast::Amount = typed(&cost["amount"], "amount")?;
            Ok(Payment::Life {
                amount: kir::Amount::Fixed {
                    value: fixed_positive_amount(&amount)?,
                },
            })
        }
        Some("discard")
            if cost.as_object().map(Map::len) == Some(2)
                && single_card(
                    &cost["what"],
                    serde_json::json!({"types": ["artifact"], "cls": "card"}),
                ) =>
        {
            Ok(Payment::DiscardArtifact {})
        }
        Some(other) => Err(unsupported("compose.control", format!("payment:{other}"))),
        None => reject("payment_shape"),
    }
}

/// A fixed comparison: `ge`, `le` and `eq` directly, `gt`/`lt` as their
/// inclusive integer neighbour.
pub(super) fn comparison(comparison: &Value) -> Result<(Cmp, Quantity), Failure> {
    let value = &comparison["value"];
    let fixed = (comparison.as_object().map(Map::len) == Some(2)
        && value.as_object().map(Map::len) == Some(2)
        && value["amount"] == "fixed")
        .then(|| value["value"].as_i64())
        .flatten()
        .filter(|n| *n >= 0)
        .ok_or_else(|| unsupported("compose.control", "comparison_shape"))?;
    let (cmp, n) = match comparison["op"].as_str() {
        Some("ge") => (Cmp::AtLeast, fixed),
        Some("gt") => (
            Cmp::AtLeast,
            fixed
                .checked_add(1)
                .ok_or_else(|| unsupported("compose.control", "comparison_range"))?,
        ),
        Some("le") => (Cmp::AtMost, fixed),
        Some("lt") if fixed > 0 => (Cmp::AtMost, fixed - 1),
        Some("eq") => (Cmp::Exactly, fixed),
        _ => return reject("comparison_op"),
    };
    Ok((cmp, Quantity::from(n)))
}

/// Attribute a wrapped node the kernel does not admit to this block
/// (`compose.control:kir_<error>`); definition admission repeats the check.
pub(super) fn admitted(
    peeled: &Peeled,
    instructions: &[Instruction],
    env: &kir::Env<'_>,
) -> Result<(), Failure> {
    if peeled.is_plain() {
        return Ok(());
    }
    for Instruction::Kernel { node, .. } in instructions {
        if matches!(
            node,
            Node::Optional { .. }
                | Node::UnlessPays { .. }
                | Node::IfPays { .. }
                | Node::If { .. }
                | Node::Seq(_)
        ) {
            kir::check(node, env)
                .map_err(|e| unsupported("compose.control", format!("kir {e}")))?;
        }
    }
    Ok(())
}

/// The core sentences before step `step` of sentence `sentence`.
fn prefix(sentences: &[Value], sentence: usize, step: usize) -> Vec<Value> {
    let mut out = sentences[..sentence].to_vec();
    if step > 0 {
        let mut partial = sentences[sentence].clone();
        if let Some(steps) = partial.get_mut("steps").and_then(Value::as_array_mut) {
            steps.truncate(step);
        }
        out.push(partial);
    }
    out
}

/// Nest the block's lowered instructions (`instructions[base..]`) under
/// their control. `count` lowers a prefix of the core sentences and returns
/// how many instructions it produced; each wrap's range is read from the
/// prefixes ending at its first and after its last step.
pub(super) fn apply(
    peeled: &Peeled,
    site: &Site<'_>,
    instructions: &mut Vec<Instruction>,
    base: usize,
    count: impl Fn(&[Value]) -> Result<usize, Failure>,
) -> Result<(), Failure> {
    if peeled.is_plain() {
        return Ok(());
    }
    let total = instructions.len() - base;
    let mut positions = BTreeMap::new();
    let mut position = |sentence: usize, step: usize| -> Result<usize, Failure> {
        // A boundary after a sentence's last step is the next sentence's start.
        let (sentence, step) = match peeled.sentences[sentence]["steps"].as_array() {
            Some(steps) if step == steps.len() => (sentence + 1, 0),
            _ => (sentence, step),
        };
        if let Some(found) = positions.get(&(sentence, step)) {
            return Ok(*found);
        }
        let found = if sentence == 0 && step == 0 {
            0
        } else if sentence == peeled.sentences.len() && step == 0 {
            total
        } else {
            count(&prefix(&peeled.sentences, sentence, step))?
        };
        positions.insert((sentence, step), found);
        Ok(found)
    };
    // Absolute instruction ranges, outermost first.
    let mut ranges = Vec::with_capacity(peeled.wraps.len());
    for (order, wrap) in peeled.wraps.iter().enumerate() {
        let (start, end) = (
            position(wrap.sentence, wrap.start)?,
            position(wrap.sentence, wrap.end)?,
        );
        if start >= end || end > total {
            return reject("empty_wrapped_range");
        }
        ranges.push((start, end, order, wrap));
    }
    ranges.sort_by_key(|(start, end, order, wrap)| {
        (*start, std::cmp::Reverse(*end), wrap.depth, *order)
    });
    // A forest of properly nested ranges.
    struct Tree<'w> {
        start: usize,
        end: usize,
        kind: Option<&'w Kind>,
        children: Vec<Tree<'w>>,
    }
    fn insert<'w>(
        tree: &mut Tree<'w>,
        start: usize,
        end: usize,
        kind: &'w Kind,
    ) -> Result<(), Failure> {
        if let Some(last) = tree.children.last_mut()
            && start < last.end
        {
            if end > last.end {
                return reject("overlapping_control");
            }
            return insert(last, start, end, kind);
        }
        tree.children.push(Tree {
            start,
            end,
            kind: Some(kind),
            children: vec![],
        });
        Ok(())
    }
    let mut root = Tree {
        start: 0,
        end: total,
        kind: None,
        children: vec![],
    };
    for (start, end, _, wrap) in &ranges {
        insert(&mut root, *start, *end, &wrap.kind)?;
    }
    let block: Vec<Instruction> = instructions.drain(base..).collect();
    let source_block = match block.first() {
        Some(Instruction::Kernel { source_block, .. }) => *source_block,
        None => return reject("empty_wrapped_range"),
    };
    if block.iter().any(
        |Instruction::Kernel {
             source_block: b, ..
         }| *b != source_block,
    ) {
        return reject("mixed_source_blocks");
    }
    fn render(
        tree: &Tree<'_>,
        block: &[Instruction],
        site: &Site<'_>,
    ) -> Result<(Vec<Node>, bool), Failure> {
        let mut nodes = vec![];
        let mut uses_result = false;
        let mut at = tree.start;
        let mut children = tree.children.iter().peekable();
        while at < tree.end {
            if let Some(child) = children.next_if(|c| c.start == at) {
                let (mut body, linked) = render(child, block, site)?;
                let body = if body.len() == 1 && !linked {
                    body.remove(0)
                } else {
                    Node::Seq(body)
                };
                let node = site.node(child.kind.expect("wrap"), body, &nodes, &mut uses_result)?;
                nodes.push(node);
                at = child.end;
            } else {
                let Instruction::Kernel { node, .. } = &block[at];
                nodes.push(node.clone());
                at += 1;
            }
        }
        Ok((nodes, uses_result))
    }
    let (mut nodes, linked) = render(&root, &block, site)?;
    if linked {
        nodes = vec![Node::Seq(nodes)];
    }
    for node in nodes {
        instructions.push(Instruction::Kernel { node, source_block });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_strict_comparison_at_the_integer_limit_rejects() {
        let gt = |value: i64| json!({"op": "gt", "value": {"amount": "fixed", "value": value}});
        assert_eq!(
            comparison(&gt(3)).unwrap(),
            (Cmp::AtLeast, Quantity::from(4))
        );
        let failure = comparison(&gt(i64::MAX)).unwrap_err();
        assert_eq!(failure.reason, "comparison_range");
    }

    #[test]
    fn nested_groups_over_the_same_steps_keep_printed_ancestry() {
        let group = |who: &str, steps: Value| json!({"effect": "optional-group", "who": {"player": who}, "steps": steps});
        let draw = json!({"effect": "draw", "who": {"player": "you"},
                          "amount": {"amount": "fixed", "value": 1}});
        let peeled =
            peel(&[json!({"steps": [group("you", json!([group("you", json!([draw]))]))]})])
                .unwrap();
        let whos: Vec<_> = peeled
            .wraps
            .iter()
            .map(|w| match &w.kind {
                Kind::Optional { who } => who.clone().unwrap(),
                other => panic!("{other:?}"),
            })
            .collect();
        // The outer group precedes the inner one over the same range, so
        // the stable nesting order encloses the inner choice in the outer.
        assert_eq!(
            whos,
            ["/effects/0/steps/0/who", "/effects/0/steps/0/steps/0/who"]
        );
        assert!(
            peeled
                .wraps
                .iter()
                .all(|w| (w.start, w.end, w.depth) == (0, 1, 1))
        );
    }

    #[test]
    fn optional_bodies_are_verbs_whose_feasibility_is_decided() {
        use kir::Verb;
        let you = PlayerExpr::you(Context::Triggered);
        let draw = Node::Do(Verb::Draw {
            player: you.clone(),
            amount: kir::Amount::fixed(1),
        });
        let tap = Node::Do(Verb::Tap {
            what: SetExpr::One {
                object: kir::Ref::Target { slot: 0 },
            },
        });
        assert!(decided(&draw) && decided(&tap));
        assert!(decided(&Node::Seq(vec![draw.clone(), tap])));
        // A sacrifice can be impossible in ways the continuation does not
        // check, and nested control is not a verb.
        let sacrifice = Node::Do(Verb::Sacrifice {
            player: you.clone(),
            count: kir::Amount::fixed(1),
            filter: kir::Pred(vec![]),
            greatest_power: false,
        });
        assert!(!decided(&sacrifice));
        assert!(!decided(&Node::Optional {
            chooser: you,
            node: Box::new(draw),
        }));
    }
}
