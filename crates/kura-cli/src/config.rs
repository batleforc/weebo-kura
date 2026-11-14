//! `kura`'s own files: `config.json` (server, issuer, device, attached
//! folders) and `credentials.json` (the refresh token, owner-only), in
//! `$KURA_CLI_HOME`, else `$XDG_CONFIG_HOME/kura`, else `~/.config/kura`.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub server: String,
    pub issuer: String,
    pub client_id: String,
    pub device: Option<Uuid>,
    pub device_name: String,
    #[serde(default)]
    pub vaults: BTreeMap<String, Attached>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attached {
    pub id: Uuid,
    pub path: PathBuf,
    #[serde(default)]
    pub last_sync: Option<u64>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Credentials {
    refresh_token: Option<String>,
}

/// The default home: `$KURA_CLI_HOME`, `$XDG_CONFIG_HOME/kura` or `~/.config/kura`.
pub fn default_home() -> Result<PathBuf, String> {
    if let Some(h) = std::env::var_os("KURA_CLI_HOME") {
        return Ok(PathBuf::from(h));
    }
    if let Some(x) = std::env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(x).join("kura"));
    }
    std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join(".config/kura"))
        .ok_or_else(|| "no home folder: set KURA_CLI_HOME".to_owned())
}

/// Written whole through a temporary file, readable by the owner only.
fn write_private(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("tmp");
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
    let mut f = o
        .open(&tmp)
        .map_err(|e| format!("{}: {e}", tmp.display()))?;
    f.write_all(bytes).map_err(|e| e.to_string())?;
    f.sync_all().map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

impl Config {
    pub fn load(home: &Path) -> Result<Option<Self>, String> {
        let p = home.join("config.json");
        match std::fs::read(&p) {
            Ok(b) => serde_json::from_slice(&b)
                .map(Some)
                .map_err(|e| format!("{}: {e}", p.display())),
            Err(_) => Ok(None),
        }
    }

    pub fn save(&self, home: &Path) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        write_private(&home.join("config.json"), &bytes)
    }
}

// shortcut: the refresh token is a 0600 file, not the OS keyring §6.2 names (plan D45);
// move it to the keyring before the CLI is distributed beyond the owner.
pub fn refresh_token(home: &Path) -> Result<Option<String>, String> {
    let p = home.join("credentials.json");
    match std::fs::read(&p) {
        Ok(b) => Ok(serde_json::from_slice::<Credentials>(&b)
            .ok()
            .and_then(|c| c.refresh_token)),
        Err(_) => Ok(None),
    }
}

/// Stored before the new token is used: rotation invalidates the old one (§6.2).
pub fn set_refresh_token(home: &Path, token: Option<&str>) -> Result<(), String> {
    let c = Credentials {
        refresh_token: token.map(str::to_owned),
    };
    let bytes = serde_json::to_vec(&c).map_err(|e| e.to_string())?;
    write_private(&home.join("credentials.json"), &bytes)
}
