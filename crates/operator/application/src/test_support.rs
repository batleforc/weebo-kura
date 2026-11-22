//! An in-memory Kura: every push is recorded and applied, or refused when a
//! document was marked bad or a failure was set.

use std::sync::{Arc, Mutex, PoisonError};

use kura_api::{ConfigError, Source};
use kura_operator_domain::verdict::{ObjectView, PushResult};
use serde_json::{Value, json};

use crate::ports::{GatewayError, KuraGateway};

#[derive(Default)]
struct State {
    pushes: Vec<Vec<Value>>,
    failure: Option<GatewayError>,
    refused: Option<(String, String)>,
}

#[derive(Default, Clone)]
pub struct FakeGateway(Arc<Mutex<State>>);

impl FakeGateway {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Every following push fails with `failure` (`None`: they succeed).
    pub fn fail_with(&self, failure: Option<GatewayError>) {
        self.state().failure = failure;
    }

    /// Every following push is refused, naming `name` with `message`.
    pub fn refuse(&self, name: &str, message: &str) {
        self.state().refused = Some((name.into(), message.into()));
    }

    /// The pushes received, oldest first.
    pub fn pushes(&self) -> Vec<Vec<Value>> {
        self.state().pushes.clone()
    }
}

fn view(doc: &Value) -> Option<ObjectView> {
    let name = kura_api::pushed_name(doc).ok()?;
    let mut parts = name.split('/');
    let kind = parts.next()?.to_string();
    let (namespace, name) = match (parts.next(), parts.next()) {
        (Some(ns), Some(n)) => (Some(ns.to_string()), n.to_string()),
        (Some(n), None) => (None, n.to_string()),
        _ => return None,
    };
    let generation = doc.pointer("/metadata/generation").and_then(Value::as_i64);
    serde_json::from_value(json!({
        "kind": kind, "namespace": namespace, "name": name,
        "conditions": [{ "type": "Ready", "status": "True", "reason": "Reconciled", "message": "", "observedGeneration": generation }]
    }))
    .ok()
}

#[async_trait::async_trait]
impl KuraGateway for FakeGateway {
    async fn push(&self, documents: &[Value]) -> Result<PushResult, GatewayError> {
        let mut state = self.state();
        state.pushes.push(documents.to_vec());
        if let Some(f) = &state.failure {
            return Err(f.clone());
        }
        if let Some((file, message)) = &state.refused {
            return Ok(PushResult {
                applied: false,
                outcome: "rejected".into(),
                errors: vec![ConfigError::new(
                    Some(&Source {
                        file: file.clone(),
                        index: 1,
                    }),
                    "",
                    message.clone(),
                )],
                objects: Vec::new(),
            });
        }
        Ok(PushResult {
            applied: true,
            outcome: "applied".into(),
            errors: Vec::new(),
            objects: documents.iter().filter_map(view).collect(),
        })
    }
}
