//! Resolving effect steps (CR 608.2): sentences of spell and ability
//! programs. One module per verb lowers its AST step; [`lower_step`] is the
//! dispatch table keyed by the step's AST `effect` tag. A step whose verb has
//! no module, or whose operands a module does not represent, is rejected
//! under its own code (`verb:<effect>:<detail>` or `operand:<kind>:<shape>`).
use super::*;

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
        || !sentence.steps[first..last].iter().all(|step| {
            matches!(
                step,
                Step::Draw { .. }
                    | Step::GainLife { .. }
                    | Step::LoseLife { .. }
                    | Step::Damage { .. }
            )
        })
    {
        return Err(operand("bindings", "reread_after_change"));
    }
    Ok(())
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
    player_binding: PlayerBinding,
    context: Context,
) -> Result<(), Failure> {
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
        let bound = lower_x_binding(&sentence.bindings, context)?;
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
            if player_binding == PlayerBinding::Unbound
                && matches!(s, Step::LoseLife { who, .. } if who.player == "that-player")
            {
                return Err(unsupported("trigger", "unbound death controller"));
            }
            let mut cx = StepCx {
                program: &mut *program,
                slots: &mut *slots,
                player_binding,
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
        Step::PutCounters { on, counter, count } => counters::lower(cx, on, counter, count),
        Step::Pump {
            what,
            power,
            toughness,
            duration,
            for_each,
        } => pump::lower(cx, what, power, toughness, duration, for_each.as_ref()),
        Step::Damage {
            source,
            amount,
            to,
            optional,
        } => damage::lower(cx, source, amount, to, *optional),
        Step::Mill { who, amount } => mill::lower(cx, who, amount),
        Step::GainLife {
            who,
            amount,
            for_each,
        } => life::lower(cx, true, who, amount, for_each.as_ref()),
        Step::LoseLife {
            who,
            amount,
            for_each,
        } => life::lower(cx, false, who, amount, for_each.as_ref()),
        Step::Discard(discard) => discard::lower(cx, discard),
        Step::Draw {
            who,
            amount,
            optional,
            for_each,
        } => draw::lower(cx, who, amount, *optional, for_each.as_ref()),
        Step::ReturnToHand { what, to, optional } => return_to_hand::lower(cx, what, to, *optional),
        Step::MoveZone {
            what,
            to,
            order,
            tapped,
            who,
        } => move_zone::lower(cx, what, to, order.as_deref(), *tapped, who.as_ref()),
        Step::Sacrifice {
            who,
            what,
            chosen_by,
        } => sacrifice::lower(cx, who, what, chosen_by),
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
        Step::AddMana { .. } => Err(step("add-mana", "unlowered")),
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
