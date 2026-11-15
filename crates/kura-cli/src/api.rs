//! The kura API and the identity provider, over HTTPS (RFC 0001 §4.4, §6.2).

use std::time::Duration;

use kura_core::ChunkHash;
use kura_core::sync::{Remote, RemoteError};
use kura_core::wire::{ChangesResponse, CommitRequest, CommitResponse};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use uuid::Uuid;

pub type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

pub const CLIENT: &str = concat!("kura-cli/", env!("CARGO_PKG_VERSION"));

pub fn http() -> Result<reqwest::Client, RemoteError> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(CLIENT)
        .build()
        .map_err(|e| RemoteError::Unavailable(e.to_string()))
}

/// A JSON answer and a chunk, at most: a hostile server cannot exhaust memory.
const MAX_JSON: usize = 8 << 20;
const MAX_CHUNK: usize = kura_core::chunk::MAX_CHUNK;

/// The body, refused past `limit` bytes as it streams.
async fn body(mut r: reqwest::Response, limit: usize) -> Result<Vec<u8>, RemoteError> {
    if r.content_length().is_some_and(|l| l > limit as u64) {
        return Err(RemoteError::Unavailable("response too large".into()));
    }
    let mut out = Vec::new();
    while let Some(part) = r.chunk().await.map_err(unreachable)? {
        if out.len().saturating_add(part.len()) > limit {
            return Err(RemoteError::Unavailable("response too large".into()));
        }
        out.extend_from_slice(&part);
    }
    Ok(out)
}

fn unreachable(e: reqwest::Error) -> RemoteError {
    RemoteError::Unavailable(e.without_url().to_string())
}

/// A `4xx`/`5xx` as the problem document it carries.
async fn check(r: reqwest::Response) -> Result<reqwest::Response, RemoteError> {
    let status = r.status();
    if status.is_success() {
        return Ok(r);
    }
    if status.is_server_error() && status != reqwest::StatusCode::SERVICE_UNAVAILABLE {
        return Err(RemoteError::Unavailable(format!("server error {status}")));
    }
    let v: Value = body(r, MAX_JSON)
        .await
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Value::Null);
    Err(RemoteError::Problem {
        status: status.as_u16(),
        kind: v
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .trim_start_matches("/v1/problems/")
            .to_owned(),
        detail: v.get("detail").and_then(Value::as_str).map(str::to_owned),
    })
}

/// Attempts per request when the server asks to wait (§4.4, §6.3).
const ATTEMPTS: usize = 6;

/// Sends, honouring `Retry-After` on `429` and `503` (capped at 60 s, a few
/// attempts), then maps a refusal to its problem document.
async fn exchange(r: reqwest::RequestBuilder) -> Result<reqwest::Response, RemoteError> {
    for _ in 1..ATTEMPTS {
        let Some(attempt) = r.try_clone() else { break };
        let resp = attempt.send().await.map_err(unreachable)?;
        let status = resp.status();
        let wait = matches!(status.as_u16(), 429 | 503).then(|| {
            resp.headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(1)
                .clamp(1, 60)
        });
        match wait {
            Some(secs) => tokio::time::sleep(Duration::from_secs(secs)).await,
            None => return check(resp).await,
        }
    }
    check(r.send().await.map_err(unreachable)?).await
}

/// A scoped token as the server shows it; `token` only when just issued.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenView {
    pub id: Uuid,
    pub prefix: String,
    pub cidrs: Vec<String>,
    pub expires_at: String,
    pub revoked: bool,
    #[serde(default)]
    pub token: Option<String>,
}

/// One search result.
#[derive(Debug, Clone, Deserialize)]
pub struct SearchHit {
    pub path: String,
    pub score: f32,
    #[serde(default)]
    pub snippet: Option<String>,
}

/// The current access token, replaced when it is renewed.
#[derive(Clone, Default)]
pub struct Token(std::sync::Arc<std::sync::Mutex<String>>);

