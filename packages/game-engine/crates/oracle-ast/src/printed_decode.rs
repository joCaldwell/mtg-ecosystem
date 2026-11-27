use serde::de::{self, DeserializeSeed};
use serde::{Serialize, Serializer};
use serde_json::{Map, Value};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    Omitted,
    Null,
    Unknown,
    Malformed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reject {
    /// RFC 6901 JSON pointer relative to the supplied evidence value.
    pub path: String,
    pub fault: Fault,
    pub detail: String,
}

impl fmt::Display for Reject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} at {:?}: {}", self.fault, self.path, self.detail)
    }
}

impl std::error::Error for Reject {}

pub(super) enum Pointer<'a> {
    Root,
    Key(&'a Pointer<'a>, &'a str),
    Index(&'a Pointer<'a>, usize),
}

impl fmt::Display for Pointer<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root => Ok(()),
            Self::Index(parent, index) => write!(f, "{parent}/{index}"),
            Self::Key(parent, key) => {
                write!(f, "{parent}/")?;
                for ch in key.chars() {
                    match ch {
                        '~' => f.write_str("~0")?,
                        '/' => f.write_str("~1")?,
                        ch => write!(f, "{ch}")?,
                    }
                }
                Ok(())
            }
        }
    }
}

impl Pointer<'_> {
    pub(super) fn reject(&self, fault: Fault, detail: impl Into<String>) -> Reject {
        Reject {
            path: self.to_string(),
            fault,
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Nullable<T> {
    Null,
    Value(T),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Presence<T> {
    Omitted,
    Null,
    Value(T),
}

impl<T> Presence<T> {
    pub fn is_omitted(&self) -> bool {
        matches!(self, Self::Omitted)
    }
}

pub(super) type Object = Map<String, Value>;

pub(super) trait Wire: Sized {
    fn present(value: &Value, at: &Pointer<'_>) -> Result<Self, Reject>;

    fn read(value: &Value, at: &Pointer<'_>) -> Result<Self, Reject> {
        if value.is_null() {
            return Err(at.reject(Fault::Null, "explicit null is forbidden"));
        }
        Self::present(value, at)
    }
}

pub(super) fn object<'v>(value: &'v Value, at: &Pointer<'_>) -> Result<&'v Object, Reject> {
    value
        .as_object()
        .ok_or_else(|| at.reject(Fault::Malformed, "expected an object"))
}

pub(super) fn text<'v>(value: &'v Value, at: &Pointer<'_>) -> Result<&'v str, Reject> {
    value
        .as_str()
        .ok_or_else(|| at.reject(Fault::Malformed, "expected a string"))
}

pub(super) fn known_fields(
    object: &Object,
    fields: &[&str],
    at: &Pointer<'_>,
) -> Result<(), Reject> {
    for key in object.keys() {
        if !fields.contains(&key.as_str()) {
            return Err(Pointer::Key(at, key).reject(Fault::Unknown, "field is not declared"));
        }
    }
    Ok(())
}

pub(super) fn presence<T: Wire>(
    object: &Object,
    key: &str,
    at: &Pointer<'_>,
) -> Result<Presence<T>, Reject> {
    let field = Pointer::Key(at, key);
    match object.get(key) {
        None => Ok(Presence::Omitted),
        Some(Value::Null) => Ok(Presence::Null),
        Some(value) => T::present(value, &field).map(Presence::Value),
    }
}

pub(super) fn required<T: Wire>(object: &Object, key: &str, at: &Pointer<'_>) -> Result<T, Reject> {
    match presence(object, key, at)? {
        Presence::Omitted => Err(Pointer::Key(at, key).reject(Fault::Omitted, "required field")),
        Presence::Null => {
            Err(Pointer::Key(at, key).reject(Fault::Null, "explicit null is forbidden"))
        }
        Presence::Value(value) => Ok(value),
    }
}

pub(super) fn optional<T: Wire>(
    object: &Object,
    key: &str,
    at: &Pointer<'_>,
) -> Result<Option<T>, Reject> {
    match presence(object, key, at)? {
        Presence::Omitted => Ok(None),
        Presence::Null => {
            Err(Pointer::Key(at, key).reject(Fault::Null, "explicit null is forbidden"))
        }
        Presence::Value(value) => Ok(Some(value)),
    }
}

pub(super) fn nullable<T: Wire>(
    object: &Object,
    key: &str,
    at: &Pointer<'_>,
) -> Result<Nullable<T>, Reject> {
    match presence(object, key, at)? {
        Presence::Omitted => Err(Pointer::Key(at, key).reject(Fault::Omitted, "required field")),
        Presence::Null => Ok(Nullable::Null),
        Presence::Value(value) => Ok(Nullable::Value(value)),
    }
}

pub(super) fn tag<'v>(object: &'v Object, key: &str, at: &Pointer<'_>) -> Result<&'v str, Reject> {
    let field = Pointer::Key(at, key);
    match object.get(key) {
        None => Err(field.reject(Fault::Omitted, "required discriminator")),
        Some(Value::Null) => Err(field.reject(Fault::Null, "explicit null is forbidden")),
        Some(value) => text(value, &field),
    }
}

pub(super) fn tuple<const N: usize>(value: &Value, at: &Pointer<'_>) -> Result<[u32; N], Reject> {
    let items = value
        .as_array()
        .ok_or_else(|| at.reject(Fault::Malformed, "expected an array"))?;
    if items.len() != N {
        return Err(at.reject(Fault::Malformed, format!("expected {N} array items")));
    }
    let mut result = [0; N];
    for (index, (destination, item)) in result.iter_mut().zip(items).enumerate() {
        *destination = u32::read(item, &Pointer::Index(at, index))?;
    }
    Ok(result)
}

impl Wire for String {
    fn present(value: &Value, at: &Pointer<'_>) -> Result<Self, Reject> {
        text(value, at).map(str::to_owned)
    }
}

impl Wire for u32 {
    fn present(value: &Value, at: &Pointer<'_>) -> Result<Self, Reject> {
        value
            .as_u64()
            .and_then(|number| u32::try_from(number).ok())
            .ok_or_else(|| at.reject(Fault::Malformed, "expected a u32 integer"))
    }
}

impl Wire for bool {
    fn present(value: &Value, at: &Pointer<'_>) -> Result<Self, Reject> {
        value
            .as_bool()
            .ok_or_else(|| at.reject(Fault::Malformed, "expected a boolean"))
    }
}

impl<T: Wire> Wire for Vec<T> {
    fn present(value: &Value, at: &Pointer<'_>) -> Result<Self, Reject> {
        value
            .as_array()
            .ok_or_else(|| at.reject(Fault::Malformed, "expected an array"))?
            .iter()
            .enumerate()
            .map(|(i, item)| T::read(item, &Pointer::Index(at, i)))
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiteralU32<const N: u32>;

impl<const N: u32> Wire for LiteralU32<N> {
    fn present(value: &Value, at: &Pointer<'_>) -> Result<Self, Reject> {
        if u32::present(value, at)? != N {
            return Err(at.reject(Fault::Unknown, format!("expected literal {N}")));
        }
        Ok(Self)
    }
}

impl<const N: u32> Serialize for LiteralU32<N> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u32(N)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiteralBool<const B: bool>;

impl<const B: bool> Wire for LiteralBool<B> {
    fn present(value: &Value, at: &Pointer<'_>) -> Result<Self, Reject> {
        if bool::present(value, at)? != B {
            return Err(at.reject(Fault::Unknown, format!("expected literal {B}")));
        }
        Ok(Self)
    }
}

impl<const B: bool> Serialize for LiteralBool<B> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bool(B)
    }
}

pub(super) fn decode<T: Wire>(json: &str) -> Result<T, Reject> {
    let mut parser = serde_json::Deserializer::from_str(json);
    let mut rejection = None;
    let at = &Pointer::Root;
    let value = Json {
        at,
        rejection: &mut rejection,
    }
    .deserialize(&mut parser)
    .map_err(|error| rejection.unwrap_or_else(|| at.reject(Fault::Malformed, error.to_string())))?;
    parser
        .end()
        .map_err(|error| at.reject(Fault::Malformed, error.to_string()))?;
    T::read(&value, at)
}

/// Build the JSON value without allowing a duplicate to overwrite an earlier key.
/// Preserve the deepest known pointer even when serde stops at invalid JSON.
struct Json<'a, 'p> {
    at: &'a Pointer<'a>,
    rejection: &'p mut Option<Reject>,
}

impl<'de> DeserializeSeed<'de> for Json<'_, '_> {
    type Value = Value;

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        let at = self.at;
        let rejection = self.rejection;
        deserializer
            .deserialize_any(Json { at, rejection })
            .inspect_err(|error| {
                rejection.get_or_insert_with(|| at.reject(Fault::Malformed, error.to_string()));
            })
    }
}

impl<'de> de::Visitor<'de> for Json<'_, '_> {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON value")
    }

    fn visit_map<M: de::MapAccess<'de>>(self, mut map: M) -> Result<Value, M::Error> {
        let mut object = Object::new();
        while let Some(key) = map.next_key::<String>()? {
            let at = Pointer::Key(self.at, &key);
            if object.contains_key(&key) {
                *self.rejection = Some(at.reject(Fault::Malformed, "duplicate object key"));
                return Err(de::Error::custom("duplicate object key"));
            }
            let value = map.next_value_seed(Json {
                at: &at,
                rejection: self.rejection,
            })?;
            object.insert(key, value);
        }
        Ok(Value::Object(object))
    }

    fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut items = Vec::new();
        while let Some(value) = seq.next_element_seed(Json {
            at: &Pointer::Index(self.at, items.len()),
            rejection: self.rejection,
        })? {
            items.push(value);
        }
        Ok(Value::Array(items))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| de::Error::custom("expected a finite JSON number"))
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
}
