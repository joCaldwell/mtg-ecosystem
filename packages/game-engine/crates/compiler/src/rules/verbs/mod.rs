//! Resolving effect steps (CR 608.2): sentences of spell and ability
//! programs. One module per verb lowers its AST step; [`lower_step`] is the
//! dispatch table keyed by the step's AST `effect` tag. A step whose verb has
//! no module, or whose operands a module does not represent, is rejected
//! under its own code (`verb:<effect>:<detail>` or `operand:<kind>:<shape>`).
use super::*;

mod antecedent;
pub(crate) mod card_result;
mod combat;
mod continuous;
mod counter;
mod counters;
mod damage;
mod destroy;
mod discard;
mod draw;
mod exile;
mod gain_abilities;
mod keyword_actions;
mod library;
mod life;
mod mill;
mod move_zone;
pub(crate) mod prevention;
pub(crate) use prevention::damage_kind;
mod pump;
mod regenerate;
mod return_to_hand;
mod sacrifice;
mod tap;
mod target_group;
mod tokens;
mod transform;

/// What every verb lowerer reads besides its own step.
pub(crate) struct StepCx<'a> {
    pub program: &'a mut SpellProgram,
    pub slots: &'a mut BTreeMap<String, u32>,
    pub player_binding: PlayerBinding,
    /// CR 201.5, 700.7: the trigger's event object is the source itself.
    pub event_is_source: bool,
    /// The card types the trigger's event object pattern names (bits).
    pub event_types: u8,
    /// CR 201.5, 700.7: the trigger condition names the source apart from its
    /// event object.
    pub source_named: bool,
    /// CR 700.2: the current mode's first target and instruction.
    pub floor: Floor,
    /// CR 608.2c: what the block's earlier steps introduced.
    pub prior: Introduced,
    /// While a "where X is" binding lowers: what its own sentence's steps
    /// introduce, which the binding follows in print but precedes in lowering.
    pub pending: Introduced,
    /// CR 608.2c: the object performing this step ("this creature deals
    /// damage equal to its power"), which its own "its" means.
    pub subject: Option<Ref>,
    pub context: Context,
    pub source_block: u32,
    /// CR 107.3, 608.2h: the sentence's bound X ("where X is ..."), offered
    /// only to a step that reads X. Its lowerer must consume it.
    pub x: Option<kir::Amount>,
    /// The lowerer read [`Self::x`] for every X the step prints.
    pub x_consumed: bool,
    pub result_from: Option<usize>,
}

impl StepCx<'_> {
    pub(crate) fn act(&self, verb: Verb) -> Instruction {
        Instruction::act(verb, self.source_block)
    }
    /// The bound X, marking it consumed. A caller uses it for every X of
    /// its step.
    pub(crate) fn bound_x(&mut self) -> Option<kir::Amount> {
        self.x_consumed |= self.x.is_some();
        self.x.clone()
    }
}

/// CR 608.2h: a bound X is read as each reading step is performed. When
/// several steps read it, every step from the first reader up to the last
/// one must be a draw, life change or damage, which move no permanent or
/// graveyard card and change no power, so each read sees the same number.
/// A count of your hand changes as you draw and is read by one step only.
/// "Damage changes no power" holds only while wither, infect and loyalty
/// damage stay unmodelled; admitting them must narrow this list.
fn stable_rereads(sentence: &Sentence, x: &kir::Amount) -> Result<(), Failure> {
    let readers: Vec<usize> = (0..sentence.steps.len())
        .filter(|&i| sentence.reads_x.get(i) == Some(&true))
        .collect();
    let (Some(&first), Some(&last)) = (readers.first(), readers.last()) else {
        return Err(unsupported("bindings", "unused X"));
    };
    if first == last {
        return Ok(());
    }
    if x.counts_hand()
        || !sentence.steps[first..last].iter().all(|step| match step {
            Step::Evaluated(effect) => matches!(
                effect.as_ref(),
                ast::Effect::Draw(_) | ast::Effect::Variant99(_) | ast::Effect::Damage(_)
            ),
            _ => false,
        })
    {
        return Err(operand("bindings", "reread_after_change"));
    }
    Ok(())
}

