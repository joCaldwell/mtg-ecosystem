//! Resolving effect steps (CR 608.2): sentences of spell and ability
//! programs. One module per verb lowers its AST step; [`lower_step`] is the
//! dispatch table keyed by the step's AST `effect` tag. A step whose verb has
//! no module, or whose operands a module does not represent, is rejected
//! under its own code (`verb:<effect>:<detail>` or `operand:<kind>:<shape>`).
use super::*;

mod counters;
mod damage;
mod destroy;
mod discard;
mod draw;
mod exile;
mod gain_abilities;
mod life;
mod mill;
mod move_zone;
mod pump;
mod return_to_hand;
mod sacrifice;
mod tap;
mod tokens;

/// What every verb lowerer reads besides its own step.
pub(crate) struct StepCx<'a> {
    pub program: &'a mut SpellProgram,
    pub slots: &'a mut BTreeMap<String, u32>,
    pub player_binding: PlayerBinding,
    pub context: Context,
    pub source_block: u32,
    /// CR 107.3, 608.2h: the sentence binds X to the number of creatures on
    /// the battlefield.
    pub counted: bool,
}

impl StepCx<'_> {
    pub(crate) fn act(&self, verb: Verb) -> Instruction {
        Instruction::act(verb, self.source_block)
    }
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
    for sentence in effects {
        if sentence.condition.is_some() || sentence.condition_kind.is_some() {
            return Err(unsupported(
                "effects.condition",
                "unconsumed conditional instruction",
            ));
        }
        let counted = counts_creatures(&sentence.bindings)?;
        // The counted X is read only by one damage step.
        if counted && !matches!(sentence.steps.as_slice(), [Step::Damage { .. }]) {
            return Err(unsupported("bindings", "unconsumed count binding"));
        }
        if sentence.steps.is_empty() {
            return Err(invalid("effects.steps", "empty instruction group"));
        }
        for s in &sentence.steps {
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
                counted,
            };
            let instruction = lower_step(&mut cx, s)?;
            program.instructions.push(instruction);
        }
    }
    Ok(())
}

/// The dispatch table: one row per AST `effect` tag. Total over the step
/// mirror; a tag without a verb module rejects under its own name.
fn lower_step(cx: &mut StepCx<'_>, s: &Step) -> Result<Instruction, Failure> {
    match s {
        Step::Exile { what } => exile::lower(cx, what),
        Step::Untap { what } => tap::untap(cx, what),
        Step::Tap { what } => tap::tap(cx, what),
        Step::Destroy {
            what,
            optional,
            regeneration_forbidden,
        } => destroy::lower(cx, what, *optional, *regeneration_forbidden),
        Step::CreateToken { who, count, token } => tokens::lower(cx, who.as_ref(), count, token),
        Step::PutCounters { on, counter, count } => counters::lower(cx, on, counter, count),
        Step::Pump {
            what,
            power,
            toughness,
            duration,
        } => pump::lower(cx, what, power, toughness, duration),
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
        Step::LoseLife { who, amount } => life::lower(cx, false, who, amount, None),
        Step::Discard(discard) => discard::lower(cx, discard),
        Step::Draw {
            who,
            amount,
            optional,
        } => draw::lower(cx, who, amount, *optional),
        Step::ReturnToHand { what, to, optional } => return_to_hand::lower(cx, what, to, *optional),
        Step::MoveZone {
            what,
            to,
            order,
            tapped,
        } => move_zone::lower(cx, what, to, order.as_deref(), *tapped),
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
        Step::Scry { .. } => Err(step("scry", "unlowered")),
        Step::Surveil { .. } => Err(step("surveil", "unlowered")),
        Step::AddMana { .. } => Err(step("add-mana", "unlowered")),
        Step::Reveal { .. } => Err(step("reveal", "unlowered")),
        Step::Search { .. } => Err(step("search", "unlowered")),
        Step::Shuffle { .. } => Err(step("shuffle", "unlowered")),
        Step::Modal { .. } => Err(step("modal", "unlowered")),
        Step::GainAbilities {
            what,
            abilities,
            duration,
        } => gain_abilities::lower(cx, what, abilities, duration),
        Step::Regenerate { .. } => Err(step("regenerate", "unlowered")),
        Step::PreventDamage { .. } => Err(step("prevent-damage", "unlowered")),
        Step::Counter { .. } => Err(step("counter", "unlowered")),
    }
}

/// Only the damage verb reads the counted X.
pub(crate) fn uncounted(cx: &StepCx<'_>) -> Result<(), Failure> {
    if cx.counted {
        return Err(unsupported("bindings", "unconsumed count binding"));
    }
    Ok(())
}
