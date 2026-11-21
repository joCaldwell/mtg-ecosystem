//! The parser's archived AST and R4 resolution as typed Rust, generated from
//! `packages/oracle-parser/src/{ast.ts,semantic/resolution.ts}` by
//! `scripts/generate-rust-ast.ts` (never edited by hand; the engine checks
//! fail when it is stale). Every union decodes by an explicit discriminant and
//! every struct rejects unknown fields, so an archive row either decodes
//! completely or names the exact shape it violates. A leaf crate: compiler
//! edits never recompile the generated code.
pub use generated::*;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

// Generated layout is the generator's; rustfmt leaves it alone.
#[rustfmt::skip]
mod generated;

/// Decode one already-dispatched union member.
fn strict<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|e| e.to_string())
}

/// The JSON text of a discriminant property, for matching literal tags.
fn tag(object: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    object.get(key).map(Value::to_string)
}

/// A field whose only legal value is the integer `N`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Int<const N: i64>;
impl<'de, const N: i64> Deserialize<'de> for Int<N> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match i64::deserialize(deserializer)? {
            n if n == N => Ok(Int),
            n => Err(serde::de::Error::custom(format!("expected {N}, found {n}"))),
        }
    }
}

/// A field whose only legal value is `true` (an optional flag that is either
/// printed or absent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct True;
impl<'de> Deserialize<'de> for True {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if bool::deserialize(deserializer)? {
            Ok(True)
        } else {
            Err(serde::de::Error::custom("expected true"))
        }
    }
}

/// A field whose only legal value is `false`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct False;
impl<'de> Deserialize<'de> for False {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if bool::deserialize(deserializer)? {
            Err(serde::de::Error::custom("expected false"))
        } else {
            Ok(False)
        }
    }
}
