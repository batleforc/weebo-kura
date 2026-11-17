//! The HTTP application: one route list for the server and the OpenAPI
//! document (weebo-api-contract), probes outside `/v1`, the client version
//! gate, and request tracing by route template.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, RwLock};

use axum::Json;
use axum::extract::{MatchedPath, Request, State};
use axum::http::{HeaderValue, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{OpenApi, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::auth::Verifier;
use crate::metrics::Metrics;
use crate::problem::{ApiError, Problem, ProblemType};
use crate::reconcile::snapshot::Snapshot;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "kura",
        description = "weebo-kura sync backend (RFC 0001)",
        contact(name = "weebo-kura", url = "https://github.com/batleforc/weebo-kura"),
    ),
    tags(
        (name = "probes", description = "Process health, no data"),
        (name = "admin", description = "Platform administration"),
        (name = "devices", description = "Device registration and revocation"),
        (name = "me", description = "The signed-in caller"),
        (name = "notifications", description = "A tenant's webhook rules and their deliveries"),
        (name = "vaults", description = "Vaults and their role bindings"),
        (name = "chunks", description = "Encrypted chunks and file manifests"),
        (name = "sync", description = "Commits and changes"),
        (name = "history", description = "Revisions and restore"),
        (name = "tokens", description = "Scoped tokens for non-interactive readers and writers"),
        (name = "search", description = "Opt-in full-text search"),
    ),
    components(schemas(Problem)),
    modifiers(&SecurityAddon),
)]
pub struct ApiDoc;

/// The OIDC access token (or, from phase 7, a scoped token).
struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "bearer_token",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .description(Some(
                        "OIDC access token from Authentik, verified per RFC8725: pinned algorithms, \
                         issuer, audience, expiry and the `scope` claim",
                    ))
                    .build(),
            ),
        );
        // A WebSocket cannot carry an Authorization header from a web view:
        // the socket presents a single-use ticket instead (decision 36).
        components.add_security_scheme(
            "ws_ticket",
            SecurityScheme::ApiKey(utoipa::openapi::security::ApiKey::Query(
                utoipa::openapi::security::ApiKeyValue::with_description(
                    "ticket",
                    "From POST /vaults/{v}/events/ticket: 256 random bits, single use, 30 s",
                ),
            )),
        );
    }
}

/// What every handler shares.
pub struct AppState {
    /// Set once a valid configuration is loaded (decision 31).
    ready: AtomicBool,
    pub metrics: Metrics,
    pub db: sqlx::PgPool,
    pub verifier: Verifier,
    pub limits: crate::limits::RateLimiter,
    pub http: HttpSettings,
    /// Object storage and data keys; `None` until configured (503 on chunk routes).
    pub storage: Option<Arc<crate::store::Storage>>,
    /// This replica's WebSocket fan-out (§6.3).
    pub hub: crate::notify::Hub,
    /// `chunks/batch` answers this replica builds at once (RFC 0002 §7.5).
    pub batches: tokio::sync::Semaphore,
    /// The web console, when `ui.enabled` (RFC 0003 §6.1).
    pub ui: Option<ui::UiSettings>,
    /// Outbound notifications, when `notifications.enabled` (RFC 0003 §6.4).
    pub notifier: Option<Arc<crate::notifier::Notifier>>,
    snapshot: RwLock<Option<Arc<Snapshot>>>,
    /// This replica's last rejected configuration, for `GET /v1/admin/status`.
    rejection: Mutex<Option<Vec<kura_api::ConfigError>>>,
}

impl AppState {
    pub fn new(metrics: Metrics, db: sqlx::PgPool, verifier: Verifier) -> Self {
        let hub = crate::notify::Hub::counting(metrics.ws_connections.clone());
        Self {
            ready: AtomicBool::new(false),
            metrics,
            db,
            verifier,
            limits: crate::limits::RateLimiter::default(),
            http: HttpSettings::default(),
            storage: None,
            ui: None,
            notifier: None,
            hub,
            batches: tokio::sync::Semaphore::new(chunks::BATCHES_IN_FLIGHT),
            snapshot: RwLock::new(None),
            rejection: Mutex::new(None),
        }
    }

