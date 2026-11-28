//! CR 701.20a-c, 608.2c-d: a publicly revealed library prefix stays in place.
//! A later instruction can move a member, test its printed characteristics,
//! or choose one and move the remainder. The binding belongs to this resolving
//! block, independent of the archive's selection verdict. A reveal after an
//! owner shuffle reads that owner's library after the shuffle (CR 701.24c).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use ast::ObjectFilterZoneChangeHistoryFromZoneRefOwner as Owner;
use kir::{Cond, Destination, Node, SetExpr, Verb};
use serde_json::{Value, json};

fn reject(detail: &str) -> Failure {
    unsupported("effects.steps", format!("verb:reveal:{detail}"))
}

/// This family owns a block beginning with a library-prefix reveal, optionally
/// after a shuffle. The typed lowerer checks the complete block.
pub(super) fn candidate(effects: &[Value]) -> bool {
    if super::private_look::candidate(effects) {
        return true;
    }
    let Some(steps) = effects.first().and_then(|s| s["steps"].as_array()) else {
        return false;
    };
    let reveal = match steps.as_slice() {
        [reveal] | [_, reveal] if reveal["effect"] == "reveal" => reveal,
        _ => return false,
    };
    reveal["what"]["ref"] == "library-cards" && reveal["what"]["position"] == "top"
}

/// In the legacy admission pass, result roles inside this family are proved
/// by the typed resolving block, including its printed "otherwise" and
/// "the rest". Neither the archive's step locator nor its cardinality is used
/// to choose the binding.
pub(crate) fn local_reference(ability: &Value, path: &str) -> bool {
    path.starts_with("/effects/")
        && ability["effects"]
            .as_array()
            .is_some_and(|effects| candidate(effects))
}

pub(super) fn lower(
    effects: &[Value],
    program: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    context: Context,
    block: u32,
) -> Result<(), Failure> {
    if super::private_look::candidate(effects) {
        return super::private_look::lower(effects, program, context, block);
    }
    let effects: Vec<ast::Sentence> = typed(&effects, "reveal effects")?;
    let Some((first, following)) = effects.split_first() else {
        return Err(reject("empty"));
    };
    let ast::Sentence {
        condition: None,
        condition_kind: None,
        condition_position: None,
        steps,
        otherwise: None,
        bindings: None,
        after_conditional: None,
    } = first
    else {
        return Err(reject("qualified"));
    };
    let (shuffle, reveal) = match steps.as_slice() {
        [ast::Effect::Reveal(reveal)] => (None, reveal),
        [
            ast::Effect::ShuffleIntoLibrary(shuffle),
            ast::Effect::Reveal(reveal),
        ] => (Some(shuffle.as_ref()), reveal),
        _ => return Err(reject("producer")),
    };
    let ast::EffectReveal {
        optional: None,
        decided_by: None,
        starting_with: None,
        for_each: None,
        for_each_beyond: None,
        for_each_vote: None,
        duration: None,
        who,
        what,
    } = reveal.as_ref()
    else {
        return Err(reject("qualified"));
    };
    let ast::ObjectRef::LibraryCards(prefix) = what.as_ref() else {
        return Err(reject("library_prefix"));
    };
    let ast::ObjectRefLibraryCards {
        position,
        count,
        of,
    } = prefix.as_ref();
    let (ast::ObjectFilterZoneChangeHistoryFromZoneRefPosition::Top, ast::Amount::Fixed(count)) =
        (position, count.as_ref())
    else {
        return Err(reject("library_prefix"));
    };
    let count = u32::try_from(count.value)
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(|| reject("count"))?;

    let (player, shuffle_slot) = match (who.as_ref(), shuffle) {
        (ast::PlayerRef::You(_), None) if library(of, Owner::Your) => {
            (kir::PlayerExpr::you(context), None)
        }
        (ast::PlayerRef::Owner(owner), Some(shuffle)) if library(of, Owner::Their) => {
            let ast::EffectShuffleIntoLibrary {
                optional: None,
                decided_by: None,
                starting_with: None,
                for_each: None,
                for_each_beyond: None,
                for_each_vote: None,
                duration: None,
                who: shuffler,
                what,
                library: shuffled,
            } = shuffle
            else {
                return Err(reject("shuffle"));
            };
            if context != Context::Spell
                || shuffler != who
                || !library(shuffled, Owner::Their)
                || !matches!(what.as_slice(), [ast::ObjectRef::It(_)])
            {
                return Err(reject("shuffle"));
            }
            let slot = bind_permanent_target(program, slots, &owner.of)?;
            (
                kir::PlayerExpr::OwnerOf {
                    object: kir::Ref::Target { slot },
                },
                Some(slot),
            )
        }
        _ => return Err(reject("library_owner")),
    };

    let body = match following {
        [branch] if count == 1 => {
            single(branch, &player, block, usize::from(shuffle_slot.is_some()))?
        }
        [choice, rest] if player.is_you() => select(choice, rest, context)?,
        _ => return Err(reject("consumer_sequence")),
    };
    if let Some(slot) = shuffle_slot {
        crate::reference_trace::record(|| crate::reference_trace::Bound {
            block,
            word: "it",
            resolved: crate::reference_trace::Resolved::Target {
                selection: program.targets[slot as usize].selection_id.clone(),
            },
        });
        program.instructions.push(Instruction::move_target(
            slot,
            Destination::OwnerLibraryShuffled,
            block,
        ));
    }
    program.instructions.push(Instruction::Kernel {
        node: Node::RevealTop {
            var: 0,
            player,
            count,
            body: Box::new(body),
        },
        source_block: block,
    });
    Ok(())
}

