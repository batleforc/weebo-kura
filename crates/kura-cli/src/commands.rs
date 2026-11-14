//! The commands of `kura` (RFC 0001 §4.5), as a library so tests drive them
//! in process. Exit codes are those of §4.5.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use kura_core::sync::{Host as _, Known, LocalState, Options, RemoteError, Stamp, sync};
use kura_core::{ChunkHash, ChunkParams, VaultPath, blob_hash, chunks};
use uuid::Uuid;

use crate::api::{self, Api, oidc};
use crate::config::{self, Attached, Config};
use crate::fs::FsHost;

/// Files larger than this are listed as unsyncable, both ways; the tenant's
/// `maxFileSize` still applies on the server.
// shortcut: a file is held whole in memory to hash, upload or download it, hence 2 GiB;
// stream it chunk by chunk before raising this.
const MAX_FILE: u64 = 2 << 30;

/// Where `kura` keeps its files, and an access token that replaces the
/// sign-in (`KURA_ACCESS_TOKEN`, for scripts and tests).
pub struct Ctx {
    pub home: PathBuf,
    pub access_token: Option<String>,
}

/// `kura devices revoke`.
#[derive(clap::Subcommand)]
pub enum DeviceAction {
    Revoke { name: String },
}

/// An exit code of §4.5 and what to tell the user.
pub struct Fail(pub u8, pub String);

impl Fail {
    fn local(e: impl std::fmt::Display) -> Self {
        Self(1, e.to_string())
    }
}

impl From<oidc::OidcError> for Fail {
    fn from(e: oidc::OidcError) -> Self {
        let code = if matches!(e, oidc::OidcError::Unavailable(_)) {
            1
        } else {
            2
        };
        Self(code, e.to_string())
    }
}

/// §4.5: 2 sign in, 3 refused, 4 unsyncable, 1 anything else.
fn code_of(e: &RemoteError) -> u8 {
    match e.kind() {
        "invalid_token" | "device_revoked" | "device_unknown" => 2,
        "insufficient_role" | "quota_exceeded" | "client_too_old" => 3,
        "invalid_path" | "payload_too_large" => 4,
        _ => 1,
    }
}

impl From<RemoteError> for Fail {
    fn from(e: RemoteError) -> Self {
        let code = code_of(&e);
        let hint = match e.kind() {
            "invalid_token" => ". Run `kura login`.",
            "device_revoked" => ". This device was revoked: run `kura login`.",
            "client_too_old" => ". Update kura.",
            _ => "",
        };
        Self(code, format!("{e}{hint}"))
    }
}

fn config(ctx: &Ctx) -> Result<Config, Fail> {
    Config::load(&ctx.home)
        .map_err(Fail::local)?
        .ok_or_else(|| Fail(2, "not signed in: run `kura login`".into()))
}

/// An access token: `KURA_ACCESS_TOKEN` (scripts, tests), else a refresh.
async fn access_token(ctx: &Ctx, cfg: &Config, http: &reqwest::Client) -> Result<String, Fail> {
    if let Some(t) = &ctx.access_token {
        return Ok(t.clone());
    }
    let refresh = config::refresh_token(&ctx.home)
        .map_err(Fail::local)?
        .ok_or_else(|| Fail(2, "not signed in: run `kura login`".into()))?;
    let d = oidc::discover(http, &cfg.issuer).await?;
    let t = oidc::refresh(http, &d, &cfg.client_id, &refresh).await?;
    if let Some(r) = &t.refresh_token {
        config::set_refresh_token(&ctx.home, Some(r)).map_err(Fail::local)?;
    }
    Ok(t.access_token)
}