fn pilot_player(cx: &mut StepCx<'_>, who: &ast::PlayerRef) -> Result<PlayerExpr, Failure> {
    match who {
        ast::PlayerRef::ThatPlayer(_) if let Some(player) = antecedent::player_target(cx) => {
            Ok(player)
        }
        ast::PlayerRef::ThatPlayer(_) | ast::PlayerRef::They(_) => {
            if matches!(cx.player_binding, PlayerBinding::DeathController)
                && cx.program.instructions.len() == cx.floor.instructions
                && cx.program.targets.len() == cx.floor.targets
            {
                // CR 603.10a: the opponent named by a creature's death
                // was its controller in the common prior battlefield.
                Ok(PlayerExpr::ControllerOf {
                    object: Ref::EventObject {
                        role: kir::EventObjectRole::Died,
                    },
                })
            } else {
                cx.player_binding
                    .event_role()
                    .map(PlayerExpr::event)
                    .ok_or_else(|| operand("player", "unbound event player"))
            }
        }
        ast::PlayerRef::DefendingPlayer(_)
            if cx.player_binding.event_object_role() == Some(kir::EventObjectRole::Attacked)
                || cx.player_binding.event_role()
                    == Some(kir::EventPlayerRole::DefendingPlayer) =>
        {
            Ok(PlayerExpr::event(kir::EventPlayerRole::DefendingPlayer))
        }
        ast::PlayerRef::Controller(relation) => Ok(PlayerExpr::ControllerOf {
            object: pilot_object(cx, &relation.of)?,
        }),
        ast::PlayerRef::Owner(relation) => Ok(PlayerExpr::OwnerOf {
            object: pilot_object(cx, &relation.of)?,
        }),
        ast::PlayerRef::EachPlayer(_) => Ok(PlayerExpr::EachPlayer),
        // CR 102.2–3: in the supported free-for-all game, each other
        // player is an opponent. Reject every additional player qualifier.
        ast::PlayerRef::Filtered(filtered)
            if matches!(&*filtered.base, ast::PlayerRef::EachPlayer(_))
                && filtered.random.is_none()
                && matches!(
                    &*filtered.filter,
                    ast::PlayerFilter {
                        other: Some(_),
                        all_of: None,
                        damage_history: None,
                        hand_size: None,
                        life_total: None,
                        counter_constraints: None,
                        controls: None,
                        life_change: None,
                        chose_option: None,
                        cast_history: None,
                        attack_history: None,
                        other_than: None,
                        attacking_player: None,
                        preceding_action: None,
                        event_results: None,
                        friend_or_foe: None,
                        searched: None,
                        voted_for: None,
                    }
                ) =>
        {
            Ok(PlayerExpr::EachOpponent)
        }
        ast::PlayerRef::TargetOpponent(target) => match target.as_ref() {
            ast::PlayerRefTargetOpponent {
                count: None,
                up_to: None,
                random: None,
                selection_id: Some(id),
            } => Ok(PlayerExpr::target(bind_target(
                cx.program,
                cx.slots,
                id,
                TargetDomain::OpponentPlayer,
            )?)),
            _ => Err(operand("player", "target_opponent:qualified")),
        },
        who => lower_player_ref(
            cx.program,
            cx.slots,
            who.clone(),
            cx.context,
            cx.player_binding.event_role(),
        ),
    }
}

/// CR 608.2d: the direct subject decides unless the printed clause
/// supplies a separate chooser.
fn check_optional_chooser(
    effect: &str,
    optional: bool,
    who: &ast::PlayerRef,
    decided_by: Option<&ast::PlayerRef>,
) -> Result<(), Failure> {
    if optional && !matches!(decided_by.unwrap_or(who), ast::PlayerRef::You(_)) {
        return Err(unsupported("compose.control", "optional_chooser"));
    }
    if !optional && decided_by.is_some() {
        return Err(step(effect, "qualified"));
    }
    Ok(())
}

