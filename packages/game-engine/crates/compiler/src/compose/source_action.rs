//! Source authority for a complete one-sentence player action spell.
//!
//! **Supported.** A single-face instant or sorcery whose only line is one
//! unconditioned sentence with one step, and nothing else:
//!
//! | Family | Printed instruction | Kernel verb |
//! | --- | --- | --- |
//! | `draw` | "draw(s) N card(s)" | `Draw` (CR 121.1–121.2c) |
//! | `mill` | "mill(s) N card(s)" | `Mill` (CR 701.17) |
//! | `gain-life` | "gain(s) N life" | `GainLife` (CR 119.3, 119.6) |
//! | `lose-life` | "lose(s) N life" | `LoseLife` (CR 119.3) |
//! | `discard` | "discard(s) N card(s) [at random]" | `Discard` (CR 701.9a–b) |
//!
//! The recipient is the controller ("you", or an imperative with no
//! subject), each player, each opponent (CR 102.2–102.3, APNAP per 101.4),
//! or one announced target player or opponent (CR 115.1a, 601.2c). N is a
//! fixed count or the spell's announced X (CR 107.3a, 601.2b), which needs
//! an X in the mana cost. A discard count is at least one; the recipient
//! chooses the cards, or they are chosen at random, at resolution
//! (CR 608.2h, 701.9b).
//!
//! **Proof.** R6 decides from the typed AST and verified printed evidence:
//! the step, recipient, count and card-noun runs come from the recorded
//! node citations, every AST field is discharged (P11) and every fact and
//! token gets a checked meaning or structural role (P12). R5 has no
//! evidence; the whole Oracle sentence must spell exactly this instruction,
//! and any target must be the line's first, or the card reports
//! `evidence:requires-r6`.
//!
//! **Left to the existing authority.** Any other ability, sentence or step
//! field: optional or chooser-qualified actions, `startingWith`, `forEach`,
//! durations, votes, quantified, repeated, up-to or random target players,
//! "any number of target players", computed counts, whole-hand discards,
//! qualified discard cards, `random: false`, and compound or modal blocks.
//! Damage and object references are a later family.
//!
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::source_authority::Site;
use super::*;
use crate::{
    ingest::evidence::{
        Basis, BlockEvidence, CardEvidence, Check, EvidenceError, StructuralPunctuation,
    },
    reference_trace::{Cardinality, Selection, SourceChoice, SourceSelection},
};
use mtg_oracle_ast::printed::{
    ImplicitReason, Nullable, PrintedFact, PrintedFactIntroducerForm as Intro,
    PrintedFactMentionForm as Mention, PrintedFactQuantityForm, PrintedNode, PrintedNodeRecorded,
    TokenRun,
};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Family {
    Draw,
    Mill,
    GainLife,
    LoseLife,
    Discard,
}

