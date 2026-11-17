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
    }
}

impl Reason for ReasonCode {
    const SUCCESS: Self = Self::Reconciled;

    fn as_str(self) -> &'static str {
        Self::as_str(self)
    }

    fn severity(self) -> Severity {
        match self {
            Self::InvalidDocument => Severity::Blocking,
            Self::Reconciled
            | Self::Orphaned
            | Self::QuotaBelowUsage
            | Self::CidrTokensWithoutTrustedProxies
            | Self::DeletionScheduled
            | Self::AwaitingKeyDestruction
            | Self::TokenWriteDisabled => Severity::Advisory,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_is_pascal_case_and_complete() {
        weebo_kit_domain::reason::assert_pascal_case(ReasonCode::ALL);
        assert_eq!(ReasonCode::ALL.len(), 8);
        assert_eq!(ReasonCode::SUCCESS.severity(), Severity::Advisory);
        assert_eq!(ReasonCode::InvalidDocument.severity(), Severity::Blocking);
    }
}