fn library(zone: &ast::ZoneRef, player: Owner) -> bool {
    let ast::ZoneRef {
        zone: kind,
        owner,
        depth,
        position,
    } = zone;
    *kind == ast::Zone::Library
        && owner.as_deref() == Some(&player)
        && depth.is_none()
        && position.is_none()
}

fn pool(var: u32) -> SetExpr {
    SetExpr::Revealed {
        var,
        filter: kir::Pred::default(),
        except: None,
    }
}

fn names_one(what: &ast::ObjectRef) -> bool {
    matches!(what, ast::ObjectRef::It(_))
        || crate::ingest::ast_is(what, json!({"ref":"that","filter":{"cls":"card"}}))
}

/// The revealed singleton's characteristic test and its two branches.
fn single(
    sentence: &ast::Sentence,
    player: &kir::PlayerExpr,
    block: u32,
    reveal: usize,
) -> Result<Node, Failure> {
    let ast::Sentence {
        condition,
        condition_kind: Some(ast::SentenceConditionKind::If),
        condition_position: None,
        steps,
        otherwise,
        bindings: None,
        after_conditional: None,
    } = sentence
    else {
        return Err(reject("condition"));
    };
    let Some(ast::Condition::IsFilter(test)) = condition.as_deref() else {
        return Err(reject("condition"));
    };
    let ast::ConditionIsFilter {
        what,
        filter,
        time: None,
    } = test.as_ref()
    else {
        return Err(reject("condition_time"));
    };
    if !names_one(what) {
        return Err(reject("condition_subject"));
    }
    let record = |object: &ast::ObjectRef| {
        crate::reference_trace::record(|| crate::reference_trace::Bound {
            block,
            word: if matches!(object, ast::ObjectRef::It(_)) {
                "it"
            } else {
                "that"
            },
            resolved: crate::reference_trace::Resolved::StepResult {
                step: format!("/effects/0/steps/{reveal}/what"),
                role: "selection",
            },
        })
    };
    record(what);
    let mut filter = (**filter).clone();
    let permanent = filter.permanent.take().is_some();
    let mut filter = lower_card_filter(&filter)?;
    if permanent {
        // CR 110.4a: a card of a permanent type. Battle is outside the
        // finite engine's represented card types.
        filter.0.push(kir::Atom::AnyType {
            kinds: CardKind::CARD_TYPES
                .into_iter()
                .filter(|k| !matches!(k, CardKind::Instant | CardKind::Sorcery))
                .collect(),
        });
    }
    let otherwise = otherwise
        .as_ref()
        .map(|other| {
            let ast::SentenceOtherwise { on, steps } = other.as_ref();
            if *on != ast::SentenceOtherwiseOn::ConditionFalse {
                return Err(reject("otherwise"));
            }
            linked_move(steps, player, record).map(Box::new)
        })
        .transpose()?;
    Ok(Node::If {
        condition: Cond::Exists {
            set: SetExpr::Revealed {
                var: 0,
                filter,
                except: None,
            },
        },
        then: Box::new(linked_move(steps, player, record)?),
        otherwise,
    })
}

