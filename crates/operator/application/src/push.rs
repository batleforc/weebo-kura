//! Push the set, then judge each resource (RFC 0009 §4.2): the instance is
//! `Ready` when the server took the push, whatever it thought of the set.

use kura_api::{KuraInstance, ReasonCode};
use kura_operator_domain::Failure;
use kura_operator_domain::documents::Document;
use kura_operator_domain::verdict::{self, Verdict};

use crate::ports::{GatewayFactory, KuraGateway};

/// What a push ended on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// The `KuraInstance`'s own verdict.
    pub instance: Verdict,
    /// One verdict per document, by the name the server knows it by.
    pub resources: Vec<(String, Verdict)>,
    /// `applied`, `unchanged`, `rejected`, or the failure's reason.
    pub summary: String,
}

impl Outcome {
    fn all_failed(documents: &[Document], failure: &Failure) -> Self {
        Self {
            instance: Verdict::failed(failure.clone()),
            resources: documents
                .iter()
                .map(|d| (d.name.clone(), Verdict::failed(failure.clone())))
                .collect(),
            summary: failure.reason.as_str().into(),
        }
    }

    /// The reason the reconcile metric is keyed by.
    pub fn reason(&self) -> ReasonCode {
        match &self.instance.ready {
            Ok(_) if self.summary == "rejected" => ReasonCode::SetRejected,
            Ok(_) => ReasonCode::Reconciled,
            Err(f) => f.reason,
        }
    }
}

/// Pushes `documents` to the one `instance`.
pub async fn push(
    gateways: &dyn GatewayFactory,
    instance: &KuraInstance,
    documents: &[Document],
) -> Outcome {
    let gateway = match gateways.gateway_for(instance).await {
        Ok(g) => g,
        Err(failure) => return Outcome::all_failed(documents, &failure),
    };
    push_with(gateway.as_ref(), documents).await
}

/// [`push`] through a gateway already built.
pub async fn push_with(gateway: &dyn KuraGateway, documents: &[Document]) -> Outcome {
    let bodies: Vec<serde_json::Value> = documents.iter().map(|d| d.body.clone()).collect();
    match gateway.push(&bodies).await {
        Err(e) => Outcome::all_failed(documents, &e.failure()),
        Ok(result) => Outcome {
            instance: Verdict {
                ready: Ok(format!(
                    "kura-server answers: the last push was {}",
                    result.outcome
                )),
                advisories: Vec::new(),
            },
            resources: documents
                .iter()
                .map(|d| (d.name.clone(), verdict::verdict(&d.name, &result)))
                .collect(),
            summary: result.outcome,
        },
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::ports::GatewayError;
    use crate::test_support::FakeGateway;

    fn doc(name: &str) -> Document {
        Document {
            name: name.into(),
            body: json!({ "kind": "KuraTenant", "metadata": { "name": name } }),
        }
    }

    #[tokio::test]
    async fn every_resource_gets_the_gateways_failure() {
        let gateway = FakeGateway::default();
        gateway.fail_with(Some(GatewayError::SourceIsFiles));
        let o = push_with(&gateway, &[doc("KuraTenant/a"), doc("KuraTenant/b")]).await;
        assert_eq!(o.reason(), ReasonCode::SourceIsFiles);
        for (_, v) in &o.resources {
            assert_eq!(
                v.ready.as_ref().unwrap_err().reason,
                ReasonCode::SourceIsFiles
            );
        }
        gateway.fail_with(Some(GatewayError::Unauthorized("403".into())));
        let o = push_with(&gateway, &[doc("KuraTenant/a")]).await;
        assert_eq!(o.reason(), ReasonCode::InstanceUnauthorized);
    }

    #[tokio::test]
    async fn a_refused_set_leaves_the_instance_ready() {
        let gateway = FakeGateway::default();
        gateway.refuse("KuraTenant/b", "spec.quota: bad");
        let o = push_with(&gateway, &[doc("KuraTenant/a"), doc("KuraTenant/b")]).await;
        assert!(o.instance.ready.is_ok());
        assert_eq!(o.reason(), ReasonCode::SetRejected);
        assert_eq!(
            o.resources[1].1.ready.as_ref().unwrap_err().reason,
            ReasonCode::InvalidDocument
        );
        assert_eq!(
            o.resources[0].1.ready.as_ref().unwrap_err().reason,
            ReasonCode::SetRejected
        );
        assert_eq!(gateway.pushes().len(), 1);
    }
}