async fn session(ctx: &Ctx, cfg: &mut Config) -> Result<Api, Fail> {
    let http = api::http()?;
    let token = access_token(ctx, cfg, &http).await?;
    let mut api = Api {
        http,
        server: cfg.server.clone(),
        token,
        device: cfg.device,
    };
    // A device purged after 180 days idle is re-registered silently (§4.4).
    match api.vaults().await {
        Err(e) if e.kind() == "device_unknown" => {
            let d = api.register(&cfg.device_name, "desktop").await?;
            cfg.device = Some(d.id);
            cfg.save(&ctx.home).map_err(Fail::local)?;
            api.device = Some(d.id);
        }
        Err(e) => return Err(e.into()),
        Ok(_) => {}
    }
    Ok(api)
}

fn host_name() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
        .map(|h| h.trim().to_owned())
        .filter(|h| VaultPath::parse(h).is_ok())
        .unwrap_or_else(|| "kura-cli".into())
}

pub async fn login(
    ctx: &Ctx,

    server: String,
    issuer: String,
    client_id: String,
    device_name: Option<String>,
) -> Result<u8, Fail> {
    let name = device_name.unwrap_or_else(host_name);
    // The name appears in conflict file names: it must be a valid name (§4.5).
    VaultPath::parse(&name).map_err(|e| Fail(3, format!("--device-name: {e}")))?;
    let http = api::http()?;
    let token = match ctx.access_token.clone() {
        Some(t) => t,
        None => {
            let d = oidc::discover(&http, &issuer).await?;
            let code = oidc::start(&http, &d, &client_id).await?;
            println!(
                "Open {} and enter the code:\n\n    {}\n",
                code.verification_uri, code.user_code
            );
            if let Some(c) = &code.verification_uri_complete {
                println!("(or open {c})");
            }
            println!("Waiting for approval...");
            let t = oidc::wait(&http, &d, &client_id, &code).await?;
            config::set_refresh_token(&ctx.home, t.refresh_token.as_deref())
                .map_err(Fail::local)?;
            t.access_token
        }
    };
    let user = oidc::username(&token).unwrap_or_else(|| "you".into());
    let api = Api {
        http,
        server: server.clone(),
        token,
        device: None,
    };
    let device = match api.register(&name, "desktop").await {
        Ok(d) => d,
        Err(e) if e.kind() == "insufficient_role" => {
            return Err(Fail(
                2,
                format!(
                    "{user} belongs to no Kura tenant. Ask your platform admin to add you to a tenant group."
                ),
            ));
        }
        Err(e) => return Err(e.into()),
    };
    let mut cfg = Config::load(&ctx.home)
        .map_err(Fail::local)?
        .unwrap_or_default();
    cfg.server = server;
    cfg.issuer = issuer;
    cfg.client_id = client_id;
    cfg.device = Some(device.id);
    cfg.device_name = name.clone();
    cfg.save(&ctx.home).map_err(Fail::local)?;
    println!("Logged in as {user}. Device \"{name}\" registered (profile: desktop).");
    Ok(0)
}

fn params(c: api::Chunking) -> Result<ChunkParams, Fail> {
    ChunkParams::new(c.min, c.avg, c.max)
        .map_err(|e| Fail(1, format!("the server's chunking parameters: {e}")))
}

fn blob_of(data: &[u8], p: ChunkParams) -> ChunkHash {
    let hashes: Vec<ChunkHash> = chunks(data, p).iter().map(|c| c.hash).collect();
    blob_hash(data.len() as u64, &hashes)
}

