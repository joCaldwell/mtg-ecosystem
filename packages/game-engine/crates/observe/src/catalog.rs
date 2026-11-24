//! Token identity is scoped to the digest of the sorted catalog vocabulary.
//! Row zero is padding/unknown. An overlength program ends in an explicit
//! truncation token; vocabulary construction still visits the whole program.
use mtg_contracts::Catalog;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
/// Immutable card data and its precomputed observation-only indexes.
pub struct ObservationCatalog {
    definitions: std::sync::Arc<Catalog>,
    spell_cost_reducers: BTreeSet<String>,
}

impl ObservationCatalog {
    pub fn new(definitions: std::sync::Arc<Catalog>) -> Self {
        use mtg_contracts::kir::{SpellCostChange, StaticEffect, StaticRule};
        let spell_cost_reducers = definitions
            .definitions
            .iter()
            .filter(|d| {
                d.static_abilities.iter().any(|ability| {
                    matches!(
                        &ability.effect,
                        StaticEffect::Rule {
                            rule: StaticRule::SpellCost {
                                change: SpellCostChange::Reduce { .. },
                                ..
                            },
                            ..
                        }
                    )
                })
            })
            .map(|d| d.oracle_id.clone())
            .collect();
        Self {
            definitions,
            spell_cost_reducers,
        }
    }

    pub(crate) fn reduces_cost(&self, oracle_id: &str) -> bool {
        self.spell_cost_reducers.contains(oracle_id)
    }

    pub(crate) fn has_visible_reducer(&self, view: &mtg_view::PermittedView) -> bool {
        !self.spell_cost_reducers.is_empty()
            && view.objects.iter().any(|o| {
                o.zone == mtg_view::Zone::Battlefield
                    && !o.phased_out
                    && self.reduces_cost(&o.oracle_id)
            })
    }
}

impl std::ops::Deref for ObservationCatalog {
    type Target = Catalog;
    fn deref(&self) -> &Catalog {
        &self.definitions
    }
}

pub struct CardTable {
    pub vocabulary: Vec<String>,
    pub vocabulary_sha256: String,
    pub card_tokens: Vec<[i32; 192]>,
    pub card_len: Vec<i32>,
    pub card_num: Vec<[i32; 16]>,
}
fn walk(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(fields) => {
            out.push("{".into());
            let mut fields: Vec<_> = fields.iter().collect();
            fields.sort_unstable_by_key(|(k, _)| *k);
            for (k, v) in fields {
                if matches!(
                    k.as_str(),
                    "name" | "oracle_id" | "source_labels" | "provenance"
                ) {
                    continue;
                }
                out.push(format!("key:{k}"));
                walk(v, out);
            }
            out.push("}".into());
        }
        Value::Array(a) => {
            out.push("[".into());
            for v in a {
                walk(v, out)
            }
            out.push("]".into());
        }
        scalar => out.push(scalar.to_string()),
    }
}
impl CardTable {
    pub fn new(catalog: &Catalog) -> Result<Self, String> {
        let mut words = BTreeSet::from(["<truncated>".to_owned()]);
        let mut programs = Vec::new();
        let mut card_num = vec![[0; 16]];
        for definition in &catalog.definitions {
            let mut program = vec![];
            walk(
                &serde_json::to_value(definition).map_err(|e| e.to_string())?,
                &mut program,
            );
            words.extend(program.iter().cloned());
            programs.push(program);
            let c = &definition.characteristics;
            let mut row = [0; 16];
            row[0] = crate::int(&c.printed_mana_value());
            for color in &c.color_identity {
                row[1] |= 1 << crate::color(*color);
            }
            for kind in std::iter::once(&c.kind).chain(&c.additional_types) {
                row[2] |= 1 << (*kind as usize);
            }
            row[3] = c.power.as_ref().map_or(0, crate::int);
            row[4] = c.toughness.as_ref().map_or(0, crate::int);
            card_num.push(row);
        }
        let vocabulary: Vec<_> = words.into_iter().collect();
        let bytes = serde_json::to_vec(&vocabulary).map_err(|e| e.to_string())?;
        let vocabulary_sha256 = format!("{:x}", Sha256::digest(bytes));
        let mut card_tokens = vec![[0; 192]];
        let mut card_len = vec![0];
        for mut p in programs {
            if p.len() > 192 {
                p.truncate(191);
                p.push("<truncated>".into());
            }
            let mut row = [0; 192];
            card_len.push(p.len() as i32);
            for (cell, word) in row.iter_mut().zip(p) {
                *cell = vocabulary
                    .binary_search(&word)
                    .expect("collected vocabulary") as i32
                    + 1;
            }
            card_tokens.push(row);
        }
        Ok(Self {
            vocabulary,
            vocabulary_sha256,
            card_tokens,
            card_len,
            card_num,
        })
    }
}
