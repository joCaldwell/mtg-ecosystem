//! Offline compiler from exact archived card records to executable IR.
//!
//! One pipeline lowers a record block by block: `ingest` (exact record,
//! source, parse and typed AST mirror) -> `operands` (total operand lowerers)
//! -> `rules` (verb and block lowerers) -> `assemble` (definition shell and
//! admission), driven by `compose` over the record's Archive R4 resolution.
//! A record without a complete resolution, or with a shape no block table
//! lowers, is rejected.
pub use catalog::*;
use mtg_contracts::kir::{
    Amount, Atom, Cmp, Context, Destination, EventPattern, PlayerExpr, Pred, Ref, SetExpr, Verb,
};
use mtg_contracts::*;
pub use mtg_oracle_ast as ast;
use normalization::source_lines;
use serde::Deserialize;
use serde_json::value::RawValue;
use std::collections::{BTreeMap, BTreeSet};

pub mod fixtures {
    include!(concat!(env!("OUT_DIR"), "/fixtures.rs"));
}
pub mod bulk;
pub mod catalog;
pub mod compile;
pub mod coverage;
pub mod reference_trace;

mod assemble;
mod compose;
mod ingest;
mod normalization;
mod operands;
mod resolution;
mod rules;
#[cfg(test)]
mod tests;
mod type_line;

use ingest::*;
use operands::*;
use rules::*;

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

fn lower(archive: &AcceptedArchive, record: &str) -> Result<Definition, Failure> {
    compose::lower(archive, record)
}