impl Family {
    fn name(self) -> &'static str {
        match self {
            Self::Draw => "draw",
            Self::Mill => "mill",
            Self::GainLife => "gain-life",
            Self::LoseLife => "lose-life",
            Self::Discard => "discard",
        }
    }

    /// The AST `effect` tag.
    fn effect(self) -> &'static str {
        self.name()
    }

    fn words(self) -> (&'static str, &'static str) {
        match self {
            Self::Draw => ("draw", "draws"),
            Self::Mill => ("mill", "mills"),
            Self::GainLife => ("gain", "gains"),
            Self::LoseLife => ("lose", "loses"),
            Self::Discard => ("discard", "discards"),
        }
    }

    /// Where the count lives below the step.
    fn count_path(self) -> &'static str {
        match self {
            Self::Discard => "/what/count",
            _ => "/amount",
        }
    }

    /// The printed noun the count measures.
    fn noun(self, count: Count) -> &'static str {
        match (self, count) {
            (Self::GainLife | Self::LoseLife, _) => "life",
            (_, Count::Fixed(1)) => "card",
            _ => "cards",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Subject {
    You,
    EachPlayer,
    EachOpponent,
    Target {
        noun: &'static str,
        id: String,
        domain: TargetDomain,
    },
}

impl Subject {
    fn tag(&self) -> &'static str {
        match self {
            Self::You => "you",
            Self::EachPlayer => "each-player",
            Self::EachOpponent => "each-opponent",
            Self::Target {
                noun: "opponent", ..
            } => "target-opponent",
            Self::Target { .. } => "target-player",
        }
    }

    fn words(&self) -> &'static [&'static str] {
        match self {
            Self::You => &["you"],
            Self::EachPlayer => &["each", "player"],
            Self::EachOpponent => &["each", "opponent"],
            Self::Target {
                noun: "opponent", ..
            } => &["target", "opponent"],
            Self::Target { .. } => &["target", "player"],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

#[derive(Debug)]
pub(super) struct SourceAction {
    family: Family,
    site: Site,
    subject: Subject,
    count: Count,
    random: bool,
}

/// The one step of a single unconditioned sentence of the card's only
/// spell ability, at the site that shape establishes.
fn single_step(abilities: &[ast::Ability]) -> Option<(Site, &ast::Effect)> {
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
    let [step] = steps.as_slice() else {
        return None;
    };
    // The card has one face and one block, and the block one sentence and step.
    let (sentence, index) = (0, 0);
    Some((
        Site {
            face: 0,
            block: 0,
            step: format!("/effects/{sentence}/steps/{index}"),
        },
        step,
    ))
}

fn subject(who: &ast::PlayerRef) -> Option<Subject> {
    Some(match who {
        ast::PlayerRef::You(you) => {
            let ast::PlayerRefYou {} = you.as_ref();
            Subject::You
        }
        ast::PlayerRef::EachPlayer(each) => {
            let ast::PlayerRefEachPlayer {} = each.as_ref();
            Subject::EachPlayer
        }
        ast::PlayerRef::EachOpponent(each) => {
            let ast::PlayerRefEachOpponent {} = each.as_ref();
            Subject::EachOpponent
        }
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
                id: id.clone(),
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
                id: id.clone(),
                domain: TargetDomain::OpponentPlayer,
            }
        }
        _ => return None,
    })
}

fn count(amount: &ast::Amount, least: i64) -> Option<Count> {
    match amount {
        ast::Amount::Fixed(n) if n.value >= least => Some(Count::Fixed(n.value)),
        ast::Amount::X(_) => Some(Count::AnnouncedX),
        _ => None,
    }
}

fn claim(
    family: Family,
    site: Site,
    who: &ast::PlayerRef,
    amount: &ast::Amount,
    random: bool,
) -> Option<SourceAction> {
    let least = if family == Family::Discard { 1 } else { 0 };
    Some(SourceAction {
        family,
        site,
        subject: subject(who)?,
        count: count(amount, least)?,
        random,
    })
}

pub(super) fn draw(abilities: &[ast::Ability]) -> Option<SourceAction> {
    let (site, ast::Effect::Draw(draw)) = single_step(abilities)? else {
        return None;
    };
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
    claim(Family::Draw, site, who, amount, false)
}

pub(super) fn mill(abilities: &[ast::Ability]) -> Option<SourceAction> {
    let (site, ast::Effect::Mill(mill)) = single_step(abilities)? else {
        return None;
    };
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
    claim(Family::Mill, site, who, amount, false)
}

fn life(
    abilities: &[ast::Ability],
    event: ast::ReplacementEventVariant4Event,
) -> Option<SourceAction> {
    let (site, ast::Effect::Variant99(life)) = single_step(abilities)? else {
        return None;
    };
    let ast::EffectVariant99 {
        optional: None,
        decided_by: None,
        starting_with: None,
        for_each: None,
        for_each_beyond: None,
        for_each_vote: None,
        duration: None,
        effect,
        who,
        amount,
    } = life.as_ref()
    else {
        return None;
    };
    if *effect != event {
        return None;
    }
    let family = match effect {
        ast::ReplacementEventVariant4Event::GainLife => Family::GainLife,
        ast::ReplacementEventVariant4Event::LoseLife => Family::LoseLife,
    };
    claim(family, site, who, amount, false)
}

pub(super) fn gain_life(abilities: &[ast::Ability]) -> Option<SourceAction> {
    life(abilities, ast::ReplacementEventVariant4Event::GainLife)
}

pub(super) fn lose_life(abilities: &[ast::Ability]) -> Option<SourceAction> {
    life(abilities, ast::ReplacementEventVariant4Event::LoseLife)
}

