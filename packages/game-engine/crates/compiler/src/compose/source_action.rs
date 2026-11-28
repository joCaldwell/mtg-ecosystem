//! Source authority for a complete one-sentence draw or mill spell.
//! CR 109.5, 115.1a, 601.2b–c, 608.2b–c, 121.1–2c, 701.17.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use crate::{
    ingest::evidence::{BlockEvidence, CardEvidence, Check, EvidenceError, StructuralPunctuation},
    reference_trace::{SourceChoice, SourcePlayer},
};
use mtg_oracle_ast::printed::{
    ImplicitReason, Nullable, PrintedFact, PrintedFactIntroducerForm as Intro,
    PrintedFactMentionForm as Mention, PrintedFactQuantityForm,
};
use serde_json::{Value, json};

const STEP: &str = "/effects/0/steps/0";
const EFFECT: &str = "/effects/0/steps/0/effect";
const PLAYER: &str = "/effects/0/steps/0/who/player";
const SELECTION: &str = "/effects/0/steps/0/who/selectionId";
const AMOUNT: &str = "/effects/0/steps/0/amount/amount";
const VALUE: &str = "/effects/0/steps/0/amount/value";

#[derive(Clone, Copy)]
enum Action {
    Draw,
    Mill,
}

impl Action {
    fn words(self) -> (&'static str, &'static str) {
        match self {
            Self::Draw => ("draw", "draws"),
            Self::Mill => ("mill", "mills"),
        }
    }

    fn verb(self, player: PlayerExpr, amount: Amount) -> Verb {
        match self {
            Self::Draw => Verb::Draw { player, amount },
            Self::Mill => Verb::Mill { player, amount },
        }
    }
}

#[derive(Clone, Copy)]
enum Subject<'a> {
    You,
    EachPlayer,
    Target {
        noun: &'static str,
        id: &'a str,
        domain: TargetDomain,
    },
}

impl Subject<'_> {
    fn tag(self) -> &'static str {
        match self {
            Self::You => "you",
            Self::EachPlayer => "each-player",
            Self::Target {
                noun: "opponent", ..
            } => "target-opponent",
            Self::Target { .. } => "target-player",
        }
    }
}

#[derive(Clone, Copy)]
enum Count {
    Fixed(i64),
    AnnouncedX,
}

impl Count {
    fn matches(self, word: &str) -> bool {
        match self {
            Self::AnnouncedX => word == "x",
            Self::Fixed(n) => {
                let parsed = if word == "a" {
                    Some(1)
                } else if word.bytes().all(|b| b.is_ascii_digit()) {
                    word.parse().ok()
                } else {
                    word_number(word).map(i64::from)
                };
                parsed == Some(n)
            }
        }
    }
}

pub(super) struct SourceAction<'a> {
    action: Action,
    subject: Subject<'a>,
    count: Count,
}

impl<'a> SourceAction<'a> {
    /// The selector uses only the generated AST. A selected card never falls
    /// back to the archive's resolution graph, including on a proof failure.
    pub(super) fn select(abilities: &'a [ast::Ability]) -> Option<Self> {
        let [ast::Ability::Spell(spell)] = abilities else {
            return None;
        };
        let ast::AbilitySpell {
            ability_word: None,
            variable_constraints: None,
            cleaved: None,
            effects,
        } = spell.as_ref()
        else {
            return None;
        };
        let [
            ast::Sentence {
                condition: None,
                condition_kind: None,
                condition_position: None,
                otherwise: None,
                bindings: None,
                after_conditional: None,
                steps,
            },
        ] = effects.as_slice()
        else {
            return None;
        };
        let (action, who, amount) = match steps.as_slice() {
            [ast::Effect::Draw(draw)] => {
                let ast::EffectDraw {
                    optional: None,
                    decided_by: None,
                    starting_with: None,
                    for_each: None,
                    for_each_beyond: None,
                    for_each_vote: None,
                    duration: None,
                    who,
                    amount,
                } = draw.as_ref()
                else {
                    return None;
                };
                (Action::Draw, who, amount)
            }
            [ast::Effect::Mill(mill)] => {
                let ast::EffectMill {
                    optional: None,
                    decided_by: None,
                    starting_with: None,
                    for_each: None,
                    for_each_beyond: None,
                    for_each_vote: None,
                    duration: None,
                    repeat_while: None,
                    who,
                    amount,
                } = mill.as_ref()
                else {
                    return None;
                };
                (Action::Mill, who, amount)
            }
            _ => return None,
        };
        let subject = match who.as_ref() {
            ast::PlayerRef::You(_) => Subject::You,
            ast::PlayerRef::EachPlayer(_) => Subject::EachPlayer,
            ast::PlayerRef::TargetPlayer(target) => {
                let ast::PlayerRefTargetPlayer {
                    count: None,
                    up_to: None,
                    random: None,
                    selection_id: Some(id),
                } = target.as_ref()
                else {
                    return None;
                };
                Subject::Target {
                    noun: "player",
                    id,
                    domain: TargetDomain::AnyPlayer,
                }
            }
            ast::PlayerRef::TargetOpponent(target) => {
                let ast::PlayerRefTargetOpponent {
                    count: None,
                    up_to: None,
                    random: None,
                    selection_id: Some(id),
                } = target.as_ref()
                else {
                    return None;
                };
                Subject::Target {
                    noun: "opponent",
                    id,
                    domain: TargetDomain::OpponentPlayer,
                }
            }
            _ => return None,
        };
        let count = match amount.as_ref() {
            ast::Amount::Fixed(n) if n.value >= 0 => Count::Fixed(n.value),
            ast::Amount::X(_) => Count::AnnouncedX,
            _ => return None,
        };
        Some(Self {
            action,
            subject,
            count,
        })
    }

