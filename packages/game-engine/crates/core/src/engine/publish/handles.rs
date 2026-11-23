//! The counter and seat of an issued handle, including its permission namespace.
//! Public requests may carry arbitrary strings. Keep those bytes on decoding and
//! never normalize them into a valid permission.
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{cmp::Ordering, fmt, sync::Arc};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandleId<const KIND: u8>(Representation);

#[derive(Clone, Debug, Eq, PartialEq)]
enum Representation {
    Issued(u64),
    Text(Arc<String>),
}

const HYPOTHESIS: &str = "hypothesis-v1";

impl<const KIND: u8> HandleId<KIND> {
    pub fn new(viewer: u8, serial: u64, namespace: Option<&str>) -> Self {
        if viewer < 4 && serial <= u64::MAX >> 3 {
            let scope = match namespace {
                None => Some(0),
                Some(HYPOTHESIS) => Some(4),
                _ => None,
            };
            if let Some(scope) = scope {
                return Self(Representation::Issued(
                    serial << 3 | scope | u64::from(viewer),
                ));
            }
        }
        let kind = char::from(KIND);
        Self(Representation::Text(match namespace {
            Some(n) => format!("{n}:v{viewer}-{kind}{serial}").into(),
            None => format!("v{viewer}-{kind}{serial}").into(),
        }))
    }

    fn parse_issued(text: &str) -> Option<Self> {
        let (text, scope) = text
            .strip_prefix("hypothesis-v1:")
            .map_or((text, 0), |s| (s, 4));
        let [b'v', seat @ b'0'..=b'3', b'-', kind, serial @ ..] = text.as_bytes() else {
            return None;
        };
        if *kind != KIND
            || !serial.first()?.is_ascii_digit()
            || serial.len() > 1 && serial[0] == b'0'
        {
            return None;
        }
        let serial: u64 = std::str::from_utf8(serial).ok()?.parse().ok()?;
        (serial <= u64::MAX >> 3).then(|| {
            Self(Representation::Issued(
                serial << 3 | scope | u64::from(*seat - b'0'),
            ))
        })
    }

    pub fn parse(text: &str) -> Self {
        Self::parse_issued(text)
            .unwrap_or_else(|| Self(Representation::Text(Arc::new(text.to_owned()))))
    }
}

impl<const KIND: u8> fmt::Display for HandleId<KIND> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Representation::Text(text) => f.write_str(text),
            Representation::Issued(bits) => {
                if bits & 4 != 0 {
                    f.write_str("hypothesis-v1:")?;
                }
                write!(f, "v{}-{}{}", bits & 3, char::from(KIND), bits >> 3)
            }
        }
    }
}

// Observations and action cursors use lexical decimal order (10 precedes 2).
// Align the leading digits, then put a shorter equal prefix first.
fn decimal_cmp(a: u64, b: u64) -> Ordering {
    let x = a.checked_ilog10().unwrap_or(0);
    let y = b.checked_ilog10().unwrap_or(0);
    match x.cmp(&y) {
        Ordering::Less => a.cmp(&(b / 10u64.pow(y - x))).then(Ordering::Less),
        Ordering::Greater => (a / 10u64.pow(x - y)).cmp(&b).then(Ordering::Greater),
        Ordering::Equal => a.cmp(&b),
    }
}

impl<const KIND: u8> Ord for HandleId<KIND> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (&self.0, &other.0) {
            (Representation::Issued(a), Representation::Issued(b)) => (b & 4)
                .cmp(&(a & 4))
                .then_with(|| (a & 3).cmp(&(b & 3)))
                .then_with(|| decimal_cmp(a >> 3, b >> 3)),
            (Representation::Text(a), Representation::Text(b)) => a.cmp(b),
            (Representation::Text(a), _) => a.as_str().cmp(&other.to_string()),
            (_, Representation::Text(b)) => self.to_string().as_str().cmp(b),
        }
    }
}
impl<const KIND: u8> PartialOrd for HandleId<KIND> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<const KIND: u8> Serialize for HandleId<KIND> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}
impl<'de, const KIND: u8> Deserialize<'de> for HandleId<KIND> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(|s| Self::parse(&s))
    }
}
impl<const KIND: u8> From<&str> for HandleId<KIND> {
    fn from(s: &str) -> Self {
        Self::parse(s)
    }
}

macro_rules! boundary_handle {
    ($kind:literal, $public:ty) => {
        impl From<&HandleId<$kind>> for $public {
            fn from(id: &HandleId<$kind>) -> Self {
                Self(id.to_string())
            }
        }
        impl From<&$public> for HandleId<$kind> {
            fn from(handle: &$public) -> Self {
                Self::parse(&handle.0)
            }
        }
        #[cfg(feature = "testkit")]
        impl PartialEq<HandleId<$kind>> for $public {
            fn eq(&self, id: &HandleId<$kind>) -> bool {
                let own: HandleId<$kind> = self.into();
                own == *id
            }
        }
        impl From<&$public> for $public {
            fn from(handle: &Self) -> Self {
                handle.clone()
            }
        }
    };
}
boundary_handle!(b'o', super::visibility::ObjectHandle);
boundary_handle!(b's', super::visibility::StackHandle);
boundary_handle!(b'e', crate::engine::EffectHandle);

impl<const KIND: u8> From<&HandleId<KIND>> for HandleId<KIND> {
    fn from(id: &Self) -> Self {
        id.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_actions_follow_exact_wire_order_and_identity() {
        type Object = HandleId<b'o'>;
        assert!(std::mem::size_of::<Object>() <= 16);
        let mut ids = Vec::new();
        for seat in 0..4 {
            for namespace in [None, Some(HYPOTHESIS), Some("table:v2")] {
                for serial in [
                    0,
                    1,
                    2,
                    10,
                    11,
                    19,
                    99,
                    100,
                    u64::MAX >> 3,
                    (u64::MAX >> 3) + 1,
                    u64::MAX,
                ] {
                    let id = Object::new(seat, serial, namespace);
                    assert_eq!(id, Object::parse(&id.to_string()));
                    ids.push(id);
                }
            }
        }
        ids.extend(["v0-o01", "v0-o+1", "v1-s0", "v2-o", "v2-o4:search:1", ""].map(Object::parse));
        for a in &ids {
            for b in &ids {
                assert_eq!(a.cmp(b), a.to_string().cmp(&b.to_string()));
                assert_eq!(*a == *b, a.cmp(b).is_eq());
            }
        }
    }
}
