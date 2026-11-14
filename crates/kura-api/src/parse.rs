//! Reading resource documents from YAML files, several per file.

use std::fmt;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_norway::Value;

use crate::spec::{API_VERSION, Document, KIND_TENANT, KIND_VAULT, KuraTenant, KuraVault};

/// Where a document came from: file name and 1-based index in the file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Source {
    pub file: String,
    pub index: usize,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}", self.file, self.index)
    }
}

/// A parsed document of either kind, with its source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resource {
    Tenant(Source, KuraTenant),
    Vault(Source, KuraVault),
}

impl Resource {
    pub fn source(&self) -> &Source {
        match self {
            Self::Tenant(s, _) | Self::Vault(s, _) => s,
        }
    }
}

/// A rejected document or set of documents (RFC 0001 §4.3), rendered as
/// `tenants.yaml#2 spec.claimMappings.roles.ownr: unknown field ...`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
pub struct ConfigError {
    /// `None` for a rule about the whole set (mass removal, no tenant).
    pub location: Option<Source>,
    /// Field path inside the document, empty when the rule is about all of it.
    pub path: String,
    pub message: String,
}

impl ConfigError {
    pub fn new(
        source: Option<&Source>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            location: source.cloned(),
            path: path.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.location, self.path.is_empty()) {
            (Some(s), false) => write!(f, "{s} {}: {}", self.path, self.message),
            (Some(s), true) => write!(f, "{s}: {}", self.message),
            (None, false) => write!(f, "{}: {}", self.path, self.message),
            (None, true) => f.write_str(&self.message),
        }
    }
}

/// Parses every document of one file. Empty documents (a trailing `---`)
/// are skipped. Returns every error found, not only the first.
pub fn parse_file(file: &str, content: &str) -> Result<Vec<Resource>, Vec<ConfigError>> {
    let mut resources = Vec::new();
    let mut errors = Vec::new();
    for (i, doc) in serde_norway::Deserializer::from_str(content).enumerate() {
        let source = Source {
            file: file.to_owned(),
            index: i.saturating_add(1),
        };
        let value = match <Value as serde::Deserialize>::deserialize(doc) {
            Ok(Value::Null) => continue,
            Ok(v) => v,
            Err(e) => {
                // A syntax error ends the stream: later documents cannot be located.
                errors.push(ConfigError::new(Some(&source), "", e.to_string()));
                break;
            }
        };
        match parse_document(&source, value) {
            Ok(r) => resources.push(r),
            Err(e) => errors.push(e),
        }
    }
    if errors.is_empty() {
        Ok(resources)
    } else {
        Err(errors)
    }
}

fn parse_document(source: &Source, value: Value) -> Result<Resource, ConfigError> {
    let Value::Mapping(mut map) = value else {
        return Err(ConfigError::new(
            Some(source),
            "",
            "a document must be a mapping",
        ));
    };
    let api_version = map.remove("apiVersion");
    if api_version.as_ref().and_then(Value::as_str) != Some(API_VERSION) {
        return Err(ConfigError::new(
            Some(source),
            "apiVersion",
            format!("expected `{API_VERSION}`"),
        ));
    }
    let kind = map.remove("kind");
    match kind.as_ref().and_then(Value::as_str) {
        Some(KIND_TENANT) => typed(source, map).map(|d| Resource::Tenant(source.clone(), d)),
        Some(KIND_VAULT) => typed(source, map).map(|d| Resource::Vault(source.clone(), d)),
        _ => Err(ConfigError::new(
            Some(source),
            "kind",
            format!("unknown kind, expected `{KIND_TENANT}` or `{KIND_VAULT}`"),
        )),
    }
}

fn typed<S: DeserializeOwned>(
    source: &Source,
    map: serde_norway::Mapping,
) -> Result<Document<S>, ConfigError> {
    serde_path_to_error::deserialize(Value::Mapping(map)).map_err(|e| {
        let path = e.path().to_string();
        let path = if path == "." { String::new() } else { path };
        ConfigError::new(Some(source), path, e.into_inner().to_string())
    })
}
