//! Resolving effect steps (CR 608.2): sentences of spell and ability
//! programs. One module per verb lowers its AST step; [`lower_step`] is the
//! dispatch table keyed by the step's AST `effect` tag. A step whose verb has
//! no module, or whose operands a module does not represent, is rejected
//! under its own code (`verb:<effect>:<detail>` or `operand:<kind>:<shape>`).
use super::*;

mod antecedent;
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
    /// CR 201.5: the trigger's event object is the source itself.
    pub event_is_source: bool,
    /// The card types the trigger's event object pattern names (bits).
    pub event_types: u8,
    /// CR 201.5: the trigger condition names the source apart from its
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
        ast::PlayerRef::ThatPlayer(_) | ast::PlayerRef::They(_) => cx
            .player_binding
            .event_role()
            .map(PlayerExpr::event)
            .ok_or_else(|| operand("player", "unbound event player")),
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

fn pilot_filter(cx: &mut StepCx<'_>, filter: &ast::ObjectFilter) -> Result<Pred, Failure> {
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
            .map(|who| pilot_player(cx, &who))
            .transpose()?,
    };
    let other = filter.other.take().unwrap_or(false);
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

fn pilot_board_set(cx: &mut StepCx<'_>, set: &ast::ObjectRef) -> Result<SetExpr, Failure> {
    let filter = match set {
        ast::ObjectRef::All(set) => &set.filter,
        ast::ObjectRef::Each(set) => &set.filter,
        _ => return Err(operand("set", variant(set))),
    };
    Ok(SetExpr::battlefield(pilot_filter(cx, filter)?))
}

fn pilot_each_filter(
    cx: &mut StepCx<'_>,
    filter: &ast::ObjectFilter,
    factor: &ast::Amount,
) -> Result<Amount, Failure> {
    let factor = fixed_positive_amount(factor)?;
    let count = pilot_count(
        cx,
        &ast::AmountCount {
            of: Box::new(filter.clone()),
        },
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

fn pilot_count(cx: &mut StepCx<'_>, count: &ast::AmountCount) -> Result<Amount, Failure> {
    lower_count_with(count, cx.context, |filter| pilot_filter(cx, filter))
}

fn pilot_amount(cx: &mut StepCx<'_>, amount: &ast::Amount) -> Result<Amount, Failure> {
    match amount {
        ast::Amount::Fixed(number) if number.value >= 0 => Ok(Amount::fixed(number.value)),
        ast::Amount::X(_) => Ok(cx.bound_x().unwrap_or_else(Amount::announced_x)),
        ast::Amount::ThatMuch(_) if cx.player_binding.supplies_amount() => Ok(Amount::Var {
            of: Ref::EventAmount,
        }),
        ast::Amount::Plus(sum) => Ok(Amount::Sum {
            terms: vec![pilot_amount(cx, &sum.a)?, pilot_amount(cx, &sum.b)?],
        }),
        ast::Amount::Minus(difference) => Ok(Amount::Difference {
            minuend: Box::new(pilot_amount(cx, &difference.a)?),
            subtrahend: Box::new(pilot_amount(cx, &difference.b)?),
        }),
        ast::Amount::Twice(twice) => Ok(Amount::Times {
            factor: 2.into(),
            of: Box::new(pilot_amount(cx, &twice.of)?),
        }),
        ast::Amount::Product(product) => {
            let factors: Vec<_> = product
                .factors
                .iter()
                .map(|factor| pilot_amount(cx, factor))
                .collect::<Result<_, _>>()?;
            crate::operands::fixed_product(factors)
        }
        ast::Amount::Devotion(devotion) => lower_devotion(devotion, cx.context),
        ast::Amount::CastManaValueSum(sum) => lower_other_casts_mana_value(sum, cx.context),
        ast::Amount::CounterCount(_) => source_counter_amount(amount),
        ast::Amount::Count(count) => pilot_count(cx, count),
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
    let target_count = cx.program.targets.len();
    let Ok(amount) = pilot_amount(cx, &binding.value) else {
        return Err(original);
    };
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
            program.instructions.push(instruction);
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
        Step::Exile { what } => exile::lower(cx, what),
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
        Step::Mill { who, amount } => mill::lower(cx, who, amount),
        Step::Discard(discard) => discard::lower(cx, discard),
        Step::Evaluated(effect) => match effect.as_ref() {
            ast::Effect::Draw(draw) => draw::lower(cx, draw),
            ast::Effect::Variant99(life) => life::lower(cx, life),
            ast::Effect::Sacrifice(action) => sacrifice::lower(cx, action),
            ast::Effect::Damage(action) => damage::lower(cx, action),
            ast::Effect::PutCounters(action) => counters::lower(cx, action),
            _ => unreachable!("evaluated AST effect"),
        },
        Step::ReturnToHand { what, to, optional } => return_to_hand::lower(cx, what, to, *optional),
        Step::MoveZone {
            what,
            to,
            order,
            tapped,
            who,
            controller,
        } => move_zone::lower(
            cx,
            what,
            to,
            order.as_deref(),
            *tapped,
            who.as_ref(),
            controller.as_deref(),
        ),
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