    pub fn set_ready(&self, ready: bool) {
        self.ready.store(ready, Ordering::Release);
        self.metrics.ready.set(i64::from(ready));
    }

    /// The reconciled configuration authorization reads.
    pub fn snapshot(&self) -> Option<Arc<Snapshot>> {
        self.snapshot
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn set_snapshot(&self, snapshot: Snapshot) {
        let warnings = i64::try_from(snapshot.status.warnings()).unwrap_or(i64::MAX);
        self.metrics.config_warnings.set(warnings);
        *self
            .snapshot
            .write()
            .unwrap_or_else(PoisonError::into_inner) = Some(Arc::new(snapshot));
        self.set_ready(true);
    }

    pub fn rejection(&self) -> Option<Vec<kura_api::ConfigError>> {
        self.rejection
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn set_rejection(&self, errors: Option<Vec<kura_api::ConfigError>>) {
        *self
            .rejection
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = errors;
    }

    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }
}

pub type SharedState = Arc<AppState>;

/// The `server.yaml` settings the HTTP layer reads.
#[derive(Debug, Clone)]
pub struct HttpSettings {
    pub allowed_origins: Vec<String>,
    pub trusted_proxies: Vec<ipnet::IpNet>,
}

impl Default for HttpSettings {
    fn default() -> Self {
        Self {
            allowed_origins: crate::config::CorsConfig::default().allowed_origins,
            trusted_proxies: Vec::new(),
        }
    }
}

impl AppState {
    pub fn with_notifier(mut self, n: crate::notifier::Notifier) -> Self {
        self.notifier = Some(Arc::new(n));
        self
    }

    pub fn with_ui(mut self, ui: ui::UiSettings) -> Self {
        self.ui = Some(ui);
        self
    }

    pub fn with_http(mut self, http: HttpSettings) -> Self {
        self.http = http;
        self
    }