pub(super) fn discard(abilities: &[ast::Ability]) -> Option<SourceAction> {
    let (site, ast::Effect::Discard(discard)) = single_step(abilities)? else {
        return None;
    };
    let ast::EffectDiscard {
        optional: None,
        decided_by: None,
        starting_with: None,
        for_each: None,
        for_each_beyond: None,
        for_each_vote: None,
        duration: None,
        who,
        what,
        random,
    } = discard.as_ref()
    else {
        return None;
    };
    let random = match random {
        None => false,
        Some(true) => true,
        Some(false) => return None,
    };
    let ast::CostDiscardWhat::ObjectRef(object) = what.as_ref() else {
        return None;
    };
    let ast::ObjectRef::Filter(cards) = object.as_ref() else {
        return None;
    };
    let ast::ObjectRefFilter {
        filter,
        count,
        up_to: None,
        group_constraints: None,
        random: None,
    } = cards.as_ref()
    else {
        return None;
    };
    // Every other filter field is absent: the cards are unqualified.
    let card = serde_json::from_value::<ast::ObjectFilter>(serde_json::json!({"cls": "card"}))
        .expect("card filter");
    if **filter != card {
        return None;
    }
    claim(Family::Discard, site, who, count, random)
}

/// Block-relative AST pointers the family discharges.
struct Paths {
    step: String,
    effect: String,
    who: String,
    player: String,
    selection: String,
    count: String,
    amount: String,
    value: String,
    what: String,
    what_ref: String,
    filter: String,
    filter_cls: String,
    random: String,
}

/// A recorded node's one contiguous run, with no folds or alias.
fn recorded_run(block: &BlockEvidence<'_>, path: &str) -> Result<[u32; 3], EvidenceError> {
    let node = block.node(path)?;
    match (node.basis, node.value) {
        (
            Basis::Recorded,
            PrintedNode::Recorded(PrintedNodeRecorded {
                folds: None,
                path: _,
                span,
            }),
        ) => match span.as_slice() {
            [TokenRun(run)] => Ok(*run),
            _ => Err(block.missing(path)),
        },
        _ => Err(block.missing(path)),
    }
}

impl SourceAction {
    pub(super) fn site(&self) -> &Site {
        &self.site
    }

    fn paths(&self) -> Paths {
        let step = self.site.step.clone();
        let at = |suffix: &str| format!("{step}{suffix}");
        let count = at(self.family.count_path());
        Paths {
            effect: at("/effect"),
            who: at("/who"),
            player: at("/who/player"),
            selection: at("/who/selectionId"),
            amount: format!("{count}/amount"),
            value: format!("{count}/value"),
            what: at("/what"),
            what_ref: at("/what/ref"),
            filter: at("/what/filter"),
            filter_cls: at("/what/filter/cls"),
            random: at("/random"),
            count,
            step,
        }
    }

    pub(super) fn check(
        self,
        source: &str,
        evidence: &CardEvidence,
    ) -> Result<CheckedAction, Failure> {
        let Site { face, block, step } = &self.site;
        if evidence.sha256().is_some() {
            let block = evidence.block(*face, *block, step)?;
            self.cover(&block)?;
        } else if !self.sentence(source)
            || matches!(&self.subject, Subject::Target { id, .. } if *id != format!("line:{block}:0"))
        {
            return Err(EvidenceError {
                check: Check::RequiresR6,
                face: *face,
                block: *block,
                obligation: format!("{step}:whole-instruction-and-target-occurrence"),
                range: None,
            }
            .into());
        }
        Ok(CheckedAction(self))
    }

    /// The R5 proof: the whole Oracle sentence spells exactly this instruction.
    fn sentence(&self, text: &str) -> bool {
        let Some(text) = text.strip_suffix('.') else {
            return false;
        };
        let text = text.to_ascii_lowercase();
        let words: Vec<&str> = text.split_ascii_whitespace().collect();
        let (base, singular) = self.family.words();
        let subject = self.subject.words();
        let rest = match (&self.subject, words.as_slice()) {
            (Subject::You, [verb, rest @ ..]) if *verb == base => rest,
            (Subject::You, ["you", verb, rest @ ..]) if *verb == base => rest,
            (Subject::You, _) => return false,
            (_, words) => match words.split_at_checked(subject.len()) {
                Some((head, [verb, rest @ ..])) if head == subject && *verb == singular => rest,
                _ => return false,
            },
        };
        let noun = self.family.noun(self.count);
        match (rest, self.random) {
            ([count, n], false) => self.count.matches(count) && *n == noun,
            ([count, n, "at", "random"], true) => self.count.matches(count) && *n == noun,
            _ => false,
        }
    }