fn pilot_object(cx: &mut StepCx<'_>, object: &ast::ObjectRef) -> Result<Ref, Failure> {
    match object {
        ast::ObjectRef::This(_) | ast::ObjectRef::SelfObject(_) => Ok(Ref::Source),
        ast::ObjectRef::Target(_) => Ok(Ref::Target {
            slot: bind_permanent_target(cx.program, cx.slots, object)?,
        }),
        ast::ObjectRef::It(_) | ast::ObjectRef::That(_) => antecedent::resolve(cx, object),
        ast::ObjectRef::Related(related)
            if cx.context == Context::Activated
                && crate::resolution::sacrifice_cost_object(related) =>
        {
            Ok(Ref::CostResult {
                role: kir::StepRole::Objects,
            })
        }
        _ => Err(operand("object", variant(object))),
    }
}

fn pilot_filter_relative(
    cx: &mut StepCx<'_>,
    filter: &ast::ObjectFilter,
    relative_player: Option<&PlayerExpr>,
) -> Result<Pred, Failure> {
    let mut filter = filter.clone();
    let controller = match filter.control {
        Some(ast::ObjectFilterControl::ThatPlayer) => {
            filter.control = None;
            Some(
                cx.player_binding
                    .event_role()
                    .map(PlayerExpr::event)
                    .ok_or_else(|| operand("filter", "unbound event player"))?,
            )
        }
        _ => filter
            .controlled_by
            .take()
            .map(|who| match (&*who, relative_player) {
                (ast::PlayerRef::They(_), Some(player)) => Ok(player.clone()),
                _ => pilot_player(cx, &who),
            })
            .transpose()?,
    };
    let other = match filter.other.take() {
        Some(false) => return Err(operand("filter", "other:false")),
        Some(true) => true,
        None => false,
    };
    let mut result = lower_filter(&filter, cx.context)?;
    if let Some(player) = controller {
        result.0.push(Atom::ControlledBy { player });
    }
    if other {
        result.0.push(Atom::IsNot {
            object: Ref::Source,
        });
    }
    Ok(result)
}

fn pilot_board_set(
    cx: &mut StepCx<'_>,
    set: &ast::ObjectRef,
    relative_player: Option<&PlayerExpr>,
) -> Result<SetExpr, Failure> {
    let filter = match set {
        ast::ObjectRef::All(set) => &set.filter,
        ast::ObjectRef::Each(set) => &set.filter,
        _ => return Err(operand("set", variant(set))),
    };
    Ok(SetExpr::battlefield(pilot_filter_relative(
        cx,
        filter,
        relative_player,
    )?))
}

/// CR 404.1, 608.2h: the matching cards in the named public graveyard(s).
/// Returns no set for an operand without an explicit graveyard.
fn graveyard_set(cx: &mut StepCx<'_>, what: &ast::ObjectRef) -> Result<Option<SetExpr>, Failure> {
    use ast::ObjectFilterZoneChangeHistoryFromZoneRefOwner as Owner;
    let filter = match what {
        ast::ObjectRef::All(set) => &set.filter,
        ast::ObjectRef::Each(set) => &set.filter,
        _ => return Ok(None),
    };
    let Some(zone) = filter
        .zone
        .as_deref()
        .filter(|zone| zone.zone == ast::Zone::Graveyard)
    else {
        return Ok(None);
    };
    if zone.depth.is_some() || zone.position.is_some() {
        return Err(operand("set", "graveyard_position"));
    }
    let zone = match zone.owner.as_deref() {
        Some(Owner::Your) => kir::ZoneExpr::Graveyard {
            of: PlayerExpr::you(cx.context),
        },
        Some(Owner::AllPlayers | Owner::Any | Owner::EachPlayer) => kir::ZoneExpr::Graveyards,
        Some(Owner::PlayerRef(player))
            if matches!(
                &**player,
                ast::PlayerRef::TargetPlayer(_) | ast::PlayerRef::TargetOpponent(_)
            ) =>
        {
            kir::ZoneExpr::Graveyard {
                of: pilot_player(cx, player)?,
            }
        }
        _ => return Err(operand("set", "graveyard_owner")),
    };
    let mut filter = (**filter).clone();
    filter.zone = None;
    Ok(Some(SetExpr::Matching {
        zone,
        filter: lower_card_filter(&filter)?,
    }))
}

