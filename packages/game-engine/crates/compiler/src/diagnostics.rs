//! Serde's internally tagged enums buffer their contents and can discard the
//! nested error path. Recover it by decoding typed children of that exact node,
//! never by searching the card for a matching discriminator value.
use super::*;
use serde_json::Value;
type Decoder = fn(&str) -> Result<(), serde_json::Error>;
// Serde visits struct fields in input order. Preserve that order when two
// children produce the same error; a sorted JSON object loses this evidence.
#[derive(Default)]
struct FieldOrder(Vec<String>);
impl<'de> Deserialize<'de> for FieldOrder {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = FieldOrder;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("object")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> Result<Self::Value, M::Error> {
                let mut keys = vec![];
                while let Some(key) = map.next_key()? {
                    keys.push(key);
                    map.next_value::<serde::de::IgnoredAny>()?;
                }
                Ok(FieldOrder(keys))
            }
        }
        d.deserialize_map(Visitor)
    }
}
fn check<T: serde::de::DeserializeOwned>(v: &str) -> Result<(), serde_json::Error> {
    serde_json::from_str::<T>(v).map(|_| ())
}
pub(super) fn ability(node: &str, path: &str) -> Failure {
    locate(node, path, check::<Ability>, "ability")
}
fn locate(raw: &str, path: &str, decoder: Decoder, context: &str) -> Failure {
    let error = decoder(raw)
        .expect_err("diagnose only a failed decode")
        .to_string();
    let error = error.split(" at line ").next().unwrap().to_owned();
    let node: Value = serde_json::from_str(raw).expect("strict JSON already checked");
    let fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(raw).unwrap_or_default();
    let mut children: Vec<(Box<RawValue>, String, Decoder, &str)> = vec![];
    macro_rules! field {
        ($key:expr, $ty:ty, $ctx:expr) => {
            if let Some(v) = fields.get($key).filter(|v| v.get() != "null") {
                children.push((v.clone(), format!("{path}.{}", $key), check::<$ty>, $ctx));
            }
        };
    }
    macro_rules! array {
        ($key:expr, $ty:ty, $ctx:expr) => {
            if let Some(raw) = fields.get($key)
                && let Ok(a) = serde_json::from_str::<Vec<Box<RawValue>>>(raw.get())
            {
                for (i, v) in a.iter().enumerate() {
                    children.push((
                        v.clone(),
                        format!("{path}.{}[{i}]", $key),
                        check::<$ty>,
                        $ctx,
                    ));
                }
            }
        };
    }
    match context {
        "ability" => match node["kind"].as_str() {
            Some("keywords") => {
                array!("keywords", SourceKeyword, "keyword");
            }
            Some("static") => {
                field!("effect", SourceStaticEffect, "static");
            }
            Some("triggered") => {
                field!("condition", SourceCondition, "condition");
                field!("trigger", SourceTrigger, "trigger");
                array!("effects", Sentence, "sentence");
            }
            Some("spell") => {
                array!("effects", Sentence, "sentence");
            }
            Some("activated") | Some("additional-cost") => {
                array!("costs", SourceCost, "cost");
                array!("effects", Sentence, "sentence");
            }
            _ => {}
        },
        "trigger" => match node["trigger"].as_str() {
            Some("any-of") => {
                array!("events", SourceTrigger, "trigger");
            }
            Some("enters" | "dies" | "put-into-graveyard") => {
                field!("what", TriggerObject, "trigger-object");
            }
            _ => {}
        },
        "trigger-object" => {
            field!("filter", TriggerFilter, "leaf");
            field!("count", Count, "leaf");
        }
        "sentence" => {
            array!("steps", Step, "step");
        }
        "step" => {
            match node["effect"].as_str() {
                Some("damage") => {
                    field!("source", DamageSource, "leaf");
                    field!("to", DamageDestination, "leaf");
                }
                Some("add-mana") => {
                    field!("mana", SourceMana, "mana");
                }
                Some("prevent-damage") => {
                    field!("prevention", SourcePrevention, "prevention");
                }
                Some("counter") => {
                    field!("what", CounterTarget, "counter-target");
                }
                Some("create-token") => {
                    field!("token", SourceToken, "token");
                }
                Some("modal") => {
                    array!("options", ModeOption, "mode");
                }
                _ => {}
            }
            field!("duration", SourceDuration, "leaf");
            // Only typed amount/count fields are decoded here. Raw Value fields
            // deliberately stay opaque until their lowering operator checks them.
            if matches!(node["effect"].as_str(), Some("draw")) {
                field!("amount", Count, "leaf");
            }
        }
        "static" => {
            field!("replacement", SourceReplacement, "replacement");
        }
        "replacement" => {
            field!("event", SourceDamageEvent, "damage-event");
            field!("replace", SourceReplacementAction, "leaf");
        }
        "damage-event" => {
            field!("source", AnyDamageSource, "leaf");
        }
        "prevention" => {
            field!("shield", SourceShield, "shield");
        }
        "shield" => {
            field!("prevent", SourcePrevent, "leaf");
        }
        "counter-target" => {
            field!("filter", CounterFilter, "leaf");
        }
        "mode" => {
            array!("effects", Sentence, "sentence");
        }
        "token" => {
            field!("power", Count, "leaf");
            field!("toughness", Count, "leaf");
        }
        "condition" => {
            field!("comparison", SourceComparison, "comparison");
        }
        "comparison" => {
            field!("value", Count, "leaf");
        }
        _ => {}
    }
    let order: FieldOrder = serde_json::from_str(raw).unwrap_or_default();
    children.sort_by_key(|(_, child_path, _, _)| {
        let field = child_path[path.len() + 1..]
            .split(['[', '.'])
            .next()
            .unwrap();
        order.0.iter().position(|key| key == field).unwrap()
    });
    // Preserve the parent's actual first error when a different child also
    // fails. Only descend into a typed child reproducing that error.
    for (child, child_path, decode, ctx) in children {
        if decode(child.get())
            .is_err_and(|e| e.to_string().split(" at line ").next() == Some(error.as_str()))
        {
            return locate(child.get(), &child_path, decode, ctx);
        }
    }
    unsupported(path, coverage::reason_code(path, &error, &node))
}