    /// The R6 proof. Every run comes from the block's recorded citations.
    fn cover(&self, block: &BlockEvidence<'_>) -> Result<(), EvidenceError> {
        let p = self.paths();
        let (line, _) = block.single_line().ok_or_else(|| block.missing(&p.step))?;
        let [step_line, start, end] = recorded_run(block, &p.step)?;
        let who = match block.node(&p.who)?.basis {
            Basis::Implicit(ImplicitReason::ImperativeController)
                if self.subject == Subject::You =>
            {
                None
            }
            _ => Some(recorded_run(block, &p.who)?),
        };
        let count = recorded_run(block, &p.count)?;
        // A discard's object phrase holds both its count and card noun.
        let (object, noun) = match self.family {
            Family::Discard => (
                recorded_run(block, &p.what)?,
                Some(recorded_run(block, &p.filter)?),
            ),
            _ => (count, None),
        };
        let single = |[l, s, e]: [u32; 3]| l == line && e == s + 1;
        let within = |[l, s, e]: [u32; 3]| l == line && start <= s && e <= end;
        if step_line != line
            || !single(count)
            || noun.is_some_and(|noun| !single(noun) || !within(noun))
            || !within(object)
            || who.is_some_and(|who| !within(who))
        {
            return Err(block.missing(&p.step));
        }
        let covers = |[_, s, e]: [u32; 3], index: u32| (s..e).contains(&index);
        let own: Vec<u32> = (start..end)
            .filter(|&index| !covers(object, index) && !who.is_some_and(|who| covers(who, index)))
            .collect();
        let run = |index: u32| [line, index, index + 1];
        let (verb, noun, random) = match (self.family, self.random, own.as_slice()) {
            (Family::Discard, false, [verb]) => (run(*verb), noun.unwrap(), None),
            (Family::Discard, true, [verb, at, last]) if *last == at + 1 => {
                (run(*verb), noun.unwrap(), Some([line, *at, last + 1]))
            }
            (Family::Discard, ..) => return Err(block.missing(&p.step)),
            (_, _, [verb, noun]) => (run(*verb), run(*noun), None),
            _ => return Err(block.missing(&p.step)),
        };
        let (base, singular) = self.family.words();
        let verb_word = match self.subject {
            Subject::You => base,
            _ => singular,
        };
        let noun_word = self.family.noun(self.count);
        let count_word = |text: &str| self.count.matches(&text.to_ascii_lowercase());
        block.check_family(|coverage| {
            coverage.meaning("/kind", verb, |value, text| {
                value == "spell" && text.eq_ignore_ascii_case(verb_word)
            })?;
            coverage.meaning(&p.effect, verb, |value, text| {
                value == self.family.effect() && text.eq_ignore_ascii_case(verb_word)
            })?;
            if self.family == Family::Discard {
                // CR 701.9a: an unqualified card noun selects cards from the hand.
                coverage.meaning(&p.what_ref, noun, |value, text| {
                    value == "filter" && text.eq_ignore_ascii_case(noun_word)
                })?;
                coverage.meaning(&p.filter_cls, noun, |value, text| {
                    value == "card" && text.eq_ignore_ascii_case(noun_word)
                })?;
                if let Some(random) = random {
                    coverage.meaning(&p.random, random, |value, text| {
                        *value == Value::Bool(true) && text.eq_ignore_ascii_case("at random")
                    })?;
                }
            } else {
                // The direct noun establishes the domain of the count.
                coverage.meaning(&p.effect, noun, |value, text| {
                    value == self.family.effect() && text.eq_ignore_ascii_case(noun_word)
                })?;
            }
            match (&self.subject, who) {
                (Subject::You, None) => coverage.implicit(&p.player, ImplicitReason::ImperativeController)?,
                (subject, Some(who)) => {
                    let phrase = subject.words().join(" ");
                    coverage.meaning(&p.player, who, |value, text| {
                        value == subject.tag() && text.eq_ignore_ascii_case(&phrase)
                    })?;
                    if let Subject::Target { id, .. } = subject {
                        let target = coverage.field(&p.selection)?;
                        if !matches!(target.value, PrintedFact::Target(fact)
                            if fact.scope.is_empty() && fact.target_index == 0
                                && fact.tok.0 == [line, who[1], who[1] + 1] && fact.selection_id == *id)
                        {
                            return Err(block.missing(&p.selection));
                        }
                    }
                }
                (_, None) => return Err(block.missing(&p.player)),
            }
            coverage.meaning(&p.amount, count, |value, text| {
                value == match self.count { Count::Fixed(_) => "fixed", Count::AnnouncedX => "x" }
                    && count_word(text)
            })?;
            if let Count::Fixed(n) = self.count {
                let quantity = coverage.field(&p.value)?;
                if !matches!(quantity.value, PrintedFact::Quantity(fact)
                    if fact.tok.0 == count && matches!(fact.value, Nullable::Value(value) if i64::from(value) == n))
                {
                    return Err(block.missing(&p.value));
                }
            }
            coverage.punctuation(line, end, StructuralPunctuation::SentenceTerminator)?;

            let first = who.map(|[_, s, _]| [line, s, s + 1]);
            let article = matches!(self.count, Count::Fixed(1))
                && block.facts().any(|(_, fact)| matches!(fact,
                    PrintedFact::Quantity(q) if q.tok.0 == count && q.form == PrintedFactQuantityForm::Article));
            let quantity_intro = match self.count {
                Count::AnnouncedX => Intro::X,
                Count::Fixed(_) if article => Intro::A,
                Count::Fixed(_) => Intro::Numeral,
            };
            for (index, fact) in block.facts() {
                let field = match fact {
                    PrintedFact::Introducer(fact) if fact.tok.0 == count && fact.form == quantity_intro => &p.amount,
                    PrintedFact::Introducer(fact) if Some(fact.tok.0) == first && matches!(
                        (&self.subject, &fact.form),
                        (Subject::EachPlayer | Subject::EachOpponent, Intro::Each) | (Subject::Target { .. }, Intro::Target)
                    ) => &p.player,
                    PrintedFact::Mention(fact) if self.subject == Subject::You
                        && fact.form == Mention::You && Some(fact.tok.0) == first => &p.player,
                    PrintedFact::Mention(fact) if self.count == Count::AnnouncedX
                        && fact.form == Mention::X && fact.tok.0 == count => &p.amount,
                    PrintedFact::Quantity(fact) if self.count == Count::AnnouncedX
                        && fact.form == PrintedFactQuantityForm::X && fact.tok.0 == count => &p.amount,
                    _ => continue,
                };
                coverage.bind_fact(index, field)?;
            }
            Ok(())
        })
    }
}

