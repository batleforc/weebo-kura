//! `PUT {url}/v1/admin/documents`, bearer = a token of the operator's own
//! ServiceAccount for the instance's audience, minted through the
//! TokenRequest API and renewed before it expires (no secret at rest). Every
//! failure maps onto a [`GatewayError`], which decides the reason code a
//! resource shows.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use k8s_openapi::api::authentication::v1::{TokenRequest, TokenRequestSpec};
use k8s_openapi::api::core::v1::ServiceAccount;
use kube::api::{Api, PostParams};

use kura_api::{KuraInstance, ReasonCode};
use kura_operator_application::ports::{GatewayError, GatewayFactory, KuraGateway};
use kura_operator_domain::Failure;
use kura_operator_domain::verdict::PushResult;
use reqwest::{Client, StatusCode, Url};
use serde_json::{Value, json};

/// A hung server must not hold a push forever.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
/// How long a minted token lives (the TokenRequest's `expirationSeconds`).
pub const TOKEN_TTL: Duration = Duration::from_secs(3600);
/// A token is renewed when less than this is left of it.
pub const TOKEN_RENEW: Duration = Duration::from_secs(600);

pub struct HttpKuraGateway {
    endpoint: Url,
    token: String,
    client: Client,
}

impl HttpKuraGateway {
    /// `url`: the instance's `spec.url`.
    pub fn new(url: &str, token: String, client: Client) -> Result<Self, String> {
        let base = Url::parse(url).map_err(|e| format!("url {url:?}: {e}"))?;
        if base.cannot_be_a_base() || !matches!(base.scheme(), "http" | "https") {
            return Err(format!("url {url:?}: not an http(s) base URL"));
        }
        let endpoint = base
            .join(&format!(
                "{}/v1/admin/documents",
                base.path().trim_end_matches('/')
            ))
            .map_err(|e| format!("url {url:?}: {e}"))?;
        Ok(Self {
            endpoint,
            token,
            client,
        })
    }
}