    pub fn with_storage(mut self, storage: Arc<crate::store::Storage>) -> Self {
        self.storage = Some(storage);
        self
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ProbeStatus {
    pub status: String,
}

/// Liveness: the process answers.
#[utoipa::path(get, path = "/healthz", tag = "probes", responses((status = 200, description = "The process is alive", body = ProbeStatus)))]
async fn healthz() -> Json<ProbeStatus> {
    Json(ProbeStatus {
        status: "ok".into(),
    })
}

/// Readiness: configuration loaded and port bound; never a shared dependency (§10.2).
#[utoipa::path(
    get,
    path = "/readyz",
    tag = "probes",
    responses(
        (status = 200, description = "Configuration loaded, serving", body = ProbeStatus),
        (status = 503, description = "No valid configuration yet", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn readyz(State(state): State<SharedState>) -> Result<Json<ProbeStatus>, ApiError> {
    if state.is_ready() {
        Ok(Json(ProbeStatus {
            status: "ready".into(),
        }))
    } else {
        Err(ApiError::of(ProblemType::Unavailable))
    }
}

/// `/v1`: every API route.
fn v1_router() -> OpenApiRouter<SharedState> {
    OpenApiRouter::new()
        .routes(routes!(admin::admin_status))
        .routes(routes!(devices::register_device, devices::list_devices))
        .routes(routes!(devices::revoke_device))
        .routes(routes!(devices::sign_out_device))
        .routes(routes!(me::me))
        .routes(routes!(
            notifications::list_rules,
            notifications::create_rule
        ))
        .routes(routes!(
            notifications::update_rule,
            notifications::delete_rule
        ))
        .routes(routes!(notifications::test_rule))
        .routes(routes!(notifications::list_deliveries))
        .routes(routes!(vaults::list_vaults, vaults::create_vault))
        .routes(routes!(vaults::delete_vault))
        .routes(routes!(vaults::list_bindings))
        .routes(routes!(vaults::put_binding, vaults::delete_binding))
        .routes(routes!(changes::get_changes))
        .routes(routes!(changes::get_entries))
        .routes(routes!(commits::get_revisions))
        .routes(routes!(commits::restore_revision))
        .routes(routes!(tokens::create_token, tokens::list_tokens))
        .routes(routes!(tokens::revoke_token))
        .routes(routes!(search::search))
        .routes(routes!(events::create_ticket))
        .routes(routes!(events::events))
        .merge(
            // 1,000 hashes (about 67 KB of JSON) and a large file's manifest
            // do not fit in 64 KiB either (found by the first-attach run).
            OpenApiRouter::new()
                .routes(routes!(chunks::missing_chunks))
                .routes(routes!(chunks::put_blob, chunks::get_blob))
                .layer(axum::extract::DefaultBodyLimit::max(
                    commits::MAX_COMMIT_BYTES,
                )),
        )
        .merge(
            // RFC 0002: 1,000 hashes of JSON, about 67 KB.
            OpenApiRouter::new()
                .routes(routes!(chunks::batch_blobs))
                .routes(routes!(chunks::batch_chunks))
                .layer(axum::extract::DefaultBodyLimit::max(chunks::MAX_BATCH_BODY)),
        )
        .merge(
            // 1,000 operations do not fit in 64 KiB.
            OpenApiRouter::new()
                .routes(routes!(commits::post_commit))
                .layer(axum::extract::DefaultBodyLimit::max(
                    commits::MAX_COMMIT_BYTES,
                )),
        )
        .merge(
            // Chunk bodies up to 4 MiB; every other route keeps 64 KiB.
            OpenApiRouter::new()
                .routes(routes!(chunks::put_chunk, chunks::get_chunk))
                .layer(axum::extract::DefaultBodyLimit::max(
                    chunks::MAX_CHUNK_BYTES,
                )),
        )
}

/// Every route, and the OpenAPI document they describe.
pub fn app_router() -> (axum::Router<SharedState>, utoipa::openapi::OpenApi) {
    let mut doc = ApiDoc::openapi();
    env!("CARGO_PKG_VERSION").clone_into(&mut doc.info.version);
    OpenApiRouter::with_openapi(doc)
        .routes(routes!(healthz))
        .routes(routes!(readyz))
        .nest(
            "/v1",
            v1_router()
                .layer(middleware::from_fn(client_version))
                .layer(axum::extract::DefaultBodyLimit::max(MAX_BODY_BYTES)),
        )
        .split_for_parts()
}

/// The spec alone, for `dump-spec` and the contract test.
pub fn openapi_spec() -> utoipa::openapi::OpenApi {
    app_router().1
}

/// Largest request body phase 1 accepts; chunk uploads get their own limit (phase 2).
pub const MAX_BODY_BYTES: usize = 64 << 10;

/// The finished application, with `/scalar`, CORS, the denial audit and
/// request tracing.
pub fn app(state: SharedState) -> axum::Router {
    let (router, spec) = app_router();
    let docs = axum::Router::<SharedState>::from(crate::scalar::scalar(spec))
        .layer(middleware::from_fn(docs_csp));
    router
        .merge(docs)
        .merge(ui::router(&state))
        .layer(middleware::from_fn_with_state(state.clone(), audit_denials))
        .layer(cors(&state.http))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(request_span)
                // A 5xx is logged once, with its cause, where it is made (ApiError);
                // the layer's own ERROR line would repeat it, and flag every
                // /readyz probe before the first configuration.
                .on_failure(
                    tower_http::trace::DefaultOnFailure::new().level(tracing::Level::DEBUG),
                ),
        )
        .with_state(state)
}

/// The origins of `cors.allowedOrigins` only, no credentials: the API takes
/// bearer tokens, never cookies (§4.1).
fn cors(http: &HttpSettings) -> tower_http::cors::CorsLayer {
    use axum::http::{HeaderName, Method};
    let origins: Vec<HeaderValue> = http
        .allowed_origins
        .iter()
        .filter_map(|o| HeaderValue::from_str(o).ok())
        .collect();
    tower_http::cors::CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            HeaderName::from_static(crate::app::device::DEVICE_HEADER),
            HeaderName::from_static(CLIENT_HEADER),
        ])
        .max_age(std::time::Duration::from_secs(600))
}

/// Every refusal is an audit event and a `kura_authz_denied_total` count,
/// once, here (§10.1, T10). It names the route template, the client address
/// and the claimed device, never the path or the token.
async fn audit_denials(State(state): State<SharedState>, request: Request, next: Next) -> Response {
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or("unmatched", MatchedPath::as_str)
        .to_owned();
    let method = request.method().clone();
    let peer = request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|c| c.0.ip());
    let client =
        peer.map(|p| crate::net::client_addr(p, request.headers(), &state.http.trusted_proxies));
    let device = request
        .headers()
        .get(crate::app::device::DEVICE_HEADER)
        .and_then(|h| h.to_str().ok())
        .map(str::to_owned);
    let response = next.run(request).await;
    match response.extensions().get::<ProblemType>() {
        Some(t @ (ProblemType::CursorExpired | ProblemType::HistoryRewound)) => {
            state
                .metrics
                .resync_total
                .with_label_values(&[t.name()])
                .inc();
        }
        Some(ProblemType::Unavailable) => state.metrics.unavailable_total.inc(),
        _ => {}
    }
    if let Some(t) = response
        .extensions()
        .get::<ProblemType>()
        .filter(|t| t.is_denial())
    {
        state
            .metrics
            .authz_denied_total
            .with_label_values(&[t.name()])
            .inc();
        tracing::warn!(
            target: "audit",
            event = "authz_denied",
            problem = t.name(),
            %method,
            route,
            client = ?client,
            claimed_device = ?device,
            "request refused"
        );
    }
    response
}

/// One span per request, named by its route template, never by the path:
/// paths carry vault names and note paths (T19).
pub fn request_span(request: &Request) -> tracing::Span {
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or("unmatched", MatchedPath::as_str);
    tracing::debug_span!("request", method = %request.method(), route)
}

async fn docs_csp(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(crate::scalar::API_DOCS_CSP),
    );
    response
}

