//! A target fact is read from the announced incarnation, with LKI on departure.
//! CR 400.7j, 608.2b/c/h/k. https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::{Value, json};

// The original conformance catalogs retain a different binder generation.
const LEGACY_PARSER: &str = "98980a967a70d57008847ce0952edf0cc277af9ad26b8cb8739a555d97082b8d";
const ROOT: &str = "/faces/0/abilities/0";
const PRODUCER: &str = "/faces/0/abilities/0/effects/0/steps/0";
const READER: &str = "/faces/0/abilities/0/effects/1/steps/0";

pub(super) fn lower(
    source: &Value,
    result: &ParseResult,
    binding: &Value,
    identity: &str,
    kind: CardKind,
    parser_sha256: &str,
) -> Result<Option<SpellProgram>, Failure> {
    // Older archives encode these references through a zone successor (or
    // retain unresolved diagnostics). The existing removal adapter owns that
    // historical provenance. This path consumes only fully-bound target-slot
    // antecedents from the current binder.
    if parser_sha256 == LEGACY_PARSER {
        return Ok(None);
    }
    let text = source["oracle_text"].as_str().unwrap_or_default();
    let Some((lead, tail)) = text.strip_suffix('.').and_then(|t| t.split_once(". ")) else {
        return Ok(None);
    };
    let (verb, noun) = match lead {
        "Destroy target creature" => ("destroy", "creature"),
        "Destroy target land" => ("destroy", "land"),
        "Exile target creature" => ("exile", "creature"),
        _ => return Ok(None),
    };
    let (no_regen, printed) = tail
        .strip_prefix("It can't be regenerated. ")
        .map_or((false, tail), |t| (true, t));
    let prefix = format!("That {noun}'s controller ");
    let self_damage = format!("{} deals ", source["name"].as_str().unwrap_or_default());
    let (of, action, damage) = if let Some(a) = printed.strip_prefix("Its controller ") {
        (json!({"ref":"it"}), a, false)
    } else if let Some(a) = printed.strip_prefix(&prefix) {
        (json!({"ref":"that","filter":{"types":[noun]}}), a, false)
    } else if let Some(a) = printed
        .strip_prefix(&self_damage)
        .and_then(|s| s.strip_suffix(&format!(" damage to that {noun}'s controller")))
    {
        (json!({"ref":"that","filter":{"types":[noun]}}), a, true)
    } else {
        return Ok(None);
    };
    // Existing complete sources outside this bounded rider grammar retain their
    // old route. An understood rider must consume its full AST and graph.
    if !(damage
        || action.starts_with("creates a ")
        || action == "gains life equal to its power"
        || action.starts_with("loses ") && action.ends_with(" life"))
    {
        return Ok(None);
    }
    let fail = || {
        unsupported(
            "target facts",
            "requires a complete mandatory movement and exact target fact rider",
        )
    };
    let timing = match kind {
        CardKind::Instant => SpellTiming::Instant,
        CardKind::Sorcery => SpellTiming::MainPhase,
        _ => return Err(fail()),
    };
    let raw: Value = decode(result.abilities.get(), "abilities")?;
    let producer = &raw[0]["effects"][0]["steps"][0];
    let reader = &raw[0]["effects"][1]["steps"][0];
    let selection = producer["what"]["selectionId"].as_str().ok_or_else(fail)?;
    let what = json!({"ref":"target","filter":{"types":[noun]},"count":{"amount":"fixed","value":1},"selectionId":selection});
    let mut expected = json!({"effect":verb,"what":what});
    if no_regen {
        if verb != "destroy" {
            return Err(fail());
        }
        expected["regenerationForbidden"] = json!(true);
    }
    if *producer != expected
        || raw != json!([{"kind":"spell","effects":[{"steps":[producer]},{"steps":[reader]}]}])
    {
        return Err(fail());
    }
    let mut p = SpellProgram {
        timing,
        functioning_zone: FunctioningZone::Stack,
        resolution: SpellResolution::Instructions,
        optional_additional_costs: vec![],
        additional_sacrifice: None,
        targets: vec![],
        instructions: vec![],
        modes: vec![],
    };
    let slot = bind_permanent_target(&mut p, &mut BTreeMap::new(), &what)?;
    p.instructions.push(match verb {
        "destroy" => Instruction::destroy_target(slot, no_regen, 0),
        "exile" => Instruction::move_target(slot, kir::Destination::Exile, 0),
        _ => return Err(fail()),
    });
    let mut graph = bindings::Bindings::new(identity, source);
    graph.ability(ROOT, "spell", "spell-controller");
    let scope = graph.id(ROOT, "ability");
    let target = graph.target(&scope, &format!("{PRODUCER}/what"), &what, selection);
    graph.event(ROOT, PRODUCER, verb, &target, false);
    if verb == "exile" {
        graph.node(PRODUCER, "objects", "event-result", json!({
            "event":graph.id(PRODUCER,"event"),"result":"objects","cardinality":"one",
            "availability":"if-produced","identity":"zone-successors","description":{"types":[noun],"cls":"card"},"rules":["400.7j"]
        }));
    }
    let who = json!({"player":"controller","of":of});
    let path = if damage {
        format!("{READER}/to/ref")
    } else {
        format!("{READER}/who")
    };
    let query = graph.node(
        &path,
        "query",
        "selection",
        json!({
            "scope":scope,"expression":who,"cardinality":"one","timing":"resolution",
            "selection":"evaluate-reference","rules":["608.2h"]
        }),
    );
    let reference = |graph: &mut bindings::Bindings<'_>, path: &str, expression: &Value| {
        graph.reference_rules(path, expression, &target, &["608.2k", "400.7j", "608.2h"]);
    };
    reference(&mut graph, &format!("{path}/of"), &of);
    graph.reference(&path, &who, &query, "608.2h");
    let recipient = EffectRecipient::TargetController { target_slot: slot };
    let i = if damage {
        let (amount, number) = printed_number(action).ok_or_else(fail)?;
        if *reader
            != json!({"effect":"damage","source":{"ref":"self"},"amount":number,"to":{"to":"player","ref":who}})
        {
            return Err(fail());
        }
        let own_source = graph.id("/source", "object");
        graph.reference(
            &format!("{READER}/source"),
            &json!({"ref":"self"}),
            &own_source,
            "201.5",
        );
        graph.node(READER, "event", "event", json!({
            "scope":scope,"operation":"damage","inputs":[own_source,query],"availability":"when-performed","rules":["608.2c"]
        }));
        Instruction::DamageTargetController {
            target_slot: slot,
            amount,
            source_block: 0,
        }
    } else if let Some(printed) = action.strip_prefix("creates a ") {
        let token: SourceToken = decode(&reader["token"].to_string(), "effects.token")?;
        if *reader
            != json!({"effect":"create-token","count":{"amount":"fixed","value":1},"who":who,"token":reader["token"]})
            || token_printed(&token).as_deref() != Some(printed)
        {
            return Err(fail());
        }
        graph.event(ROOT, READER, "create-token", &query, false);
        graph.result(
            READER,
            "objects",
            "new-objects",
            Some(&json!({"cls":"token","types":token.types})),
        );
        Instruction::CreateTokensFor {
            recipient,
            count: 1,
            token: lower_creature_token(&token)?,
            source_block: 0,
        }
    } else if action == "gains life equal to its power" {
        let it = json!({"ref":"it"});
        if noun != "creature"
            || *reader
                != json!({"effect":"gain-life","who":who,"amount":{"amount":"attribute","of":it,"attribute":"power"}})
        {
            return Err(fail());
        }
        reference(&mut graph, &format!("{READER}/amount/of"), &it);
        graph.event(ROOT, READER, "gain-life", &query, false);
        Instruction::ChangeLifeFromTarget {
            recipient,
            amount: TargetAmount::Attribute {
                target_slot: slot,
                attribute: TargetAttribute::Power,
            },
            gain: true,
            source_block: 0,
        }
    } else {
        let text = action
            .strip_prefix("loses ")
            .and_then(|s| s.strip_suffix(" life"))
            .ok_or_else(fail)?;
        let (amount, number) = printed_number(text).ok_or_else(fail)?;
        if *reader != json!({"effect":"lose-life","who":who,"amount":number}) {
            return Err(fail());
        }
        graph.event(ROOT, READER, "lose-life", &query, false);
        Instruction::ChangeLifeFromTarget {
            recipient,
            amount: TargetAmount::Fixed { amount },
            gain: false,
            source_block: 0,
        }
    };
    graph.result(READER, "quantity", "measured-quantity", None);
    graph.validate(
        binding,
        "target fact requires the exact announcement producer and ordered reader graph",
    )?;
    p.instructions.push(i);
    Ok(Some(p))
}

fn printed_number(text: &str) -> Option<(Quantity, Value)> {
    let value = text.parse::<u32>().ok()?;
    (value > 0 && text == value.to_string()).then(|| {
        (
            i64::from(value).into(),
            json!({"amount":"fixed","value":value}),
        )
    })
}

fn token_printed(token: &SourceToken) -> Option<String> {
    if token.types != ["creature"] || !token.supertypes.is_empty() || token.subtypes.is_empty() {
        return None;
    }
    let colors = if token.colors.is_empty() {
        "colorless".into()
    } else {
        token.colors.join(" and ")
    };
    let keyword = match token.keywords.as_slice() {
        [] => String::new(),
        [keyword] => format!(" with {}", keyword["keyword"].as_str()?),
        _ => return None,
    };
    Some(format!(
        "{}/{} {colors} {} creature token{keyword}",
        token.power.value,
        token.toughness.value,
        token.subtypes.join(" ")
    ))
}