impl Token {
    pub fn new(t: String) -> Self {
        Self(std::sync::Arc::new(std::sync::Mutex::new(t)))
    }

    pub fn get(&self) -> String {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn set(&self, t: String) {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = t;
    }
}

/// Where a refused access token is renewed from: the stored refresh token
/// (§6.2: refresh once, then sign in). Absent with `KURA_ACCESS_TOKEN`.
#[derive(Clone)]
pub struct Renew {
    pub issuer: String,
    pub client_id: String,
    pub home: std::path::PathBuf,
}

/// The server, as one signed-in device.
#[derive(Clone)]
pub struct Api {
    pub http: reqwest::Client,
    pub server: String,
    pub token: Token,
    pub device: Option<Uuid>,
    pub renew: Option<Renew>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultView {
    pub id: Uuid,
    pub name: String,
    pub role: String,
    pub chunking: Chunking,
    /// The `.obsidian` categories that sync (§6.8); empty: none.
    #[serde(default)]
    pub config_sync: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Chunking {
    pub min: u64,
    pub avg: u64,
    pub max: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    pub id: Uuid,
    pub name: String,
    pub profile: String,
    pub last_seen: String,
    pub revoked: bool,
    #[serde(default)]
    pub owner: Option<String>,
}

impl Api {
    fn req(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let mut r = self
            .http
            .request(
                method,
                format!("{}/v1{path}", self.server.trim_end_matches('/')),
            )
            .header("x-kura-client", CLIENT);
        if let Some(d) = self.device {
            r = r.header("x-kura-device", d.to_string());
        }
        r
    }

    /// Sends with the current token; an access token that expired during a
    /// long run (an attach of thousands of files outlives it) is renewed
    /// once and the request sent again.
    async fn exchange(&self, r: reqwest::RequestBuilder) -> Result<reqwest::Response, RemoteError> {
        let again = r.try_clone();
        match exchange(r.bearer_auth(self.token.get())).await {
            Err(e) if e.kind() == "invalid_token" && self.renew.is_some() => {
                let Some(again) = again else { return Err(e) };
                let fresh = self.renew().await?;
                exchange(again.bearer_auth(fresh)).await
            }
            other => other,
        }
    }

    async fn renew(&self) -> Result<String, RemoteError> {
        let signed_out = |detail: String| RemoteError::Problem {
            status: 401,
            kind: "invalid_token".into(),
            detail: Some(detail),
        };
        let Some(rn) = &self.renew else {
            return Err(signed_out("no refresh token".into()));
        };
        let refresh = crate::config::refresh_token(&rn.home)
            .map_err(signed_out)?
            .ok_or_else(|| signed_out("no refresh token".into()))?;
        let d = oidc::discover(&self.http, &rn.issuer)
            .await
            .map_err(|e| signed_out(e.to_string()))?;
        let t = oidc::refresh(&self.http, &d, &rn.client_id, &refresh)
            .await
            .map_err(|e| signed_out(e.to_string()))?;
        // The rotated refresh token is stored before the new access token is used.
        if let Some(r) = &t.refresh_token {
            crate::config::set_refresh_token(&rn.home, Some(r)).map_err(signed_out)?;
        }
        self.token.set(t.access_token.clone());
        Ok(t.access_token)
    }

    async fn json<T: DeserializeOwned>(
        &self,
        r: reqwest::RequestBuilder,
    ) -> Result<T, RemoteError> {
        let r = self.exchange(r).await?;
        let b = body(r, MAX_JSON).await?;
        serde_json::from_slice(&b)
            .map_err(|e| RemoteError::Unavailable(format!("unexpected answer: {e}")))
    }

    pub async fn vaults(&self) -> Result<Vec<VaultView>, RemoteError> {
        self.json(self.req(reqwest::Method::GET, "/vaults")).await
    }

    pub async fn devices(&self) -> Result<Vec<DeviceView>, RemoteError> {
        self.json(self.req(reqwest::Method::GET, "/devices")).await
    }

    pub async fn register(&self, name: &str, profile: &str) -> Result<DeviceView, RemoteError> {
        let r = self
            .req(reqwest::Method::POST, "/devices")
            .json(&json!({ "name": name, "profile": profile }));
        self.json(r).await
    }

    pub async fn revoke(&self, id: Uuid) -> Result<(), RemoteError> {
        let r = self.req(reqwest::Method::DELETE, &format!("/devices/{id}"));
        self.exchange(r).await.map(drop)
    }

    pub async fn entries(
        &self,
        vault: Uuid,
        after: Option<&str>,
    ) -> Result<ChangesResponse, RemoteError> {
        let mut r = self.req(reqwest::Method::GET, &format!("/vaults/{vault}/entries"));
        if let Some(a) = after {
            r = r.query(&[("after", a)]);
        }
        self.json(r).await
    }

    /// An entry's revisions, newest first, every page (§4.4).
    pub async fn revisions(
        &self,
        vault: Uuid,
        entry: Uuid,
    ) -> Result<Vec<kura_core::wire::RevisionInfo>, RemoteError> {
        let mut out = Vec::new();
        let mut before: Option<u64> = None;
        loop {
            let mut r = self.req(
                reqwest::Method::GET,
                &format!("/vaults/{vault}/entries/{entry}/revisions"),
            );
            if let Some(b) = before {
                r = r.query(&[("before", b.to_string())]);
            }
            let page: kura_core::wire::RevisionsResponse = self.json(r).await?;
            out.extend(page.items);
            match page.next {
                Some(n) if before.is_none_or(|b| n < b) => before = Some(n),
                _ => return Ok(out),
            }
        }
    }

    /// Restores a revision as the entry's new head (§6.9).
    pub async fn restore(
        &self,
        vault: Uuid,
        entry: Uuid,
        revision: Uuid,
    ) -> Result<kura_core::wire::OpResult, RemoteError> {
        let r = self
            .req(
                reqwest::Method::POST,
                &format!("/vaults/{vault}/entries/{entry}/restore"),
            )
            .json(&json!({ "revision": revision }));
        self.json(r).await
    }

    /// Full-text and file-name search; the query travels in the body (T19).
    pub async fn search(&self, vault: Uuid, query: &str) -> Result<Vec<SearchHit>, RemoteError> {
        #[derive(Deserialize)]
        struct Found {
            results: Vec<SearchHit>,
        }
        let r = self
            .req(reqwest::Method::POST, &format!("/vaults/{vault}/search"))
            .json(&json!({ "query": query }));
        Ok(self.json::<Found>(r).await?.results)
    }

    /// Issues a scoped token; the answer holds its secret, once.
    pub async fn create_token(
        &self,
        vault: Uuid,
        prefix: &str,
        days: u32,
        cidrs: &[String],
    ) -> Result<TokenView, RemoteError> {
        let r = self
            .req(reqwest::Method::POST, &format!("/vaults/{vault}/tokens"))
            .json(&json!({ "prefix": prefix, "expiresInDays": days, "cidrs": cidrs }));
        self.json(r).await
    }

    pub async fn tokens(&self, vault: Uuid) -> Result<Vec<TokenView>, RemoteError> {
        self.json(self.req(reqwest::Method::GET, &format!("/vaults/{vault}/tokens")))
            .await
    }

    pub async fn revoke_token(&self, vault: Uuid, id: Uuid) -> Result<(), RemoteError> {
        let r = self.req(
            reqwest::Method::DELETE,
            &format!("/vaults/{vault}/tokens/{id}"),
        );
        self.exchange(r).await.map(drop)
    }

    /// A single-use ticket for the vault's notifications (§6.3).
    pub async fn ticket(&self, vault: Uuid) -> Result<String, RemoteError> {
        #[derive(Deserialize)]
        struct Ticket {
            ticket: String,
        }
        let r = self.req(
            reqwest::Method::POST,
            &format!("/vaults/{vault}/events/ticket"),
        );
        Ok(self.json::<Ticket>(r).await?.ticket)
    }

    /// The notifications socket of a vault, opened with a ticket.
    pub async fn events(&self, vault: Uuid) -> Result<Socket, RemoteError> {
        let ticket = self.ticket(vault).await?;
        let base = self.server.trim_end_matches('/');
        let ws = match (base.strip_prefix("https://"), base.strip_prefix("http://")) {
            (Some(rest), _) => format!("wss://{rest}"),
            (_, Some(rest)) => format!("ws://{rest}"),
            _ => base.to_owned(),
        };
        let url = format!("{ws}/v1/vaults/{vault}/events?ticket={ticket}");
        let connect = tokio_tungstenite::connect_async(url);
        match tokio::time::timeout(Duration::from_secs(30), connect).await {
            Ok(Ok((socket, _))) => Ok(socket),
            Ok(Err(e)) => Err(RemoteError::Unavailable(e.to_string())),
            Err(_) => Err(RemoteError::Unavailable(
                "notifications: no answer within 30 s".into(),
            )),
        }
    }

    pub fn vault(&self, vault: Uuid) -> VaultRemote<'_> {
        VaultRemote {
            api: self,
            vault,
            space: None,
        }
    }

    /// The caller's own Obsidian configuration of the vault (§6.8).
    pub fn vault_config(&self, vault: Uuid) -> VaultRemote<'_> {
        VaultRemote {
            api: self,
            vault,
            space: Some("config"),
        }
    }
}

/// One vault of the server, for the sync engine.
pub struct VaultRemote<'a> {
    api: &'a Api,
    vault: Uuid,
    /// `config`, or the notes.
    space: Option<&'static str>,
}