/// Oldest client version the server still serves, per client name (§13).
/// A client absent from this table, or sending no header, is not gated.
const MIN_CLIENT_VERSIONS: &[(&str, (u64, u64, u64))] =
    &[("kura-cli", (0, 1, 0)), ("kura-plugin", (0, 1, 0))];

pub const CLIENT_HEADER: &str = "x-kura-client";

fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    // Pre-release and build suffixes compare as their release.
    let core = v.split(['-', '+']).next()?;
    let mut parts = core.split('.').map(str::parse::<u64>);
    let version = (
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
    );
    parts.next().is_none().then_some(version)
}

/// `true` when the header names a known client below its minimum, or a known
/// client with a version that does not parse.
pub fn client_too_old(header: &str) -> bool {
    let Some((name, version)) = header.split_once('/') else {
        return false;
    };
    let Some((_, min)) = MIN_CLIENT_VERSIONS.iter().find(|(n, _)| *n == name) else {
        return false;
    };
    parse_version(version).is_none_or(|v| v < *min)
}

async fn client_version(request: Request, next: Next) -> Response {
    let too_old = request
        .headers()
        .get(CLIENT_HEADER)
        .is_some_and(|h| h.to_str().map_or(true, client_too_old));
    if too_old {
        return ApiError::of(ProblemType::ClientTooOld).into_response();
    }
    next.run(request).await
}

pub mod admin;
pub mod body;
pub mod caller;
pub mod changes;
pub mod chunks;
pub mod commits;
pub mod device;
pub mod devices;
pub mod events;
pub mod me;
pub mod notifications;
pub mod reader;
pub mod search;
pub mod space;
pub mod tokens;
pub mod ui;
pub mod vaults;

#[cfg(test)]
mod tests;