fn pilot_each_filter(
    cx: &mut StepCx<'_>,
    filter: &ast::ObjectFilter,
    factor: &ast::Amount,
    subject: Option<&PlayerExpr>,
) -> Result<Amount, Failure> {
    let factor = fixed_positive_amount(factor)?;
    let count = pilot_count(
        cx,
        &ast::AmountCount {
            of: Box::new(filter.clone()),
        },
        subject,
    )?;
    Ok(if factor == Quantity::from(1) {
        count
    } else {
        Amount::Times {
            factor,
            of: Box::new(count),
        }
    })
}

fn pilot_count(
    cx: &mut StepCx<'_>,
    count: &ast::AmountCount,
    subject: Option<&PlayerExpr>,
) -> Result<Amount, Failure> {
    use ast::ObjectFilterZoneChangeHistoryFromZoneRefOwner as Owner;
    if let Some(amount) = card_result::linked_count(cx, &count.of)? {
        return Ok(amount);
    }
    // CR 402.3, 404.1, 608.2c,h: an explicit owner, or the sole named
    // subject of the instruction, identifies whose zone to count.
    let owner = match count.of.zone.as_deref() {
        Some(ast::ZoneRef {
            zone: ast::Zone::Graveyard | ast::Zone::Hand,
            owner: Some(owner),
            depth: None,
            position: None,
        }) => match &**owner {
            Owner::PlayerRef(who) => Some(pilot_player(cx, who)?),
            Owner::Their | Owner::ThatPlayer => subject.filter(|player| !player.is_you()).cloned(),
            _ => None,
        },
        _ => None,
    };
    lower_count_with(count, cx.context, owner, |filter| {
        pilot_measure_filter(cx, filter, subject)
    })
}

fn pilot_measure_filter(
    cx: &mut StepCx<'_>,
    filter: &ast::ObjectFilter,
    subject: Option<&PlayerExpr>,
) -> Result<Pred, Failure> {
    // "Each player ... they control" needs a separate read per member,
    // whereas this operand is one shared quantity (CR 608.2c,h).
    if matches!(
        subject,
        Some(PlayerExpr::EachOpponent | PlayerExpr::EachPlayer)
    ) && matches!(
        filter.controlled_by.as_deref(),
        Some(ast::PlayerRef::They(_))
    ) {
        return Err(operand("filter", "per_player_controller"));
    }
    // A step's "you" is second person. Its third-person count filter
    // can refer to an event player and cannot inherit that local "you".
    pilot_filter_relative(cx, filter, subject.filter(|player| !player.is_you()))
}

fn pilot_amount(cx: &mut StepCx<'_>, amount: &ast::Amount) -> Result<Amount, Failure> {
    pilot_amount_relative(cx, amount, None)
}

