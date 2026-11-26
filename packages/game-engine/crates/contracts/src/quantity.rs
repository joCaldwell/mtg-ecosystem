use num_bigint::{BigInt, Sign};
use num_traits::ToPrimitive;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize, de};
use std::{borrow::Cow, cmp::Ordering, fmt, str::FromStr};

use QuantityRepr::{Big, Small};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Quantity(QuantityRepr);

#[derive(Clone, PartialEq, Eq, Hash)]
enum QuantityRepr {
    Small(i64),
    // Always outside the i64 range, so each quantity has one representation.
    Big(Box<BigInt>),
}

impl Quantity {
    pub fn zero() -> Self {
        Self(Small(0))
    }

    /// The value as a `u32`, when it is one.
    pub fn to_u32(&self) -> Option<u32> {
        match &self.0 {
            Small(value) => u32::try_from(*value).ok(),
            Big(_) => None,
        }
    }

    pub fn is_negative(&self) -> bool {
        match &self.0 {
            Small(value) => *value < 0,
            Big(value) => value.sign() == Sign::Minus,
        }
    }

    pub fn add(&mut self, other: &Self) {
        self.apply(other, i64::checked_add, |left, right| *left += right);
    }

    pub fn multiply(&mut self, other: &Self) {
        self.apply(other, i64::checked_mul, |left, right| *left *= right);
    }

    pub fn subtract(&mut self, other: &Self) {
        self.apply(other, i64::checked_sub, |left, right| *left -= right);
    }

    /// Exact half, rounded toward positive infinity when `round_up` and
    /// toward negative infinity otherwise (CR 107.1a).
    pub fn halved(&self, round_up: bool) -> Self {
        match &self.0 {
            Small(value) => {
                let value = i128::from(*value);
                let half = if round_up {
                    -((-value).div_euclid(2))
                } else {
                    value.div_euclid(2)
                };
                Self::from_big(BigInt::from(half))
            }
            Big(value) => {
                let two = BigInt::from(2);
                let (mut half, rest) = (value.as_ref() / &two, value.as_ref() % &two);
                if round_up && rest.sign() == Sign::Plus {
                    half += 1;
                } else if !round_up && rest.sign() == Sign::Minus {
                    half -= 1;
                }
                Self::from_big(half)
            }
        }
    }

    fn from_big(value: BigInt) -> Self {
        match value.to_i64() {
            Some(value) => Self(Small(value)),
            None => Self(Big(Box::new(value))),
        }
    }

    fn apply(
        &mut self,
        other: &Self,
        checked: impl FnOnce(i64, i64) -> Option<i64>,
        exact: impl FnOnce(&mut BigInt, &BigInt),
    ) {
        if let (Small(left), Small(right)) = (&mut self.0, &other.0)
            && let Some(result) = checked(*left, *right)
        {
            *left = result;
            return;
        }

        let right = match &other.0 {
            Small(value) => Cow::Owned(BigInt::from(*value)),
            Big(value) => Cow::Borrowed(value.as_ref()),
        };
        match &mut self.0 {
            Small(left) => {
                let mut value = BigInt::from(*left);
                exact(&mut value, &right);
                *self = Self::from_big(value);
            }
            Big(left) => {
                exact(left, &right);
                if let Some(value) = left.to_i64() {
                    self.0 = Small(value);
                }
            }
        }
    }
}

impl Default for Quantity {
    fn default() -> Self {
        Self::zero()
    }
}

impl Ord for Quantity {
    fn cmp(&self, other: &Self) -> Ordering {
        match (&self.0, &other.0) {
            (Small(left), Small(right)) => left.cmp(right),
            (Big(left), Big(right)) => left.cmp(right),
            // All negative Big values precede the Small range, and positive ones follow it.
            (Small(_), Big(right)) => Sign::NoSign.cmp(&right.sign()),
            (Big(left), Small(_)) => left.sign().cmp(&Sign::NoSign),
        }
    }
}

impl PartialOrd for Quantity {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<i64> for Quantity {
    fn from(value: i64) -> Self {
        Self(Small(value))
    }
}

impl FromStr for Quantity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let value = match s.parse::<i64>() {
            Ok(value) => Self::from(value),
            Err(_) => Self::from_big(
                BigInt::from_str(s).map_err(|_| "invalid exact quantity".to_string())?,
            ),
        };
        if value.to_string() != s {
            return Err("quantity must use canonical decimal spelling".into());
        }
        Ok(value)
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl fmt::Display for QuantityRepr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Small(value) => fmt::Display::fmt(value, f),
            Big(value) => fmt::Display::fmt(value, f),
        }
    }
}

impl fmt::Debug for QuantityRepr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // BigInt delegates Debug to Display, including under hexadecimal debug flags.
        fmt::Display::fmt(self, f)
    }
}

impl Serialize for Quantity {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Quantity {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?.parse().map_err(de::Error::custom)
    }
}

impl JsonSchema for Quantity {
    fn schema_name() -> Cow<'static, str> {
        "ExactQuantity".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({"type":"string","pattern":"^(0|[1-9][0-9]*|-[1-9][0-9]*)$"})
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn halving_is_exact_in_both_directions_and_representations() {
        let q = |s: &str| s.parse::<super::Quantity>().unwrap();
        for (value, down, up) in [
            ("0", "0", "0"),
            ("7", "3", "4"),
            ("-7", "-4", "-3"),
            ("4294967297", "2147483648", "2147483649"),
            (
                "9223372036854775807",
                "4611686018427387903",
                "4611686018427387904",
            ),
            (
                "-9223372036854775808",
                "-4611686018427387904",
                "-4611686018427387904",
            ),
            (
                "18446744073709551617",
                "9223372036854775808",
                "9223372036854775809",
            ),
            (
                "-18446744073709551617",
                "-9223372036854775809",
                "-9223372036854775808",
            ),
            (
                "36893488147419103232",
                "18446744073709551616",
                "18446744073709551616",
            ),
        ] {
            assert_eq!(q(value).halved(false), q(down), "{value} down");
            assert_eq!(q(value).halved(true), q(up), "{value} up");
        }
    }

