//! Source authority for complete player action spells.
//!
//! **Supported.** A single-face instant or sorcery whose only line is one of
//! two shapes, and nothing else.
//!
//! *One action* (`draw`, `mill`, `gain-life`, `lose-life`, `discard`): one
//! unconditioned sentence with one step.
//!
//! | Family | Printed instruction | Kernel verb |
//! | --- | --- | --- |
//! | `draw` | "draw(s) N card(s)" | `Draw` (CR 121.1–121.2c) |
//! | `mill` | "mill(s) N card(s)" | `Mill` (CR 701.17) |
//! | `gain-life` | "gain(s) N life" | `GainLife` (CR 119.3, 119.6) |
//! | `lose-life` | "lose(s) N life" | `LoseLife` (CR 119.3) |
//! | `discard` | "discard(s) N card(s) [at random]" | `Discard` (CR 701.9a–b) |
//!
//! *Linked count* (`linked-count`): two unconditioned one-step sentences.
//! The first is a discard above; the second is a draw, mill, gain-life or
//! lose-life above for you, with a fixed N of at least one and "for each
//! [type] card discarded this way", read from the first step's actual result
//! (CR 608.2c, 608.2h, 701.9c). One kernel sequence lowers both steps, and
//! the second counts the cards the first moved, from their last public
//! characteristics; a type is one card type.
//!
//! The recipient is the controller ("you", or an imperative with no
//! subject), each player, each opponent, each other player (CR 102.2,
//! 806.1, 903.2: Commander is two-player or Free-for-All, where every other
//! player is an opponent; a team game (CR 102.3) would need a different
//! lowering; APNAP per 101.4), or one announced target player or opponent
//! (CR 115.1a, 601.2c).
//! N is a fixed count or the spell's announced X (CR 107.3a, 601.2b), which
//! needs an X in the mana cost. A discard count is at least one; the
//! recipient chooses the cards, or they are chosen at random, at resolution
//! (CR 608.2h, 701.9b).
//!
//! **Proof.** R6 decides from the typed AST and verified printed evidence:
//! the step, recipient, count, card-noun and for-each runs come from the
//! recorded node citations, every AST field is discharged (P11) and every
//! fact and token gets a checked meaning or structural role (P12). The
//! `each-player` base of "each other player" has no span of its own; the
//! whole recorded phrase proves it. R5 has no evidence; the whole Oracle text
//! must spell exactly these sentences, and any target must be the line's
//! first, or the card reports `evidence:requires-r6`.
//!
//! **Left to the existing authority.** Any other ability, sentence or step
//! field: optional or chooser-qualified actions, `startingWith`, other
//! `forEach` filters or producers, durations, votes, quantified, repeated,
//! up-to or random target players, other player filters, "any number of
//! target players", computed counts, whole-hand discards, qualified discard
//! cards, `random: false`, and compound or modal blocks. Damage and object
//! references are a later family.
//!
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::source_authority::Site;
use super::*;
use crate::{
    ingest::evidence::{
        Basis, BlockEvidence, CardEvidence, Check, Coverage, EvidenceError, StructuralPunctuation,
    },
    reference_trace::{Cardinality, Selection, SourceChoice, SourceSelection},
    rules::card_result::{self, ResultFamily},
};
use mtg_oracle_ast::printed::{
    ImplicitReason, Nullable, PrintedFact, PrintedFactExceptionForm,
    PrintedFactIntroducerForm as Intro, PrintedFactMentionForm as Mention, PrintedFactQuantityForm,
    PrintedNode, PrintedNodeRecorded, TokenRun,
};
use serde_json::{Value, json};

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
    EachOtherPlayer,
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
            Self::EachOtherPlayer => "filtered",
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
            Self::EachOtherPlayer => &["each", "other", "player"],
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

/// One step's player action.
#[derive(Debug)]
struct Action {
    family: Family,
    site: Site,
    subject: Subject,
    count: Count,
    random: bool,
}