pub async fn attach(ctx: &Ctx, vault: &str, folder: PathBuf, yes: bool) -> Result<u8, Fail> {
    let mut cfg = config(ctx)?;
    let api = session(ctx, &mut cfg).await?;
    let v = api
        .vaults()
        .await?
        .into_iter()
        .find(|x| x.name == vault)
        .ok_or_else(|| Fail(3, format!("no vault {vault} visible to you")))?;
    let p = params(v.chunking)?;
    std::fs::create_dir_all(&folder).map_err(Fail::local)?;
    let folder = folder.canonicalize().map_err(Fail::local)?;
    let host = FsHost::new(folder.clone());
    // The server's entries, by path.
    let mut remote = HashMap::new();
    let mut after: Option<String> = None;
    loop {
        let page = api.entries(v.id, after.as_deref()).await?;
        for it in page.items {
            remote.insert(it.path.clone(), it);
        }
        match page.next {
            // A page that does not move the keyset forward would loop forever.
            Some(n) if after.as_deref() != Some(n.as_str()) => after = Some(n),
            Some(_) => return Err(Fail(1, "the server's entry pages do not advance".into())),
            None => break,
        }
    }
    let device = cfg
        .device
        .ok_or_else(|| Fail(2, "no device: run `kura login`".into()))?;
    let mut state = LocalState::adopt(host.load_state(), device, v.id);
    let (mut matched, mut only_here, mut differ) = (0usize, 0usize, 0usize);
    let mut seen = std::collections::HashSet::new();
    for f in host.list().await.map_err(Fail::local)? {
        let Ok(path) = VaultPath::from_local(&f.path) else {
            continue;
        };
        let Some(it) = remote.get(&path) else {
            only_here = only_here.saturating_add(1);
            continue;
        };
        seen.insert(path.clone());
        let data = host.read(&path).await.map_err(Fail::local)?;
        // Same content: known as that entry, nothing moves. Other content is
        // pushed as new, so the server keeps both as a conflict file.
        match it.blob {
            Some(b) if b == blob_of(&data, p) => {
                matched = matched.saturating_add(1);
                state.entries.insert(
                    it.entry,
                    Known {
                        path,
                        revision: it.revision,
                        blob: b,
                        size: it.size,
                        stamp: Some(Stamp::from(&f)),
                    },
                );
            }
            _ => differ = differ.saturating_add(1),
        }
    }
    let only_server = remote.len().saturating_sub(seen.len());
    println!("Attach {} to {vault}:", folder.display());
    println!("  {matched:>6} files match the server");
    println!("  {only_here:>6} only here, will be pushed");
    println!("  {only_server:>6} only on the server, will be downloaded");
    if differ > 0 {
        println!("  {differ:>6} differ from the server, both kept (a conflict file each)");
    }
    if !yes && !confirm("Continue?") {
        println!("Nothing changed.");
        return Ok(3);
    }
    host.save(&state).await.map_err(Fail::local)?;
    cfg.vaults.insert(
        vault.to_owned(),
        Attached {
            id: v.id,
            path: folder,
            last_sync: None,
        },
    );
    cfg.save(&ctx.home).map_err(Fail::local)?;
    let code = sync_one(ctx, &api, &mut cfg, vault, false).await?;
    if code == 0 {
        println!("Attached. {vault} is in sync.");
    }
    Ok(code)
}

fn confirm(question: &str) -> bool {
    use std::io::{BufRead as _, Write as _};
    print!("{question} [y/N] ");
    drop(std::io::stdout().flush());
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line).is_ok() && matches!(line.trim(), "y" | "Y" | "yes")
}

pub async fn sync_cmd(ctx: &Ctx, vault: Option<&str>, dry_run: bool) -> Result<u8, Fail> {
    let mut cfg = config(ctx)?;
    let names: Vec<String> = match vault {
        Some(v) if cfg.vaults.contains_key(v) => vec![v.to_owned()],
        Some(v) => {
            return Err(Fail(
                3,
                format!("{v} is not attached: run `kura attach {v} <folder>`"),
            ));
        }
        None => cfg.vaults.keys().cloned().collect(),
    };
    if names.is_empty() {
        println!("No vault attached. Run `kura attach <vault> <folder>`.");
        return Ok(0);
    }
    let api = session(ctx, &mut cfg).await?;
    let mut worst = 0u8;
    for name in names {
        let code = match sync_one(ctx, &api, &mut cfg, &name, dry_run).await {
            Ok(c) => c,
            Err(Fail(c, msg)) => {
                eprintln!("error: {name}: {msg}");
                c
            }
        };
        worst = worst.max(code);
    }
    Ok(worst)
}

