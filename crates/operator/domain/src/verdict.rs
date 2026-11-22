//! What each resource shows from the server's answer to a push (RFC 0009
//! §4.2, §4.3): the server's own status when the set was applied, the
//! server's message on the document it refused, and on every other one the
//! name of that document — a refusal is never silent on a resource the user
//! did not touch.

use kura_api::{ConfigError, ReasonCode};
use serde::Deserialize;
use weebo_kit_api::Condition;

use crate::{Advisory, Failure};

/// The answer of `PUT /v1/admin/documents`, as far as the operator reads it.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushResult {
    pub applied: bool,
    /// `applied`, `unchanged` or `rejected`.
    pub outcome: String,
    #[serde(default)]
    pub errors: Vec<ConfigError>,
    /// The stored configuration's status, one entry per object.
    #[serde(default)]
    pub objects: Vec<ObjectView>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectView {
    pub kind: String,
    #[serde(default)]
    pub namespace: Option<String>,
    pub name: String,
    #[serde(default)]
    pub conditions: Vec<Condition>,
}

impl ObjectView {
    fn name(&self) -> String {
        match &self.namespace {
            Some(ns) => format!("{}/{ns}/{}", self.kind, self.name),
            None => format!("{}/{}", self.kind, self.name),
        }
    }
}

/// What one resource shows: `Ready` and the advisories next to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub ready: Result<String, Failure>,
    pub advisories: Vec<Advisory>,
}

impl Verdict {
    pub fn failed(failure: Failure) -> Self {
        Self {
            ready: Err(failure),
            advisories: Vec::new(),
        }
    }
}

fn code(reason: &str) -> Option<ReasonCode> {
    ReasonCode::ALL
        .iter()
        .copied()
        .find(|c| c.as_str() == reason)
}

fn describe(e: &ConfigError) -> String {
    if e.path.is_empty() {
        e.message.clone()
    } else {
        format!("{}: {}", e.path, e.message)
    }
}

/// The verdict of the resource the server knows as `name`
/// (`documents::name_of`).
pub fn verdict(name: &str, result: &PushResult) -> Verdict {
    if result.outcome == "rejected" {
        let own: Vec<String> = result
            .errors
            .iter()
            .filter(|e| e.location.as_ref().is_some_and(|l| l.file == name))
            .map(describe)
            .collect();
        if !own.is_empty() {
            return Verdict::failed(Failure::new(
                ReasonCode::InvalidDocument,
                format!(
                    "{name} {}; the last valid configuration stays in force",
                    own.join("; ")
                ),
            ));
        }
        let mut offenders: Vec<&str> = result
            .errors
            .iter()
            .filter_map(|e| e.location.as_ref().map(|l| l.file.as_str()))
            .collect();
        offenders.sort_unstable();
        offenders.dedup();
        let whole: Vec<String> = result
            .errors
            .iter()
            .filter(|e| e.location.is_none())
            .map(describe)
            .collect();
        let why = match (offenders.is_empty(), whole.is_empty()) {
            (false, _) => format!("{} was refused", offenders.join(", ")),
            (true, false) => whole.join("; "),
            (true, true) => "the server gave no reason".into(),
        };
        return Verdict::failed(Failure::new(
            ReasonCode::SetRejected,
            format!("the set was refused: {why}; the last valid configuration stays in force"),
        ));
    }
    let Some(object) = result.objects.iter().find(|o| o.name() == name) else {
        // Applied or unchanged without this object: a newer kura-server
        // version wrote last, and this one did not write (RFC 0001 §6.10).
        return Verdict::failed(Failure::new(
            ReasonCode::SetRejected,
            format!("kura-server did not apply the set and reports no status for {name}"),
        ));
    };
    let advisories = object
        .conditions
        .iter()
        .filter(|c| c.type_ != Condition::READY && c.status == "True")
        .filter_map(|c| {
            code(&c.reason).map(|reason| Advisory {
                reason,
                message: c.message.clone(),
            })
        })
        .collect();
    let ready = match weebo_kit_api::status::ready(&object.conditions) {
        Some(c) if c.status == "True" => Ok(if c.message.is_empty() {
            "applied by kura-server".into()
        } else {
            c.message.clone()
        }),
        Some(c) => Err(Failure::new(
            code(&c.reason).unwrap_or(ReasonCode::InvalidDocument),
            c.message.clone(),
        )),
        None => Err(Failure::new(
            ReasonCode::SetRejected,
            format!("kura-server reports no Ready condition for {name}"),
        )),
    };
    Verdict { ready, advisories }
}

#[cfg(test)]
mod tests {
    use kura_api::Source;
    use serde_json::json;

    use super::*;

    fn result(value: serde_json::Value) -> PushResult {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn an_applied_set_shows_the_servers_status_and_advisories() {
        let r = result(json!({
            "applied": true, "outcome": "applied",
            "objects": [{
                "kind": "KuraVault", "namespace": "maxime", "name": "notes",
                "conditions": [
                    { "type": "Ready", "status": "True", "reason": "Reconciled", "message": "" },
                    { "type": "RestrictedBindingWidened", "status": "True", "reason": "RestrictedBindingWidened", "message": "g is owner" }
                ]
            }, {
                "kind": "KuraTenant", "name": "old",
                "conditions": [{ "type": "Ready", "status": "False", "reason": "Orphaned", "message": "removed" }]
            }]
        }));
        let v = verdict("KuraVault/maxime/notes", &r);
        assert!(v.ready.is_ok());
        assert_eq!(v.advisories[0].reason, ReasonCode::RestrictedBindingWidened);
        let gone = verdict("KuraTenant/old", &r);
        assert_eq!(gone.ready.unwrap_err().reason, ReasonCode::Orphaned);
        // Not in the stored status: the set was not written.
        let missing = verdict("KuraVault/maxime/other", &r);
        assert_eq!(missing.ready.unwrap_err().reason, ReasonCode::SetRejected);
    }

    #[test]
    fn a_refused_set_names_the_offender_on_every_other_resource() {
        let r = PushResult {
            applied: false,
            outcome: "rejected".into(),
            errors: vec![
                ConfigError::new(
                    Some(&Source {
                        file: "KuraVault/family/recipes".into(),
                        index: 1,
                    }),
                    "spec.chunking",
                    "min 64 to 1Mi",
                ),
                ConfigError::new(None, "", "more than one tenant removed at once"),
            ],
            objects: Vec::new(),
        };
        let offender = verdict("KuraVault/family/recipes", &r).ready.unwrap_err();
        assert_eq!(offender.reason, ReasonCode::InvalidDocument);
        assert!(
            offender.message.contains("spec.chunking: min 64 to 1Mi"),
            "{}",
            offender.message
        );
        let other = verdict("KuraTenant/maxime", &r).ready.unwrap_err();
        assert_eq!(other.reason, ReasonCode::SetRejected);
        assert!(
            other
                .message
                .contains("KuraVault/family/recipes was refused"),
            "{}",
            other.message
        );

        let whole = PushResult {
            errors: vec![ConfigError::new(None, "", "no KuraTenant is declared")],
            ..r
        };
        let v = verdict("KuraVault/family/recipes", &whole)
            .ready
            .unwrap_err();
        assert!(
            v.message.contains("no KuraTenant is declared"),
            "{}",
            v.message
        );
    }
}