    pub(super) fn check(
        self,
        source: &str,
        evidence: &CardEvidence,
    ) -> Result<CheckedAction<'a>, Failure> {
        if evidence.sha256().is_some() {
            let block = evidence.block(0, 0, STEP)?;
            let (line, text) = block.single_line().ok_or_else(|| block.missing(STEP))?;
            let sentence = self.sentence(text).ok_or_else(|| block.missing(STEP))?;
            self.cover(&block, line, &sentence)?;
        } else if self.sentence(source).is_none()
            || matches!(self.subject, Subject::Target { id, .. } if id != "line:0:0")
        {
            return Err(EvidenceError {
                check: Check::RequiresR6,
                face: 0,
                block: 0,
                obligation: format!("{STEP}:whole-instruction-and-target-occurrence"),
                range: None,
            }
            .into());
        }
        Ok(CheckedAction(self))
    }

    fn sentence(&self, text: &str) -> Option<SentenceProof> {
        let text = text.strip_suffix('.')?.to_ascii_lowercase();
        let mut words = text.split_ascii_whitespace();
        let (base, singular) = self.action.words();
        let first = words.next()?;
        let verb = match (self.subject, first) {
            (Subject::You, first) if first == base => 0,
            (Subject::You, "you") if words.next()? == base => 1,
            (Subject::EachPlayer, "each")
                if words.next()? == "player" && words.next()? == singular =>
            {
                2
            }
            (Subject::Target { noun, .. }, "target")
                if words.next()? == noun && words.next()? == singular =>
            {
                2
            }
            _ => return None,
        };
        let count = words.next()?;
        let noun = words.next()?;
        let expected = if matches!(self.count, Count::Fixed(1)) {
            "card"
        } else {
            "cards"
        };
        (self.count.matches(count) && noun == expected && words.next().is_none()).then(|| {
            SentenceProof {
                verb,
                count: count.into(),
                noun: expected,
            }
        })
    }

    fn cover(
        &self,
        block: &BlockEvidence<'_>,
        line: u32,
        proof: &SentenceProof,
    ) -> Result<(), EvidenceError> {
        let verb = proof.verb;
        let run = |start, end| [line, start, end];
        let count_run = run(verb + 1, verb + 2);
        let (base, singular) = self.action.words();
        let verb_word = match self.subject {
            Subject::You => base,
            _ => singular,
        };
        block.check_family(|coverage| {
            coverage.meaning("/kind", run(verb, verb + 1), |value, text| {
                value == "spell" && text.eq_ignore_ascii_case(verb_word)
            })?;
            coverage.meaning(EFFECT, run(verb, verb + 1), |value, text| {
                value == base && text.eq_ignore_ascii_case(verb_word)
            })?;
            // The direct draw/mill noun establishes the domain of the count.
            coverage.meaning(EFFECT, run(verb + 2, verb + 3), |value, text| {
                value == base && text.eq_ignore_ascii_case(proof.noun)
            })?;
            match self.subject {
                Subject::You if verb == 0 => coverage.implicit(PLAYER, ImplicitReason::ImperativeController)?,
                subject => {
                    let phrase = match subject {
                        Subject::You => "you",
                        Subject::EachPlayer => "each player",
                        Subject::Target { noun: "opponent", .. } => "target opponent",
                        Subject::Target { .. } => "target player",
                    };
                    coverage.meaning(PLAYER, run(0, verb), |value, text| {
                        value == subject.tag() && text.eq_ignore_ascii_case(phrase)
                    })?;
                    if let Subject::Target { id, .. } = subject {
                        let target = coverage.field(SELECTION)?;
                        if !matches!(target.value, PrintedFact::Target(fact)
                            if fact.scope.is_empty() && fact.target_index == 0
                                && fact.tok.0 == run(0, 1) && fact.selection_id == id)
                        {
                            return Err(block.missing(SELECTION));
                        }
                    }
                }
            }
            coverage.meaning(AMOUNT, count_run, |value, text| {
                value == match self.count { Count::Fixed(_) => "fixed", Count::AnnouncedX => "x" }
                    && text.eq_ignore_ascii_case(&proof.count)
            })?;
            if let Count::Fixed(n) = self.count {
                let quantity = coverage.field(VALUE)?;
                if !matches!(quantity.value, PrintedFact::Quantity(fact)
                    if fact.tok.0 == count_run && matches!(fact.value, Nullable::Value(value) if i64::from(value) == n))
                {
                    return Err(block.missing(VALUE));
                }
            }
            coverage.punctuation(line, verb + 3, StructuralPunctuation::SentenceTerminator)?;

            let quantity_intro = match self.count {
                Count::AnnouncedX => Intro::X,
                Count::Fixed(_) if proof.count == "a" => Intro::A,
                Count::Fixed(_) => Intro::Numeral,
            };
            for (index, fact) in block.facts() {
                let field = match fact {
                    PrintedFact::Introducer(fact) if fact.tok.0 == count_run && fact.form == quantity_intro => AMOUNT,
                    PrintedFact::Introducer(fact) if fact.tok.0 == run(0, 1) && matches!(
                        (self.subject, &fact.form),
                        (Subject::EachPlayer, Intro::Each) | (Subject::Target { .. }, Intro::Target)
                    ) => PLAYER,
                    PrintedFact::Mention(fact) if matches!(self.subject, Subject::You)
                        && verb == 1 && fact.form == Mention::You && fact.tok.0 == run(0, 1) => PLAYER,
                    PrintedFact::Mention(fact) if matches!(self.count, Count::AnnouncedX)
                        && fact.form == Mention::X && fact.tok.0 == count_run => AMOUNT,
                    PrintedFact::Quantity(fact) if matches!(self.count, Count::AnnouncedX)
                        && fact.form == PrintedFactQuantityForm::X && fact.tok.0 == count_run => AMOUNT,
                    _ => continue,
                };
                coverage.bind_fact(index, field)?;
            }
            Ok(())
        })
    }
}