pub(super) struct CheckedAction(SourceAction);

impl CheckedAction {
    pub(super) fn lower(self, definition: &mut Definition) -> Result<(), Failure> {
        let paths = self.0.paths();
        let SourceAction {
            family,
            site,
            subject,
            count,
            random,
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
        let (player, recipient) = match subject {
            Subject::You => (PlayerExpr::you(Context::Spell), Selection::SpellController),
            Subject::EachPlayer => (PlayerExpr::EachPlayer, Selection::EachPlayer),
            Subject::EachOpponent => (PlayerExpr::EachOpponent, Selection::EachOpponent),
            Subject::Target { id, domain, .. } => {
                let slot = program.targets.len() as u32;
                program
                    .targets
                    .push(TargetSpec::new(slot, id.clone(), domain));
                (
                    PlayerExpr::target(slot),
                    Selection::Target {
                        slot,
                        selection_id: id,
                        domain,
                    },
                )
            }
        };
        let verb = match family {
            Family::Draw => Verb::Draw { player, amount },
            Family::Mill => Verb::Mill { player, amount },
            Family::GainLife => Verb::GainLife { player, amount },
            Family::LoseLife => Verb::LoseLife { player, amount },
            Family::Discard => Verb::Discard {
                player,
                amount: Some(amount),
                random,
            },
        };
        program
            .instructions
            .push(Instruction::act(verb, site.block as u32));
        crate::reference_trace::record_source(|| {
            let mut selections = vec![SourceSelection {
                path: paths.who,
                selection: recipient,
            }];
            if family == Family::Discard {
                selections.push(SourceSelection {
                    path: paths.what,
                    selection: Selection::ResolutionCards {
                        cardinality: match count {
                            Count::Fixed(1) => Cardinality::One,
                            Count::Fixed(_) => Cardinality::Many,
                            Count::AnnouncedX => Cardinality::Unknown,
                        },
                        random,
                    },
                });
            }
            if count == Count::AnnouncedX {
                selections.push(SourceSelection {
                    path: paths.count,
                    selection: Selection::AnnouncedX,
                });
            }
            SourceChoice {
                family: family.name(),
                face: site.face,
                block: site.block,
                selections,
            }
        });
        Ok(())
    }
}