/// The `detail` of a problem document, else the status line.
async fn detail(response: reqwest::Response) -> String {
    let status = response.status();
    response
        .json::<Value>()
        .await
        .ok()
        .and_then(|p| {
            p.get("detail")
                .or(p.get("type"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| status.to_string())
}

#[async_trait::async_trait]
impl KuraGateway for HttpKuraGateway {
    async fn push(&self, documents: &[Value]) -> Result<PushResult, GatewayError> {
        let response = self
            .client
            .put(self.endpoint.clone())
            .bearer_auth(&self.token)
            .json(&json!({ "documents": documents }))
            .send()
            .await
            .map_err(|e| GatewayError::Unreachable(e.to_string()))?;
        match response.status() {
            StatusCode::OK => response
                .json::<PushResult>()
                .await
                .map_err(|e| GatewayError::Refused(format!("unreadable answer: {e}"))),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                Err(GatewayError::Unauthorized(detail(response).await))
            }
            StatusCode::CONFLICT => Err(GatewayError::SourceIsFiles),
            s if s == StatusCode::REQUEST_TIMEOUT
                || s == StatusCode::TOO_MANY_REQUESTS
                || s.is_server_error() =>
            {
                Err(GatewayError::Unreachable(detail(response).await))
            }
            _ => Err(GatewayError::Refused(detail(response).await)),
        }
    }
}

/// A minted token: for which audience, until when.
struct Minted {
    audience: String,
    token: String,
    renew_at: Instant,
}

/// Builds the gateway of a `KuraInstance`: its URL, its CA (`tls`), a token
/// of the operator's ServiceAccount for its audience.
pub struct HttpGatewayFactory {
    client: kube::Client,
    /// The operator's own ServiceAccount: `(namespace, name)`.
    service_account: (String, String),
    minted: Mutex<Option<Minted>>,
}

impl HttpGatewayFactory {
    pub fn new(client: kube::Client, namespace: String, service_account: String) -> Self {
        Self {
            client,
            service_account: (namespace, service_account),
            minted: Mutex::new(None),
        }
    }

    /// A token for `audience`: the cached one while it has more than
    /// [`TOKEN_RENEW`] left, else a new one.
    async fn token(&self, audience: &str) -> Result<String, Failure> {
        let cached = self
            .minted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .filter(|m| m.audience == audience && Instant::now() < m.renew_at)
            .map(|m| m.token.clone());
        if let Some(token) = cached {
            return Ok(token);
        }
        let (namespace, name) = &self.service_account;
        let refused = |m: String| Failure::new(ReasonCode::InstanceUnauthorized, m);
        let request = TokenRequest {
            spec: Some(TokenRequestSpec {
                audiences: Some(vec![audience.to_owned()]),
                expiration_seconds: Some(i64::try_from(TOKEN_TTL.as_secs()).unwrap_or(3600)),
                bound_object_ref: None,
            }),
            ..TokenRequest::default()
        };
        let answer = Api::<ServiceAccount>::namespaced(self.client.clone(), namespace)
            .create_token_request(name, &PostParams::default(), &request)
            .await
            .map_err(|e| {
                refused(format!(
                    "no token of ServiceAccount {namespace}/{name} for audience {audience:?}: {e}"
                ))
            })?;
        let token = answer
            .status
            .and_then(|s| s.token)
            .filter(|t| !t.is_empty())
            .ok_or_else(|| refused("the TokenRequest answered without a token".into()))?;
        *self.minted.lock().unwrap_or_else(PoisonError::into_inner) = Some(Minted {
            audience: audience.to_owned(),
            token: token.clone(),
            renew_at: Instant::now()
                .checked_add(TOKEN_TTL.saturating_sub(TOKEN_RENEW))
                .unwrap_or_else(Instant::now),
        });
        Ok(token)
    }

    async fn http_client(&self, instance: &KuraInstance) -> Result<Client, Failure> {
        let invalid = |m: String| Failure::new(ReasonCode::InstanceUnreachable, m);
        let mut builder = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none());
        if let Some(tls) = &instance.spec.tls {
            if tls.insecure_skip_verify {
                builder = builder.tls_danger_accept_invalid_certs(true);
            }
            if let Some(secret) = &tls.ca_secret_ref {
                let pem = weebo_kit_adapters::read_secret_key(&self.client, secret)
                    .await
                    .map_err(|e| invalid(format!("tls.caSecretRef: {e}")))?
                    .ok_or_else(|| {
                        invalid(format!(
                            "tls.caSecretRef: secret {}/{} has no key {:?}",
                            secret.namespace, secret.name, secret.key
                        ))
                    })?;
                let certs = reqwest::Certificate::from_pem_bundle(&pem)
                    .map_err(|e| invalid(format!("tls.caSecretRef: {e}")))?;
                builder = builder.tls_certs_merge(certs);
            }
        }
        builder
            .build()
            .map_err(|e| invalid(format!("building the HTTP client: {e}")))
    }
}

#[async_trait::async_trait]
impl GatewayFactory for HttpGatewayFactory {
    async fn gateway_for(&self, instance: &KuraInstance) -> Result<Arc<dyn KuraGateway>, Failure> {
        let client = self.http_client(instance).await?;
        let token = self
            .token(&instance.spec.auth.service_account_token.audience)
            .await?;
        let gateway = HttpKuraGateway::new(&instance.spec.url, token, client)
            .map_err(|e| Failure::new(ReasonCode::InstanceUnreachable, e))?;
        Ok(Arc::new(gateway))
    }
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, StatusCode as Code};
    use axum::routing::put;

    use super::*;

    /// A stand-in kura-server answering with `status` (and recording the bearer).
    async fn server(status: Code, body: Value) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/base/", listener.local_addr().unwrap());
        let app = axum::Router::new().route(
            "/base/v1/admin/documents",
            put(move |h: HeaderMap| {
                let body = body.clone();
                async move {
                    if h.get("authorization")
                        .is_none_or(|v| v != "Bearer projected-token")
                    {
                        return (
                            Code::UNAUTHORIZED,
                            axum::Json(json!({ "type": "/v1/problems/invalid_token" })),
                        );
                    }
                    (status, axum::Json(body))
                }
            }),
        );
        let handle = tokio::spawn(async move {
            drop(axum::serve(listener, app).await);
        });
        (url, handle)
    }

    async fn push(status: Code, body: Value, token: &str) -> Result<PushResult, GatewayError> {
        provider();
        let (url, handle) = server(status, body).await;
        let gw = HttpKuraGateway::new(&url, token.into(), Client::new()).unwrap();
        let r = gw.push(&[json!({})]).await;
        handle.abort();
        r
    }

    #[tokio::test]
    async fn every_answer_maps_to_a_gateway_error() {
        let ok = push(
            Code::OK,
            json!({ "applied": true, "outcome": "applied", "objects": [] }),
            "projected-token",
        )
        .await;
        assert_eq!(ok.unwrap().outcome, "applied");
        assert!(matches!(
            push(Code::OK, json!({}), "someone-else").await,
            Err(GatewayError::Unauthorized(_))
        ));
        let conflict = json!({ "type": "/v1/problems/documents_source_files" });
        assert_eq!(
            push(Code::CONFLICT, conflict, "projected-token").await,
            Err(GatewayError::SourceIsFiles)
        );
        assert!(matches!(
            push(Code::SERVICE_UNAVAILABLE, json!({}), "projected-token").await,
            Err(GatewayError::Unreachable(_))
        ));
        assert!(matches!(
            push(Code::TOO_MANY_REQUESTS, json!({}), "projected-token").await,
            Err(GatewayError::Unreachable(_))
        ));
        let bad = json!({ "type": "/v1/problems/invalid_request", "detail": "documents[0]: kind" });
        assert_eq!(
            push(Code::BAD_REQUEST, bad, "projected-token").await,
            Err(GatewayError::Refused("documents[0]: kind".into()))
        );
        // Nobody listening.
        provider();
        let gw = HttpKuraGateway::new("http://127.0.0.1:1", "x".into(), Client::new()).unwrap();
        assert!(matches!(
            gw.push(&[]).await,
            Err(GatewayError::Unreachable(_))
        ));
    }

    /// kube's rustls and reqwest's are both linked: as in `main`, one provider.
    fn provider() {
        drop(rustls::crypto::ring::default_provider().install_default());
    }

    #[test]
    fn only_http_base_urls_are_taken() {
        provider();
        for bad in ["ftp://kura", "not a url", "mailto:a@b"] {
            assert!(
                HttpKuraGateway::new(bad, "/t".into(), Client::new()).is_err(),
                "{bad}"
            );
        }
        let gw =
            HttpKuraGateway::new("http://kura.kura.svc:8080", "/t".into(), Client::new()).unwrap();
        assert_eq!(
            gw.endpoint.as_str(),
            "http://kura.kura.svc:8080/v1/admin/documents"
        );
    }
}
