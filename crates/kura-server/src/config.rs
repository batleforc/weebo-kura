//! `server.yaml`: the deployment's own settings, rendered from the Helm
//! values (RFC 0001 §4.1). Never a custom resource.

use std::net::SocketAddr;
use std::path::Path;

use ipnet::IpNet;
use serde::Deserialize;

/// Signature algorithms a token may use. `none` has no variant, so it can
/// never be configured (RFC 0001 §6.2, T1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Algorithm {
    RS256,
    RS384,
    RS512,
    PS256,
    PS384,
    PS512,
    ES256,
    ES384,
    EdDSA,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServerConfig {
    pub oidc: OidcConfig,
    /// Absent or empty: forwarding headers are ignored and the peer address is used.
    #[serde(default)]
    pub trusted_proxies: Vec<IpNet>,
    #[serde(default)]
    pub cors: CorsConfig,
    /// Read from phase 2 on (credentials from Vault KV); accepted now so the
    /// Helm chart can render one file for every phase.
    #[serde(default)]
    pub vault: Option<VaultConfig>,
    #[serde(default)]
    pub listen: ListenConfig,
    #[serde(default)]
    pub maintenance: MaintenanceConfig,
    /// The web console (RFC 0003 §4.1); absent: `/app` answers 404.
    #[serde(default)]
    pub ui: Option<UiConfig>,
    /// Webhooks (RFC 0003 §4.1); absent: off, rule routes answer 409.
    #[serde(default)]
    pub notifications: NotificationsConfig,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NotificationsConfig {
    /// Off by default: run the delivery worker and accept rules.
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub webhook: WebhookConfig,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebhookConfig {
    /// Private ranges a webhook may reach; empty: public addresses only.
    #[serde(default)]
    pub allowed_cidrs: Vec<IpNet>,
    /// An extra trust root, for webhooks only (a private CA).
    #[serde(default)]
    pub ca_file: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiConfig {
    /// Off by default: an HTML surface on the API origin is opt-in.
    #[serde(default)]
    pub enabled: bool,
    /// Where the built application is (the image's default).
    #[serde(default = "default_ui_dir")]
    pub dir: std::path::PathBuf,
    pub oidc: UiOidc,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiOidc {
    /// The console's public client; also one of `oidc.audiences`.
    pub client_id: String,
    /// The console's own discovery document. With Authentik it must be the
    /// `kura-web` application's: Authentik answers a browser's cross-origin
    /// request with the origins of that application's redirect URIs only.
    /// Absent: `oidc.discovery`, else the issuer.
    #[serde(default)]
    pub discovery: Option<String>,
}

fn default_ui_dir() -> std::path::PathBuf {
    "/usr/share/kura/ui".into()
}

/// Retention, garbage collection and the `Delete` purge (§6.9).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MaintenanceConfig {
    /// Off-peak window, `HH:MM-HH:MM` in UTC; `01:00-05:00` by default, and
    /// equal ends mean all day.
    #[serde(default)]
    pub window: crate::maintenance::Window,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OidcConfig {
    pub issuer: String,
    /// Where the discovery document lives when it is not under `issuer`:
    /// with Authentik's global issuer mode, one issuer serves every client
    /// type and each application keeps its own discovery URL (plan D52).
    #[serde(default)]
    pub discovery: Option<String>,
    /// One `client_id` per client type.
    pub audiences: Vec<String>,
    #[serde(default = "default_algorithms")]
    pub algorithms: Vec<Algorithm>,
    pub platform_admin: ClaimMatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaimMatch {
    pub claim: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CorsConfig {
    pub allowed_origins: Vec<String>,
}

/// The three origins the Obsidian plugin calls from (RFC 0001 §4.1). Not
/// empty: the plugin cannot work without them, and CORS is not the access
/// control here (bearer tokens only, never ambient credentials).
impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: [
                "app://obsidian.md",
                "capacitor://localhost",
                "http://localhost",
            ]
            .map(String::from)
            .to_vec(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VaultConfig {
    pub address: String,
    pub kubernetes_auth: VaultKubernetesAuth,
    pub secrets: VaultSecrets,
    /// The Transit mount holding one key per tenant (§6.6).
    #[serde(default)]
    pub transit: TransitConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransitConfig {
    pub mount: String,
}

impl Default for TransitConfig {
    fn default() -> Self {
        Self {
            mount: "transit".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VaultKubernetesAuth {
    pub mount: String,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VaultSecrets {
    pub database: String,
    pub bucket: String,
}

/// The API and the metrics listen on separate ports; the ingress exposes only
/// the first (RFC 0001 §4.4).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListenConfig {
    pub api: SocketAddr,
    pub metrics: SocketAddr,
}

impl Default for ListenConfig {
    fn default() -> Self {
        Self {
            api: SocketAddr::from(([0, 0, 0, 0], 8080)),
            metrics: SocketAddr::from(([0, 0, 0, 0], 9090)),
        }
    }
}

fn default_algorithms() -> Vec<Algorithm> {
    vec![Algorithm::RS256]
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("{file} {path}: {message}")]
    Invalid {
        file: String,
        path: String,
        message: String,
    },
}

impl ServerConfig {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.display().to_string(),
            source,
        })?;
        Self::parse(&path.display().to_string(), &content)
    }

    /// Parses and checks the `oidc` row of RFC 0001 §4.3: the issuer is an
    /// HTTPS URL, at least one audience, at least one algorithm.
    pub fn parse(file: &str, content: &str) -> Result<Self, ConfigError> {
        let invalid = |path: &str, message: String| ConfigError::Invalid {
            file: file.to_owned(),
            path: path.to_owned(),
            message,
        };
        let de = serde_norway::Deserializer::from_str(content);
        let config: Self = serde_path_to_error::deserialize(de)
            .map_err(|e| invalid(&e.path().to_string(), e.into_inner().to_string()))?;
        let issuer: Result<axum::http::Uri, _> = config.oidc.issuer.parse();
        let https = issuer.is_ok_and(|u| u.scheme_str() == Some("https") && u.host().is_some());
        if !https {
            return Err(invalid("oidc.issuer", "must be an https:// URL".into()));
        }
        let discovery_https = config.oidc.discovery.as_deref().is_none_or(|d| {
            d.parse::<axum::http::Uri>()
                .is_ok_and(|u| u.scheme_str() == Some("https") && u.host().is_some())
        });
        if !discovery_https {
            return Err(invalid("oidc.discovery", "must be an https:// URL".into()));
        }
        if config.oidc.audiences.is_empty() || config.oidc.audiences.iter().any(String::is_empty) {
            return Err(invalid(
                "oidc.audiences",
                "at least one non-empty audience".into(),
            ));
        }
        if config.oidc.algorithms.is_empty() {
            return Err(invalid("oidc.algorithms", "at least one algorithm".into()));
        }
        // The console's tokens are verified like any client's (RFC 0003 §4.1).
        // A range covering everything would disable the guard (§4.3).
        if config
            .notifications
            .webhook
            .allowed_cidrs
            .iter()
            .any(|n| n.prefix_len() == 0)
        {
            return Err(invalid(
                "notifications.webhook.allowedCidrs",
                "0.0.0.0/0 and ::/0 would allow every address".into(),
            ));
        }
        if let Some(ui) = config.ui.as_ref().filter(|u| u.enabled) {
            if !config.oidc.audiences.contains(&ui.oidc.client_id) {
                return Err(invalid(
                    "ui.oidc.clientId",
                    format!("{} must also be in oidc.audiences", ui.oidc.client_id),
                ));
            }
            let https = ui.oidc.discovery.as_deref().is_none_or(|d| {
                d.parse::<axum::http::Uri>()
                    .is_ok_and(|u| u.scheme_str() == Some("https") && u.host().is_some())
            });
            if !https {
                return Err(invalid(
                    "ui.oidc.discovery",
                    "must be an https:// URL".into(),
                ));
            }
        }
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = r#"
oidc:
  issuer: https://auth.example.org/application/o/kura/
  audiences: [kura-plugin, kura-cli]
  algorithms: [RS256]
  platformAdmin: { claim: groups, value: kura-platform-admins }
trustedProxies: [10.42.0.0/16]
cors:
  allowedOrigins: [app://obsidian.md, capacitor://localhost, http://localhost]
vault:
  address: https://vault.vault:8200
  kubernetesAuth: { mount: kubernetes, role: kura }
  secrets: { database: mv/data/kura/database, bucket: mv/data/kura/bucket }
"#;

    fn err(content: &str) -> String {
        ServerConfig::parse("server.yaml", content)
            .unwrap_err()
            .to_string()
    }

    #[test]
    fn the_rfc_example_loads() {
        let c = ServerConfig::parse("server.yaml", EXAMPLE).unwrap();
        assert_eq!(
            c.trusted_proxies,
            vec!["10.42.0.0/16".parse::<IpNet>().unwrap()]
        );
        assert_eq!(c.cors, CorsConfig::default());
        assert_eq!(c.listen, ListenConfig::default());
        assert_eq!(c.oidc.algorithms, vec![Algorithm::RS256]);
    }

    #[test]
    fn discovery_is_optional_and_https() {
        let with = EXAMPLE.replace(
            "  audiences:",
            "  discovery: https://auth.example.org/application/o/kura-cli/\n  audiences:",
        );
        let c = ServerConfig::parse("server.yaml", &with).unwrap();
        assert_eq!(
            c.oidc.discovery.as_deref(),
            Some("https://auth.example.org/application/o/kura-cli/")
        );
        let bad = EXAMPLE.replace("  audiences:", "  discovery: http://auth/\n  audiences:");
        assert_eq!(
            err(&bad),
            "server.yaml oidc.discovery: must be an https:// URL"
        );
    }

    #[test]
    fn issuer_must_be_https() {
        let e = err(&EXAMPLE.replace("https://auth", "http://auth"));
        assert_eq!(e, "server.yaml oidc.issuer: must be an https:// URL");
        assert!(
            err(&EXAMPLE.replace("https://auth.example.org/application/o/kura/", "not a url"))
                .contains("oidc.issuer")
        );
    }

    #[test]
    fn the_console_client_is_an_audience() {
        let ui =
            |id: &str| format!("{EXAMPLE}ui:\n  enabled: true\n  oidc: {{ clientId: {id} }}\n");
        assert!(err(&ui("kura-web")).contains("ui.oidc.clientId"));
        let with = |d: &str| {
            format!(
                "{EXAMPLE}ui:\n  enabled: true\n  oidc: {{ clientId: kura-cli, discovery: '{d}' }}\n"
            )
        };
        assert!(
            err(&with("http://auth.example.org/application/o/kura-web/"))
                .contains("ui.oidc.discovery")
        );
        let own = ServerConfig::parse(
            "server.yaml",
            &with("https://auth.example.org/application/o/kura-web/"),
        )
        .unwrap();
        assert_eq!(
            own.ui.unwrap().oidc.discovery.as_deref(),
            Some("https://auth.example.org/application/o/kura-web/")
        );
        let ok = ServerConfig::parse("server.yaml", &ui("kura-cli")).unwrap();
        let console = ok.ui.unwrap();
        assert_eq!(console.dir, std::path::PathBuf::from("/usr/share/kura/ui"));
        assert!(console.enabled);
        // Off, nothing is checked: the block may sit there unused.
        let off = format!("{EXAMPLE}ui:\n  oidc: {{ clientId: kura-web }}\n");
        assert!(
            !ServerConfig::parse("server.yaml", &off)
                .unwrap()
                .ui
                .unwrap()
                .enabled
        );
    }

    #[test]
    fn webhook_ranges_never_cover_everything() {
        let with = |r: &str| {
            format!(
                "{EXAMPLE}notifications:\n  enabled: true\n  webhook: {{ allowedCidrs: [{r}] }}\n"
            )
        };
        assert!(err(&with("0.0.0.0/0")).contains("allowedCidrs"));
        assert!(err(&with("'::/0'")).contains("allowedCidrs"));
        assert!(err(&with("nope")).contains("allowedCidrs"));
        let ok = ServerConfig::parse("server.yaml", &with("10.42.7.15/32")).unwrap();
        assert!(ok.notifications.enabled);
        assert_eq!(ok.notifications.webhook.allowed_cidrs.len(), 1);
        assert!(
            !ServerConfig::parse("server.yaml", EXAMPLE)
                .unwrap()
                .notifications
                .enabled,
            "off by default"
        );
    }

    #[test]
    fn audiences_must_not_be_empty() {
        assert!(err(&EXAMPLE.replace("[kura-plugin, kura-cli]", "[]")).contains("oidc.audiences"));
        assert!(
            err(&EXAMPLE.replace("[kura-plugin, kura-cli]", "['']")).contains("oidc.audiences")
        );
    }

    #[test]
    fn alg_none_cannot_be_configured() {
        let e = err(&EXAMPLE.replace("algorithms: [RS256]", "algorithms: [none]"));
        assert!(
            e.contains("oidc.algorithms") && e.contains("unknown variant `none`"),
            "{e}"
        );
        assert!(
            err(&EXAMPLE.replace("algorithms: [RS256]", "algorithms: []"))
                .contains("at least one algorithm")
        );
        assert!(
            err(&EXAMPLE.replace("algorithms: [RS256]", "algorithms: [HS256]"))
                .contains("unknown variant")
        );
    }

    #[test]
    fn unknown_fields_and_bad_cidrs_are_refused() {
        assert!(err(&EXAMPLE.replace("trustedProxies", "trustedProxy")).contains("unknown field"));
        assert!(err(&EXAMPLE.replace("10.42.0.0/16", "10.42.0.0/33")).contains("trustedProxies"));
    }

    #[test]
    fn missing_file_is_a_read_error() {
        let e = ServerConfig::load(Path::new("/nonexistent/server.yaml")).unwrap_err();
        assert!(matches!(e, ConfigError::Read { .. }));
    }
}
