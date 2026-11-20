//! Closed source binding for an ordinary instruction sequence. The shared
//! contract adapter independently verifies the printed clauses and IR.
use super::*;
use serde_json::{Value, json};

const ROOT: &str = "/faces/0/abilities/0";

struct BindingEnv<'a> {
    graph: bindings::Bindings<'a>,
    scope: String,
    controller: String,
    target: String,
}

impl<'a> BindingEnv<'a> {
    fn new(identity: &'a str, source: &Value) -> Self {
        let mut graph = bindings::Bindings::new(identity, source);
        graph.ability(ROOT, "spell", "spell-controller");
        Self {
            scope: graph.id(ROOT, "ability"),
            controller: graph.id(ROOT, "you"),
            graph,
            target: String::new(),
        }
    }

    fn bind_controller(&mut self, path: &str) {
        self.graph
            .reference(path, &json!({"player":"you"}), &self.controller, "109.5");
    }

    fn bind_counter(&mut self, path: &str, on: &Value, target: &TargetSpec) {
        self.bind_controller(&format!("{path}/who"));
        if target.domain == TargetDomain::ControlledBattlefieldCreature {
            self.bind_controller(&format!("{path}/on/filter/control"));
        }
        self.target =
            self.graph
                .target(&self.scope, &format!("{path}/on"), on, &target.selection_id);
        self.graph
            .event(ROOT, path, "put-counters", &self.target, false);
        self.graph
            .result(path, "quantity", "measured-quantity", None);
    }

    fn bind_same_target(&mut self, path: &str, step: &Value) {
        self.graph.reference_rules(
            &format!("{path}/what"),
            &step["what"],
            &self.target,
            &["608.2k", "400.7j", "608.2h"],
        );
    }

    fn bind_grant(&mut self, path: &str, keywords: &[KeywordAbility]) -> Result<(), Failure> {
        for (index, keyword) in keywords.iter().enumerate() {
            // Archive keyword scope uses the rules reference from
            // oracle-parser/src/semantic/keyword-scopes.ts.
            let rule = match keyword {
                KeywordAbility::Deathtouch => "702.2",
                KeywordAbility::DoubleStrike => "702.4",
                KeywordAbility::FirstStrike => "702.7",
                KeywordAbility::Flying => "702.9",
                KeywordAbility::Haste => "702.10",
                KeywordAbility::Hexproof => "702.11",
                KeywordAbility::Indestructible => "702.12",
                KeywordAbility::Lifelink => "702.15",
                KeywordAbility::Reach => "702.17",
                KeywordAbility::Trample => "702.19",
                KeywordAbility::Vigilance => "702.20",
                _ => return Err(unsupported("binding", "unsupported granted keyword scope")),
            };
            let ability = format!("{path}/abilities/{index}");
            let recipient = self.graph.node(
                &ability,
                "recipient",
                "recipient",
                json!({
                    "scope":self.scope,"of":self.target,"each":false,"rules":["113.10","201.5a"]
                }),
            );
            let scope = self.graph.node(&ability, "ability", "ability", json!({
                "source":recipient,"face":self.graph.id("/faces/0","face"),"abilityKind":"keywords",
                "you":self.graph.id(&ability,"you"),"rules":["113","113.10"]
            }));
            self.graph.node(&ability, "you", "runtime-player", json!({
                "scope":scope,"role":"static-source-controller","rules":["109.5","301.5d","303.4e"]
            }));
            let keyword_path = format!("{ability}/keyword");
            let you = self.graph.node(
                &keyword_path,
                "you",
                "runtime-player",
                json!({
                    "scope":scope,"role":"static-source-controller","rules":["109.5",rule]
                }),
            );
            self.graph.node(
                &keyword_path,
                "keyword",
                "keyword-ability",
                json!({
                    "scope":scope,"keyword":keyword,"abilities":["static"],"you":you,"rules":[rule]
                }),
            );
        }
        self.graph
            .event(ROOT, path, "gain-abilities", &self.target, false);
        Ok(())
    }
}

