//! CR 109.5, 614.12, 800.4a and 806.1 (2026-09-25): the prospective
//! controller's opponents are other players still in this free-for-all game.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

const ENTRY: &str = "/faces/0/abilities/0";
const BAD: &str = "requires a complete opponent-count entry and tap-only dual mana source";

pub(super) fn lower(effect: &SourceStaticEffect) -> Result<Option<StaticAbility>, Failure> {
    let SourceStaticEffect::EntersTapped {
        what,
        condition_kind,
        condition: Some(condition),
    } = effect
    else {
        return Ok(None);
    };
    if condition["condition"] != "opponent-count" {
        return Ok(None);
    }
    let bad = || unsupported("static.entry", BAD);
    let minimum = condition["comparison"]["value"]["value"]
        .as_i64()
        .filter(|n| *n > 0)
        .ok_or_else(bad)?;
    if *what != json!({"ref":"this","noun":"land"})
        || condition_kind.as_ref() != Some(&json!("unless"))
        || *condition != predicate(minimum)
    {
        return Err(bad());
    }
    Ok(Some(StaticAbility {
        functioning_zone: FunctioningZone::Battlefield,
        effect: StaticEffect::EntersTappedUnlessOpponentCount {
            minimum: minimum.into(),
        },
    }))
}

fn predicate(minimum: i64) -> Value {
    json!({"condition":"opponent-count","who":{"player":"you"},
        "comparison":{"op":"ge","value":{"amount":"fixed","value":minimum}}})
}

pub(super) fn bind(
    abilities: &[Ability],
    raw: &str,
    source: &Value,
    binding: &Value,
    identity: &str,
) -> Result<Option<ManaAbility>, Failure> {
    if !abilities.iter().any(|ability| {
        matches!(ability,
        Ability::Static { effect: SourceStaticEffect::EntersTapped {condition:Some(c), ..}, .. }
            if c["condition"] == "opponent-count")
    }) {
        return Ok(None);
    }
    let bad = || unsupported("binding.entry", BAD);
    let [
        Ability::Static { effect, .. },
        Ability::Activated { effects, .. },
    ] = abilities
    else {
        return Err(bad());
    };
    let Some(StaticAbility {
        effect: StaticEffect::EntersTappedUnlessOpponentCount { minimum },
        ..
    }) = lower(effect)?
    else {
        return Err(bad());
    };
    let [sentence] = effects.as_slice() else {
        return Err(bad());
    };
    let [
        Step::AddMana {
            spend_restriction: None,
            mana: SourceMana::Choice { options },
            ..
        },
    ] = sentence.steps.as_slice()
    else {
        return Err(bad());
    };
    let colors = listed_mana::colors(options).ok_or_else(bad)?;
    let [first, second] = colors.as_slice() else {
        return Err(bad());
    };
    let minimum = minimum.to_string().parse().map_err(|_| bad())?;
    let ast = json!([
        {"kind":"static","effect":{"static":"enters-tapped","what":{"ref":"this","noun":"land"},
            "conditionKind":"unless","condition":predicate(minimum)}},
        {"kind":"activated","costs":[{"cost":"tap-self"}],"effects":[{"steps":[{
            "effect":"add-mana","who":{"player":"you"},"mana":{"mana":"choice","options":[
                {"mana":"fixed","symbols":[first]},{"mana":"fixed","symbols":[second]}
            ]}
        }]}]}
    ]);
    if decode::<Value>(raw, "abilities")? != ast {
        return Err(bad());
    }

    let root = format!("{ENTRY}/effect");
    let mut graph = bindings::Bindings::new(identity, source);
    graph.ability(ENTRY, "static", "static-source-controller");
    let controller = graph.node(&root, "entering-controller", "runtime-player", json!({
        "scope":graph.id(ENTRY,"ability"),"role":"entering-controller","rules":["110.2a","614.12"]
    }));
    graph.reference(
        &format!("{root}/what"),
        &json!({"ref":"this","noun":"land"}),
        &graph.id("/source", "object"),
        "201.5",
    );
    graph.reference(
        &format!("{root}/condition/who"),
        &json!({"player":"you"}),
        &controller,
        "109.5",
    );
    listed_mana::bind_tap_mana(&mut graph, "/faces/0/abilities/1");
    // Archive v1 binds each explicit actor and reports one unresolved
    // quantifier scope. The exact predicate above supplies all its semantics
    // for the finite free-for-all engine. Reject any other diagnostic or gap.
    let expected = graph.with_scope_diagnostic(json!({
        "code":"unsupported-scope","path":format!("{root}/condition"),
        "message":"Game and player designations, turn ownership and per-player comparisons need the participating player set and its quantifier bound.",
        "rules":["102.1","102.2","102.3","608.2h"]
    }));
    if *binding != expected {
        return Err(bad());
    }
    Ok(Some(ManaAbility {
        costs: PermanentActivationCosts {
            tap: true,
            sacrifice: false,
            mana: None,
            remove_source_plus_one_counters: None,
        },
        production: ManaProduction::Choice { colors },
        self_damage: None,
        restriction: None,
        source_block: 1,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = include_str!("../../../tests/fixtures/archive-manifest-current.json");
    const SOURCE: &str =
        include_str!("../../../tests/fixtures/e3570ac7-c593-40e3-bbd6-ec3da6d8158d.json");

    #[test]
    fn opponent_count_entry_binds_exact_scope_and_controller() {
        let result = compile(MANIFEST, SOURCE);
        let CompileResult::Lowered { definition } = result else {
            panic!("{result:?}")
        };
        assert!(
            definition
                .capabilities
                .contains(&Capability::ConditionalOpponentCountEntryV1)
        );
        assert_eq!(
            definition.static_abilities[0].effect,
            StaticEffect::EntersTappedUnlessOpponentCount { minimum: 2.into() }
        );

        let changed = crate::tests::changed_source(SOURCE, |r| {
            r["binding"]["diagnostics"][0]["path"] = json!("/faces/0/abilities/0");
        });
        assert!(
            matches!(compile(MANIFEST, &changed), CompileResult::Unsupported { path, .. }
            if path == "binding.entry")
        );

        let mut definition = *definition;
        definition.static_abilities[0].effect =
            StaticEffect::EntersTappedUnlessOpponentCount { minimum: 1.into() };
        assert!(definition.validate().is_err());
    }
}
