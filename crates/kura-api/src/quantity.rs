//! Kubernetes binary quantities (`256Ki`, `4Mi`, `20Gi`) and durations
//! (`12h`, `180d`), the two scalar formats of the resource documents
//! (RFC 0001 §4.1). The constructors are the only check.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

const UNITS: [(&str, u64); 5] = [
    ("Ei", 1 << 60),
    ("Ti", 1 << 40),
    ("Gi", 1 << 30),
    ("Mi", 1 << 20),
    ("Ki", 1 << 10),
];

/// A size in bytes, written as an integer with an optional binary suffix.
/// A bare YAML integer is accepted too, as Kubernetes does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "QuantityRepr", into = "String")]
pub struct Quantity(u64);

#[derive(Deserialize)]
#[serde(untagged)]
enum QuantityRepr {
    Bytes(u64),
    Text(String),
}

impl TryFrom<QuantityRepr> for Quantity {
    type Error = ScalarError;
    fn try_from(r: QuantityRepr) -> Result<Self, Self::Error> {
        match r {
            QuantityRepr::Bytes(b) => Ok(Self(b)),
            QuantityRepr::Text(s) => s.parse(),
        }
    }
}

impl Quantity {
    pub const fn from_bytes(bytes: u64) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScalarError {
    #[error(
        "`{0}` is not a size: expected an integer with an optional Ki, Mi, Gi, Ti or Ei suffix"
    )]
    Quantity(String),
    #[error("`{0}` is not a duration: expected an integer followed by h or d")]
    Duration(String),
}

/// Digits only, no sign, no leading `+`: `u64::from_str` alone accepts `+1`.
fn digits(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

impl FromStr for Quantity {
    type Err = ScalarError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || ScalarError::Quantity(s.to_owned());
        let (number, factor) = UNITS
            .iter()
            .find_map(|(suffix, factor)| s.strip_suffix(suffix).map(|n| (n, *factor)))
            .unwrap_or((s, 1));
        digits(number)
            .and_then(|n| n.checked_mul(factor))
            .map(Self)
            .ok_or_else(err)
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let unit = UNITS
            .iter()
            .find(|(_, factor)| self.0 != 0 && self.0.checked_rem(*factor) == Some(0));
        match unit {
            Some((suffix, factor)) => {
                write!(f, "{}{suffix}", self.0.checked_div(*factor).unwrap_or(0))
            }
            None => write!(f, "{}", self.0),
        }
    }
}

impl TryFrom<String> for Quantity {
    type Error = ScalarError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<Quantity> for String {
    fn from(q: Quantity) -> Self {
        q.to_string()
    }
}

/// A duration in whole hours, written `<n>h` or `<n>d`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Duration {
    hours: u64,
}

impl Duration {
    pub const fn hours(self) -> u64 {
        self.hours
    }

    pub fn as_std(self) -> std::time::Duration {
        std::time::Duration::from_secs(self.hours.saturating_mul(3600))
    }
}

impl FromStr for Duration {
    type Err = ScalarError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || ScalarError::Duration(s.to_owned());
        let hours = if let Some(n) = s.strip_suffix('d') {
            digits(n).and_then(|d| d.checked_mul(24))
        } else if let Some(n) = s.strip_suffix('h') {
            digits(n)
        } else {
            None
        };
        // Bounded so that `as_std` never saturates.
        hours
            .filter(|h| h.checked_mul(3600).is_some())
            .map(|hours| Self { hours })
            .ok_or_else(err)
    }
}

impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.hours != 0 && self.hours.checked_rem(24) == Some(0) {
            write!(f, "{}d", self.hours.checked_div(24).unwrap_or(0))
        } else {
            write!(f, "{}h", self.hours)
        }
    }
}

impl TryFrom<String> for Duration {
    type Error = ScalarError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<Duration> for String {
    fn from(d: Duration) -> Self {
        d.to_string()
    }
}

#[cfg(feature = "crd")]
impl schemars::JsonSchema for Quantity {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Quantity".into()
    }

    // An integer or a binary-suffixed string, as Kubernetes writes int-or-string.
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "x-kubernetes-int-or-string": true,
            "anyOf": [
                { "type": "integer", "minimum": 0 },
                { "type": "string", "pattern": "^[0-9]+(Ki|Mi|Gi|Ti|Ei)?$" }
            ],
        })
    }
}

#[cfg(feature = "crd")]
impl schemars::JsonSchema for Duration {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Duration".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string", "pattern": "^[0-9]+[hd]$" })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantities_parse_binary_suffixes() {
        assert_eq!("256Ki".parse::<Quantity>().unwrap().bytes(), 256 * 1024);
        assert_eq!("4Mi".parse::<Quantity>().unwrap().bytes(), 4 << 20);
        assert_eq!("20Gi".parse::<Quantity>().unwrap().bytes(), 20 << 30);
        assert_eq!("1Ti".parse::<Quantity>().unwrap().bytes(), 1 << 40);
        assert_eq!("1Ei".parse::<Quantity>().unwrap().bytes(), 1 << 60);
        assert_eq!("1000".parse::<Quantity>().unwrap().bytes(), 1000);
        assert_eq!("0".parse::<Quantity>().unwrap().bytes(), 0);
    }

    #[test]
    fn quantities_refuse_anything_else() {
        for bad in [
            "",
            "Mi",
            "1M",
            "1 Mi",
            "-1Mi",
            "+1Mi",
            "1.5Gi",
            "16Ei",
            "1mi",
            "99999999999999999999",
        ] {
            assert!(bad.parse::<Quantity>().is_err(), "{bad}");
        }
    }

    #[test]
    fn quantities_render_with_the_largest_exact_suffix() {
        for s in ["256Ki", "1Mi", "4Mi", "20Gi", "1000", "0", "1025"] {
            assert_eq!(s.parse::<Quantity>().unwrap().to_string(), s);
        }
        assert_eq!(Quantity::from_bytes(1024 * 1024).to_string(), "1Mi");
    }

    #[test]
    fn a_bare_integer_is_a_size_in_bytes() {
        let q: Quantity = serde_norway::from_str("1024").unwrap();
        assert_eq!(q.bytes(), 1024);
        let q: Quantity = serde_norway::from_str("4Mi").unwrap();
        assert_eq!(q.bytes(), 4 << 20);
        assert!(serde_norway::from_str::<Quantity>("-1").is_err());
    }

    #[test]
    fn durations_parse_hours_and_days() {
        assert_eq!("12h".parse::<Duration>().unwrap().hours(), 12);
        assert_eq!("180d".parse::<Duration>().unwrap().hours(), 180 * 24);
        assert_eq!(
            "2d".parse::<Duration>().unwrap().as_std(),
            std::time::Duration::from_secs(2 * 86_400)
        );
        assert_eq!("48h".parse::<Duration>().unwrap().to_string(), "2d");
        assert_eq!("0h".parse::<Duration>().unwrap().to_string(), "0h");
        assert_eq!("13h".parse::<Duration>().unwrap().to_string(), "13h");
    }

    #[test]
    fn durations_refuse_anything_else() {
        for bad in [
            "",
            "d",
            "1",
            "1m",
            "1s",
            "1w",
            "-1d",
            "+1d",
            "1.5d",
            "1 d",
            "99999999999999999999d",
        ] {
            assert!(bad.parse::<Duration>().is_err(), "{bad}");
        }
    }
}