struct SentenceProof {
    verb: u32,
    count: String,
    noun: &'static str,
}

pub(super) struct CheckedAction<'a>(SourceAction<'a>);

impl CheckedAction<'_> {
    pub(super) fn lower(self, definition: &mut Definition) -> Result<(), Failure> {
        let SourceAction {
            action,
            subject,
            count,
        } = self.0;
        let amount = match count {
            Count::Fixed(n) => Amount::fixed(n),
            // CR 107.3a, 601.2b: the spell's mana cost supplies the
            // announced X. This production has no resolution-time binding.
            Count::AnnouncedX if definition.x_mana_cost => Amount::announced_x(),
            Count::AnnouncedX => {
                return Err(unsupported(
                    "source.mana_cost",
                    "announced X has no mana-cost declaration",
                ));
            }
        };
        let program = definition.spell.as_mut().expect("ordinary spell shell");
        let (player, selection) = match subject {
            Subject::You => (
                PlayerExpr::you(Context::Spell),
                SourcePlayer::SpellController,
            ),
            Subject::EachPlayer => (PlayerExpr::EachPlayer, SourcePlayer::EachPlayer),
            Subject::Target { id, domain, .. } => {
                program.targets.push(TargetSpec::new(0, id, domain));
                (
                    PlayerExpr::target(0),
                    SourcePlayer::Target {
                        slot: 0,
                        selection_id: id.into(),
                        domain,
                    },
                )
            }
        };
        program
            .instructions
            .push(Instruction::act(action.verb(player, amount), 0));
        crate::reference_trace::record_source(|| SourceChoice {
            face: 0,
            block: 0,
            step: STEP,
            player: selection,
            announced_x: matches!(count, Count::AnnouncedX),
        });
        Ok(())
    }
}

pub(super) fn compose(
    archive: &AcceptedArchive,
    card: &mut CardSource,
    action: SourceAction<'_>,
    evidence: Option<&CardEvidence>,
    types: type_line::ParsedTypes,
) -> Result<Definition, Failure> {
    let (source, result) = card.faces.pop().unwrap();
    let printed: Value = decode(result.abilities.get(), "abilities")?;
    if let Some(field) = null_field(&printed, "abilities") {
        return Err(unsupported("operand", format!("operand:ast:null:{field}")));
    }
    let line = &result.lines[0];
    if printed != json!([decode::<Value>(line.ability.get(), "lines.ability")?])
        || source_lines(&source.oracle_text, &source.name, &source.type_line)?
            != [normalization::literal_spelling(&line.text)]
    {
        return Err(invalid(
            "lines",
            "source, line and aggregate ability disagree",
        ));
    }
    let evidence = evidence.ok_or_else(|| invalid("printed", "missing verified card evidence"))?;
    let checked = action.check(&source.oracle_text, evidence)?;
    let (mut definition, labels) = assemble::shell(archive, card, 0, source, &result, types)?;
    checked.lower(&mut definition)?;
    keywords::printed(&card.shape, &[])?;
    definition.provenance.printed_sha256 = evidence.sha256().map(str::to_owned);
    let definition = assemble::finish(definition, labels);
    definition
        .validate()
        .map_err(|e| unsupported("composition", e))?;
    Ok(definition)
}
