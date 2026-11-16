//! Complete fixed controller scry and the closed artifact-or-creature selector.
//! CR 109.5, 601.2c, 608.2c, 701.22a-d.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

pub(super) fn validate_source_shape(raw: &str) -> Result<(), Failure> {
    let abilities: serde_json::Value = decode(raw, "abilities")?;
    for ability in abilities
        .as_array()
        .ok_or_else(|| unsupported("scry", "ordinary source required"))?
    {
        if ability["kind"] != "spell" {
            return Err(unsupported("scry", "ordinary source required"));
        }
        for sentence in ability["effects"]
            .as_array()
            .ok_or_else(|| unsupported("scry", "missing complete effects"))?
        {
            if sentence.as_object().is_none_or(|s| {
                s.keys()
                    .any(|k| !["bindings", "steps"].contains(&k.as_str()))
            }) {
                return Err(unsupported(
                    "scry",
                    "conditional or qualified sentence unsupported",
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn instruction(
    amount: &serde_json::Value,
    source_block: u32,
) -> Result<Instruction, Failure> {
    let count: Count = serde_json::from_value(amount.clone())
        .map_err(|_| unsupported("scry", "requires complete fixed nonnegative amount"))?;
    if count.amount != "fixed" {
        return Err(unsupported("scry", "requires fixed nonnegative amount"));
    }
    Ok(Instruction::Scry {
        amount: Quantity::from(i64::from(count.value)),
        source_block,
    })
}

pub(super) fn surveil_instruction(
    amount: &serde_json::Value,
    source_block: u32,
) -> Result<Instruction, Failure> {
    let Instruction::Scry {
        amount,
        source_block,
    } = instruction(amount, source_block)?
    else {
        unreachable!()
    };
    Ok(Instruction::Surveil {
        amount,
        source_block,
    })
}

pub(super) fn artifact_creature_target(what: &serde_json::Value) -> bool {
    lower_permanent_filter(&what["filter"]).is_ok_and(|f| {
        f.types.len() == 2
            && f.types.contains(&CardKind::Creature)
            && f.types.contains(&CardKind::Artifact)
            && f.non_types.is_empty()
            && f.non_colors.is_empty()
            && !f.opponent
            && !f.tapped
    })
}

pub(super) fn bind_grant_target(
    p: &mut SpellProgram,
    slots: &mut BTreeMap<String, u32>,
    what: &serde_json::Value,
) -> Result<u32, Failure> {
    if !artifact_creature_target(what) {
        return Err(unsupported(
            "targets",
            "requires unqualified artifact-or-creature selector",
        ));
    }
    let slot = bind_permanent_target(p, slots, what)?;
    // Alternatives are commutative. Canonicalize ONLY this new closed family;
    // preserve every historical permanent-removal filter's serialized order.
    p.targets[slot as usize]
        .filter
        .as_mut()
        .ok_or_else(|| unsupported("targets", "missing permanent filter"))?
        .types
        .sort();
    Ok(slot)
}

pub(super) fn validate_bindings(
    binding: &serde_json::Value,
    identity: &str,
    raw: &str,
) -> Result<(), Failure> {
    let fail = || {
        unsupported(
            "binding",
            "requires complete controller-scry binding closure",
        )
    };
    let abilities: serde_json::Value = decode(raw, "abilities")?;
    let mut expected = vec![];
    let nodes = binding["nodes"].as_array().ok_or_else(fail)?;
    for (block, ability) in abilities.as_array().ok_or_else(fail)?.iter().enumerate() {
        if ability["kind"] != "spell" {
            return Err(fail());
        }
        let scope = format!("card:{identity}/faces/0/abilities/{block}#ability");
        let you = format!("card:{identity}/faces/0/abilities/{block}#you");
        if !nodes.iter().any(|n| {
            n["id"] == you
                && n["node"] == "runtime-player"
                && n["scope"] == scope
                && n["role"] == "spell-controller"
        }) {
            return Err(fail());
        }
        for (sentence, effect) in ability["effects"]
            .as_array()
            .ok_or_else(fail)?
            .iter()
            .enumerate()
        {
            for (step, value) in effect["steps"]
                .as_array()
                .ok_or_else(fail)?
                .iter()
                .enumerate()
            {
                if value["effect"] == "draw" {
                    if value["who"] != serde_json::json!({"player":"you"}) {
                        return Err(fail());
                    }
                    expected.push((
                        format!("/faces/0/abilities/{block}/effects/{sentence}/steps/{step}/who"),
                        value["who"].clone(),
                        you.clone(),
                    ));
                }
            }
        }
    }
    let refs = binding["references"].as_array().ok_or_else(fail)?;
    if binding["identity"] != identity
        || binding["fullyBound"] != true
        || binding["referencesBound"] != true
        || binding["diagnostics"] != serde_json::json!([])
        || binding["counts"]
            != serde_json::json!({"obligations":expected.len(),"bound":expected.len(),"unresolved":0})
        || refs.len() != expected.len()
        || expected.iter().any(|(path, expression, target)| {
            let matches: Vec<_> = refs.iter().filter(|r| r["path"] == *path).collect();
            matches.len() != 1
                || matches[0]["expression"] != *expression
                || matches[0]["target"] != *target
                || matches[0]["binding"] != "bound"
        })
    {
        return Err(fail());
    }
    Ok(())
}
