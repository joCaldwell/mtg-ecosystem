//! CR 201.5: a source-name reference denotes that object, not another object
//! with the same name. Require the archive's exact source binding at its AST
//! location before canonicalizing it to the existing `this` form.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
use serde_json::Value;

pub(super) fn canonicalize(
    raw: &str,
    binding: &Value,
    identity: &str,
    noun: &str,
    base: &str,
) -> Result<String, Failure> {
    #[derive(Default)]
    struct OrderedObject(Vec<(String, Box<RawValue>)>);
    impl<'de> Deserialize<'de> for OrderedObject {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct Visitor;
            impl<'de> serde::de::Visitor<'de> for Visitor {
                type Value = OrderedObject;
                fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    f.write_str("object")
                }
                fn visit_map<M: serde::de::MapAccess<'de>>(
                    self,
                    mut map: M,
                ) -> Result<Self::Value, M::Error> {
                    let mut fields = vec![];
                    while let Some(field) = map.next_entry()? {
                        fields.push(field);
                    }
                    Ok(OrderedObject(fields))
                }
            }
            d.deserialize_map(Visitor)
        }
    }
    fn visit(
        raw: &str,
        path: &str,
        binding: &Value,
        identity: &str,
        noun: &str,
    ) -> Result<String, Failure> {
        let node: Value = decode(raw, "abilities")?;
        if node == serde_json::json!({"ref":"self"}) {
            // Damage sources already lower `self` to the source incarnation.
            // Preserve that encoding; it is independently checked by its operator.
            if path.ends_with("/source") {
                return Ok(raw.to_owned());
            }
            let source_id = format!("card:{identity}/source#object");
            let source = binding["nodes"].as_array().is_some_and(|nodes| {
                nodes.iter().any(|n| {
                    n["node"] == "source"
                        && n["id"] == source_id
                        && n["identity"] == identity
                        && n["path"] == "/source"
                })
            });
            let refs = binding["references"]
                .as_array()
                .map(|refs| {
                    refs.iter()
                        .filter(|r| r["path"] == path)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if binding["identity"] != identity
                || !source
                || refs.len() != 1
                || refs[0]["binding"] != "bound"
                || refs[0]["target"] != source_id
                || refs[0]["expression"] != node
            {
                return Err(unsupported(path, "ref:self:unbound_source"));
            }
            return Ok(serde_json::json!({"ref":"this", "noun":noun}).to_string());
        }
        if node.is_array() {
            let a: Vec<Box<RawValue>> = decode(raw, "abilities")?;
            let values = a
                .iter()
                .enumerate()
                .map(|(i, v)| visit(v.get(), &format!("{path}/{i}"), binding, identity, noun))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("[{}]", values.join(",")))
        } else if node.is_object() {
            let o: OrderedObject = decode(raw, "abilities")?;
            let values =
                o.0.iter()
                    .map(|(key, v)| {
                        Ok(format!(
                            "{}:{}",
                            serde_json::to_string(key).unwrap(),
                            visit(
                                v.get(),
                                &format!("{path}/{}", key.replace('~', "~0").replace('/', "~1")),
                                binding,
                                identity,
                                noun
                            )?
                        ))
                    })
                    .collect::<Result<Vec<_>, Failure>>()?;
            Ok(format!("{{{}}}", values.join(",")))
        } else {
            Ok(raw.to_owned())
        }
    }
    visit(raw, base, binding, identity, noun)
}