fn pilot_amount_relative(
    cx: &mut StepCx<'_>,
    amount: &ast::Amount,
    subject: Option<&PlayerExpr>,
) -> Result<Amount, Failure> {
    match amount {
        ast::Amount::Fixed(number) if number.value >= 0 => Ok(Amount::fixed(number.value)),
        ast::Amount::X(_) => Ok(cx.bound_x().unwrap_or_else(Amount::announced_x)),
        ast::Amount::ThatMuch(_) if cx.player_binding.supplies_amount() => Ok(Amount::Var {
            of: Ref::EventAmount,
        }),
        ast::Amount::Plus(sum) => Ok(Amount::Sum {
            terms: vec![
                pilot_amount_relative(cx, &sum.a, subject)?,
                pilot_amount_relative(cx, &sum.b, subject)?,
            ],
        }),
        ast::Amount::Minus(difference) => Ok(Amount::Difference {
            minuend: Box::new(pilot_amount_relative(cx, &difference.a, subject)?),
            subtrahend: Box::new(pilot_amount_relative(cx, &difference.b, subject)?),
        }),
        ast::Amount::Twice(twice) => Ok(Amount::Times {
            factor: 2.into(),
            of: Box::new(pilot_amount_relative(cx, &twice.of, subject)?),
        }),
        ast::Amount::Product(product) => {
            let factors: Vec<_> = product
                .factors
                .iter()
                .map(|factor| pilot_amount_relative(cx, factor, subject))
                .collect::<Result<_, _>>()?;
            crate::operands::fixed_product(factors)
        }
        ast::Amount::Devotion(devotion) => lower_devotion(devotion, cx.context),
        ast::Amount::CastManaValueSum(sum) => lower_other_casts_mana_value(sum, cx.context),
        ast::Amount::CounterCount(count) => {
            crate::operands::lower_counter_amount(count, |on| pilot_object(cx, on))
        }
        ast::Amount::Count(count) => pilot_count(cx, count, subject),
        ast::Amount::AttributeExtremum(extremum) => {
            crate::operands::lower_greatest(extremum, cx.context, "amount", |filter| {
                pilot_measure_filter(cx, filter, subject)
            })
        }
        ast::Amount::Attribute(attribute) => {
            use ast::PropertyConstraintProperty as Property;
            let stat = match attribute.attribute {
                Property::Power => kir::Stat::Power,
                Property::Toughness => kir::Stat::Toughness,
                Property::ManaValue => kir::Stat::ManaValue,
                _ => return Err(operand("amount", "unsupported object measure")),
            };
            let object = pilot_object(cx, &attribute.of)?;
            Ok(Amount::StatOf { object, stat })
        }
        ast::Amount::LifeTotal(total) => Ok(Amount::LifeOf {
            player: pilot_player(cx, &total.of)?,
        }),
        _ => Err(operand("amount", variant(amount))),
    }
}

fn pilot_x_binding(cx: &mut StepCx<'_>, sentence: &Sentence) -> Result<Option<Amount>, Failure> {
    fn contains_x(amount: &Amount) -> bool {
        match amount {
            Amount::Var {
                of: Ref::AnnouncedX,
            } => true,
            Amount::Times { of, .. } => contains_x(of),
            Amount::Sum { terms } => terms.iter().any(contains_x),
            Amount::Difference {
                minuend,
                subtrahend,
            } => contains_x(minuend) || contains_x(subtrahend),
            _ => false,
        }
    }
    let original = match lower_x_binding(&sentence.bindings, cx.context) {
        Ok(amount) => return Ok(amount),
        Err(failure) => failure,
    };
    if !sentence.steps.iter().all(|step| step.effect().is_some()) {
        return Err(original);
    }
    let [binding] = sentence.bindings.as_slice() else {
        return Err(original);
    };
    let binding: ast::SentenceBindingsItem = typed(binding, "bindings")?;
    if binding.as_of.is_some() {
        return Err(original);
    }
    if let ast::Amount::Count(count) = &*binding.value
        && let Some(zone) = &count.of.zone
        && matches!(zone.zone, ast::Zone::Graveyard | ast::Zone::Hand)
    {
        use ast::ObjectFilterZoneChangeHistoryFromZoneRefOwner as Owner;
        let subject = if matches!(
            zone.owner.as_deref(),
            Some(Owner::Their | Owner::ThatPlayer)
        ) {
            // CR 608.2c: a trailing "their graveyard" can name the common
            // direct target subject of the sentence. Reject intervening or
            // differently addressed steps rather than guessing its owner.
            let mut shared = None;
            for step in &sentence.steps {
                let Step::Evaluated(effect) = step else {
                    return Err(original);
                };
                let who = match &**effect {
                    ast::Effect::Draw(draw) => &draw.who,
                    ast::Effect::Mill(mill) => &mill.who,
                    ast::Effect::Variant99(life) => &life.who,
                    _ => return Err(original),
                };
                if !matches!(
                    &**who,
                    ast::PlayerRef::TargetPlayer(_) | ast::PlayerRef::TargetOpponent(_)
                ) || shared.is_some_and(|previous| previous != who)
                {
                    return Err(original);
                }
                shared = Some(who);
            }
            shared.map(|who| pilot_player(cx, who)).transpose()?
        } else {
            None
        };
        let value = pilot_count(cx, count, subject.as_ref())?;
        if cx.result_from.is_some() {
            return Err(operand("bindings", "linked_result_lifetime"));
        }
        return Ok(Some(value));
    }
    let target_count = cx.program.targets.len();
    // CR 608.2c: "that creature" in a trailing quantity cannot mean a
    // card just drawn into the hand. Keep all competing mentions when
    // the consuming steps name anything beyond "you" and the quantity.
    if matches!(
        &*binding.value,
        ast::Amount::CounterCount(count)
            if matches!(&*count.on, ast::AmountCounterCountOn::ObjectRef(object)
                if matches!(&**object, ast::ObjectRef::That(that)
                    if that.filter.cls != Some(ast::ObjectFilterCls::Card)))
    ) && sentence.steps.iter().all(|step| match step.effect() {
        Some(ast::Effect::Draw(draw)) => {
            matches!(&*draw.who, ast::PlayerRef::You(_))
                && matches!(&*draw.amount, ast::Amount::X(_))
                && draw.for_each.is_none()
                && draw.for_each_beyond.is_none()
                && draw.for_each_vote.is_none()
                && draw.decided_by.is_none()
        }
        Some(ast::Effect::Variant99(life)) => {
            matches!(&*life.who, ast::PlayerRef::You(_))
                && matches!(&*life.amount, ast::Amount::X(_))
                && life.for_each.is_none()
                && life.for_each_beyond.is_none()
                && life.for_each_vote.is_none()
                && life.decided_by.is_none()
        }
        _ => false,
    }) {
        cx.pending.other = false;
    }
    let Ok(amount) = pilot_amount(cx, &binding.value) else {
        return Err(original);
    };
    if cx.result_from.is_some() {
        return Err(operand("bindings", "linked_result_lifetime"));
    }
    if contains_x(&amount) || cx.program.targets.len() != target_count {
        return Err(original);
    }
    Ok(Some(amount))
}

