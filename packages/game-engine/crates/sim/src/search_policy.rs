//! Stateless rollout policy. Each call receives only the simulated chooser's
//! current permitted observation and the immutable public catalog.
use super::search_candidates::{self, ActionPlan, TargetPlan};
use mtg_contracts::{Catalog, Quantity, SpellProgram};
use mtg_core::{
    Action, Answer, Candidate, Decision, Flag, Observation, PayAnswer, PayStep, PlayerTargetEntity,
    Reason, Shape, TurnStep, Viewer,
};

pub const ROLLOUT_POLICY: &str = "foundations-current-view-development-v1";
fn selected_program<'a>(
    catalog: &'a Catalog,
    observation: &Observation,
) -> Option<&'a SpellProgram> {
    let entry = observation.view.stack.last()?;
    let program = if let Some(trigger) = &entry.triggered {
        let mtg_core::VisibleTriggerAbility::Intrinsic { index } = &trigger.ability;
        &search_candidates::definition(catalog, &trigger.oracle_id)
            .ok()?
            .triggered
            .get(usize::try_from(*index).ok()?)?
            .program
    } else if let Some(ability) = &entry.ability {
        &search_candidates::definition(catalog, &ability.oracle_id)
            .ok()?
            .activated
            .get(usize::try_from(ability.ability).ok()?)?
            .program
    } else {
        let object = observation
            .view
            .objects
            .iter()
            .find(|o| Some(&o.handle) == entry.spell.as_ref())?;
        search_candidates::definition(catalog, &object.oracle_id)
            .ok()?
            .spell
            .as_ref()?
    };
    Some(program)
}
pub fn initial_action(plan: &ActionPlan) -> Action<Viewer> {
    match plan {
        ActionPlan::Pass => Action::Pass,
        ActionPlan::PlayLand { object } => Action::PlayLand {
            object: object.clone(),
        },
        ActionPlan::Cast { card, .. } => Action::Cast { card: card.clone() },
        ActionPlan::Activate {
            object, ability, ..
        } => Action::ActivateAbility {
            object: object.clone(),
            ability: *ability,
        },
    }
}
pub fn choose(
    catalog: &Catalog,
    observation: &Observation,
    check: &mut dyn FnMut() -> Result<(), String>,
) -> Result<Answer, String> {
    check()?;
    let view = &observation.view;
    let request = observation
        .decision
        .as_ref()
        .ok_or("rollout observation has no decision")?;
    let d = request;
    let beneficial =
        selected_program(catalog, observation).is_some_and(search_candidates::beneficial);
    Ok(match (&d.reason,&d.shape) {
        (Reason::Priority,_)=>{
            let mut action=Action::Pass;
            if view.active==Some(view.viewer) && view.stack.is_empty() && matches!(view.turn.step,TurnStep::PrecombatMain|TurnStep::PostcombatMain){
                let plans=search_candidates::generate(catalog,observation,check)?;
                if let Some(c)=plans.candidates.iter().find(|c|!matches!(c.plan,ActionPlan::Pass)){action=initial_action(&c.plan);}
            }
            d.pick_one(&Candidate::Action{action})?
        }
        (Reason::CommanderReturn{..},_)=>crate::policy::flag(d,Flag::CommandZone)?,
        (Reason::Mulligan{..},_)=>crate::policy::flag(d,Flag::Keep)?,
        (_,Shape::Pay{cost,pool,step:PayStep::Spend})=>crate::mana_spend::spend_answer(cost,&pool.unrestricted).ok_or("cannot spend rollout cost")?,
        (_,Shape::Pay{cost,pool,step:PayStep::Produce{..}})=>{
            if search_candidates::spend(cost,&pool.unrestricted).is_none() && let Some((sources,_))=search_candidates::funding(catalog,observation,cost) && let Some(object)=sources.first() {
                crate::policy::payment_source(d,object.clone())?
            }else{Answer::Pay{pay:PayAnswer::Finish{undo_on_failure:true}}}
        }
        (_,Shape::Order{items,min,..})=>Answer::Order{order:items.iter().take(*min as usize).copied().collect()},
        (_,Shape::Distribute{total,buckets,gates})=>crate::policy::damage_assignment(total,buckets,gates),
        (Reason::Targets{..},Shape::Pick{slots,rules})=>{
            let mut picks=Vec::<Vec<u32>>::new();
            for (slot_index,slot) in slots.iter().enumerate(){
                let legal=slot.allowed.iter().copied().filter(|i| !rules.iter().any(|r|matches!(r,mtg_core::JointRule::Distinct{slots} if slots.contains(&(slot_index as u32)) && slots.iter().any(|s|picks.get(*s as usize).is_some_and(|p|p.contains(i))))));
                let i=legal.clone().find(|i| match &d.candidates[*i as usize]{
                    Candidate::Player{player}=>(*player==view.viewer)==beneficial,
                    Candidate::Object{object}=>view.objects.iter().any(|o|o.handle==*object && (o.controller==view.viewer)==beneficial),
                    _=>false
                }).or_else(||legal.into_iter().next()).ok_or("empty rollout target domain")?;
                picks.push(vec![i]);
            }
            Answer::Pick{picks}
        }
        (Reason::Attackers,Shape::Pick{slots,..})=>Answer::Pick{picks:slots.iter().map(|s|{
            let attack=s.subject.is_some_and(|i|matches!(&d.candidates[i as usize],Candidate::Object{object} if view.objects.iter().any(|o|o.handle==*object && o.characteristics.as_ref().and_then(|c|c.power.as_ref()).is_some_and(|p|p>&Quantity::zero()))));
            s.allowed.iter().copied().min_by_key(|i|match d.candidates[*i as usize]{Candidate::Player{player}=>(player.0+4-view.viewer.0)%4,_=>u8::MAX}).filter(|_|attack).into_iter().collect()
        }).collect()},
        (Reason::Discard|Reason::ResolveDiscard|Reason::LegendRule{..},Shape::Pick{slots,..})=>Answer::Pick{picks:slots.iter().map(|s|s.allowed.iter().rev().take(s.min as usize).copied().collect()).collect()},
        (Reason::DamageEffect{..}|Reason::Sacrifice|Reason::Blockers,Shape::Pick{slots,..})=>Answer::Pick{picks:slots.iter().map(|s|s.allowed.iter().take(s.min as usize).copied().collect()).collect()},
        _=>return Err("outside foundations rollout policy".into())
    })
}
pub fn target_answer(targets: &TargetPlan, d: &Decision<Viewer>) -> Result<Answer, String> {
    let offered: Vec<_> = match targets {
        TargetPlan::None => vec![],
        TargetPlan::Objects { objects } => objects
            .iter()
            .cloned()
            .map(|object| Candidate::Object { object })
            .collect(),
        TargetPlan::Players { players } => players
            .iter()
            .copied()
            .map(|player| Candidate::Player { player })
            .collect(),
        TargetPlan::Entities { entities } => entities
            .iter()
            .map(|e| match e {
                PlayerTargetEntity::Player { player } => Candidate::Player { player: *player },
                PlayerTargetEntity::Object { object } => Candidate::Object {
                    object: object.clone(),
                },
            })
            .collect(),
    };
    let answer = Answer::Pick {
        picks: offered
            .iter()
            .map(|row| {
                d.index(row)
                    .map(|i| vec![i])
                    .ok_or("target outside decision")
            })
            .collect::<Result<_, _>>()?,
    };
    d.shape.check(&answer)?;
    Ok(answer)
}
