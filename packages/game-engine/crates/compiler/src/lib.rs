//! Offline archive-3 adapter and the first complete-card lowering subset.
pub use catalog::*;
use mtg_contracts::kir::{
    Amount, Atom, Cmp, Context, Destination, PlayerExpr, Pred, SetExpr, Verb,
};
use mtg_contracts::*;
pub use mtg_oracle_ast as ast;
use normalization::{matches_fixed_amount, source_lines};
use optional_costs::KickedCondition;
use serde::Deserialize;
use serde_json::value::RawValue;
use std::collections::{BTreeMap, BTreeSet};

pub mod fixtures {
    include!(concat!(env!("OUT_DIR"), "/fixtures.rs"));
}
pub mod bulk;
pub mod catalog;
pub mod coverage;

mod compose;
mod diagnostics;
mod families;
mod ingest;
mod normalization;
mod operands;
mod resolution;
mod rules;
mod shells;
#[cfg(test)]
mod tests;
mod type_line;

use families::*;
use ingest::*;
use operands::*;
use rules::*;
// Named modules shadow same-named `mtg_contracts` modules from its glob import.
use families::{
    activated_artifact_draw, announced_x_effects, bindings, cast_origin, conditional_lands,
    controlled_land_entry, controller_land_types, controller_library, entry_graveyard_exile,
    fixed_mill, graveyard_battlefield_return, hand_return, impulse_play, land_entry_gain_life,
    land_entry_scry, land_sacrifice_draw, library_order, library_selection, paid_counted_draw,
    power_entry_draw, source_dies_nonland_destroy, spell_cascade_grants,
    static_permanent_indestructible, successor_tuck, typed_fetchland, typed_land_search,
    unlimited_hand_size,
};
use shells::{bestow, linked_hideaway, modal_land, optional_costs, saga};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum CompileResult {
    Lowered { definition: Box<Definition> },
    Unsupported { path: String, reason: String },
    Invalid { path: String, reason: String },
}

#[derive(Debug)]
struct Failure {
    unsupported: bool,
    path: String,
    reason: String,
}

fn invalid(path: &str, reason: impl Into<String>) -> Failure {
    Failure {
        unsupported: false,
        path: path.into(),
        reason: reason.into(),
    }
}

fn unsupported(path: &str, reason: impl Into<String>) -> Failure {
    Failure {
        unsupported: true,
        path: path.into(),
        reason: reason.into(),
    }
}

fn decode<T: serde::de::DeserializeOwned>(s: &str, path: &str) -> Result<T, Failure> {
    strict_read(s).map_err(|e| invalid(path, e))
}

pub fn compile(manifest: &str, record: &str) -> CompileResult {
    compile_result(
        accepted_manifest(manifest)
            .map_err(|e| invalid("manifest", e))
            .and_then(|archive| lower(archive, record)),
    )
}

fn compile_result(result: Result<Definition, Failure>) -> CompileResult {
    match result {
        Ok(definition) => CompileResult::Lowered {
            definition: Box::new(definition),
        },
        Err(e) if e.unsupported => CompileResult::Unsupported {
            path: e.path,
            reason: e.reason,
        },
        Err(e) => CompileResult::Invalid {
            path: e.path,
            reason: e.reason,
        },
    }
}

/// Whole-card families first; a record no family supports falls back to the
/// block-by-block compositional pipeline. A family rejection that is not
/// recovered keeps its original path and reason.
fn lower(archive: &AcceptedArchive, record: &str) -> Result<Definition, Failure> {
    match lower_families(archive, record) {
        // A family that recognised the source and rejected its reference
        // graph has given an integrity verdict; only shape rejections fall back.
        Err(family)
            if family.unsupported
                && family.path != "binding"
                && !family.path.starts_with("binding.") =>
        {
            compose::lower(archive, record).map_err(|_| family)
        }
        other => other,
    }
}