pub(super) fn lower(
    source: &Value,
    result: &ParseResult,
    binding: &Value,
    identity: &str,
    kind: CardKind,
) -> Result<Option<SpellProgram>, Failure> {
    use mtg_contracts::composed_spell::{candidate, may_have_counter, printed_candidate};

    let raw = result.abilities.get();
    let text = source["oracle_text"].as_str().unwrap_or_default();
    let printed = printed_candidate(text);
    if !printed && !may_have_counter(raw) {
        return Ok(None);
    }
    let ast: Value = decode(raw, "abilities")?;
    if !printed && !candidate(&ast) {
        return Ok(None);
    }
    let fail = || {
        unsupported(
            "ordinary sequence",
            "requires complete ordinary printed clauses",
        )
    };
    let timing = match kind {
        CardKind::Instant => SpellTiming::Instant,
        CardKind::Sorcery => SpellTiming::MainPhase,
        _ => return Err(fail()),
    };
    let program =
        mtg_contracts::composed_spell::archived_program(text, &ast, timing).ok_or_else(fail)?;

    let mut env = BindingEnv::new(identity, source);
    let effects = ast[0]["effects"].as_array().ok_or_else(fail)?;
    for (index, (effect, instruction)) in effects.iter().zip(&program.instructions).enumerate() {
        let path = format!("{ROOT}/effects/{index}/steps/0");
        let step = &effect["steps"][0];
        match instruction {
            Instruction::PutPlusOneCounters { .. } => {
                env.bind_counter(&path, &step["on"], &program.targets[0]);
            }
            Instruction::GrantKeywords { keywords, .. } => {
                env.bind_same_target(&path, step);
                env.bind_grant(&path, keywords)?;
            }
            Instruction::PhaseOutTargetPermanent { .. } => {
                env.bind_same_target(&path, step);
                env.graph
                    .event(ROOT, &path, "phase-out", &env.target, false);
            }
            Instruction::GainLife { .. } => {
                env.bind_controller(&format!("{path}/who"));
                env.graph
                    .event(ROOT, &path, "gain-life", &env.controller, false);
                env.graph
                    .result(&path, "quantity", "measured-quantity", None);
            }
            _ => return Err(fail()),
        }
    }
    env.graph.validate(
        binding,
        "ordinary sequence binding graph differs from its complete operands",
    )?;
    Ok(Some(program))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    const GIFT: &str =
        include_str!("../../../tests/fixtures/2cb95721-56c8-4008-8055-579a5a97fc8b.json");
    const SHIELD: &str =
        include_str!("../../../tests/fixtures/e8d0eafe-540a-4b2e-a989-e98ff3c31105.json");
    const SLIP: &str =
        include_str!("../../../tests/fixtures/82506e51-3fe1-446f-83c9-69e67583cefc.json");
    const PROMOTION: &str =
        include_str!("../../../tests/fixtures/303d74d0-84db-4694-bb1a-470cf470bc4f.json");
    const FEAT: &str =
        include_str!("../../../tests/fixtures/2038f505-7b03-4c63-bc44-c5139729dd05.json");

    fn compiled(raw: &str) -> Definition {
        match compile(MANIFEST, raw) {
            CompileResult::Lowered { definition } => *definition,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn composed_spell_exact_sources_share_a_slot_and_preserve_cost_and_order() {
        use KeywordAbility::*;
        for (raw, domain, color, generic, keywords, phase, life) in [
            (
                GIFT,
                TargetDomain::ControlledBattlefieldCreature,
                Color::G,
                1,
                &[Reach, Trample, Hexproof, Indestructible][..],
                false,
                false,
            ),
            (
                SHIELD,
                TargetDomain::BattlefieldCreature,
                Color::W,
                1,
                &[Lifelink, Indestructible][..],
                false,
                false,
            ),
            (
                SLIP,
                TargetDomain::BattlefieldCreature,
                Color::U,
                0,
                &[][..],
                true,
                false,
            ),
            (
                PROMOTION,
                TargetDomain::BattlefieldCreature,
                Color::W,
                1,
                &[FirstStrike][..],
                false,
                true,
            ),
        ] {
            let d = compiled(raw);
            assert_eq!(
                d.characteristics.mana_cost,
                ManaCost {
                    generic: generic.into(),
                    colored: vec![color]
                }
            );
            assert_eq!(d.capabilities, d.requirements());
            assert!(
                d.capabilities
                    .contains(&Capability::ComposedOrdinarySpellV1)
            );
            assert!(d.capabilities.contains(&Capability::PlusOneCountersV1));
            assert!(
                !d.capabilities
                    .contains(&Capability::TemporaryCreatureKeywordProgramV1)
            );
            let p = d.spell.unwrap();
            assert_eq!(p.targets.len(), 1);
            assert_eq!(p.targets[0].domain, domain);
            let mut expected = vec![Instruction::PutPlusOneCounters {
                on: CounterRecipient::Target { target_slot: 0 },
                amount: 1.into(),
                source_block: 0,
            }];
            if phase {
                expected.push(Instruction::PhaseOutTargetPermanent {
                    target_slot: 0,
                    source_block: 0,
                });
            } else {
                expected.push(Instruction::GrantKeywords {
                    target_slot: 0,
                    modifier: None,
                    keywords: keywords.to_vec(),
                    duration: Duration::UntilEndOfTurn,
                    source_block: 0,
                });
            }
            if life {
                expected.push(Instruction::GainLife {
                    recipient: LifeRecipient::Controller,
                    amount: 2.into(),
                    source_block: 0,
                });
            }
            assert_eq!(p.instructions, expected);
        }
    }

    #[test]
    fn composed_spell_rejects_unconsumed_clauses_nulls_and_forged_references() {
        assert!(!matches!(
            compile(MANIFEST, FEAT),
            CompileResult::Lowered { .. }
        ));
        let mutate = |change: fn(&mut Value)| super::super::tests::changed_source(GIFT, change);
        let extra = mutate(|r| {
            let source = &mut r["source"]["oracle_text"];
            *source = json!(
                source
                    .as_str()
                    .unwrap()
                    .replace("until end of turn.", "until end of turn. Draw a card.")
            );
        });
        let null = mutate(|r| {
            let result = &mut r["faces"][0]["result"];
            result["abilities"][0]["effects"][1]["steps"][0]["optional"] = Value::Null;
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        let dropped = super::super::tests::changed_source(SHIELD, |r| {
            let result = &mut r["faces"][0]["result"];
            result["abilities"][0]["effects"]
                .as_array_mut()
                .unwrap()
                .pop();
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        let wrong_edge = mutate(|r| {
            let edge = r["binding"]["references"]
                .as_array_mut()
                .unwrap()
                .last_mut()
                .unwrap();
            edge["target"] = json!("wrong");
        });
        for (record, path) in [
            (extra, "ordinary sequence"),
            (null, "ordinary sequence"),
            (dropped, "ordinary sequence"),
            (wrong_edge, "binding"),
        ] {
            assert!(
                matches!(compile(MANIFEST, &record), CompileResult::Unsupported { path: p, .. } if p == path)
            );
        }
    }

    #[test]
    fn composed_spell_ir_rejects_reordered_erased_or_rebound_operands() {
        let d = compiled(GIFT);
        for change in [
            (|p: &mut SpellProgram| p.instructions.swap(0, 1)) as fn(&mut SpellProgram),
            |p| {
                p.instructions.pop();
            },
            |p| {
                p.targets[0].domain = TargetDomain::BattlefieldCreature;
            },
            |p| {
                p.timing = SpellTiming::MainPhase;
            },
        ] {
            let mut bad = d.clone();
            change(bad.spell.as_mut().unwrap());
            bad.capabilities = bad.requirements();
            assert!(bad.validate().is_err());
        }
        let mut missing = d;
        missing
            .capabilities
            .remove(&Capability::ComposedOrdinarySpellV1);
        assert!(missing.validate().is_err());
    }
}