async fn sync_one(
    ctx: &Ctx,
    api: &Api,
    cfg: &mut Config,
    name: &str,
    dry_run: bool,
) -> Result<u8, Fail> {
    let a = cfg
        .vaults
        .get(name)
        .cloned()
        .ok_or_else(|| Fail(3, format!("{name} is not attached")))?;
    let v = api
        .vaults()
        .await?
        .into_iter()
        .find(|x| x.id == a.id)
        .ok_or_else(|| Fail(3, format!("{name}: no longer visible to you")))?;
    let device = cfg
        .device
        .ok_or_else(|| Fail(2, "no device: run `kura login`".into()))?;
    let host = FsHost::new(a.path.clone());
    let mut state = LocalState::adopt(host.load_state(), device, a.id);
    let opts = Options {
        params: params(v.chunking)?,
        max_file_size: MAX_FILE,
        dry_run,
    };
    let r = sync(&host, &api.vault(a.id), &mut state, &opts)
        .await
        .map_err(|e| match e {
            kura_core::sync::SyncError::Remote(r) => Fail::from(r),
            other => Fail::local(other),
        })?;
    let mut code = 0u8;
    if dry_run {
        println!("{name}: would push {} files.", r.pushed);
    }
    if let Some(e) = &r.refused {
        eprintln!("error: {name}: push refused, your role is {} ({e})", v.role);
        eprintln!(
            "Local changes are kept in {} files and were not uploaded.",
            r.pushed
        );
        if e.kind() == "insufficient_role" {
            eprintln!("Ask a vault manager for the editor role to upload them.");
        }
        code = code_of(e);
    }
    if !r.unsyncable.is_empty() {
        println!("Unsyncable ({}):", r.unsyncable.len());
        for (p, why) in &r.unsyncable {
            println!("  {p}    {why}");
        }
        code = code.max(4);
    }
    if !r.failed.is_empty() {
        eprintln!(
            "error: {name}: {} remote files could not be written here; retried next sync:",
            r.failed.len()
        );
        for (p, why) in &r.failed {
            eprintln!("  {p}    {why}");
        }
        code = code.max(1);
    }
    for c in &r.conflicts {
        println!("conflict: {c}");
    }
    if !dry_run {
        if !r.deferred.is_empty() {
            println!(
                "{} remote changes wait for local edits to be pushed.",
                r.deferred.len()
            );
        }
        println!(
            "{name}: pushed {}, pulled {}, removed {}, {} conflicts.",
            if r.refused.is_some() { 0 } else { r.pushed },
            r.pulled,
            r.trashed,
            r.conflicts.len()
        );
        if let Some(x) = cfg.vaults.get_mut(name) {
            x.last_sync = Some(host.now_ms());
        }
        cfg.save(&ctx.home).map_err(Fail::local)?;
    }
    Ok(code)
}