/// A step outside every verb module's rows: `verb:<effect>:<detail>`.
pub(crate) fn step(effect: &str, detail: &str) -> Failure {
    unsupported("effects.steps", format!("verb:{effect}:{detail}"))
}

pub(crate) fn append_spell_effects(
    program: &mut SpellProgram,
    effects: &[Sentence],
    block: usize,
    slots: &mut BTreeMap<String, u32>,
    scope: impl Into<Scope>,
    context: Context,
) -> Result<(), Failure> {
    let scope = scope.into();
    let player_binding = scope.player;
    let mut prior = scope.prior;
    if effects.is_empty() {
        return Err(unsupported("abilities.effects", "empty spell effects"));
    }
    let source_block = source_block(block)?;
    let mut sentences = effects.iter().peekable();
    while let Some(sentence) = sentences.next() {
        if let Some(next) = sentences.peek()
            && let Some(verb) = library::look_choose_hand(sentence, next, context)?
        {
            program
                .instructions
                .push(Instruction::act(verb, source_block));
            for introduced in sentence.introduces.iter().chain(&next.introduces) {
                prior = prior.join(*introduced);
            }
            sentences.next();
            continue;
        }
        if sentence.condition.is_some() || sentence.condition_kind.is_some() {
            return Err(unsupported(
                "effects.condition",
                "unconsumed conditional instruction",
            ));
        }
        if sentence.steps.is_empty() {
            return Err(invalid("effects.steps", "empty instruction group"));
        }
        // CR 608.2c: "where X is its power" follows the sentence's steps.
        let pending = sentence
            .introduces
            .iter()
            .fold(Introduced::default(), |all, step| all.join(*step));
        let bound = pilot_x_binding(
            &mut StepCx {
                program,
                slots,
                context,
                player_binding,
                event_is_source: scope.event_is_source,
                event_types: scope.event_types,
                source_named: scope.source_named,
                floor: scope.floor,
                prior,
                pending,
                subject: None,
                source_block,
                x: None,
                x_consumed: false,
                result_from: None,
            },
            sentence,
        )?;
        if let Some(x) = &bound {
            stable_rereads(sentence, x)?;
        }
        let reads = |range: std::ops::Range<usize>| {
            bound.is_some()
                && sentence
                    .reads_x
                    .get(range)
                    .is_some_and(|r| r.contains(&true))
        };
        let mut at = 0;
        while let Some(s) = sentence.steps.get(at) {
            let step_prior = sentence.introduces[..at]
                .iter()
                .fold(prior, |all, step| all.join(*step));
            let mut cx = StepCx {
                program: &mut *program,
                slots: &mut *slots,
                player_binding,
                event_is_source: scope.event_is_source,
                event_types: scope.event_types,
                source_named: scope.source_named,
                floor: scope.floor,
                prior: step_prior,
                pending: Introduced::default(),
                subject: None,
                context,
                source_block,
                x: None,
                x_consumed: false,
                result_from: None,
            };
            // CR 701.23a, 701.24a: a search run spans its reveal, move and
            // shuffle steps. Neither run reads a bound X.
            if matches!(s, Step::Search { .. }) {
                let (instructions, consumed) = library::search(&mut cx, &sentence.steps[at..])?;
                if reads(at..at + consumed) {
                    return Err(unsupported("bindings", "unconsumed count binding"));
                }
                program.instructions.extend(instructions);
                at += consumed;
                continue;
            }
            if matches!(s, Step::LookAt { .. }) {
                let instruction = library::reorder_top(&mut cx, &sentence.steps[at..])?;
                if reads(at..at + 2) {
                    return Err(unsupported("bindings", "unconsumed count binding"));
                }
                program.instructions.push(instruction);
                at += 2;
                continue;
            }
            // A group step that reads a bound X stays on the single-target
            // path, which rejects the counted occurrence: one printed count
            // is never re-evaluated per member (CR 608.2h).
            if !reads(at..at + 1)
                && let Some(instructions) = target_group::lower(&mut cx, s)?
            {
                program.instructions.extend(instructions);
                at += 1;
                continue;
            }
            if reads(at..at + 1) {
                cx.x = bound.clone();
            }
            let instruction = lower_step(&mut cx, s)?;
            if cx.x.is_some() && !cx.x_consumed {
                return Err(unsupported("bindings", "unconsumed count binding"));
            }
            let result_from = cx.result_from;
            if let Some(from) = result_from {
                card_result::join_instructions(program, from, instruction);
            } else {
                program.instructions.push(instruction);
            }
            at += 1;
        }
        prior = prior.join(pending);
    }
    Ok(())
}