/// CR 608.2c: the second step counts the cards the first discarded.
#[derive(Debug)]
struct Link {
    /// The printed card type word qualifying "card", if any.
    card_type: Option<&'static str>,
    filter: ast::ObjectFilter,
}

/// The card types a linked "for each [type] card" may name (CR 205.2a).
const CARD_TYPES: &[&str] = &[
    "artifact",
    "creature",
    "enchantment",
    "instant",
    "land",
    "planeswalker",
    "sorcery",
];

#[derive(Debug)]
pub(super) struct SourceAction {
    /// One action per sentence, in printed order.
    actions: Vec<Action>,
    /// Present exactly when there are two actions.
    link: Option<Link>,
}

/// The unconditioned one-step sentences of the card's only spell ability,
/// each with the site that shape establishes.
fn sentences(abilities: &[ast::Ability]) -> Option<Vec<(Site, &ast::Effect)>> {
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
    // The card has one face and one block.
    effects
        .iter()
        .enumerate()
        .map(|(sentence, effect)| {
            let ast::Sentence {
                condition: None,
                condition_kind: None,
                condition_position: None,
                otherwise: None,
                bindings: None,
                after_conditional: None,
                steps,
            } = effect
            else {
                return None;
            };
            let [step] = steps.as_slice() else {
                return None;
            };
            Some((
                Site {
                    face: 0,
                    block: 0,
                    step: format!("/effects/{sentence}/steps/0"),
                },
                step,
            ))
        })
        .collect()
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
        ast::PlayerRef::Filtered(filtered) => {
            let ast::PlayerRefFiltered {
                base,
                filter,
                random: None,
            } = filtered.as_ref()
            else {
                return None;
            };
            let ast::PlayerRef::EachPlayer(each) = base.as_ref() else {
                return None;
            };
            let ast::PlayerRefEachPlayer {} = each.as_ref();
            let ast::PlayerFilter {
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
                other: Some(_),
                attacking_player: None,
                preceding_action: None,
                event_results: None,
                friend_or_foe: None,
                searched: None,
                voted_for: None,
            } = filter.as_ref()
            else {
                return None;
            };
            Subject::EachOtherPlayer
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

fn action(
    family: Family,
    site: Site,
    who: &ast::PlayerRef,
    amount: &ast::Amount,
    random: bool,
) -> Option<Action> {
    let least = if family == Family::Discard { 1 } else { 0 };
    Some(Action {
        family,
        site,
        subject: subject(who)?,
        count: count(amount, least)?,
        random,
    })
}

type ForEach<'a> = Option<&'a ast::ObjectFilter>;

fn draw_step(site: Site, step: &ast::Effect) -> Option<(Action, ForEach<'_>)> {
    let ast::Effect::Draw(draw) = step else {
        return None;
    };
    let ast::EffectDraw {
        optional: None,
        decided_by: None,
        starting_with: None,
        for_each,
        for_each_beyond: None,
        for_each_vote: None,
        duration: None,
        who,
        amount,
    } = draw.as_ref()
    else {
        return None;
    };
    Some((
        action(Family::Draw, site, who, amount, false)?,
        for_each.as_deref(),
    ))
}

fn mill_step(site: Site, step: &ast::Effect) -> Option<(Action, ForEach<'_>)> {
    let ast::Effect::Mill(mill) = step else {
        return None;
    };
    let ast::EffectMill {
        optional: None,
        decided_by: None,
        starting_with: None,
        for_each,
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
    Some((
        action(Family::Mill, site, who, amount, false)?,
        for_each.as_deref(),
    ))
}

fn life_step(
    site: Site,
    step: &ast::Effect,
    event: ast::ReplacementEventVariant4Event,
) -> Option<(Action, ForEach<'_>)> {
    let ast::Effect::Variant99(life) = step else {
        return None;
    };
    let ast::EffectVariant99 {
        optional: None,
        decided_by: None,
        starting_with: None,
        for_each,
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
    Some((
        action(family, site, who, amount, false)?,
        for_each.as_deref(),
    ))
}

fn gain_life_step(site: Site, step: &ast::Effect) -> Option<(Action, ForEach<'_>)> {
    life_step(site, step, ast::ReplacementEventVariant4Event::GainLife)
}

fn lose_life_step(site: Site, step: &ast::Effect) -> Option<(Action, ForEach<'_>)> {
    life_step(site, step, ast::ReplacementEventVariant4Event::LoseLife)
}

fn discard_step(site: Site, step: &ast::Effect) -> Option<Action> {
    let ast::Effect::Discard(discard) = step else {
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
    if **filter != typed_filter(json!({"cls": "card"})) {
        return None;
    }
    action(Family::Discard, site, who, count, random)
}

fn typed_filter(value: Value) -> ast::ObjectFilter {
    serde_json::from_value(value).expect("typed object filter")
}

/// The one-step families: the card's only sentence, with no `forEach`.
fn single(
    abilities: &[ast::Ability],
    step: impl FnOnce(Site, &ast::Effect) -> Option<(Action, ForEach<'_>)>,
) -> Option<SourceAction> {
    let [(site, effect)] = sentences(abilities)?.try_into().ok()?;
    let (action, None) = step(site, effect)? else {
        return None;
    };
    Some(SourceAction {
        actions: vec![action],
        link: None,
    })
}

pub(super) fn draw(abilities: &[ast::Ability]) -> Option<SourceAction> {
    single(abilities, draw_step)
}

pub(super) fn mill(abilities: &[ast::Ability]) -> Option<SourceAction> {
    single(abilities, mill_step)
}

pub(super) fn gain_life(abilities: &[ast::Ability]) -> Option<SourceAction> {
    single(abilities, gain_life_step)
}

pub(super) fn lose_life(abilities: &[ast::Ability]) -> Option<SourceAction> {
    single(abilities, lose_life_step)
}

pub(super) fn discard(abilities: &[ast::Ability]) -> Option<SourceAction> {
    single(abilities, |site, step| {
        Some((discard_step(site, step)?, None))
    })
}

/// A discard, then an action repeated for each card it discarded.
pub(super) fn linked_count(abilities: &[ast::Ability]) -> Option<SourceAction> {
    let [(first, producer), (second, consumer)] = sentences(abilities)?.try_into().ok()?;
    let producer = discard_step(first, producer)?;
    let (consumer, Some(each)) = draw_step(second.clone(), consumer)
        .or_else(|| mill_step(second.clone(), consumer))
        .or_else(|| gain_life_step(second.clone(), consumer))
        .or_else(|| lose_life_step(second.clone(), consumer))?
    else {
        return None;
    };
    if !matches!(consumer.count, Count::Fixed(n) if n >= 1) {
        return None;
    }
    // Only "you" is witnessed as the counted step's recipient. With "each
    // player draws a card for each card discarded this way", every player
    // would receive the whole total; leave such a card unclaimed until a
    // non-controller recipient is witnessed.
    if !matches!(consumer.subject, Subject::You) {
        return None;
    }
    let relation = json!({"relation": "event-result", "event": "discard", "scope": "this-way"});
    let card_type = CARD_TYPES.iter().copied().find(|kind| {
        *each == typed_filter(json!({"types": [kind], "cls": "card", "relation": relation.clone()}))
    });
    if card_type.is_none() && *each != typed_filter(json!({"cls": "card", "relation": relation})) {
        return None;
    }
    Some(SourceAction {
        actions: vec![producer, consumer],
        link: Some(Link {
            card_type,
            filter: each.clone(),
        }),
    })
}

/// Block-relative AST pointers an action discharges.
struct Paths {
    step: String,
    effect: String,
    who: String,
    player: String,
    base_player: String,
    filter_other: String,
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

impl Action {
    fn paths(&self) -> Paths {
        let step = self.site.step.clone();
        let at = |suffix: &str| format!("{step}{suffix}");
        let count = at(self.family.count_path());
        Paths {
            effect: at("/effect"),
            who: at("/who"),
            player: at("/who/player"),
            base_player: at("/who/base/player"),
            filter_other: at("/who/filter/other"),
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

    /// The R5 proof: one sentence's words spell exactly this action, with
    /// `tail` after it.
    fn spelled(&self, words: &[&str], tail: &[&str]) -> bool {
        let (base, singular) = self.family.words();
        let subject = self.subject.words();
        let rest = match (&self.subject, words) {
            (Subject::You, [verb, rest @ ..]) if *verb == base => rest,
            (Subject::You, ["you", verb, rest @ ..]) if *verb == base => rest,
            (Subject::You, _) => return false,
            (_, words) => match words.split_at_checked(subject.len()) {
                Some((head, [verb, rest @ ..])) if head == subject && *verb == singular => rest,
                _ => return false,
            },
        };
        let noun = self.family.noun(self.count);
        let random: &[&str] = if self.random { &["at", "random"] } else { &[] };
        match rest {
            [count, n, rest @ ..] => {
                self.count.matches(count) && *n == noun && rest == [random, tail].concat()
            }
            _ => false,
        }
    }
}

/// A recorded node's one contiguous run, and its one fold if present.
fn recorded(
    block: &BlockEvidence<'_>,
    path: &str,
) -> Result<([u32; 3], Option<[u32; 3]>), EvidenceError> {
    let node = block.node(path)?;
    let (
        Basis::Recorded,
        PrintedNode::Recorded(PrintedNodeRecorded {
            folds,
            path: _,
            span,
        }),
    ) = (node.basis, node.value)
    else {
        return Err(block.missing(path));
    };
    let fold = match folds.as_deref() {
        None => None,
        Some([TokenRun(fold)]) => Some(*fold),
        Some(_) => return Err(block.missing(path)),
    };
    match span.as_slice() {
        [TokenRun(run)] => Ok((*run, fold)),
        _ => Err(block.missing(path)),
    }
}

/// A recorded node's one contiguous run, with no folds or alias.
fn recorded_run(block: &BlockEvidence<'_>, path: &str) -> Result<[u32; 3], EvidenceError> {
    match recorded(block, path)? {
        (run, None) => Ok(run),
        _ => Err(block.missing(path)),
    }
}

/// The runs an action's step proof established.
struct Covered {
    /// Its sentence terminator follows this token.
    end: u32,
    /// The step's folded clause, for a linked count.
    fold: Option<[u32; 3]>,
    verb: [u32; 3],
}

impl SourceAction {
    pub(super) fn site(&self) -> &Site {
        &self.actions[0].site
    }

    fn name(&self) -> &'static str {
        match (&self.link, self.actions.as_slice()) {
            (Some(_), _) => "linked-count",
            (None, [action]) => action.family.name(),
            (None, _) => unreachable!("one action per unlinked card"),
        }
    }

    pub(super) fn check(
        self,
        source: &str,
        evidence: &CardEvidence,
    ) -> Result<CheckedAction, Failure> {
        let Site { face, block, step } = self.site();
        if evidence.sha256().is_some() {
            let block = evidence.block(*face, *block, step)?;
            self.cover(&block)?;
        } else if !self.sentences(source)
            || self.actions.iter().any(|action| {
                matches!(&action.subject, Subject::Target { id, .. } if *id != format!("line:{block}:0"))
            })
            || self.targets() > 1
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

    fn targets(&self) -> usize {
        self.actions
            .iter()
            .filter(|action| matches!(action.subject, Subject::Target { .. }))
            .count()
    }

    /// The R5 proof: the whole Oracle text spells exactly these sentences.
    fn sentences(&self, text: &str) -> bool {
        let Some(text) = text.strip_suffix('.') else {
            return false;
        };
        let text = text.to_ascii_lowercase();
        let printed: Vec<Vec<&str>> = text
            .split(". ")
            .map(|sentence| sentence.split_ascii_whitespace().collect())
            .collect();
        if printed.len() != self.actions.len() {
            return false;
        }
        let tail: Vec<&str> = match &self.link {
            None => vec![],
            Some(link) => ["for", "each"]
                .into_iter()
                .chain(link.card_type)
                .chain(["card", "discarded", "this", "way"])
                .collect(),
        };
        self.actions
            .iter()
            .zip(&printed)
            .enumerate()
            .all(|(index, (action, words))| {
                action.spelled(words, if index == 1 { &tail } else { &[] })
            })
    }

    /// The R6 proof. Every run comes from the block's recorded citations.
    fn cover(&self, block: &BlockEvidence<'_>) -> Result<(), EvidenceError> {
        let (line, _) = block
            .single_line()
            .ok_or_else(|| block.missing(&self.actions[0].site.step))?;
        block.check_family(|coverage| {
            for (index, action) in self.actions.iter().enumerate() {
                let linked = index == 1;
                let covered = action.cover(block, coverage, line, linked)?;
                if index == 0 {
                    coverage.meaning("/kind", covered.verb, |value, text| {
                        value == "spell" && text.eq_ignore_ascii_case(action.verb_word())
                    })?;
                }
                let end = match (&self.link, covered.fold) {
                    (Some(link), Some(fold)) if linked => {
                        cover_link(block, coverage, action, link, fold)?
                    }
                    (_, None) if !linked => covered.end,
                    _ => return Err(block.missing(&action.site.step)),
                };
                coverage.punctuation(line, end, StructuralPunctuation::SentenceTerminator)?;
            }
            Ok(())
        })
    }
}

impl Action {
    fn verb_word(&self) -> &'static str {
        let (base, singular) = self.family.words();
        match self.subject {
            Subject::You => base,
            _ => singular,
        }
    }

    /// Cover one step's own runs and facts; a linked count's step also
    /// returns its folded for-each clause.
    fn cover(
        &self,
        block: &BlockEvidence<'_>,
        coverage: &mut Coverage<'_, '_>,
        line: u32,
        linked: bool,
    ) -> Result<Covered, EvidenceError> {
        let p = self.paths();
        let ([step_line, start, end], fold) = recorded(block, &p.step)?;
        let fold = match fold {
            Some(fold @ [l, s, e]) if linked && l == line && s == end && s < e => Some(fold),
            None if !linked => None,
            _ => return Err(block.missing(&p.step)),
        };
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
        let verb_word = self.verb_word();
        let noun_word = self.family.noun(self.count);
        let count_word = |text: &str| self.count.matches(&text.to_ascii_lowercase());
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
            (Subject::You, None) => {
                coverage.implicit(&p.player, ImplicitReason::ImperativeController)?
            }
            (Subject::EachOtherPlayer, Some(who)) => {
                let phrase = |text: &str| text.eq_ignore_ascii_case("each other player");
                coverage.meaning(&p.player, who, |value, text| {
                    value == "filtered" && phrase(text)
                })?;
                // The parser synthesizes the base inside the recorded phrase.
                coverage.synthesized(&p.base_player, who, |value, text| {
                    value == "each-player" && phrase(text)
                })?;
                let other = coverage.field(&p.filter_other)?;
                if !matches!(other.value, PrintedFact::Exception(fact)
                    if fact.form == PrintedFactExceptionForm::Other
                        && fact.tok.0 == [line, who[1] + 1, who[1] + 2])
                {
                    return Err(block.missing(&p.filter_other));
                }
            }
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
            value
                == match self.count {
                    Count::Fixed(_) => "fixed",
                    Count::AnnouncedX => "x",
                }
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
                PrintedFact::Introducer(fact)
                    if fact.tok.0 == count && fact.form == quantity_intro =>
                {
                    &p.amount
                }
                PrintedFact::Introducer(fact)
                    if Some(fact.tok.0) == first
                        && matches!(
                            (&self.subject, &fact.form),
                            (
                                Subject::EachPlayer
                                    | Subject::EachOpponent
                                    | Subject::EachOtherPlayer,
                                Intro::Each
                            ) | (Subject::Target { .. }, Intro::Target)
                        ) =>
                {
                    &p.player
                }
                PrintedFact::Mention(fact)
                    if self.subject == Subject::You
                        && fact.form == Mention::You
                        && Some(fact.tok.0) == first =>
                {
                    &p.player
                }
                PrintedFact::Mention(fact)
                    if self.count == Count::AnnouncedX
                        && fact.form == Mention::X
                        && fact.tok.0 == count =>
                {
                    &p.amount
                }
                PrintedFact::Quantity(fact)
                    if self.count == Count::AnnouncedX
                        && fact.form == PrintedFactQuantityForm::X
                        && fact.tok.0 == count =>
                {
                    &p.amount
                }
                _ => continue,
            };
            coverage.bind_fact(index, field)?;
        }
        Ok(Covered { end, fold, verb })
    }
}

/// Cover "for each [type] card discarded this way" in a linked count's
/// folded clause, and return the index of its sentence terminator.
fn cover_link(
    block: &BlockEvidence<'_>,
    coverage: &mut Coverage<'_, '_>,
    action: &Action,
    link: &Link,
    [line, start, end]: [u32; 3],
) -> Result<u32, EvidenceError> {
    let step = &action.site.step;
    let each = format!("{step}/forEach");
    let relation = format!("{each}/relation");
    let filter = recorded_run(block, &each)?;
    let linked = recorded_run(block, &relation)?;
    let nouns = 1 + u32::from(link.card_type.is_some());
    // "for each" | [type] "card" | "discarded" "this" "way"
    if filter != [line, start + 2, end]
        || linked != [line, start + 2 + nouns, end]
        || linked[2] - linked[1] != 3
    {
        return Err(block.missing(&each));
    }
    let run = |index: u32| [line, index, index + 1];
    let introducer = [line, start, start + 2];
    coverage.introducer(&each, introducer, |text| {
        text.eq_ignore_ascii_case("for each")
    })?;
    let mut noun = start + 2;
    if let Some(card_type) = link.card_type {
        coverage.meaning(&format!("{each}/types"), run(noun), |value, text| {
            *value == json!([card_type]) && text.eq_ignore_ascii_case(card_type)
        })?;
        noun += 1;
    }
    coverage.meaning(&format!("{each}/cls"), run(noun), |value, text| {
        value == "card" && text.eq_ignore_ascii_case("card")
    })?;
    let [_, verb, _] = linked;
    // CR 608.2c, 701.9c: "discarded this way" names the earlier discard's result.
    coverage.meaning(&format!("{relation}/relation"), linked, |value, text| {
        value == "event-result" && text.eq_ignore_ascii_case("discarded this way")
    })?;
    coverage.meaning(&format!("{relation}/event"), run(verb), |value, text| {
        value == "discard" && text.eq_ignore_ascii_case("discarded")
    })?;
    let scope = format!("{relation}/scope");
    let this_way = coverage.field(&scope)?;
    if !matches!(this_way.value, PrintedFact::Mention(fact)
        if fact.form == Mention::ThisWay && fact.tok.0 == [line, verb + 1, end])
    {
        return Err(block.missing(&scope));
    }
    for (index, fact) in block.facts() {
        let field = match fact {
            PrintedFact::Introducer(fact)
                if fact.form == Intro::Each && fact.tok.0 == run(start + 1) =>
            {
                &each
            }
            PrintedFact::Mention(fact)
                if fact.form == Mention::This && fact.tok.0 == run(verb + 1) =>
            {
                &scope
            }
            _ => continue,
        };
        coverage.bind_fact(index, field)?;
    }
    Ok(end)
}

pub(super) struct CheckedAction(SourceAction);

impl CheckedAction {
    pub(super) fn lower(self, definition: &mut Definition) -> Result<(), Failure> {
        let name = self.0.name();
        let SourceAction { actions, link } = self.0;
        let x_mana_cost = definition.x_mana_cost;
        let program = definition.spell.as_mut().expect("ordinary spell shell");
        let block = actions[0].site.block;
        let face = actions[0].site.face;
        let mut nodes = vec![];
        let mut selections = vec![];
        for (index, action) in actions.iter().enumerate() {
            let paths = action.paths();
            let amount = match (action.count, &link) {
                // CR 608.2c, 608.2h: N for each card the first step discarded.
                (Count::Fixed(n), Some(link)) if index == 1 => {
                    let Some((ResultFamily::Discard, filter)) =
                        card_result::card_result(&link.filter)?
                    else {
                        return Err(invalid(
                            "source.link",
                            "linked count without a discard result",
                        ));
                    };
                    let count = Amount::Count {
                        set: SetExpr::Result { step: 0, filter },
                    };
                    if n == 1 {
                        count
                    } else {
                        Amount::Times {
                            factor: Quantity::from(n),
                            of: Box::new(count),
                        }
                    }
                }
                (Count::Fixed(n), _) => Amount::fixed(n),
                // CR 107.3a, 601.2b: the spell's mana cost supplies the
                // announced X. This production has no resolution-time binding.
                (Count::AnnouncedX, _) if x_mana_cost => Amount::announced_x(),
                (Count::AnnouncedX, _) => {
                    return Err(unsupported(
                        "source.mana_cost",
                        "announced X has no mana-cost declaration",
                    ));
                }
            };
            let (player, recipient) = match &action.subject {
                Subject::You => (PlayerExpr::you(Context::Spell), Selection::SpellController),
                Subject::EachPlayer => (PlayerExpr::EachPlayer, Selection::EachPlayer),
                Subject::EachOpponent => (PlayerExpr::EachOpponent, Selection::EachOpponent),
                // CR 102.2, 806.1, 903.2: Commander is two-player or
                // Free-for-All, so each player other than you is an opponent.
                // Team games (CR 102.3) break this; change this lowering if
                // team formats are ever supported.
                Subject::EachOtherPlayer => {
                    selections.push(SourceSelection {
                        path: format!("{}/base", paths.who),
                        selection: Selection::EachPlayer,
                    });
                    (PlayerExpr::EachOpponent, Selection::EachOtherPlayer)
                }
                Subject::Target { id, domain, .. } => {
                    let slot = program.targets.len() as u32;
                    program
                        .targets
                        .push(TargetSpec::new(slot, id.clone(), *domain));
                    (
                        PlayerExpr::target(slot),
                        Selection::Target {
                            slot,
                            selection_id: id.clone(),
                            domain: *domain,
                        },
                    )
                }
            };
            let random = action.random;
            nodes.push(kir::Node::Do(match action.family {
                Family::Draw => Verb::Draw { player, amount },
                Family::Mill => Verb::Mill { player, amount },
                Family::GainLife => Verb::GainLife { player, amount },
                Family::LoseLife => Verb::LoseLife { player, amount },
                Family::Discard => Verb::Discard {
                    player,
                    amount: Some(amount),
                    random,
                },
            }));
            selections.push(SourceSelection {
                path: paths.who,
                selection: recipient,
            });
            if action.family == Family::Discard {
                selections.push(SourceSelection {
                    path: paths.what,
                    selection: Selection::ResolutionCards {
                        cardinality: match action.count {
                            Count::Fixed(1) => Cardinality::One,
                            Count::Fixed(_) => Cardinality::Many,
                            Count::AnnouncedX => Cardinality::Unknown,
                        },
                        random,
                    },
                });
            }
            if action.count == Count::AnnouncedX {
                selections.push(SourceSelection {
                    path: paths.count,
                    selection: Selection::AnnouncedX,
                });
            }
            if link.is_some() && index == 1 {
                selections.push(SourceSelection {
                    path: format!("{}/forEach/relation", action.site.step),
                    selection: Selection::LinkedResult {
                        step: actions[0].site.step.clone(),
                    },
                });
            }
        }
        // One kernel sequence keeps the first step's result for the second.
        let node = match <[_; 1]>::try_from(nodes) {
            Ok([node]) => node,
            Err(nodes) => kir::Node::Seq(nodes),
        };
        program.instructions.push(Instruction::Kernel {
            node,
            source_block: block as u32,
        });
        crate::reference_trace::record_source(|| SourceChoice {
            family: name,
            face,
            block,
            selections,
        });
        Ok(())
    }
}