pub async fn status(ctx: &Ctx) -> Result<u8, Fail> {
    let mut cfg = config(ctx)?;
    if cfg.vaults.is_empty() {
        println!("No vault. Run `kura attach <vault> <folder>`.");
        return Ok(0);
    }
    let api = session(ctx, &mut cfg).await?;
    let roles: BTreeMap<Uuid, api::VaultView> =
        api.vaults().await?.into_iter().map(|v| (v.id, v)).collect();
    println!(
        "{:<12} {:<8} {:<11} {:>7} {:>9}  LAST SYNC",
        "VAULT", "ROLE", "STATE", "PENDING", "CONFLICTS"
    );
    let now = FsHost::new(PathBuf::new()).now_ms();
    for (name, a) in &cfg.vaults {
        let Some(v) = roles.get(&a.id) else {
            println!("{name:<12} {:<8} {:<11}", "-", "no access");
            continue;
        };
        let host = FsHost::new(a.path.clone());
        let device = cfg.device.unwrap_or_default();
        let mut state = LocalState::adopt(host.load_state(), device, a.id);
        let opts = Options {
            params: params(v.chunking)?,
            max_file_size: MAX_FILE,
            dry_run: true,
        };
        let r = sync(&host, &api.vault(a.id), &mut state, &opts)
            .await
            .map_err(Fail::local)?;
        let conflicts = host
            .list()
            .await
            .map_err(Fail::local)?
            .iter()
            .filter(|f| f.path.contains(" (conflict "))
            .count();
        let st = if !r.unsyncable.is_empty() {
            "unsyncable"
        } else if conflicts > 0 {
            "conflict"
        } else if r.pushed > 0 && v.role == "viewer" {
            "read-only"
        } else if r.pushed > 0 {
            "syncing"
        } else {
            "in sync"
        };
        let last = a.last_sync.map_or_else(
            || "never".to_owned(),
            |t| format!("{}s ago", now.saturating_sub(t) / 1000),
        );
        println!(
            "{name:<12} {:<8} {st:<11} {:>7} {conflicts:>9}  {last}",
            v.role, r.pushed
        );
    }
    Ok(0)
}

pub async fn devices(ctx: &Ctx, action: Option<DeviceAction>) -> Result<u8, Fail> {
    let mut cfg = config(ctx)?;
    let api = session(ctx, &mut cfg).await?;
    let list = api.devices().await?;
    match action {
        None => {
            println!("{:<16} {:<8} {:<22} STATE", "NAME", "PROFILE", "LAST SEEN");
            for d in &list {
                let state = if Some(d.id) == cfg.device {
                    "this device"
                } else if d.revoked {
                    "revoked"
                } else {
                    "active"
                };
                let owner = d
                    .owner
                    .as_deref()
                    .map(|o| format!(" ({o})"))
                    .unwrap_or_default();
                println!(
                    "{:<16} {:<8} {:<22} {state}{owner}",
                    d.name, d.profile, d.last_seen
                );
            }
            Ok(0)
        }
        Some(DeviceAction::Revoke { name }) => {
            let d = list
                .iter()
                .find(|d| d.name == name && !d.revoked)
                .ok_or_else(|| Fail(3, format!("no active device named {name}")))?;
            api.revoke(d.id).await?;
            println!("Device \"{name}\" can no longer sync.");
            println!("To fully lock it out, also end its session in your identity provider.");
            Ok(0)
        }
    }
}

/// Local edits quiet this long are pushed (§6.3: Obsidian saves coalesce).
const QUIET: std::time::Duration = std::time::Duration::from_secs(5);
/// No notice (the server pings every 30 s) for this long: the socket is down.
const SILENCE: std::time::Duration = std::time::Duration::from_secs(75);
/// Reconnection backoff bounds, with decorrelated jitter (§6.3).
const BACKOFF_MIN: u64 = 1;
const BACKOFF_MAX: u64 = 60;

/// Decorrelated jitter: a delay between the minimum and three times the last, capped.
fn next_backoff(last: u64) -> u64 {
    let r = u64::from_le_bytes(
        uuid::Uuid::new_v4().as_bytes()[..8]
            .try_into()
            .unwrap_or_default(),
    );
    let span = last.saturating_mul(3).saturating_sub(BACKOFF_MIN).max(1);
    BACKOFF_MIN
        .saturating_add(r.checked_rem(span).unwrap_or_default())
        .min(BACKOFF_MAX)
}

/// A fingerprint of the folder's files, to tell when local edits went quiet.
async fn fingerprint(host: &FsHost) -> u64 {
    use std::hash::{Hash as _, Hasher as _};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    if let Ok(mut files) = host.list().await {
        files.sort_by(|a, b| a.path.cmp(&b.path));
        for f in files {
            (f.path, f.size, f.mtime).hash(&mut h);
        }
    }
    h.finish()
}