impl VaultRemote<'_> {
    fn req(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.api
            .req(method, &format!("/vaults/{}{path}", self.vault))
    }
}

/// A `chunks/batch` answer: 4 MiB of chunks and a header per record.
const MAX_RECORDS: usize = MAX_CHUNK + 1000 * kura_core::wire::RECORD_HEADER;

/// A server without the batch routes answers `405` (RFC 0002 decision 2).
fn older_server(e: &RemoteError) -> bool {
    matches!(e, RemoteError::Problem { status: 405, .. })
}

impl Remote for VaultRemote<'_> {
    async fn missing(&self, hashes: &[ChunkHash]) -> Result<Vec<ChunkHash>, RemoteError> {
        #[derive(Deserialize)]
        struct Missing {
            missing: Vec<ChunkHash>,
        }
        let r = self
            .req(reqwest::Method::POST, "/chunks/missing")
            .json(&json!({ "hashes": hashes }));
        Ok(self.api.json::<Missing>(r).await?.missing)
    }

    async fn put_chunk(&self, hash: &ChunkHash, data: &[u8]) -> Result<(), RemoteError> {
        let r = self
            .req(reqwest::Method::PUT, &format!("/chunks/{hash}"))
            .header("content-type", "application/octet-stream")
            .body(data.to_vec());
        self.api.exchange(r).await.map(drop)
    }

    async fn get_chunk(&self, hash: &ChunkHash) -> Result<Vec<u8>, RemoteError> {
        let r = self
            .api
            .exchange(self.req(reqwest::Method::GET, &format!("/chunks/{hash}")))
            .await?;
        body(r, MAX_CHUNK).await
    }

    async fn put_blob(
        &self,
        hash: &ChunkHash,
        size: u64,
        chunks: &[ChunkHash],
    ) -> Result<(), RemoteError> {
        let r = self
            .req(reqwest::Method::PUT, &format!("/blobs/{hash}"))
            .json(&json!({ "size": size, "chunks": chunks }));
        self.api.exchange(r).await.map(drop)
    }

    async fn get_blob(&self, hash: &ChunkHash) -> Result<(u64, Vec<ChunkHash>), RemoteError> {
        #[derive(Deserialize)]
        struct Manifest {
            size: u64,
            chunks: Vec<ChunkHash>,
        }
        let m: Manifest = self
            .api
            .json(self.req(reqwest::Method::GET, &format!("/blobs/{hash}")))
            .await?;
        Ok((m.size, m.chunks))
    }

    async fn get_blobs(
        &self,
        hashes: &[ChunkHash],
    ) -> Result<Option<Vec<(ChunkHash, u64, Vec<ChunkHash>)>>, RemoteError> {
        #[derive(Deserialize)]
        struct Manifest {
            hash: ChunkHash,
            size: u64,
            chunks: Vec<ChunkHash>,
        }
        #[derive(Deserialize)]
        struct Batch {
            manifests: Vec<Manifest>,
        }
        let r = self
            .req(reqwest::Method::POST, "/blobs/batch")
            .json(&json!({ "blobs": hashes }));
        match self.api.json::<Batch>(r).await {
            Ok(b) => Ok(Some(
                b.manifests
                    .into_iter()
                    .map(|m| (m.hash, m.size, m.chunks))
                    .collect(),
            )),
            Err(e) if older_server(&e) => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn get_chunk_records(
        &self,
        hashes: &[ChunkHash],
    ) -> Result<Option<Vec<u8>>, RemoteError> {
        let r = self
            .req(reqwest::Method::POST, "/chunks/batch")
            .json(&json!({ "chunks": hashes }));
        match self.api.exchange(r).await {
            Ok(r) => body(r, MAX_RECORDS).await.map(Some),
            Err(e) if older_server(&e) => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn commit(&self, req: &CommitRequest) -> Result<CommitResponse, RemoteError> {
        let mut req = req.clone();
        req.space = self.space.map(str::to_owned);
        self.api
            .json(self.req(reqwest::Method::POST, "/commits").json(&req))
            .await
    }

    async fn changes(
        &self,
        since: u64,
        epoch: Option<Uuid>,
        start: u64,
    ) -> Result<ChangesResponse, RemoteError> {
        let mut r = self
            .req(reqwest::Method::GET, "/changes")
            .query(&[("since", since.to_string()), ("start", start.to_string())]);
        if let Some(e) = epoch {
            r = r.query(&[("epoch", e.to_string())]);
        }
        if let Some(s) = self.space {
            r = r.query(&[("space", s)]);
        }
        self.api.json(r).await
    }
}

/// The device authorization grant (RFC 8628) against the issuer.
pub mod oidc {
    use super::{Deserialize, Duration, Value};

    #[derive(Debug, Deserialize)]
    pub struct Discovery {
        pub token_endpoint: String,
        pub device_authorization_endpoint: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct DeviceCode {
        pub device_code: String,
        pub user_code: String,
        pub verification_uri: String,
        #[serde(default)]
        pub verification_uri_complete: Option<String>,
        pub expires_in: u64,
        #[serde(default = "five")]
        pub interval: u64,
    }

    const fn five() -> u64 {
        5
    }

    #[derive(Debug, Deserialize)]
    pub struct Tokens {
        pub access_token: String,
        #[serde(default)]
        pub refresh_token: Option<String>,
    }

    #[derive(Debug, thiserror::Error)]
    pub enum OidcError {
        #[error("the code was not approved within {0} seconds. Run `kura login` again.")]
        Expired(u64),
        #[error("the sign-in was refused. Run `kura login` to try again.")]
        Denied,
        #[error("signed out: run `kura login`")]
        InvalidGrant,
        #[error("cannot reach the identity provider: {0}")]
        Unavailable(String),
    }

    fn unavailable(e: impl std::fmt::Display) -> OidcError {
        OidcError::Unavailable(e.to_string())
    }

    pub async fn discover(http: &reqwest::Client, issuer: &str) -> Result<Discovery, OidcError> {
        let url = format!(
            "{}/.well-known/openid-configuration",
            issuer.trim_end_matches('/')
        );
        let r = http
            .get(url)
            .send()
            .await
            .map_err(unavailable)?
            .error_for_status()
            .map_err(unavailable)?;
        r.json().await.map_err(unavailable)
    }

    pub async fn start(
        http: &reqwest::Client,
        d: &Discovery,
        client_id: &str,
    ) -> Result<DeviceCode, OidcError> {
        let r = http
            .post(&d.device_authorization_endpoint)
            .form(&[
                ("client_id", client_id),
                ("scope", "openid profile email offline_access"),
            ])
            .send()
            .await
            .map_err(unavailable)?
            .error_for_status()
            .map_err(unavailable)?;
        r.json().await.map_err(unavailable)
    }

    async fn token(
        http: &reqwest::Client,
        d: &Discovery,
        form: &[(&str, &str)],
    ) -> Result<Result<Tokens, String>, OidcError> {
        let r = http
            .post(&d.token_endpoint)
            .form(form)
            .send()
            .await
            .map_err(unavailable)?;
        let ok = r.status().is_success();
        let v: Value = r.json().await.map_err(unavailable)?;
        if ok {
            return serde_json::from_value(v).map(Ok).map_err(unavailable);
        }
        Ok(Err(v
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned()))
    }

    /// Polls until the user approves, at the interval the provider asks for.
    pub async fn wait(
        http: &reqwest::Client,
        d: &Discovery,
        client_id: &str,
        code: &DeviceCode,
    ) -> Result<Tokens, OidcError> {
        let mut interval = code.interval.max(1);
        let deadline = tokio::time::Instant::now()
            .checked_add(Duration::from_secs(code.expires_in))
            .unwrap_or_else(tokio::time::Instant::now);
        loop {
            tokio::time::sleep(Duration::from_secs(interval)).await;
            if tokio::time::Instant::now() > deadline {
                return Err(OidcError::Expired(code.expires_in));
            }
            let form = [
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", code.device_code.as_str()),
                ("client_id", client_id),
            ];
            match token(http, d, &form).await? {
                Ok(t) => return Ok(t),
                Err(e) if e == "authorization_pending" => {}
                Err(e) if e == "slow_down" => interval = interval.saturating_add(5),
                Err(e) if e == "expired_token" => return Err(OidcError::Expired(code.expires_in)),
                Err(_) => return Err(OidcError::Denied),
            }
        }
    }

    /// A new access token; the provider rotates the refresh token (§6.2).
    pub async fn refresh(
        http: &reqwest::Client,
        d: &Discovery,
        client_id: &str,
        refresh_token: &str,
    ) -> Result<Tokens, OidcError> {
        let form = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", client_id),
        ];
        match token(http, d, &form).await? {
            Ok(t) => Ok(t),
            Err(e) if e == "invalid_grant" => Err(OidcError::InvalidGrant),
            Err(e) => Err(OidcError::Unavailable(e)),
        }
    }

    /// `preferred_username`, read for display only, never trusted.
    pub fn username(access_token: &str) -> Option<String> {
        use base64::Engine as _;
        let payload = access_token.split('.').nth(1)?;
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload)
            .ok()?;
        let v: Value = serde_json::from_slice(&bytes).ok()?;
        v.get("preferred_username")
            .and_then(Value::as_str)
            .map(str::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 0002 decision 2: a `405` (an older server's answer to the batch
    /// routes) means "read one by one"; a problem `404` is still an error.
    #[tokio::test]
    async fn a_405_is_an_older_server_and_a_404_is_not() {
        let answer = |status: u16, body: &'static str| {
            reqwest::Response::from(
                http::Response::builder()
                    .status(status)
                    .body(body)
                    .unwrap_or_default(),
            )
        };
        let e = check(answer(405, "")).await.err();
        assert!(e.as_ref().is_some_and(older_server), "{e:?}");
        let e = check(answer(404, r#"{"type":"/v1/problems/not_found"}"#))
            .await
            .err();
        assert!(
            e.as_ref()
                .is_some_and(|e| !older_server(e) && e.kind() == "not_found"),
            "{e:?}"
        );
    }
}
