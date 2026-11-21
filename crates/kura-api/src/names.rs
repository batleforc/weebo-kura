//! Object names: RFC 1123 labels, as Kubernetes requires of namespaces and
//! as Kura requires of tenant and vault names (they appear in paths).

use std::fmt;

use serde::{Deserialize, Serialize};

/// A lowercase RFC 1123 label: `[a-z0-9]([-a-z0-9]*[a-z0-9])?`, at most 63
/// characters.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DnsLabel(String);

/// The `metadata.name` of a `KuraTenant` (RFC 0001 §6.11).
pub type TenantSlug = DnsLabel;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "`{0}` is not a valid name: lowercase letters, digits and inner dashes, 1 to 63 characters"
)]
pub struct NameError(pub String);

impl DnsLabel {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for DnsLabel {
    type Error = NameError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        let bytes = s.as_bytes();
        let edge_ok =
            |b: Option<&u8>| b.is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
        let ok = (1..=63).contains(&bytes.len())
            && edge_ok(bytes.first())
            && edge_ok(bytes.last())
            && bytes
                .iter()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-');
        if ok { Ok(Self(s)) } else { Err(NameError(s)) }
    }
}

impl TryFrom<&str> for DnsLabel {
    type Error = NameError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::try_from(s.to_owned())
    }
}

impl From<DnsLabel> for String {
    fn from(l: DnsLabel) -> Self {
        l.0
    }
}

// Same ordering and hash as the inner String, so maps keyed by label can be queried by `&str`.
impl std::borrow::Borrow<str> for DnsLabel {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DnsLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(feature = "crd")]
impl schemars::JsonSchema for DnsLabel {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "DnsLabel".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "pattern": "^[a-z0-9]([-a-z0-9]*[a-z0-9])?$",
            "maxLength": 63,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_rfc1123_labels() {
        for ok in ["a", "maxime", "kura-1", "0abc", &"a".repeat(63)] {
            assert!(DnsLabel::try_from(ok).is_ok(), "{ok}");
        }
    }

    #[test]
    fn refuses_everything_else() {
        for bad in [
            "",
            "-a",
            "a-",
            "Maxime",
            "a_b",
            "a.b",
            "a/b",
            "é",
            &"a".repeat(64),
        ] {
            assert!(DnsLabel::try_from(bad).is_err(), "{bad}");
        }
    }
}