enum Socketed {
    Stop,
    Reconnect,
}

/// `kura watch`: sync once, then pull on every notification, push local
/// edits once quiet for 5 s, reconnect with backoff, and pull on each retry
/// while the socket is down, until `stop` (§6.3).
pub async fn watch(
    ctx: &Ctx,
    vault: Option<&str>,
    stop: impl std::future::Future<Output = ()>,
) -> Result<u8, Fail> {
    let mut cfg = config(ctx)?;
    let name = match vault {
        Some(v) if cfg.vaults.contains_key(v) => v.to_owned(),
        Some(v) => {
            return Err(Fail(
                3,
                format!("{v} is not attached: run `kura attach {v} <folder>`"),
            ));
        }
        None if cfg.vaults.len() == 1 => cfg.vaults.keys().next().cloned().unwrap_or_default(),
        None => return Err(Fail(3, "name the vault to watch".into())),
    };
    tokio::pin!(stop);
    let mut backoff = BACKOFF_MIN;
    loop {
        // A fresh access token per connection: the server closes the socket
        // when the one behind its ticket expires. Only "sign in again" and a
        // refusal end the watch; anything else is retried (review 4 L3).
        let wait = match connect_and_watch(ctx, &mut cfg, &name, &mut stop).await {
            Ok(Watched::Stop) => return Ok(0),
            Ok(Watched::Dropped { lasted }) if lasted >= std::time::Duration::from_secs(60) => {
                backoff = BACKOFF_MIN;
                continue;
            }
            Ok(Watched::Dropped { .. }) => format!("{name}: notifications dropped"),
            Err(Fail(code @ (2 | 3), msg)) => return Err(Fail(code, msg)),
            Err(Fail(_, msg)) => format!("{name}: {msg}"),
        };
        backoff = next_backoff(backoff);
        eprintln!("{wait}; retrying in {backoff} s");
        tokio::select! {
            () = &mut stop => return Ok(0),
            () = tokio::time::sleep(std::time::Duration::from_secs(backoff)) => {}
        }
    }
}

enum Watched {
    Stop,
    Dropped { lasted: std::time::Duration },
}

/// One connection: a sync, the socket, then notices until it drops.
async fn connect_and_watch(
    ctx: &Ctx,
    cfg: &mut Config,
    name: &str,
    stop: &mut std::pin::Pin<&mut impl std::future::Future<Output = ()>>,
) -> Result<Watched, Fail> {
    let api = session(ctx, cfg).await?;
    if sync_one(ctx, &api, cfg, name, false).await? == 2 {
        return Err(Fail(2, "sign in again: run `kura login`".into()));
    }
    let id = cfg
        .vaults
        .get(name)
        .map(|a| a.id)
        .ok_or_else(|| Fail(3, format!("{name} is not attached")))?;
    let mut socket = api
        .events(id)
        .await
        .map_err(|e| Fail(1, format!("notifications unavailable ({e})")))?;
    println!("{name}: watching for changes");
    let opened = tokio::time::Instant::now();
    match on_socket(ctx, &api, cfg, name, &mut socket, stop).await? {
        Socketed::Stop => Ok(Watched::Stop),
        Socketed::Reconnect => Ok(Watched::Dropped {
            lasted: opened.elapsed(),
        }),
    }
}

