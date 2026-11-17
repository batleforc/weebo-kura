//! The web console's files under `/app` (RFC 0003 §6.1), when `ui.enabled`:
//! static files from `ui.dir`, `index.html` for client routes, and
//! `/app/config.json`, which tells the page where to sign in. Every answer
//! carries the console's Content-Security-Policy; `/` redirects to `/app/`.

use axum::Json;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use serde::Serialize;

use super::SharedState;

/// What the console needs from `server.yaml`.
#[derive(Debug, Clone)]
pub struct UiSettings {
    pub dir: std::path::PathBuf,
    pub client_id: String,
    /// `oidc.issuer`, to check the tokens it gets.
    pub issuer: String,
    /// Where the discovery document is (`oidc.discovery`, else the issuer).
    pub discovery: String,
    /// The Content-Security-Policy of every `/app` answer, built once.
    csp: HeaderValue,
}

impl UiSettings {
    /// `None` when the policy cannot be a header (a control character in a
    /// configured URL): the console is then not served.
    pub fn new(
        dir: std::path::PathBuf,
        client_id: String,
        issuer: String,
        discovery: Option<String>,
    ) -> Option<Self> {
        let discovery = discovery.unwrap_or_else(|| issuer.clone());
        // The page calls the token endpoint itself (decision 13): only the
        // identity provider's origin is added to `connect-src`.
        let idp = origin(&discovery)?;
        // RFC 0005 §6.5: the note frame inherits this policy. It may run its
        // one bridge script, by hash, and show images inlined as `data:`.
        let bridge = frame_hash(&dir)
            .map(|h| format!(" {h}"))
            .unwrap_or_default();
        let csp = HeaderValue::from_str(&format!(
            "default-src 'none'; script-src 'self'{bridge}; style-src 'self'; img-src 'self' blob: data:; \
             font-src 'self'; connect-src 'self' {idp}; frame-ancestors 'none'; base-uri 'none'; \
             form-action 'none'; worker-src 'self'; manifest-src 'self'"
        ))
        .ok()?;
        Some(Self {
            dir,
            client_id,
            issuer,
            discovery,
            csp,
        })
    }
}

/// The bridge's `'sha256-…'` from `frame-csp.txt`, written by the build;
/// anything else in that file is ignored, so it can never widen the policy.
fn frame_hash(dir: &std::path::Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("frame-csp.txt")).ok()?;
    let hash = text.trim();
    let digest = hash.strip_prefix("'sha256-")?.strip_suffix('\'')?;
    let base64 = digest.len() == 44
        && digest.ends_with('=')
        && digest
            .trim_end_matches('=')
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/');
    base64.then(|| hash.to_owned())
}

/// `scheme://host[:port]` of a URL, the form a CSP source takes.
fn origin(url: &str) -> Option<String> {
    let uri: axum::http::Uri = url.parse().ok()?;
    Some(format!("{}://{}", uri.scheme_str()?, uri.authority()?))
}

/// `/app/config.json`: public, the same for everyone.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConsoleConfig {
    issuer: String,
    discovery: String,
    client_id: String,
}

async fn config(State(state): State<SharedState>) -> Response {
    match &state.ui {
        Some(ui) => Json(ConsoleConfig {
            issuer: ui.issuer.clone(),
            discovery: ui.discovery.clone(),
            client_id: ui.client_id.clone(),
        })
        .into_response(),
        None => axum::http::StatusCode::NOT_FOUND.into_response(),
    }
}

/// The console's security headers, and its caching: hashed assets forever,
/// the rest (the shell, `config.json`, the service worker) revalidated.
async fn headers(State(state): State<SharedState>, request: Request, next: Next) -> Response {
    let immutable = request.uri().path().starts_with("/app/assets/");
    let mut response = next.run(request).await;
    let h = response.headers_mut();
    if let Some(ui) = &state.ui {
        h.insert(header::CONTENT_SECURITY_POLICY, ui.csp.clone());
    }
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(if immutable {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        }),
    );
    response
}

/// `/app` and `/`, or nothing at all when the console is off.
pub fn router(state: &SharedState) -> axum::Router<SharedState> {
    let Some(ui) = &state.ui else {
        return axum::Router::new();
    };
    let index = ui.dir.join("index.html");
    let files = tower_http::services::ServeDir::new(&ui.dir)
        .fallback(tower_http::services::ServeFile::new(index));
    // A missing hashed asset is a 404, never the shell: served as the asset
    // and marked immutable, the shell would be kept for a year by the
    // browser and the service worker (code review of 2026-11-17).
    let assets = tower_http::services::ServeDir::new(ui.dir.join("assets"));
    axum::Router::new()
        .route("/app/config.json", get(config))
        .nest_service("/app/assets", assets)
        .nest_service("/app", files)
        .route_layer(middleware::from_fn_with_state(state.clone(), headers))
        .route("/", get(|| async { Redirect::to("/app/") }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_policy_names_the_identity_provider_only() {
        let ui = UiSettings::new(
            "/x".into(),
            "kura-web".into(),
            "https://auth.example.org/".into(),
            Some("https://auth.example.org/application/o/kura-web/".into()),
        )
        .unwrap();
        let csp = ui.csp.to_str().unwrap();
        assert!(
            csp.contains("connect-src 'self' https://auth.example.org;"),
            "{csp}"
        );
        assert!(csp.contains("script-src 'self';") && csp.contains("frame-ancestors 'none'"));
        assert!(!csp.contains("unsafe"));
        assert!(csp.contains("img-src 'self' blob: data:;"));
        assert!(
            csp.contains("script-src 'self';"),
            "no bridge file, no hash: {csp}"
        );
    }

    #[test]
    fn the_bridge_hash_is_read_strictly() {
        let dir = std::env::temp_dir().join(format!("kura-frame-csp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let good = "'sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU='";
        for (text, want) in [
            (format!("{good}\n"), Some(good)),
            ("'unsafe-inline'".to_owned(), None),
            (format!("{good}; script-src *"), None),
            ("'sha256-short='".to_owned(), None),
        ] {
            std::fs::write(dir.join("frame-csp.txt"), &text).unwrap();
            assert_eq!(frame_hash(&dir).as_deref(), want, "{text}");
        }
        let ui =
            UiSettings::new(dir.clone(), "c".into(), "https://a.example/".into(), None).unwrap();
        std::fs::write(dir.join("frame-csp.txt"), good).unwrap();
        let ui2 = UiSettings::new(dir, "c".into(), "https://a.example/".into(), None).unwrap();
        assert!(!ui.csp.to_str().unwrap().contains("sha256"));
        assert!(
            ui2.csp
                .to_str()
                .unwrap()
                .contains(&format!("script-src 'self' {good};"))
        );
        assert_eq!(
            origin("https://a.example:8443/p?q").as_deref(),
            Some("https://a.example:8443")
        );
        assert_eq!(origin("not a url"), None);
    }
}
