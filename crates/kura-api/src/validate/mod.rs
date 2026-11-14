//! `validate()`: every rule of RFC 0001 §4.3 that concerns resource
//! documents. The `oidc` row is about `server.yaml` and is checked by the
//! server's own loader. Three rules need the stored state, not the files
//! alone, so the previous reconciled state is an input; the future operator's
//! webhook passes the cluster's.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::names::TenantSlug;
use crate::parse::{ConfigError, Resource, Source};
use crate::spec::{Chunking, KuraTenant, KuraVault, VaultKey};

/// Who manages an object (RFC 0001 §6.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ManagedBy {
    Config,
    Api,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredVault {
    /// The chunking the vault was created with.
    pub chunking: Chunking,
    pub managed_by: ManagedBy,
}

/// What the last reconciliation stored, as far as validation needs it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PreviousState {
    /// Tenants currently declared by the files (not the disabled ones).
    pub tenants: BTreeSet<TenantSlug>,
    pub vaults: BTreeMap<VaultKey, StoredVault>,
}

/// A validated document and where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sourced<T> {
    pub source: Source,
    pub doc: T,
}

/// A configuration every rule accepted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    pub tenants: BTreeMap<TenantSlug, Sourced<KuraTenant>>,
    pub vaults: BTreeMap<VaultKey, Sourced<KuraVault>>,
}

/// Checks a whole set of documents. On any error the caller keeps its
/// previous configuration (§4.3); every error found is returned.
pub fn validate(
    resources: Vec<Resource>,
    previous: &PreviousState,
) -> Result<Config, Vec<ConfigError>> {
    let mut errors = Vec::new();
    let mut config = Config::default();

    for resource in resources {
        match resource {
            Resource::Tenant(source, doc) => {
                let name = doc.metadata.name.clone();
                if let Some(first) = config.tenants.get(&name) {
                    errors.push(err(
                        &source,
                        "metadata.name",
                        format!("tenant `{name}` is already declared at {}", first.source),
                    ));
                } else {
                    config.tenants.insert(name, Sourced { source, doc });
                }
            }
            Resource::Vault(source, doc) => {
                let Some(namespace) = doc.metadata.namespace.clone() else {
                    errors.push(err(
                        &source,
                        "metadata.namespace",
                        "a KuraVault must have a namespace",
                    ));
                    continue;
                };
                let key = VaultKey {
                    namespace,
                    name: doc.metadata.name.clone(),
                };
                if let Some(first) = config.vaults.get(&key) {
                    errors.push(err(
                        &source,
                        "metadata.name",
                        format!("vault `{key}` is already declared at {}", first.source),
                    ));
                } else {
                    config.vaults.insert(key, Sourced { source, doc });
                }
            }
        }
    }

    check_tenants(&config, &mut errors);
    check_vaults(&config, previous, &mut errors);
    check_removals(&config, previous, &mut errors);

    if errors.is_empty() {
        Ok(config)
    } else {
        Err(errors)
    }
}

fn err(source: &Source, path: &str, message: impl Into<String>) -> ConfigError {
    ConfigError::new(Some(source), path, message)
}

fn check_tenants(config: &Config, errors: &mut Vec<ConfigError>) {
    // (claim, value) -> first tenant mapping it: one group must not grant two tenants.
    let mut owners: BTreeMap<(&str, &str), &TenantSlug> = BTreeMap::new();
    for (slug, t) in &config.tenants {
        let Some(m) = &t.doc.spec.claim_mappings else {
            continue;
        };
        let roles = &m.roles;
        let all = [
            ("owner", &roles.owner),
            ("admin", &roles.admin),
            ("member", &roles.member),
        ];
        for (role, values) in all {
            for value in values {
                let path = format!("spec.claimMappings.roles.{role}");
                if value.is_empty() {
                    errors.push(err(&t.source, &path, "a claim value cannot be empty"));
                    continue;
                }
                match owners.get(&(m.claim.as_str(), value.as_str())) {
                    Some(other) if *other != slug => errors.push(err(
                        &t.source,
                        &path,
                        format!("claim value `{value}` is already mapped by tenant `{other}`"),
                    )),
                    _ => {
                        owners.insert((m.claim.as_str(), value.as_str()), slug);
                    }
                }
            }
        }
    }
}

fn check_vaults(config: &Config, previous: &PreviousState, errors: &mut Vec<ConfigError>) {
    for (key, v) in &config.vaults {
        let spec = &v.doc.spec;
        match config.tenants.get(spec.tenant_ref.name.as_str()) {
            None => errors.push(err(
                &v.source,
                "spec.tenantRef.name",
                format!("no tenant `{}` is declared", spec.tenant_ref.name),
            )),
            Some(t) if !t.doc.spec.vault_namespaces.contains(&key.namespace) => errors.push(err(
                &v.source,
                "metadata.namespace",
                format!(
                    "namespace `{}` is not in the vaultNamespaces of tenant `{}`",
                    key.namespace, spec.tenant_ref.name
                ),
            )),
            Some(_) => {}
        }
        if let Some(c) = &spec.chunking {
            if !(c.min < c.avg && c.avg < c.max) {
                errors.push(err(&v.source, "spec.chunking", "expected min < avg < max"));
            }
            if c.max > Chunking::MAX_CHUNK {
                errors.push(err(
                    &v.source,
                    "spec.chunking.max",
                    format!("at most {}", Chunking::MAX_CHUNK),
                ));
            }
            // The bounds content-defined chunking accepts (FastCDC 2020, plan D37).
            let (min, avg, max) = (c.min.bytes(), c.avg.bytes(), c.max.bytes());
            let in_bounds =
                (64..=1 << 20).contains(&min) && (256..=4 << 20).contains(&avg) && max >= 1024;
            if !in_bounds || min % 2 != 0 || avg % 2 != 0 || max % 2 != 0 {
                errors.push(err(
                    &v.source,
                    "spec.chunking",
                    "min 64 to 1Mi, avg 256 to 4Mi, max at least 1Ki, all even",
                ));
            }
        }
        if let Some(stored) = previous.vaults.get(key) {
            if stored.managed_by == ManagedBy::Api {
                errors.push(err(
                    &v.source,
                    "metadata.name",
                    format!("vault `{key}` exists and is managed through the API"),
                ));
            }
            if spec.chunking.is_some_and(|c| c != stored.chunking) {
                errors.push(err(
                    &v.source,
                    "spec.chunking",
                    "differs from the value the vault was created with",
                ));
            }
        }
        for (i, b) in spec.bindings.iter().enumerate() {
            let path = format!("spec.bindings[{i}]");
            match (&b.claim_value, &b.subject) {
                (Some(s), None) | (None, Some(s)) if !s.is_empty() => {}
                _ => errors.push(err(
                    &v.source,
                    &path,
                    "set exactly one of claimValue or subject, not empty",
                )),
            }
        }
    }
}

fn check_removals(config: &Config, previous: &PreviousState, errors: &mut Vec<ConfigError>) {
    if config.tenants.is_empty() {
        errors.push(ConfigError::new(
            None,
            "",
            "no KuraTenant is declared: an empty directory is never read as a removal",
        ));
        return;
    }
    let removed: Vec<&str> = previous
        .tenants
        .iter()
        .filter(|t| !config.tenants.contains_key(*t))
        .map(|t| t.as_str())
        .collect();
    if removed.len() > 1 {
        errors.push(ConfigError::new(
            None,
            "",
            format!(
                "the change removes {} tenants ({}); remove them one at a time",
                removed.len(),
                removed.join(", ")
            ),
        ));
    }
}

#[cfg(test)]
mod tests;