fn linked_move(
    actions: &[ast::Effect],
    player: &kir::PlayerExpr,
    record: impl Fn(&ast::ObjectRef),
) -> Result<Node, Failure> {
    let [ast::Effect::MoveZone(movement)] = actions else {
        return Err(reject("linked_action"));
    };
    let MoveFields {
        what,
        to,
        who,
        optional,
    } = movement_fields(movement)?;
    if optional || !names_one(what) {
        return Err(reject("linked_object"));
    }
    let destination = if crate::ingest::ast_is(to, json!({"zone":"battlefield"}))
        && if player.is_you() {
            who.is_none()
        } else {
            matches!(who, Some(ast::PlayerRef::They(_)))
        } {
        Destination::Battlefield {
            controller: player.clone(),
            tapped: false,
        }
    } else if player.is_you()
        && who.is_none()
        && crate::ingest::ast_is(to, json!({"zone":"hand","owner":"your"}))
    {
        Destination::OwnerHand
    } else {
        return Err(reject("linked_destination"));
    };
    record(what);
    Ok(Node::Do(Verb::Move {
        what: pool(0),
        to: destination,
    }))
}

/// A single optional pick and the set difference, both over the original
/// public prefix even when the selected card couldn't move.
fn select(choice: &ast::Sentence, rest: &ast::Sentence, context: Context) -> Result<Node, Failure> {
    fn plain(sentence: &ast::Sentence) -> Result<&ast::EffectMoveZone, Failure> {
        let ast::Sentence {
            condition: None,
            condition_kind: None,
            condition_position: None,
            steps,
            otherwise: None,
            bindings: None,
            after_conditional: None,
        } = sentence
        else {
            return Err(reject("selection_sentence"));
        };
        let [ast::Effect::MoveZone(movement)] = steps.as_slice() else {
            return Err(reject("selection_action"));
        };
        Ok(movement)
    }
    let MoveFields {
        what,
        to,
        who,
        optional,
    } = movement_fields(plain(choice)?)?;
    let ast::ObjectRef::Filter(selection) = what else {
        return Err(reject("selection"));
    };
    let ast::ObjectRefFilter {
        filter,
        count,
        up_to: None,
        group_constraints: None,
        random: None,
    } = selection.as_ref()
    else {
        return Err(reject("selection"));
    };
    let mut filter = (**filter).clone();
    if !optional
        || who.is_some()
        || !matches!(count.as_ref(), ast::Amount::Fixed(n) if n.value == 1)
        || !matches!(
            filter.from_among.take().as_deref(),
            Some(ast::ObjectRef::Them(_))
        )
        || !crate::ingest::ast_is(to, json!({"zone":"hand","owner":"your"}))
    {
        return Err(reject("selection"));
    }
    let MoveFields {
        what,
        to,
        who,
        optional,
    } = movement_fields(plain(rest)?)?;
    if optional
        || who.is_some()
        || !crate::ingest::ast_is(what, json!({"ref":"rest"}))
        || !crate::ingest::ast_is(to, json!({"zone":"graveyard","owner":"your"}))
    {
        return Err(reject("remainder"));
    }
    Ok(Node::Choose {
        var: 1,
        chooser: kir::PlayerExpr::you(context),
        from: SetExpr::Revealed {
            var: 0,
            filter: lower_card_filter(&filter)?,
            except: None,
        },
        minimum: kir::Amount::fixed(0),
        maximum: kir::Amount::fixed(1),
        body: Box::new(Node::Seq(vec![
            Node::Do(Verb::Move {
                what: pool(1),
                to: Destination::OwnerHand,
            }),
            Node::Do(Verb::Move {
                what: SetExpr::Revealed {
                    var: 0,
                    filter: kir::Pred::default(),
                    except: Some(1),
                },
                to: Destination::OwnerGraveyard,
            }),
        ])),
    })
}

struct MoveFields<'a> {
    what: &'a ast::ObjectRef,
    to: &'a ast::ZoneRef,
    who: Option<&'a ast::PlayerRef>,
    optional: bool,
}

fn movement_fields(movement: &ast::EffectMoveZone) -> Result<MoveFields<'_>, Failure> {
    let ast::EffectMoveZone {
        optional,
        decided_by: None,
        starting_with: None,
        for_each: None,
        for_each_beyond: None,
        for_each_vote: None,
        duration: None,
        what,
        to,
        from: None,
        tapped: None,
        controller: None,
        order: None,
        who,
        entry: None,
    } = movement
    else {
        return Err(reject("qualified_movement"));
    };
    Ok(MoveFields {
        what,
        to,
        who: who.as_deref(),
        optional: optional.is_some(),
    })
}
