//! CR 608.2c/h, 701.9c, 701.17c: a printed "discarded/milled this way"
//! selects the actual card result of a unique earlier local action.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

/// Read the printed relation, leaving the same card predicate shared by
/// library and other public result reads. A different relation is handled
/// by its own operand lowerer.
pub(crate) fn card_result(
    filter: &ast::ObjectFilter,
) -> Result<Option<(kir::EvaluatedVerb, Pred)>, Failure> {
    use ast::ObjectRelationEventResultEvent as Event;
    let Some(ast::ObjectRelation::EventResult(relation)) = filter.relation.as_deref() else {
        return Ok(None);
    };
    let family = match relation.event {
        Event::Discard => kir::EvaluatedVerb::Discard,
        Event::Mill => kir::EvaluatedVerb::Mill,
        _ => return Ok(None),
    };
    if relation.scope != ast::ObjectRelationEventResultScope::ThisWay
        || relation.who.is_some()
        || relation.period.is_some()
    {
        return Err(operand("result", "qualified_event_result"));
    }
    let mut filter = filter.clone();
    filter.relation = None;
    Ok(Some((family, crate::operands::lower_card_filter(&filter)?)))
}

/// Select one direct producer in the same written sequence. Where any
/// competing action appears in a nested node, refuse to guess which result
/// the phrase links to.
pub(crate) fn producer<'a>(
    nodes: impl IntoIterator<Item = &'a kir::Node>,
    family: kir::EvaluatedVerb,
) -> Result<usize, Failure> {
    let is_family = |verb: &Verb| {
        verb.evaluated()
            .is_some_and(|(signature, _)| signature.verb == family)
    };
    let mut producer = None;
    for (index, node) in nodes.into_iter().enumerate() {
        let mut count = 0;
        kir::visit_verbs(node, &mut |verb| count += usize::from(is_family(verb)));
        if count > 0
            && (count != 1
                || !node.verb().is_some_and(is_family)
                || producer.replace(index).is_some())
        {
            return Err(operand("result", "ambiguous_event_result"));
        }
    }
    producer.ok_or_else(|| operand("result", "unbound_event_result"))
}

pub(super) fn linked_count(
    cx: &mut StepCx<'_>,
    filter: &ast::ObjectFilter,
) -> Result<Option<Amount>, Failure> {
    let Some((family, filter)) = card_result(filter)? else {
        return Ok(None);
    };
    let instructions = &cx.program.instructions;
    let base = instructions
        .iter()
        .rposition(|instruction| instruction.source_block() != cx.source_block)
        .map_or(0, |index| index + 1);
    let index = base
        + producer(
            instructions[base..]
                .iter()
                .map(|Instruction::Kernel { node, .. }| node),
            family,
        )?;
    if cx.result_from.is_some_and(|previous| previous != index) {
        return Err(operand("result", "multiple_projections"));
    }
    cx.result_from = Some(index);
    Ok(Some(Amount::Count {
        set: SetExpr::Result { step: 0, filter },
    }))
}

pub(super) fn join_instructions(program: &mut SpellProgram, from: usize, instruction: Instruction) {
    let Instruction::Kernel { node, source_block } = instruction;
    let mut nodes: Vec<_> = program
        .instructions
        .drain(from..)
        .map(|Instruction::Kernel { node, .. }| node)
        .collect();
    nodes.push(node);
    program.instructions.push(Instruction::Kernel {
        node: kir::Node::Seq(nodes),
        source_block,
    });
}