    use super::*;
    use std::hash::{BuildHasher, BuildHasherDefault, DefaultHasher};

    #[test]
    fn arithmetic_promotes_on_i64_overflow() {
        let cases = [
            (Quantity::add as fn(&mut Quantity, &Quantity), i64::MAX, 1),
            (Quantity::subtract, i64::MIN, 1),
            (Quantity::multiply, i64::MIN, -1),
            (Quantity::multiply, i64::MAX, i64::MAX),
        ];
        let expected = [
            "9223372036854775808",
            "-9223372036854775809",
            "9223372036854775808",
            "85070591730234615847396907784232501249",
        ];

        for ((apply, left, right), decimal) in cases.into_iter().zip(expected) {
            let mut value = Quantity::from(left);
            apply(&mut value, &Quantity::from(right));
            assert!(matches!(&value.0, Big(_)));
            assert_eq!(value.to_string(), decimal);
        }
    }

    #[test]
    fn exact_arithmetic_returns_to_small_when_in_range() {
        let mut above: Quantity = "9223372036854775808".parse().unwrap();
        let mut below: Quantity = "-9223372036854775809".parse().unwrap();
        let mut sum = above.clone();
        sum.add(&below);
        assert!(matches!(sum.0, Small(-1)));

        above.subtract(&Quantity::from(1));
        below.add(&Quantity::from(1));
        assert!(matches!(above.0, Small(i64::MAX)));
        assert!(matches!(below.0, Small(i64::MIN)));

        let mut zero = Quantity::zero();
        zero.multiply(&"9223372036854775808".parse().unwrap());
        assert!(matches!(zero.0, Small(0)));
        assert_eq!(Quantity::default(), zero);
    }

    #[test]
    fn serde_round_trips_exact_decimal_strings() {
        for decimal in [
            "0",
            "-5",
            "-9223372036854775808",
            "9223372036854775807",
            "12345678901234567890123456789012345678901234567890",
        ] {
            let value: Quantity = decimal.parse().unwrap();
            let json = serde_json::to_string(&value).unwrap();
            assert_eq!(json, format!("\"{decimal}\""));
            assert_eq!(serde_json::from_str::<Quantity>(&json).unwrap(), value);
        }
        assert!(serde_json::from_str::<Quantity>("1").is_err());
    }

    #[test]
    fn noncanonical_spellings_are_rejected_with_exact_errors() {
        for decimal in ["01", "-0", "+1", " 1", "1_0", "+9223372036854775808"] {
            let expected = match decimal.parse::<BigInt>() {
                Ok(_) => "quantity must use canonical decimal spelling",
                Err(_) => "invalid exact quantity",
            };
            assert_eq!(decimal.parse::<Quantity>().unwrap_err(), expected);
            let json = serde_json::to_string(decimal).unwrap();
            assert!(
                serde_json::from_str::<Quantity>(&json)
                    .unwrap_err()
                    .to_string()
                    .starts_with(expected)
            );
        }
    }

    #[test]
    fn equality_hashing_and_ordering_use_exact_values() {
        let mut computed = Quantity::from(i64::MAX);
        computed.add(&Quantity::from(1));
        computed.subtract(&Quantity::from(i64::MAX));
        let direct = Quantity::from(1);
        assert_eq!(computed, direct);
        assert_eq!(computed.cmp(&direct), Ordering::Equal);
        let hasher = BuildHasherDefault::<DefaultHasher>::default();
        assert_eq!(hasher.hash_one(&computed), hasher.hash_one(&direct));

        let ordered = [
            "-9223372036854775810".parse().unwrap(),
            "-9223372036854775809".parse().unwrap(),
            Quantity::from(i64::MIN),
            Quantity::zero(),
            Quantity::from(i64::MAX),
            "9223372036854775808".parse().unwrap(),
            "9223372036854775809".parse().unwrap(),
        ];
        for pair in ordered.windows(2) {
            assert!(pair[0] < pair[1]);
            assert!(pair[1] > pair[0]);
        }
        assert_eq!(
            ordered.map(|value| value.is_negative()),
            [true, true, true, false, false, false, false,]
        );
    }

    #[test]
    fn display_and_debug_match_the_bigint_wrapper() {
        #[derive(Debug)]
        struct Quantity(BigInt);

        for decimal in [
            "0",
            "17",
            "-9223372036854775808",
            "123456789012345678901234567890",
            "-123456789012345678901234567890",
        ] {
            let reference = Quantity(decimal.parse().unwrap());
            let actual: super::Quantity = decimal.parse().unwrap();
            assert_eq!(format!("{actual}"), format!("{}", reference.0));
            assert_eq!(format!("{actual:+030}"), format!("{:+030}", reference.0));
            assert_eq!(format!("{actual:*^32}"), format!("{:*^32}", reference.0));
            assert_eq!(format!("{actual:?}"), format!("{reference:?}"));
            assert_eq!(format!("{actual:#?}"), format!("{reference:#?}"));
            assert_eq!(format!("{actual:x?}"), format!("{reference:x?}"));
        }
    }
}