/// The dispatch table: one row per AST `effect` tag. Total over the step
/// mirror; a tag without a verb module rejects under its own name.
fn lower_step(cx: &mut StepCx<'_>, s: &Step) -> Result<Instruction, Failure> {
    match s {
        Step::GainControl {
            who,
            what,
            duration,
        } => continuous::control(cx, who, what, duration.as_ref()),
        Step::Become {
            what,
            spec,
            duration,
        } => continuous::lower_become(cx, what, spec, duration),
        Step::ModifyCharacteristics {
            what,
            modifiers,
            snapshot,
            stated_as,
            duration,
        } => {
            let ast::LitEffectCreated::EffectCreated = snapshot;
            continuous::modify(cx, what, modifiers, *stated_as, duration)
        }
        Step::Cant {
            what,
            action,
            duration,
        } => combat::lower(cx, what, action, duration),
        Step::CombatRule {
            what,
            rule,
            duration,
        } => combat::lower_rule(cx, what, rule, duration),
        Step::Untap { what } => tap::untap(cx, what),
        Step::Tap { what } => tap::tap(cx, what),
        Step::Transform {
            what,
            who,
            optional,
        } => transform::lower(cx, what, who, *optional),
        Step::Destroy {
            what,
            optional,
            regeneration_forbidden,
        } => destroy::lower(cx, what, *optional, *regeneration_forbidden),
        Step::CreateToken {
            who,
            count,
            token,
            for_each,
        } => tokens::lower(cx, who.as_ref(), count, token, for_each.as_ref()),
        Step::KeywordAction { action, who, times } => {
            keyword_actions::lower(cx, action, who, times.as_ref())
        }
        Step::Fight { a, b } => keyword_actions::fight(cx, a, b),
        Step::GetCounters {
            who,
            counter,
            count,
        } => counters::player(cx, who, counter, count),
        Step::RemoveCounters {
            from,
            counter,
            count,
        } => counters::remove(cx, from, counter, count),
        Step::Pump {
            what,
            power,
            toughness,
            duration,
            for_each,
        } => pump::lower(cx, what, power, toughness, duration, for_each.as_ref()),
        Step::Evaluated(effect) => match effect.as_ref() {
            ast::Effect::Draw(draw) => draw::lower(cx, draw),
            ast::Effect::Mill(action) => mill::lower(cx, action),
            ast::Effect::Discard(action) => discard::lower(cx, action),
            ast::Effect::Variant99(life) => life::lower(cx, life),
            ast::Effect::Sacrifice(action) => sacrifice::lower(cx, action),
            ast::Effect::Damage(action) => damage::lower(cx, action),
            ast::Effect::PutCounters(action) => counters::lower(cx, action),
            ast::Effect::Exile(action) => exile::lower(cx, action),
            ast::Effect::ReturnToHand(action) => return_to_hand::lower(cx, action),
            ast::Effect::MoveZone(action) => move_zone::lower(cx, action),
            // CR 610.3: a return linked to a duration has no executable
            // delayed form yet.
            ast::Effect::ExileUntil(_) => Err(step("exile-until", "linked_return")),
            _ => unreachable!("evaluated AST effect"),
        },
        Step::CreateReplacement { .. } => Err(step("create-replacement", "replacement")),
        Step::LoseAbilities { .. } => Err(step("lose-abilities", "unlowered")),
        Step::PlayCard { .. } => Err(step("play-card", "unlowered")),
        Step::LoseGame { .. } => Err(step("lose-game", "unlowered")),
        Step::AdditionalLandPlays { .. } => Err(step("additional-land-plays", "unlowered")),
        Step::LookAt { .. } => Err(step("look-at", "unlowered")),
        Step::Simultaneous { .. } => Err(step("simultaneous", "unlowered")),
        Step::ShuffleIntoLibrary {
            who,
            what,
            library: into,
        } => library::shuffle_into(cx, who, what, into),
        Step::Scry { amount } => library::look(cx, amount, false),
        Step::Surveil { amount } => library::look(cx, amount, true),
        Step::AddMana {
            who,
            mana,
            for_each: Some(filter),
            spend_restriction: None,
            riders: None,
        } if cx.context == Context::Spell
            && who.player == "you"
            && who.selection_id.is_none()
            && who.of.is_none() =>
        {
            uncounted(cx)?;
            let (color, amount) = super::activations::counted_mana(mana, filter, cx.context)?;
            Ok(cx.act(Verb::AddMana {
                player: kir::PlayerExpr::you(cx.context),
                mana: kir::ManaSpec::Color { color, amount },
            }))
        }
        Step::AddMana { .. } => Err(step("add-mana", "unlowered")),
        Step::ChooseValue { .. } => Err(step("choose-value", "unlowered")),
        Step::Reveal { .. } => Err(step("reveal", "unlowered")),
        // A search run is lowered from its first step by the sentence loop.
        Step::Search { .. } => Err(step("search", "run")),
        Step::Shuffle {
            who,
            library,
            optional,
        } => library::shuffle(cx, who, library, *optional),
        Step::Modal { .. } => Err(step("modal", "unlowered")),
        Step::GainAbilities {
            what,
            abilities,
            duration,
        } => gain_abilities::lower(cx, what, abilities, duration),
        Step::Regenerate { what } => regenerate::lower(cx, what),
        Step::PreventDamage {
            prevention,
            duration,
        } => prevention::lower(cx, prevention, duration),
        Step::Counter { what } => counter::lower(cx, what),
    }
}

/// A lowerer that never reads a bound X rejects a step that prints one.
pub(crate) fn uncounted(cx: &StepCx<'_>) -> Result<(), Failure> {
    if cx.x.is_some() {
        return Err(unsupported("bindings", "unconsumed count binding"));
    }
    Ok(())
}
