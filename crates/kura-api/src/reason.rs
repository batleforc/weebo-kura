//! Kura's reason-code catalogue (RFC 0001 §4.3): every status condition,
//! metric label and log of the reconciler carries one of these.

use weebo_kit_domain::{Reason, Severity};

weebo_kit_domain::reason_codes! {
    pub enum ReasonCode {
        /// `Ready: True`.
        Reconciled,
        /// A document failed a rule of §4.3; the last valid configuration stays.
        InvalidDocument,
        /// Removed from the files with `Orphan` (or force-orphan): disabled, data kept.
        Orphaned,
        /// Reads continue, writes are refused.
        QuotaBelowUsage,
        /// Scoped tokens bound to a CIDR are refused while `trustedProxies` is empty.
        CidrTokensWithoutTrustedProxies,
        /// Removed with `deletionPolicy: Delete`; purged after 30 days unless it comes back.
        DeletionScheduled,
        /// A purged tenant's Transit key still exists.
        AwaitingKeyDestruction,
        /// Write tokens exist while `scopedTokens.write` is off: they read,
        /// their writes are refused (RFC 0004 §4.3).
        TokenWriteDisabled,
        /// A viewer binding lists folders while its claim value also opens
        /// the whole vault: the widest grant wins (RFC 0005 §4.3).
        RestrictedBindingWidened,
        /// Another document of the pushed set was refused, so the whole set
        /// was; the message names it (RFC 0009 §4.2).
        SetRejected,
        /// kura-server reads its documents from files (`documents.source:
        /// files`): the operator's pushes are refused.
        SourceIsFiles,
        /// A `KuraVault` in a namespace its tenant's `vaultNamespaces` does
        /// not list (refused at admission).
        VaultNamespaceNotAllowed,
        /// A `KuraVault` whose `tenantRef` names no `KuraTenant`.
        TenantNotFound,
        /// A `KuraVault` naming a vault created through the API: never taken over.
        VaultManagedByApi,
        /// The Kura server did not answer (network error, timeout, 5xx).
        InstanceUnreachable,
        /// The Kura server refused the operator's identity (401/403).
        InstanceUnauthorized,
    }
}

impl Reason for ReasonCode {
    const SUCCESS: Self = Self::Reconciled;

    fn as_str(self) -> &'static str {
        Self::as_str(self)
    }

    fn severity(self) -> Severity {
        match self {
            Self::InvalidDocument
            | Self::SetRejected
            | Self::SourceIsFiles
            | Self::VaultNamespaceNotAllowed
            | Self::TenantNotFound
            | Self::VaultManagedByApi
            | Self::InstanceUnreachable
            | Self::InstanceUnauthorized => Severity::Blocking,
            Self::Reconciled
            | Self::Orphaned
            | Self::QuotaBelowUsage
            | Self::CidrTokensWithoutTrustedProxies
            | Self::DeletionScheduled
            | Self::AwaitingKeyDestruction
            | Self::TokenWriteDisabled
            | Self::RestrictedBindingWidened => Severity::Advisory,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_is_pascal_case_and_complete() {
        weebo_kit_domain::reason::assert_pascal_case(ReasonCode::ALL);
        assert_eq!(ReasonCode::ALL.len(), 16);
        assert_eq!(ReasonCode::SUCCESS.severity(), Severity::Advisory);
        assert_eq!(ReasonCode::InvalidDocument.severity(), Severity::Blocking);
    }
}
