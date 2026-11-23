//! The latest published release, read from GitHub once a day, so the
//! console can tell platform admins when this server is behind
//! (`updateCheck.enabled`, on by default). One unauthenticated request a day,
//! carrying no data of this instance but the server's version in its
//! User-Agent; a failure is logged at `debug` and retried the next day.

use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::app::SharedState;

const LATEST: &str = "https://api.github.com/repos/batleforc/weebo-kura/releases/latest";
const EVERY: Duration = Duration::from_secs(24 * 3600);
const TIMEOUT: Duration = Duration::from_secs(10);

/// `latest` is a higher release than `current` (major.minor.patch; a
/// pre-release compares as its release).
pub fn newer(latest: &str, current: &str) -> bool {
    match (
        crate::app::parse_version(latest),
        crate::app::parse_version(current),
    ) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

/// The version of a GitHub release document: its `tag_name` without `v`.
fn version_of(release: &serde_json::Value) -> Option<String> {
    let tag = release.get("tag_name")?.as_str()?;
    let v = tag.strip_prefix('v').unwrap_or(tag);
    crate::app::parse_version(v).map(|_| v.to_owned())
}

async fn fetch(client: &reqwest::Client) -> Result<String, String> {
    let release: serde_json::Value = client
        .get(LATEST)
        .header("accept", "application/vnd.github+json")
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    version_of(&release).ok_or_else(|| "no version in the latest release".into())
}

pub async fn run(state: SharedState, shutdown: CancellationToken) {
    let client = match reqwest::Client::builder()
        .timeout(TIMEOUT)
        .user_agent(concat!("kura-server/", env!("CARGO_PKG_VERSION")))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "update check off: no HTTP client");
            return;
        }
    };
    loop {
        match fetch(&client).await {
            Ok(v) => {
                if newer(&v, env!("CARGO_PKG_VERSION")) {
                    tracing::info!(latest = %v, "a newer kura-server release is published");
                }
                state.set_latest_release(v);
            }
            Err(e) => tracing::debug!(error = %e, "update check failed"),
        }
        tokio::select! {
            () = shutdown.cancelled() => return,
            () = tokio::time::sleep(EVERY) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_as_releases() {
        assert!(newer("0.5.0", "0.4.0"));
        assert!(newer("1.0.0", "0.9.9"));
        assert!(!newer("0.4.0", "0.4.0"));
        assert!(!newer("0.4.0", "0.5.0-rc.1"));
        assert!(!newer("junk", "0.4.0"));
        let doc = serde_json::json!({ "tag_name": "v0.5.0" });
        assert_eq!(version_of(&doc).as_deref(), Some("0.5.0"));
        assert_eq!(
            version_of(&serde_json::json!({ "tag_name": "latest" })),
            None
        );
        assert_eq!(version_of(&serde_json::json!({})), None);
    }
}
