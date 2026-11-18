//! The commands of `kura` (RFC 0001 §4.5), as a library so tests drive them
//! in process. Exit codes are those of §4.5.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use kura_core::sync::{Host as _, Known, LocalState, Options, RemoteError, Stamp, sync};
use kura_core::wire::EntryState;
use kura_core::{ChunkHash, ChunkParams, PathError, VaultPath, blob_hash, chunks};
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

/// The id of the scoped token this run uses instead of a signed-in device
/// (`KURA_TOKEN` or `--token-file`, RFC 0004 §6.6), if it does.
fn scoped(ctx: &Ctx) -> Option<Uuid> {
    let t = ctx.access_token.as_deref()?;
    kura_core::token::ScopedToken::parse(t).ok().map(|t| t.id)
}

/// Folders compare as the server compares them (RFC 0004 §4.2).
use kura_core::scope::fold;

fn config(ctx: &Ctx) -> Result<Config, Fail> {
    if let Some(cfg) = Config::load(&ctx.home).map_err(Fail::local)? {
        return Ok(cfg);
    }
    // A scoped token never signs in: it only needs to know the server.
    match (scoped(ctx), std::env::var("KURA_SERVER")) {
        (Some(_), Ok(server)) => Ok(Config {
            server,
            ..Config::default()
        }),
        (Some(_), Err(_)) => Err(Fail(
            2,
            "no server: set KURA_SERVER to the server's address".into(),
        )),
        (None, _) => Err(Fail(2, "not signed in: run `kura login`".into())),
    }
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
        token: api::Token::new(token),
        device: cfg.device,
        // A script's fixed token is never renewed.
        renew: ctx.access_token.is_none().then(|| api::Renew {
            issuer: cfg.issuer.clone(),
            client_id: cfg.client_id.clone(),
            home: ctx.home.clone(),
        }),
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
        token: api::Token::new(token),
        device: None,
        renew: None,
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

/// The server's live notes, by path, every page.
async fn remote_entries(api: &Api, vault: Uuid) -> Result<HashMap<VaultPath, EntryState>, Fail> {
    let mut remote = HashMap::new();
    let mut after: Option<String> = None;
    loop {
        let page = api.entries(vault, after.as_deref()).await?;
        for it in page.items {
            remote.insert(it.path.clone(), it);
        }
        match page.next {
            // A page that does not move the keyset forward would loop forever.
            Some(n) if after.as_deref() != Some(n.as_str()) => after = Some(n),
            Some(_) => return Err(Fail(1, "the server's entry pages do not advance".into())),
            None => return Ok(remote),
        }
    }
}

/// A folder against the server's notes, by content (attach, verify).
#[derive(Default)]
struct Diff {
    /// Same path, same content.
    matched: Vec<(EntryState, Stamp)>,
    /// Syncable here, absent on the server.
    only_here: Vec<String>,
    /// Same path, other content.
    differ: Vec<String>,
    /// Names that cannot sync (§6.8), with the rule they break.
    unsyncable: Vec<(String, String)>,
    /// Bytes of `only_here` and `differ`: what a sync would upload.
    push_bytes: u64,
    only_server: usize,
    /// The folder's syncable files and their bytes.
    files: usize,
    bytes: u64,
}

async fn diff(
    host: &FsHost,
    remote: &HashMap<VaultPath, EntryState>,
    p: ChunkParams,
) -> Result<Diff, Fail> {
    let mut d = Diff::default();
    let mut seen = std::collections::HashSet::new();
    for f in host.list().await.map_err(Fail::local)? {
        let path = match VaultPath::from_local(&f.path) {
            Ok(path) => path,
            // Dot-folders (.obsidian, .trash, .git) are not notes (decision 28).
            Err(PathError::Hidden) => continue,
            Err(e) => {
                d.unsyncable.push((f.path, e.to_string()));
                continue;
            }
        };
        d.files = d.files.saturating_add(1);
        d.bytes = d.bytes.saturating_add(f.size);
        let Some(it) = remote.get(&path) else {
            d.only_here.push(path.to_string());
            d.push_bytes = d.push_bytes.saturating_add(f.size);
            continue;
        };
        seen.insert(path.clone());
        let data = host.read(&path).await.map_err(Fail::local)?;
        if it.blob == Some(blob_of(&data, p)) {
            d.matched.push((it.clone(), Stamp::from(&f)));
        } else {
            d.differ.push(path.to_string());
            d.push_bytes = d.push_bytes.saturating_add(f.size);
        }
    }
    d.only_server = remote.len().saturating_sub(seen.len());
    Ok(d)
}

/// Bytes for people: `1.2 GiB`.
fn human(bytes: u64) -> String {
    let mut v = bytes as f64;
    for unit in ["B", "KiB", "MiB", "GiB"] {
        if v < 1024.0 {
            return if unit == "B" {
                format!("{bytes} B")
            } else {
                format!("{v:.1} {unit}")
            };
        }
        v /= 1024.0;
    }
    format!("{v:.1} TiB")
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
    let remote = remote_entries(&api, v.id).await?;
    // A scoped token stands in for the device it does not have (§6.3).
    let device = scoped(ctx)
        .or(cfg.device)
        .ok_or_else(|| Fail(2, "no device: run `kura login`".into()))?;
    let mut state = LocalState::adopt(host.load_state(), device, v.id);
    let d = diff(&host, &remote, p).await?;
    let (matched, only_here, differ, only_server) = (
        d.matched.len(),
        d.only_here.len(),
        d.differ.len(),
        d.only_server,
    );
    // Same content: known as that entry, nothing moves. Other content is
    // pushed as new, so the server keeps both as a conflict file.
    for (it, stamp) in d.matched {
        if let Some(blob) = it.blob {
            state.entries.insert(
                it.entry,
                Known {
                    path: it.path,
                    revision: it.revision,
                    blob,
                    size: it.size,
                    stamp: Some(stamp),
                    resend: false,
                },
            );
        }
    }
    println!("Attach {} to {vault}:", folder.display());
    println!("  {matched:>6} files match the server");
    println!(
        "  {only_here:>6} only here, will be pushed ({})",
        human(d.push_bytes)
    );
    println!("  {only_server:>6} only on the server, will be downloaded");
    if differ > 0 {
        println!("  {differ:>6} differ from the server, both kept (a conflict file each)");
    }
    // The migration check of §13: what cannot sync is named before anything moves.
    if !d.unsyncable.is_empty() {
        println!("Unsyncable ({}), left here:", d.unsyncable.len());
        for (p, why) in &d.unsyncable {
            println!("  {p}    {why}");
        }
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
    let code = sync_one(ctx, &api, &mut cfg, vault, false, false).await?;
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

/// `yes`: a full resync may move files to `.trash` past the guard without asking.
pub async fn sync_cmd(
    ctx: &Ctx,
    vault: Option<&str>,
    dry_run: bool,
    yes: bool,
) -> Result<u8, Fail> {
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
        let code = match sync_one(ctx, &api, &mut cfg, &name, dry_run, yes).await {
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
    yes: bool,
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
    let device = scoped(ctx)
        .or(cfg.device)
        .ok_or_else(|| Fail(2, "no device: run `kura login`".into()))?;
    let host = FsHost::new(a.path.clone());
    let mut state = LocalState::adopt(host.load_state(), device, a.id);
    // A token's write folders, folded: a change elsewhere is refused here,
    // before anything is uploaded (RFC 0004 §6.6).
    let write_folders: Option<Vec<String>> = v.scopes.as_ref().map(|rules| {
        rules
            .iter()
            .filter(|r| r.mode == "write")
            .map(|r| r.path.clone())
            .collect()
    });
    let writable = write_folders.clone().map(|folders| {
        let keys: Vec<String> = folders.iter().map(|f| fold(f)).collect();
        Box::new(move |path: &str| {
            let key = fold(path);
            keys.iter().any(|k| key.starts_with(k))
        }) as kura_core::sync::Writable
    });
    let mut opts = Options {
        params: params(v.chunking)?,
        max_file_size: MAX_FILE,
        dry_run,
        allow_deletions: yes,
        prefer_remote: false,
        writable,
    };
    let r = loop {
        match sync(&host, &api.vault(a.id), &mut state, &opts).await {
            Ok(r) => break r,
            // A full resync stops before trashing many files (§6.3).
            Err(kura_core::sync::SyncError::TooManyDeletions { deletions, files }) => {
                let share = deletions
                    .saturating_mul(100)
                    .checked_div(files)
                    .unwrap_or(100);
                println!(
                    "resync: {name}: {deletions} local files ({share}%) were deleted elsewhere."
                );
                if std::io::IsTerminal::is_terminal(&std::io::stdin())
                    && confirm("Move them to .trash?")
                {
                    opts.allow_deletions = true;
                    continue;
                }
                return Err(Fail(
                    3,
                    format!(
                        "Nothing removed. Run `kura sync {name}` again to decide, or with --yes."
                    ),
                ));
            }
            Err(kura_core::sync::SyncError::Remote(r)) => return Err(Fail::from(r)),
            Err(other) => return Err(Fail::local(other)),
        }
    };
    // RFC 0006 phase 3: the console shows what this run left undone. A
    // device only (a token has no row), never on a dry run, best effort.
    if !dry_run
        && scoped(ctx).is_none()
        && let Err(e) = api.report(a.id, &r.counts()).await
    {
        eprintln!("note: {name}: the console was not told this run's state ({e}).");
    }
    // The words of the §4.5 mocks.
    match r.resynced.as_deref() {
        Some("history_rewound") => {
            println!(
                "resync: {name}: the server lost recent history. Nothing is removed on this device."
            );
            println!("Re-pushed {} files the server no longer had.", r.pushed);
        }
        Some(_) => {
            println!(
                "resync: {name}: this device was offline longer than the vault keeps deletions."
            );
            println!(
                "Removed {} local files deleted elsewhere (moved to .trash). Pushed {} files changed here.",
                r.trashed, r.pushed
            );
        }
        None => {}
    }
    let mut code = 0u8;
    if !r.not_writable.is_empty() {
        let folders = write_folders.unwrap_or_default();
        let allowed = if folders.is_empty() {
            "it has none".to_owned()
        } else {
            folders.join(", ")
        };
        eprintln!(
            "error: {name}: {} is outside this token's write folders ({allowed}).",
            r.not_writable.join(", ")
        );
        eprintln!("Nothing was pushed. Move the file back, or ask a manager for another token.");
        code = 3;
    }
    if dry_run {
        println!(
            "{name}: would push {} files, {}.",
            r.pushed,
            human(r.pushed_bytes)
        );
    }
    if let Some(e) = &r.refused {
        eprintln!("error: {name}: push refused, your role is {} ({e})", v.role);
        eprintln!(
            "Local changes are kept in {} files and were not uploaded.",
            r.pushed
        );
        if e.kind() == "insufficient_role" {
            if scoped(ctx).is_some() {
                eprintln!("Ask a vault manager for a token that may write here.");
            } else {
                eprintln!("Ask a vault manager for the editor role to upload them.");
            }
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
        // A token says who is writing (RFC 0004 §4.5).
        if let Some(token) = &v.token_name {
            println!(
                "{name}  token {token}  {} pushed  {} pulled",
                if r.refused.is_some() || !r.not_writable.is_empty() {
                    0
                } else {
                    r.pushed
                },
                r.pulled
            );
        }
        println!(
            "{name}: pushed {}, pulled {}, removed {}, {} conflicts.",
            if r.refused.is_some() || !r.not_writable.is_empty() {
                0
            } else {
                r.pushed
            },
            r.pulled,
            r.trashed,
            r.conflicts.len()
        );
        if let Some(x) = cfg.vaults.get_mut(name) {
            x.last_sync = Some(host.now_ms());
        }
        cfg.save(&ctx.home).map_err(Fail::local)?;
        // A token never reads or writes an Obsidian configuration (§6.8).
        if scoped(ctx).is_none() {
            code = code.max(sync_config(api, name, &a, &v, device, yes).await?);
        }
    }
    Ok(code)
}

/// The vault's `.obsidian`, desktop profile, when the vault syncs it and the
/// folder has one (§6.8). Its own state; a path the categories no longer
/// cover is forgotten here, never deleted on the user's other devices.
async fn sync_config(
    api: &Api,
    name: &str,
    a: &Attached,
    v: &api::VaultView,
    device: Uuid,
    yes: bool,
) -> Result<u8, Fail> {
    if v.config_sync.is_empty() || !a.path.join(".obsidian").is_dir() {
        return Ok(0);
    }
    let host = FsHost::config(a.path.clone(), v.config_sync.clone());
    let mut state = LocalState::adopt(host.load_state(), device, a.id);
    state.entries.retain(|_, k| host.keeps(k.path.as_str()));
    let opts = Options {
        params: params(v.chunking)?,
        max_file_size: MAX_FILE,
        dry_run: false,
        allow_deletions: yes,
        prefer_remote: true,
        writable: None,
    };
    match sync(&host, &api.vault_config(a.id), &mut state, &opts).await {
        Ok(r) => {
            println!(
                "{name}: configuration pushed {}, pulled {}, removed {}.",
                r.pushed, r.pulled, r.trashed
            );
            Ok(0)
        }
        Err(e) => {
            eprintln!("error: {name}: configuration: {e}");
            Ok(1)
        }
    }
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
            allow_deletions: false,
            prefer_remote: false,
            writable: None,
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
    if sync_one(ctx, &api, cfg, name, false, false).await? == 2 {
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
                        let code = sync_one(ctx, api, cfg, name, false, false).await?;
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
                    sync_one(ctx, api, cfg, name, false, false).await?;
                    seen = fingerprint(&host).await;
                }
            }
        }
    }
}

/// The entry a path of an attached vault names, from this device's state.
fn entry_of(cfg: &Config, vault: &str, path: &str) -> Result<(Uuid, Uuid), Fail> {
    let a = cfg
        .vaults
        .get(vault)
        .ok_or_else(|| Fail(3, format!("{vault} is not attached")))?;
    let p = VaultPath::from_local(path).map_err(|e| Fail(4, format!("{path}: {e}")))?;
    let device = cfg.device.unwrap_or_default();
    let host = FsHost::new(a.path.clone());
    let state = LocalState::adopt(host.load_state(), device, a.id);
    let entry = state
        .entries
        .iter()
        .find(|(_, k)| k.path == p)
        .map(|(e, _)| *e.as_uuid())
        .ok_or_else(|| {
            Fail(
                3,
                format!("{path} is not synced here: run `kura sync {vault}` first"),
            )
        })?;
    Ok((a.id, entry))
}

/// `kura history <vault> <path>`: the file's revisions, newest first (§6.9).
/// `kura verify <vault>`: the server holds exactly the folder's notes, file
/// for file, by content; the check that ends a migration (§13). Exit 0 when
/// equal, 1 otherwise.
pub async fn verify(ctx: &Ctx, vault: &str) -> Result<u8, Fail> {
    let mut cfg = config(ctx)?;
    let a = cfg
        .vaults
        .get(vault)
        .cloned()
        .ok_or_else(|| Fail(3, format!("{vault} is not attached here")))?;
    let api = session(ctx, &mut cfg).await?;
    let v = api
        .vaults()
        .await?
        .into_iter()
        .find(|x| x.id == a.id)
        .ok_or_else(|| Fail(3, format!("no vault {vault} visible to you")))?;
    let remote = remote_entries(&api, a.id).await?;
    let server_bytes = remote
        .values()
        .map(|e| e.size)
        .fold(0u64, u64::saturating_add);
    let d = diff(&FsHost::new(a.path.clone()), &remote, params(v.chunking)?).await?;
    println!(
        "{vault}: server {} files, {} bytes ({}); folder {} files, {} bytes ({}).",
        remote.len(),
        server_bytes,
        human(server_bytes),
        d.files,
        d.bytes,
        human(d.bytes)
    );
    for (what, paths) in [
        ("only here", &d.only_here),
        ("differ from the server", &d.differ),
    ] {
        for p in paths {
            println!("  {what}: {p}");
        }
    }
    for (p, why) in &d.unsyncable {
        println!("  cannot sync: {p}    {why}");
    }
    if d.only_server > 0 {
        println!("  {} files only on the server", d.only_server);
    }
    let equal = d.only_here.is_empty()
        && d.differ.is_empty()
        && d.unsyncable.is_empty()
        && d.only_server == 0;
    println!("{}", if equal { "Equal." } else { "Not equal." });
    Ok(if equal { 0 } else { 1 })
}

pub async fn history(ctx: &Ctx, vault: &str, path: &str) -> Result<u8, Fail> {
    let mut cfg = config(ctx)?;
    let (id, entry) = entry_of(&cfg, vault, path)?;
    let api = session(ctx, &mut cfg).await?;
    let revs = api.revisions(id, entry).await?;
    println!(
        "{:<36} {:>7} {:<20} {:>10}  PATH",
        "REVISION", "SEQ", "WHEN (UTC)", "SIZE"
    );
    for r in &revs {
        let when: String = r.created_at.chars().take(19).collect();
        let size = if r.deleted {
            "deleted".to_owned()
        } else {
            r.size.to_string()
        };
        println!(
            "{:<36} {:>7} {:<20} {:>10}  {}",
            r.revision, r.seq, when, size, r.path
        );
    }
    Ok(0)
}

/// `kura restore <vault> <path> <revision>`: that revision's content becomes
/// a new revision, then the vault syncs to bring it here (§6.9).
pub async fn restore(ctx: &Ctx, vault: &str, path: &str, revision: Uuid) -> Result<u8, Fail> {
    let mut cfg = config(ctx)?;
    let (id, entry) = entry_of(&cfg, vault, path)?;
    let api = session(ctx, &mut cfg).await?;
    let r = api.restore(id, entry, revision).await?;
    println!(
        "{path}: revision {revision} restored as {} (seq {}).",
        r.head.path, r.head.seq
    );
    sync_one(ctx, &api, &mut cfg, vault, false, false).await
}

/// `kura token`: scoped read-only tokens for agents and scripts (§6.2).
#[derive(clap::Subcommand)]
pub enum TokenAction {
    /// Issue a token; its secret is shown once.
    Create {
        vault: String,
        /// Shown in lists and conflict file names.
        #[arg(long)]
        name: Option<String>,
        /// A folder the token reads, ending with `/`; repeat for more. With
        /// no --read, --write or --prefix: the whole vault, read only.
        #[arg(long)]
        read: Vec<String>,
        /// A folder the token reads and writes, ending with `/`; repeat for
        /// more. The tenant must allow it (`scopedTokens.write`).
        #[arg(long)]
        write: Vec<String>,
        /// One --read, as before folder rules existed.
        #[arg(long)]
        prefix: Option<String>,
        /// Days, at most 90: `30d` or `30`.
        #[arg(long)]
        expires: String,
        /// An address range the token may be used from; repeat for more.
        #[arg(long)]
        cidr: Vec<String>,
    },
    /// The vault's tokens, never their secrets.
    List { vault: String },
    /// Revoke a token by id.
    Revoke { vault: String, id: Uuid },
}

/// The folder rules `kura token create` asks for (RFC 0004 §4.2): --prefix
/// is one --read; nothing at all is the whole vault, read only.
fn token_scopes(
    read: Vec<String>,
    write: Vec<String>,
    prefix: Option<String>,
) -> Result<Vec<api::ScopeRule>, Fail> {
    if write.iter().any(|w| matches!(w.as_str(), "" | "/")) {
        return Err(Fail(
            3,
            "--write needs a folder; a token never writes the whole vault.".into(),
        ));
    }
    let rule = |mode: &str| {
        let mode = mode.to_owned();
        move |path: String| api::ScopeRule {
            path,
            mode: mode.clone(),
        }
    };
    let mut scopes: Vec<api::ScopeRule> = read
        .into_iter()
        .chain(prefix)
        .map(rule("read"))
        .chain(write.into_iter().map(rule("write")))
        .collect();
    if scopes.is_empty() {
        scopes.push(rule("read")(String::new()));
    }
    Ok(scopes)
}

async fn vault_named(api: &Api, name: &str) -> Result<Uuid, Fail> {
    api.vaults()
        .await?
        .into_iter()
        .find(|v| v.name == name)
        .map(|v| v.id)
        .ok_or_else(|| Fail(3, format!("{name}: no such vault, or not visible to you")))
}

pub async fn token(ctx: &Ctx, action: TokenAction) -> Result<u8, Fail> {
    let mut cfg = config(ctx)?;
    // Checked before any call: the closed defaults of §6.2.
    if let TokenAction::Create { cidr, .. } = &action
        && cidr.is_empty()
    {
        return Err(Fail(
            3,
            "--cidr is required; a scoped token is always bound to addresses.".into(),
        ));
    }
    let api = session(ctx, &mut cfg).await?;
    match action {
        TokenAction::Create {
            vault,
            name,
            read,
            write,
            prefix,
            expires,
            cidr,
        } => {
            let days: u32 = expires
                .trim_end_matches('d')
                .parse()
                .map_err(|_| Fail(3, format!("--expires {expires}: days, e.g. 30d")))?;
            let scopes = token_scopes(read, write, prefix)?;
            let v = vault_named(&api, &vault).await?;
            let t = match api
                .create_token(v, name.as_deref(), &scopes, days, &cidr)
                .await
            {
                Err(e) if e.kind() == "token_write_disabled" => {
                    return Err(Fail(
                        3,
                        "this tenant does not allow tokens to write (scopedTokens.write).".into(),
                    ));
                }
                other => other?,
            };
            let secret = t.token.unwrap_or_default();
            let shown: String = secret.chars().take(13).collect();
            println!(
                "Token {}  {shown}..._****  expires {}  from {}",
                t.name,
                t.expires_at.get(..10).unwrap_or_default(),
                cidr.join(", ")
            );
            for r in &t.scopes {
                println!("  {:<5}  {vault}/{}", r.mode, r.path);
            }
            // A broader rule of the same token covers this one: both hold. A
            // write rule covers a read one; a read rule never covers a write.
            for r in &scopes {
                if scopes.iter().any(|o| {
                    o.path != r.path
                        && fold(&r.path).starts_with(&fold(&o.path))
                        && (o.mode == r.mode || o.mode == "write")
                }) {
                    println!("note: {} is also covered by a broader rule.", r.path);
                }
            }
            println!("\n    {secret}\n");
            println!("This secret is shown once. Store it now.");
            Ok(0)
        }
        TokenAction::List { vault } => {
            let v = vault_named(&api, &vault).await?;
            let tokens = api.tokens(v).await?;
            if tokens.is_empty() {
                println!("No token for {vault}.");
                return Ok(0);
            }
            println!(
                "{:<16} {:<36} {:<36} {:<11} {:<11} {:<8} FROM",
                "NAME", "ID", "SCOPES", "EXPIRES", "LAST USED", "STATE"
            );
            for t in tokens {
                let state = if t.revoked { "revoked" } else { "active" };
                // `r:` and `w:`: a write token is told apart by a letter,
                // not by a colour (§11).
                let scopes: Vec<String> = t
                    .scopes
                    .iter()
                    .map(|r| {
                        let mode = if r.mode == "write" { "w" } else { "r" };
                        if r.path.is_empty() {
                            format!("{mode}:(whole vault)")
                        } else {
                            format!("{mode}:{}", r.path)
                        }
                    })
                    .collect();
                println!(
                    "{:<16} {:<36} {:<36} {:<11} {:<11} {:<8} {}",
                    t.name,
                    t.id,
                    scopes.join(" "),
                    t.expires_at.get(..10).unwrap_or_default(),
                    t.last_used_at
                        .as_deref()
                        .and_then(|l| l.get(..10))
                        .unwrap_or("never"),
                    state,
                    t.cidrs.join(", ")
                );
            }
            Ok(0)
        }
        TokenAction::Revoke { vault, id } => {
            let v = vault_named(&api, &vault).await?;
            api.revoke_token(v, id).await?;
            println!("Token {id} revoked.");
            Ok(0)
        }
    }
}

/// `kura search <vault> <query>`: best matches first, with a snippet (§6.7).
pub async fn search(ctx: &Ctx, vault: &str, query: &str) -> Result<u8, Fail> {
    let mut cfg = config(ctx)?;
    let api = session(ctx, &mut cfg).await?;
    let v = vault_named(&api, vault).await?;
    let hits = match api.search(v, query).await {
        Err(e) if e.kind() == "search_disabled" => {
            return Err(Fail(
                3,
                format!("{vault}: search is not enabled for this tenant"),
            ));
        }
        other => other?,
    };
    if hits.is_empty() {
        println!("No match.");
    }
    // Note text is written by other people: no control character reaches
    // the terminal (review 7 L7).
    let clean = |s: &str| -> String { s.chars().filter(|c| !c.is_control()).collect() };
    for h in &hits {
        println!("{}", clean(&h.path));
        if let Some(s) = &h.snippet {
            println!("    {}", clean(s));
        }
    }
    Ok(0)
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

    #[test]
    fn bytes_for_people() {
        assert_eq!(human(512), "512 B");
        assert_eq!(human(1536), "1.5 KiB");
        assert_eq!(human(1_288_490_189), "1.2 GiB");
        assert_eq!(human(3 << 40), "3.0 TiB");
    }
}