async fn on_socket(
    ctx: &Ctx,
    api: &Api,
    cfg: &mut Config,
    name: &str,
    socket: &mut api::Socket,
    stop: &mut std::pin::Pin<&mut impl std::future::Future<Output = ()>>,
) -> Result<Socketed, Fail> {
    use futures_util::StreamExt as _;
    use tokio_tungstenite::tungstenite::Message;
    let path = cfg
        .vaults
        .get(name)
        .map(|a| a.path.clone())
        .unwrap_or_default();
    let host = FsHost::new(path);
    let mut last_notice = tokio::time::Instant::now();
    let mut scan = tokio::time::interval(QUIET);
    let mut seen = fingerprint(&host).await;
    let mut changed_at: Option<tokio::time::Instant> = None;
    loop {
        tokio::select! {
            () = stop.as_mut() => {
                drop(socket.close(None).await);
                return Ok(Socketed::Stop);
            }
            m = socket.next() => match m {
                Some(Ok(Message::Text(t))) => {
                    last_notice = tokio::time::Instant::now();
                    let kind = serde_json::from_str::<serde_json::Value>(&t).ok().and_then(|v| v.get("type").and_then(|k| k.as_str()).map(str::to_owned));
                    if matches!(kind.as_deref(), Some("change" | "pull")) {
                        let code = sync_one(ctx, api, cfg, name, false).await?;
                        if code == 2 {
                            return Err(Fail(2, "sign in again: run `kura login`".into()));
                        }
                        seen = fingerprint(&host).await;
                    }
                }
                Some(Ok(Message::Close(_)) | Err(_)) | None => return Ok(Socketed::Reconnect),
                Some(Ok(_)) => last_notice = tokio::time::Instant::now(),
            },
            _ = scan.tick() => {
                if last_notice.elapsed() > SILENCE {
                    return Ok(Socketed::Reconnect);
                }
                let now = fingerprint(&host).await;
                if now != seen {
                    seen = now;
                    changed_at = Some(tokio::time::Instant::now());
                } else if changed_at.is_some_and(|t| t.elapsed() >= QUIET) {
                    changed_at = None;
                    sync_one(ctx, api, cfg, name, false).await?;
                    seen = fingerprint(&host).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_stays_between_one_and_sixty_seconds() {
        let mut b = BACKOFF_MIN;
        for _ in 0..200 {
            b = next_backoff(b);
            assert!((BACKOFF_MIN..=BACKOFF_MAX).contains(&b), "{b}");
        }
        assert!((BACKOFF_MIN..=3).contains(&next_backoff(1)));
    }

    #[test]
    fn exit_codes_follow_section_4_5() {
        let p = |kind: &str| RemoteError::Problem {
            status: 400,
            kind: kind.into(),
            detail: None,
        };
        for (kind, code) in [
            ("invalid_token", 2),
            ("device_revoked", 2),
            ("device_unknown", 2),
            ("insufficient_role", 3),
            ("quota_exceeded", 3),
            ("client_too_old", 3),
            ("invalid_path", 4),
            ("payload_too_large", 4),
            ("rate_limited", 1),
        ] {
            assert_eq!(code_of(&p(kind)), code, "{kind}");
        }
        assert_eq!(code_of(&RemoteError::Unavailable("x".into())), 1);
        let f = Fail::from(p("client_too_old"));
        assert_eq!((f.0, f.1.ends_with("Update kura.")), (3, true));
        assert_eq!(Fail::from(oidc::OidcError::Denied).0, 2);
        assert_eq!(Fail::from(oidc::OidcError::Unavailable("x".into())).0, 1);
    }

    #[test]
    fn a_host_name_is_a_valid_device_name() {
        assert!(VaultPath::parse(&host_name()).is_ok());
    }

    #[test]
    fn server_chunking_is_checked() {
        assert!(
            params(api::Chunking {
                min: 64,
                avg: 256,
                max: 1024
            })
            .is_ok()
        );
        assert_eq!(
            params(api::Chunking {
                min: 0,
                avg: 1,
                max: 2
            })
            .err()
            .map(|f| f.0),
            Some(1)
        );
        let a = blob_of(
            b"x",
            params(api::Chunking {
                min: 64,
                avg: 256,
                max: 1024,
            })
            .unwrap_or_else(|f| panic!("{}", f.1)),
        );
        assert_eq!(a, blob_hash(1, &[ChunkHash::of(b"x")]));
    }
}
