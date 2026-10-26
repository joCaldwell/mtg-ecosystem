//! Trusted evidence adapter. Never supplies facts to the strategy.
use super::*;
use mtg_contracts::{Definition, SpellProgram};
use mtg_core::{BatchCause, CardId, Event, EventOutcome, EvidenceRecord, StackId};
use std::collections::BTreeSet;

type Counts = BTreeMap<String, BTreeMap<String, u64>>;
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Summary {
    instructions: Counts,
    instruction_sites: Counts,
    triggers: Counts,
    keywords: Counts,
    cards_used: BTreeMap<String, u64>,
}
fn bump(counts: &mut Counts, kind: &str, metric: &str) {
    *counts
        .entry(kind.into())
        .or_default()
        .entry(metric.into())
        .or_default() += 1;
}
fn tag(value: impl Serialize, field: &str) -> Result<String, String> {
    serde_json::to_value(value).map_err(|e| e.to_string())?[field]
        .as_str()
        .map(str::to_owned)
        .ok_or("missing IR tag".into())
}
fn sites(d: &Definition) -> Vec<(String, &SpellProgram)> {
    let mut result = vec![];
    fn add<'a>(out: &mut Vec<(String, &'a SpellProgram)>, path: String, p: &'a SpellProgram) {
        out.push((path.clone(), p));
        for (n, m) in p.modes.iter().enumerate() {
            add(out, format!("{path}/mode/{n}"), &m.program);
        }
    }
    if let Some(p) = &d.spell {
        add(&mut result, "spell".into(), p);
    }
    for (i, a) in d.activated.iter().enumerate() {
        add(&mut result, format!("activated/{i}"), &a.program);
    }
    for (i, a) in d.triggered.iter().enumerate() {
        add(&mut result, format!("triggered/{i}"), &a.program);
    }
    result
}
fn site_key(d: &Definition, path: &str, cursor: usize) -> String {
    format!("{}/{path}/{cursor}", d.oracle_id)
}
#[derive(Clone)]
struct Invocation<'a> {
    definition: &'a Definition,
    path: String,
    program: &'a SpellProgram,
}
impl Invocation<'_> {
    fn selected(&mut self, mode: Option<u32>) {
        if let Some(m) = mode
            && let Some(p) = self.program.modes.get(m as usize)
        {
            self.path = format!("{}/mode/{m}", self.path);
            self.program = &p.program;
        }
    }
    fn chosen(&self, out: &mut Summary) -> Result<(), String> {
        for (cursor, instruction) in self.program.instructions.iter().enumerate() {
            bump(&mut out.instructions, &tag(instruction, "op")?, "chosen");
            bump(
                &mut out.instruction_sites,
                &site_key(self.definition, &self.path, cursor),
                "chosen",
            );
        }
        Ok(())
    }
}
pub(super) fn game(catalog: &Catalog, evidence: &Evidence) -> Result<serde_json::Value, String> {
    let p = Prepared::admit_pod(catalog.clone(), evidence.config.clone())
        .map_err(|e| format!("{e:?}"))?;
    let physical = p.config().cards.clone();
    let g = Game::replay_with_trace(p, &evidence.journal, TraceMode::Detailed)?;
    let lookup = |card: CardId| {
        physical
            .iter()
            .find(|c| c.id == card)
            .and_then(|c| definition(catalog, &c.oracle_id).ok())
    };
    let mut out = Summary::default();
    let mut invocations = BTreeMap::<StackId, Invocation<'_>>::new();
    let mut pending = BTreeMap::new();
    let mut modes = BTreeMap::new();
    let mut executed = BTreeSet::new();
    let mut mana_uses = BTreeSet::new();
    for record in g.trace().records.iter() {
        match &record.record {
            EvidenceRecord::Event { event } => match event.as_ref() {
                Event::SpellProposed { card, stack_id } => {
                    let d = lookup(*card).ok_or("unknown proposed card")?;
                    invocations.insert(
                        *stack_id,
                        Invocation {
                            definition: d,
                            path: "spell".into(),
                            program: d.spell.as_ref().ok_or("spell program")?,
                        },
                    );
                }
                Event::ModeSelected { stack_id, mode, .. } => {
                    modes.insert(*stack_id, *mode);
                }
                Event::SpellCast { stack_id } => {
                    let i = invocations.get_mut(stack_id).ok_or("cast invocation")?;
                    i.selected(modes.get(stack_id).copied());
                    i.chosen(&mut out)?;
                    *out.cards_used
                        .entry(i.definition.oracle_id.clone())
                        .or_default() += 1;
                }
                Event::AbilityActivated {
                    stack_id,
                    source,
                    ability,
                    ..
                } => {
                    let d = lookup(source.card).ok_or("activated card")?;
                    let mut i = Invocation {
                        definition: d,
                        path: format!("activated/{ability}"),
                        program: &d.activated[*ability as usize].program,
                    };
                    i.selected(modes.get(stack_id).copied());
                    i.chosen(&mut out)?;
                    *out.cards_used.entry(d.oracle_id.clone()).or_default() += 1;
                    invocations.insert(*stack_id, i);
                }
                Event::TriggerCollected {
                    trigger_id,
                    source,
                    ability,
                    ..
                } => {
                    let d = lookup(source.card).ok_or("triggered card")?;
                    let a = &d.triggered[*ability as usize];
                    bump(
                        &mut out.triggers,
                        serde_json::to_value(a.condition)
                            .map_err(|e| e.to_string())?
                            .as_str()
                            .ok_or("trigger tag")?,
                        "collected",
                    );
                    pending.insert(
                        *trigger_id,
                        Invocation {
                            definition: d,
                            path: format!("triggered/{ability}"),
                            program: &a.program,
                        },
                    );
                }
                Event::TriggerPlaced {
                    trigger_id,
                    stack_id,
                    ..
                } => {
                    let i = pending.get(trigger_id).ok_or("placed trigger")?.clone();
                    let ability = i
                        .path
                        .rsplit('/')
                        .next()
                        .unwrap()
                        .parse::<usize>()
                        .map_err(|e| e.to_string())?;
                    bump(
                        &mut out.triggers,
                        serde_json::to_value(i.definition.triggered[ability].condition)
                            .map_err(|e| e.to_string())?
                            .as_str()
                            .ok_or("trigger tag")?,
                        "placed",
                    );
                    i.chosen(&mut out)?;
                    invocations.insert(*stack_id, i);
                }
                Event::TriggeredAbilityCompleted {
                    stack_id,
                    resolved: true,
                } => {
                    let i = invocations.get(stack_id).ok_or("resolved trigger")?;
                    let ability = i
                        .path
                        .split('/')
                        .nth(1)
                        .unwrap()
                        .parse::<usize>()
                        .map_err(|e| e.to_string())?;
                    bump(
                        &mut out.triggers,
                        serde_json::to_value(i.definition.triggered[ability].condition)
                            .map_err(|e| e.to_string())?
                            .as_str()
                            .ok_or("trigger tag")?,
                        "resolved",
                    );
                }
                _ => {}
            },
            EvidenceRecord::Batch { batch } => {
                if let BatchCause::Instruction {
                    stack_id, cursor, ..
                } = batch.cause
                    && !batch.occurrences.is_empty()
                    && executed.insert((stack_id, cursor))
                {
                    let i = invocations.get(&stack_id).ok_or("instruction invocation")?;
                    let instruction = i
                        .program
                        .instructions
                        .get(cursor as usize)
                        .ok_or("instruction cursor")?;
                    bump(&mut out.instructions, &tag(instruction, "op")?, "committed");
                    bump(
                        &mut out.instruction_sites,
                        &site_key(i.definition, &i.path, cursor as usize),
                        "committed",
                    );
                }
                let used = match batch.cause {
                    BatchCause::LandPlay { source, .. } => Some(source.card),
                    BatchCause::ManaActivation {
                        source,
                        decision_id,
                        ..
                    } if mana_uses.insert((source, decision_id)) => Some(source.card),
                    _ => None,
                };
                if let Some(d) = used.and_then(lookup) {
                    *out.cards_used.entry(d.oracle_id.clone()).or_default() += 1;
                }
                for o in &batch.occurrences {
                    // Keywords on actual declared participants and damage sources,
                    // including tokens and continuous grants. Counts involvement,
                    // not a claim that each keyword changed the outcome.
                    let (objects, metric) = match &o.actual {
                        EventOutcome::AttackersDeclared { attacks } => (
                            attacks.iter().map(|(o, _)| *o).collect::<BTreeSet<_>>(),
                            "attackers",
                        ),
                        EventOutcome::BlockersDeclared { blocks, .. } => (
                            blocks.iter().flat_map(|(b, a)| [*b, *a]).collect(),
                            "block_participants",
                        ),
                        _ => (BTreeSet::new(), ""),
                    };
                    for object in objects {
                        if let Some(f) = o.before.objects.iter().find(|f| f.object == object) {
                            for k in &f.keywords {
                                bump(
                                    &mut out.keywords,
                                    serde_json::to_value(k)
                                        .map_err(|e| e.to_string())?
                                        .as_str()
                                        .ok_or("keyword tag")?,
                                    metric,
                                );
                            }
                        }
                    }
                    if let EventOutcome::DamageCommitted { actual, .. } = &o.actual {
                        for damage in actual {
                            for k in &damage.source.keywords {
                                bump(
                                    &mut out.keywords,
                                    serde_json::to_value(k)
                                        .map_err(|e| e.to_string())?
                                        .as_str()
                                        .ok_or("keyword tag")?,
                                    "damage_occurrences",
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    serde_json::to_value(out).map_err(|e| e.to_string())
}
pub(super) fn summarize(
    catalog: &Catalog,
    reports: &[serde_json::Value],
) -> Result<serde_json::Value, String> {
    let mut total = Summary::default();
    fn merge(target: &mut Counts, source: Counts) {
        for (kind, metrics) in source {
            for (metric, count) in metrics {
                *target
                    .entry(kind.clone())
                    .or_default()
                    .entry(metric)
                    .or_default() += count;
            }
        }
    }
    for report in reports {
        let s: Summary =
            serde_json::from_value(report["interactions"].clone()).map_err(|e| e.to_string())?;
        merge(&mut total.instructions, s.instructions);
        merge(&mut total.instruction_sites, s.instruction_sites);
        merge(&mut total.triggers, s.triggers);
        merge(&mut total.keywords, s.keywords);
        for (id, count) in s.cards_used {
            *total.cards_used.entry(id).or_default() += count;
        }
    }
    let mut unreached = vec![];
    let mut unused = vec![];
    for d in &catalog.definitions {
        if !total.cards_used.contains_key(&d.oracle_id) {
            unused.push(serde_json::json!({"oracle_id":d.oracle_id,"name":d.name}));
        }
        for (path, p) in sites(d) {
            for (cursor, instruction) in p.instructions.iter().enumerate() {
                let key = site_key(d, &path, cursor);
                let counts = total.instruction_sites.entry(key.clone()).or_default();
                total
                    .instructions
                    .entry(tag(instruction, "op")?)
                    .or_default();
                if counts.get("committed").copied().unwrap_or(0) == 0 {
                    unreached.push(serde_json::json!({"site":key,"name":d.name,"instruction":instruction,"chosen":counts.get("chosen").copied().unwrap_or(0)}));
                }
            }
        }
        for t in &d.triggered {
            total
                .triggers
                .entry(
                    serde_json::to_value(t.condition)
                        .map_err(|e| e.to_string())?
                        .as_str()
                        .ok_or("trigger tag")?
                        .into(),
                )
                .or_default();
        }
        for k in &d.keywords {
            total
                .keywords
                .entry(
                    serde_json::to_value(k)
                        .map_err(|e| e.to_string())?
                        .as_str()
                        .ok_or("keyword tag")?
                        .into(),
                )
                .or_default();
        }
    }
    Ok(
        serde_json::json!({"counts":total,"unreached_instruction_sites":unreached,"unused_catalog_cards":unused,"units":{"chosen":"instructions in a selected mode of a cast spell, activated ability or placed trigger; does not imply execution","committed":"distinct (stack_id,cursor) with a nonempty committed instruction batch; multi-target and resumable batches count once; no-op, declined and fizzled effects may have no batch","keywords":"keyword on a declared combat participant or actual damage source, including grants and tokens; involvement, not proof the keyword changed a result","cards_used":"spell cast, nonmana activation, land play or mana activation; Oracle identities aggregated across seats"}}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_mode_counts_only_its_instructions_and_never_claims_a_commit() {
        let c = mtg_compiler::catalog::embedded_catalog("wipes").unwrap();
        let d = c
            .definitions
            .iter()
            .find(|d| d.name == "Healing Salve")
            .unwrap();
        let mut invocation = Invocation {
            definition: d,
            path: "spell".into(),
            program: d.spell.as_ref().unwrap(),
        };
        invocation.selected(Some(1));
        let mut counts = Summary::default();
        invocation.chosen(&mut counts).unwrap();
        assert_eq!(counts.instructions.len(), 1);
        assert_eq!(counts.instructions["prevent_next_damage"]["chosen"], 1);
        let reports = vec![serde_json::json!({"interactions":counts})];
        let summary = summarize(&c, &reports).unwrap();
        let site = site_key(d, "spell/mode/1", 0);
        assert!(
            summary["unreached_instruction_sites"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s["site"] == site && s["chosen"] == 1)
        );
        assert!(
            summary["counts"]["instructions"]["prevent_next_damage"]
                .get("committed")
                .is_none()
        );
    }
}
