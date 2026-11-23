//! Complete frozen d705 witnesses and explicitly synthetic compiler AST controls.
//! All tests added for this increment are native UNRUN; the parent owns the
//! single combined contracts/compiler/runtime/schema pass.
use super::*;
use serde_json::{Value, json};

fn empty_program() -> SpellProgram {
    SpellProgram {
        optional_additional_costs: vec![],
        additional_sacrifice: None,
        timing: SpellTiming::Instant,
        resolution: SpellResolution::Instructions,
        functioning_zone: FunctioningZone::Stack,
        targets: vec![],
        instructions: vec![],
        modes: vec![],
    }
}

#[test]
fn controlled_temporary_keyword_other_creature_operators_keep_strict_scope() {
    // Direct compiler decoder probes: no global Target.filter broadening for
    // counter placement/removal/Ramos callers of bind_creature_target.
    for control in [json!("you"), Value::Null, json!("opponent")] {
        let target = json!({"ref":"target","filter":{"types":["creature"],"control":control},"count":{"amount":"fixed","value":1},"selectionId":"line:0:0"});
        assert!(serde_json::from_value::<Target>(target).is_err());
    }
    // Valid typed plain Target still binds exactly the old domain.
    let target: Target = serde_json::from_value(json!({"ref":"target","filter":{"types":["creature"]},"count":{"amount":"fixed","value":1},"selectionId":"line:0:0"})).unwrap();
    let mut p = empty_program();
    bind_creature_target(&mut p, &mut BTreeMap::new(), &target).unwrap();
    assert_eq!(p.targets[0].domain, TargetDomain::BattlefieldCreature);
}
