//! The sync engine (RFC 0001 §6.3, §6.8): local state, change detection,
//! push and pull. File, state and network access go through [`Host`] and
//! [`Remote`], so each shell supplies its own and tests supply fakes. No
//! panic, no clock and no random source here: this runs in WASM too.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::chunk::{ChunkHash, ChunkParams, blob_hash, chunks};
use crate::path::{PathError, VaultPath};
use crate::wire::{
    ChangesResponse, CommitRequest, CommitResponse, EntryId, EntryState, Operation, Outcome,
};

/// Bumped when the local state's shape changes; an unknown version is discarded.
pub const STATE_VERSION: u32 = 1;
/// Operations per commit and hashes per `chunks/missing` call (§4.4).
pub const BATCH: usize = 1000;
/// Attempts of one commit when the server is unreachable (§6.11).
const COMMIT_ATTEMPTS: usize = 3;

/// A file as the host sees it. `path` is relative and `/`-separated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMeta {
    pub path: String,
    pub size: u64,
    /// Modification time in milliseconds; only compared for equality.
    pub mtime: i64,
}

/// Size and modification time when the file was last hashed or written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stamp {
    pub size: u64,
    pub mtime: i64,
}

impl From<&FileMeta> for Stamp {
    fn from(m: &FileMeta) -> Self {
        Self {
            size: m.size,
            mtime: m.mtime,
        }
    }
}

/// What this device knows of one entry: the revision its local file holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Known {
    pub path: VaultPath,
    pub revision: Uuid,
    pub blob: ChunkHash,
    pub size: u64,
    pub stamp: Option<Stamp>,
    /// Pushed again against `revision` even if unchanged, and never written
    /// over by a pull until then: after `history_rewound`, the server may
    /// have lost this version (§6.3).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub resend: bool,
}

/// A remote version about to be written, recorded first (§6.8 journal).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pending {
    pub state: EntryState,
}

/// One device's state for one vault, persisted by the host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalState {
    pub version: u32,
    pub device: Uuid,
    pub vault: Uuid,
    pub epoch: Option<Uuid>,
    pub cursor: u64,
    pub entries: BTreeMap<EntryId, Known>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending: Option<Pending>,
}

impl LocalState {
    pub fn new(device: Uuid, vault: Uuid) -> Self {
        Self {
            version: STATE_VERSION,
            device,
            vault,
            epoch: None,
            cursor: 0,
            entries: BTreeMap::new(),
            pending: None,
        }
    }

    /// A stored state, unless its version is unknown or it belongs to another
    /// device or vault (a copied folder): then a fresh one (§6.8).
    pub fn adopt(stored: Option<Self>, device: Uuid, vault: Uuid) -> Self {
        stored
            .filter(|s| s.version == STATE_VERSION && s.device == device && s.vault == vault)
            .unwrap_or_else(|| Self::new(device, vault))
    }

    fn entry_at(&self, path: &VaultPath) -> Option<EntryId> {
        self.entries
            .iter()
            .find(|(_, k)| &k.path == path)
            .map(|(e, _)| *e)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct HostError(pub String);

/// A problem document's type name (`insufficient_role`…) and status.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RemoteError {
    #[error("{kind} ({status}){}", detail.as_deref().map(|d| format!(": {d}")).unwrap_or_default())]
    Problem {
        status: u16,
        kind: String,
        detail: Option<String>,
    },
    #[error("server unreachable: {0}")]
    Unavailable(String),
}

impl RemoteError {
    pub fn kind(&self) -> &str {
        match self {
            Self::Problem { kind, .. } => kind,
            Self::Unavailable(_) => "unavailable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SyncError {
    #[error(transparent)]
    Host(#[from] HostError),
    #[error(transparent)]
    Remote(#[from] RemoteError),
    #[error("the server sent content that does not match its address")]
    Corrupt,
    /// A full resync would move this many of the local files to the trash;
    /// nothing was changed. Run again with [`Options::allow_deletions`] once
    /// the user agreed (§6.3).
    #[error("a full resync would move {deletions} of {files} local files to the trash")]
    TooManyDeletions { deletions: usize, files: usize },
    /// Local changes outside what [`Options::writable`] allows; nothing was
    /// uploaded (RFC 0004 §6.6).
    #[error("{} local changes are outside the folders this caller may write", paths.len())]
    NotWritable { paths: Vec<String> },
}

/// The local side: files, the persisted state, time and randomness.
#[expect(
    async_fn_in_trait,
    reason = "shells run single-threaded executors; no Send bound wanted in WASM"
)]
pub trait Host {
    /// Every file of the vault folder, recursively; no symbolic link followed.
    async fn list(&self) -> Result<Vec<FileMeta>, HostError>;
    async fn read(&self, path: &VaultPath) -> Result<Vec<u8>, HostError>;
    async fn stat(&self, path: &VaultPath) -> Result<Option<FileMeta>, HostError>;
    /// Writes through a temporary file renamed into place, creating parents,
    /// never following a symbolic link.
    async fn write(&self, path: &VaultPath, data: &[u8]) -> Result<FileMeta, HostError>;
    async fn rename(&self, from: &VaultPath, to: &VaultPath) -> Result<FileMeta, HostError>;
    /// Moves a file to the vault's `.trash`, never unlinks it (§6.3).
    async fn trash(&self, path: &VaultPath) -> Result<(), HostError>;
    async fn save(&self, state: &LocalState) -> Result<(), HostError>;
    fn now_ms(&self) -> u64;
    fn random(&self) -> [u8; 16];
}

/// The server, one vault.
#[expect(
    async_fn_in_trait,
    reason = "shells run single-threaded executors; no Send bound wanted in WASM"
)]
pub trait Remote {
    async fn missing(&self, hashes: &[ChunkHash]) -> Result<Vec<ChunkHash>, RemoteError>;
    async fn put_chunk(&self, hash: &ChunkHash, data: &[u8]) -> Result<(), RemoteError>;
    async fn get_chunk(&self, hash: &ChunkHash) -> Result<Vec<u8>, RemoteError>;
    async fn put_blob(
        &self,
        hash: &ChunkHash,
        size: u64,
        chunks: &[ChunkHash],
    ) -> Result<(), RemoteError>;
    async fn get_blob(&self, hash: &ChunkHash) -> Result<(u64, Vec<ChunkHash>), RemoteError>;
    /// Many manifests at once (RFC 0002): those the server has, by blob
    /// hash; `None` when it has no batch route (an older server answers
    /// `405`), and the engine reads one by one for the rest of the run.
    async fn get_blobs(
        &self,
        hashes: &[ChunkHash],
    ) -> Result<Option<Vec<(ChunkHash, u64, Vec<ChunkHash>)>>, RemoteError> {
        let _ = hashes;
        Ok(None)
    }
    /// The raw `chunks/batch` answer for these chunks, parsed by
    /// [`crate::wire::parse_records`]; `None` as for [`Self::get_blobs`].
    async fn get_chunk_records(
        &self,
        hashes: &[ChunkHash],
    ) -> Result<Option<Vec<u8>>, RemoteError> {
        let _ = hashes;
        Ok(None)
    }
    async fn commit(&self, req: &CommitRequest) -> Result<CommitResponse, RemoteError>;
    /// `start` is the cursor the pass began from (`since` for its first
    /// page): the server checks its horizon against it.
    async fn changes(
        &self,
        since: u64,
        epoch: Option<Uuid>,
        start: u64,
    ) -> Result<ChangesResponse, RemoteError>;
}

/// What one run did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub pushed: usize,
    /// Bytes of the files `pushed` counts (what a dry run would upload, §4.5).
    pub pushed_bytes: u64,
    pub pulled: usize,
    pub trashed: usize,
    /// Conflict files created by this run, as named on the server.
    pub conflicts: Vec<VaultPath>,
    /// Files that cannot sync, with the rule they break.
    pub unsyncable: Vec<(String, String)>,
    /// Remote changes not applied because the local file changed meanwhile;
    /// the next run pushes the local edit against its old base.
    pub deferred: Vec<VaultPath>,
    /// The push was refused (role, quota): local changes are kept.
    pub refused: Option<RemoteError>,
    /// Remote files this device could not write, with why; retried next run.
    pub failed: Vec<(VaultPath, String)>,
    /// A full resync ran, after this answer: `cursor_expired` or
    /// `history_rewound` (§6.3).
    pub resynced: Option<String>,
}

/// Whether a vault path may be written by this caller.
pub type Writable = Box<dyn Fn(&str) -> bool + Send + Sync>;

pub struct Options {
    pub params: ChunkParams,
    /// Larger files are listed as unsyncable (§6.8).
    pub max_file_size: u64,
    /// Only report what would be pushed; nothing is uploaded or pulled.
    pub dry_run: bool,
    /// The user agreed that a full resync may trash more than the guard allows.
    pub allow_deletions: bool,
    /// The configuration space (§6.8): pull before pushing, and a local file
    /// never synced here gives way to the one already on the server (to
    /// `.trash`), so a new device never overwrites the user's settings with
    /// its defaults (phase 8 review H1).
    pub prefer_remote: bool,
    /// Where this caller may write, when it is not everywhere (a scoped
    /// token, RFC 0004 §6.6): a change elsewhere stops the push before any
    /// upload, with [`SyncError::NotWritable`].
    pub writable: Option<Writable>,
}

/// A full resync asks first above this share of the local files, or this
/// many files (§6.3).
pub const GUARD_PERCENT: usize = 10;
pub const GUARD_FILES: usize = 50;

/// A local edit waiting for its commit result.
struct Pushed {
    local: VaultPath,
    stamp: Stamp,
    blob: ChunkHash,
    size: u64,
}

/// One sync run: recover the journal, push local changes, pull remote ones.
pub async fn sync<H: Host, R: Remote>(
    host: &H,
    remote: &R,
    state: &mut LocalState,
    opts: &Options,
) -> Result<Report, SyncError> {
    let mut report = Report::default();
    recover(host, state, opts.params).await?;
    loop {
        match run(host, remote, state, opts, &mut report).await {
            // The server lost or purged history this device holds: once per
            // run, match everything again, then push and pull (§6.3).
            Err(SyncError::Remote(RemoteError::Problem {
                status: 410, kind, ..
            })) if report.resynced.is_none() && !opts.dry_run => {
                resync(
                    host,
                    remote,
                    state,
                    opts,
                    kind == "history_rewound",
                    &mut report,
                )
                .await?;
                report.resynced = Some(kind);
            }
            other => return other.map(|()| report),
        }
    }
}

async fn run<H: Host, R: Remote>(
    host: &H,
    remote: &R,
    state: &mut LocalState,
    opts: &Options,
    report: &mut Report,
) -> Result<(), SyncError> {
    if opts.prefer_remote && !opts.dry_run {
        pull(host, remote, state, opts, report).await?;
    }
    match push(host, remote, state, opts, report).await {
        Ok(()) => {}
        Err(SyncError::Remote(
            e @ RemoteError::Problem {
                status: 403 | 422, ..
            },
        )) => {
            report.refused = Some(e);
            // Files waiting to be resent after a rewind cannot go up: they
            // are forgotten, so the file stays as an unsynced local file,
            // never written over, instead of holding every later pull.
            state.entries.retain(|_, k| !k.resend);
        }
        Err(e) => return Err(e),
    }
    if !opts.dry_run {
        pull(host, remote, state, opts, report).await?;
    }
    Ok(())
}

/// Full resync (§6.3, decision 32): every entry the server holds, matched to
/// the local state by entry id, never by path. A known entry the server no
/// longer has was purged after its deletion (`cursor_expired`): its file goes
/// to the trash if unchanged here, otherwise it is pushed as new. After
/// `history_rewound` the server lost history this device saw: nothing local
/// is deleted, and what the server lacks or holds older is pushed back.
/// Above the guard, nothing changes until the user agrees.
async fn resync<H: Host, R: Remote>(
    host: &H,
    remote: &R,
    state: &mut LocalState,
    opts: &Options,
    rewound: bool,
    report: &mut Report,
) -> Result<(), SyncError> {
    let mut server: HashMap<EntryId, EntryState> = HashMap::new();
    let mut pos = 0;
    let epoch = loop {
        let page = remote.changes(pos, None, 0).await?;
        let before = pos;
        for item in page.items {
            pos = pos.max(item.seq);
            server.insert(item.entry, item);
        }
        if page.next.is_none() {
            break page.epoch;
        }
        if pos <= before {
            return Err(
                RemoteError::Unavailable("the server's changes do not advance".into()).into(),
            );
        }
    };
    // What each known entry becomes: forgotten (its file, if any, is pushed
    // as new), resent against its base, trashed, or left to the pull below.
    let mut forget = Vec::new();
    let mut resend = Vec::new();
    let mut trash = Vec::new();
    for (e, k) in &state.entries {
        let exists = host.stat(&k.path).await?.is_some();
        match server.get(e) {
            None if rewound => forget.push(*e),
            None => {
                if exists && !k.resend && untouched(host, k).await? {
                    trash.push(*e);
                } else {
                    forget.push(*e);
                }
            }
            // A deletion the server holds, or a version other than this
            // device's: never applied over the file after a rewind. Resent,
            // it restores a deletion and becomes a conflict file if the
            // server lost it; unchanged since a base the server knows, it
            // takes the server's head.
            Some(s)
                if rewound
                    && (s.deleted || (s.revision != k.revision && s.blob != Some(k.blob))) =>
            {
                if exists {
                    resend.push(*e);
                } else {
                    forget.push(*e);
                }
            }
            // A file still to be resent holds a version the server may lack.
            Some(s) if s.deleted && exists && !k.resend && untouched(host, k).await? => {
                trash.push(*e);
            }
            Some(_) => {}
        }
    }
    if !trash.is_empty() && !opts.allow_deletions {
        let files = host
            .list()
            .await?
            .iter()
            .filter(|f| VaultPath::from_local(&f.path).is_ok())
            .count();
        let deletions = trash.len();
        if deletions > GUARD_FILES
            || deletions.saturating_mul(100) > files.saturating_mul(GUARD_PERCENT)
        {
            return Err(SyncError::TooManyDeletions { deletions, files });
        }
    }
    for e in forget {
        state.entries.remove(&e);
    }
    for e in resend {
        if let Some(k) = state.entries.get_mut(&e) {
            k.resend = true;
        }
    }
    for e in trash {
        if let Some(k) = state.entries.remove(&e) {
            host.trash(&k.path).await?;
            report.trashed = report.trashed.saturating_add(1);
        }
    }
    // Everything is pulled again from 0 under the new epoch; entries already
    // in place are skipped by revision.
    state.epoch = Some(epoch);
    state.cursor = 0;
    host.save(state).await?;
    Ok(())
}

/// A write interrupted by a crash: adopt the file if it holds the new content.
async fn recover<H: Host>(
    host: &H,
    state: &mut LocalState,
    params: ChunkParams,
) -> Result<(), SyncError> {
    let Some(p) = state.pending.take() else {
        return Ok(());
    };
    let s = p.state;
    if let (Some(meta), Some(blob)) = (host.stat(&s.path).await?, s.blob) {
        let data = host.read(&s.path).await?;
        // Otherwise the file is left alone: the next scan sees it as a local edit.
        if blob_of(&data, params).0 == blob {
            // Killed between writing the new path and trashing the old one
            // (a remote move with an edit): the old file, if untouched, goes
            // now, or it would be pushed as a new note (review 2026-11-15 #10).
            if let Some(old) = state.entries.get(&s.entry).filter(|k| k.path != s.path)
                && host.stat(&old.path).await?.is_some()
                && untouched(host, old).await?
            {
                host.trash(&old.path).await?;
            }
            state.entries.insert(
                s.entry,
                Known {
                    path: s.path.clone(),
                    revision: s.revision,
                    blob,
                    size: s.size,
                    stamp: Some(Stamp::from(&meta)),
                    resend: false,
                },
            );
        }
    }
    host.save(state).await?;
    Ok(())
}

/// The blob address of a file's content, chunked with the vault's parameters.
fn blob_of(data: &[u8], params: ChunkParams) -> (ChunkHash, Vec<ChunkHash>) {
    let hashes: Vec<ChunkHash> = chunks(data, params).iter().map(|c| c.hash).collect();
    (blob_hash(data.len() as u64, &hashes), hashes)
}

async fn push<H: Host, R: Remote>(
    host: &H,
    remote: &R,
    state: &mut LocalState,
    opts: &Options,
    report: &mut Report,
) -> Result<(), SyncError> {
    let files = host.list().await?;
    let mut local: HashMap<VaultPath, FileMeta> = HashMap::new();
    // Too large to sync, but present: never a deletion (review 2026-11-15 #1).
    let mut oversized: std::collections::HashSet<VaultPath> = std::collections::HashSet::new();
    for f in files {
        match VaultPath::from_local(&f.path) {
            Ok(p) if f.size > opts.max_file_size => {
                oversized.insert(p.clone());
                report.unsyncable.push((
                    p.to_string(),
                    format!("larger than {} bytes", opts.max_file_size),
                ));
            }
            Ok(p) => {
                local.insert(p, f);
            }
            // Dot-folders (.obsidian, .trash, .git) are not notes (decision 28).
            Err(PathError::Hidden) => {}
            Err(e) => report.unsyncable.push((f.path, e.to_string())),
        }
    }
    // Entries whose file is gone: deletions, unless a new file has their content (a rename).
    let mut gone: Vec<EntryId> = state
        .entries
        .iter()
        .filter(|(_, k)| !local.contains_key(&k.path) && !oversized.contains(&k.path))
        .map(|(e, _)| *e)
        .collect();
    let mut ops: Vec<(Operation, Option<Pushed>)> = Vec::new();
    let by_path: HashMap<VaultPath, EntryId> = state
        .entries
        .iter()
        .map(|(e, k)| (k.path.clone(), *e))
        .collect();
    let mut paths: Vec<&VaultPath> = local.keys().collect();
    paths.sort();
    for path in paths {
        let Some(meta) = local.get(path) else {
            continue;
        };
        let stamp = Stamp::from(meta);
        let known_id = by_path.get(path).copied();
        if let Some(k) = known_id.and_then(|e| state.entries.get(&e))
            && k.stamp == Some(stamp)
            && !k.resend
        {
            continue;
        }
        let data = host.read(path).await?;
        let (blob, _) = blob_of(&data, opts.params);
        let size = data.len() as u64;
        let (entry, base) = match known_id.and_then(|e| state.entries.get_mut(&e).map(|k| (e, k))) {
            Some((_, k)) if k.blob == blob && !k.resend => {
                k.stamp = Some(stamp);
                continue;
            }
            Some((e, k)) => (e, Some(k.revision)),
            None => match gone
                .iter()
                .position(|e| state.entries.get(e).is_some_and(|k| k.blob == blob))
            {
                Some(i) => {
                    let e = gone.swap_remove(i);
                    (e, state.entries.get(&e).map(|k| k.revision))
                }
                None => (EntryId::new(host.now_ms(), &ten(host.random())), None),
            },
        };
        ops.push((
            Operation::Put {
                entry,
                path: path.clone(),
                base,
                blob,
                size,
            },
            Some(Pushed {
                local: path.clone(),
                stamp,
                blob,
                size,
            }),
        ));
    }
    // Deletions first, so a path they free is free for the puts that follow
    // in the same commit (review 3.6 F7).
    let mut deletes: Vec<(Operation, Option<Pushed>)> = gone
        .iter()
        .filter_map(|e| {
            state.entries.get(e).map(|k| {
                (
                    Operation::Delete {
                        entry: *e,
                        base: k.revision,
                    },
                    None,
                )
            })
        })
        .collect();
    deletes.extend(ops);
    let ops = deletes;
    // Counted before any upload, so a refusal can say how many files it kept.
    report.pushed = ops.len();
    report.pushed_bytes = ops
        .iter()
        .filter_map(|(_, p)| p.as_ref().map(|p| p.size))
        .fold(0u64, u64::saturating_add);
    if let Some(writable) = &opts.writable {
        // A put is checked where it lands and where the entry lies (a rename
        // writes both ends); a deletion, where the entry lies.
        let mut paths: Vec<String> = ops
            .iter()
            .flat_map(|(op, _)| {
                let (entry, to) = match op {
                    Operation::Put { entry, path, .. } => (entry, Some(path)),
                    Operation::Delete { entry, .. } => (entry, None),
                };
                state
                    .entries
                    .get(entry)
                    .map(|k| &k.path)
                    .into_iter()
                    .chain(to)
            })
            .map(|p| p.as_str().to_owned())
            .filter(|p| !writable(p))
            .collect();
        paths.sort();
        paths.dedup();
        if !paths.is_empty() {
            return Err(SyncError::NotWritable { paths });
        }
    }
    if opts.dry_run || ops.is_empty() {
        return Ok(());
    }
    // Batch by batch: upload what the commit references, then commit, so an
    // interrupted push keeps what it committed.
    for batch in ops.chunks(BATCH) {
        let batch = upload_batch(host, remote, batch, opts.params).await?;
        if batch.is_empty() {
            continue;
        }
        let req = CommitRequest {
            idempotency_key: hex(&host.random()),
            operations: batch.iter().map(|(o, _)| (*o).clone()).collect(),
            epoch: state.epoch,
            space: None,
        };
        let resp = commit(remote, &req).await?;
        for (&(op, pushed), result) in batch.iter().zip(resp.results) {
            // A local file that cannot be moved or written now (an open note)
            // fails that file only: the rest of the batch is settled and saved,
            // and the retried push lands on the same conflict file (review 4 H2).
            let path = pushed
                .as_ref()
                .map(|p| p.local.clone())
                .unwrap_or_else(|| result.head.path.clone());
            match settle(
                host,
                remote,
                state,
                opts,
                op,
                pushed.as_ref(),
                result,
                report,
            )
            .await
            {
                Ok(()) => {}
                Err(e @ (SyncError::Host(_) | SyncError::Corrupt)) => {
                    report.failed.push((path, e.to_string()))
                }
                Err(e) => {
                    host.save(state).await?;
                    return Err(e);
                }
            }
        }
        host.save(state).await?;
    }
    Ok(())
}

fn ten(r: [u8; 16]) -> [u8; 10] {
    let mut out = [0u8; 10];
    for (o, b) in out.iter_mut().zip(r) {
        *o = b;
    }
    out
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

async fn commit<R: Remote>(remote: &R, req: &CommitRequest) -> Result<CommitResponse, SyncError> {
    let mut last = None;
    for _ in 0..COMMIT_ATTEMPTS {
        match remote.commit(req).await {
            Ok(r) => return Ok(r),
            // A retry with the same key returns the stored result (§6.3).
            Err(e @ RemoteError::Unavailable(_)) => last = Some(e),
            Err(e) => return Err(e.into()),
        }
    }
    Err(last
        .unwrap_or_else(|| RemoteError::Unavailable("no attempt".into()))
        .into())
}

/// Chunks the server lacks, then the manifest.
/// Uploads the chunks a batch's files need, asking the server with one call
/// per 1,000 hashes across files, then each manifest. A file changed since
/// the scan is left out, for the next run. Files are read twice at most, so
/// the batch is never held in memory whole.
async fn upload_batch<'a, H: Host, R: Remote>(
    host: &H,
    remote: &R,
    batch: &'a [(Operation, Option<Pushed>)],
    params: ChunkParams,
) -> Result<Vec<(&'a Operation, &'a Option<Pushed>)>, SyncError> {
    let mut ready = Vec::with_capacity(batch.len());
    let mut manifests: Vec<(usize, Vec<ChunkHash>)> = Vec::new();
    for (op, pushed) in batch {
        if let Some(p) = pushed {
            let data = host.read(&p.local).await?;
            let (blob, hashes) = blob_of(&data, params);
            if blob != p.blob {
                continue;
            }
            manifests.push((ready.len(), hashes));
        }
        ready.push((op, pushed));
    }
    let mut wanted: Vec<ChunkHash> = manifests
        .iter()
        .flat_map(|(_, h)| h.iter().copied())
        .collect();
    wanted.sort();
    wanted.dedup();
    let mut missing = std::collections::HashSet::new();
    for part in wanted.chunks(BATCH) {
        missing.extend(remote.missing(part).await?);
    }
    for (i, hashes) in &manifests {
        let Some((_, Some(p))) = ready.get(*i) else {
            continue;
        };
        if hashes.iter().any(|h| missing.contains(h)) {
            let data = host.read(&p.local).await?;
            for span in chunks(&data, params) {
                if missing.remove(&span.hash) {
                    let part = data
                        .get(span.offset..span.offset.saturating_add(span.len))
                        .ok_or(SyncError::Corrupt)?;
                    if ChunkHash::of(part) != span.hash {
                        return Err(SyncError::Corrupt);
                    }
                    remote.put_chunk(&span.hash, part).await?;
                }
            }
        }
        remote.put_blob(&p.blob, p.size, hashes).await?;
    }
    Ok(ready)
}

/// Applies one commit result to the local state and files.
#[expect(
    clippy::too_many_arguments,
    reason = "one call site, each argument distinct"
)]
async fn settle<H: Host, R: Remote>(
    host: &H,
    remote: &R,
    state: &mut LocalState,
    opts: &Options,
    op: &Operation,
    pushed: Option<&Pushed>,
    result: crate::wire::OpResult,
    report: &mut Report,
) -> Result<(), SyncError> {
    let head = result.head;
    match (result.outcome, pushed, result.conflict) {
        (Outcome::Conflicted, Some(p), Some(conflict)) => {
            // Our content lives on as the conflict file; the head takes its path.
            let moved = host.rename(&p.local, &conflict.path).await?;
            state.entries.insert(
                conflict.entry,
                Known {
                    path: conflict.path.clone(),
                    revision: conflict.revision,
                    blob: p.blob,
                    size: p.size,
                    // Edited since the scan: no stamp, so the next run rehashes it.
                    stamp: (Stamp::from(&moved) == p.stamp).then_some(p.stamp),
                    resend: false,
                },
            );
            // For an edit, the head keeps the old base and is applied below as
            // a remote change; for a new file, its id now names the conflict file.
            report.conflicts.push(conflict.path);
            apply(host, remote, state, &head, opts, report).await?;
        }
        (_, None, _) if head.deleted => {
            state.entries.remove(&head.entry);
        }
        (Outcome::Conflicted, None, _) => {
            // A delete of an entry edited since: dropped, the file comes back.
            apply(host, remote, state, &head, opts, report).await?;
        }
        (_, Some(p), _) if head.blob.is_some_and(|b| b != p.blob) => {
            // Merged with edits made elsewhere (phase 4): our edit is in the
            // head. Known as our pushed version on its old base, so the head is
            // written only if the file is still what was pushed; an edit made
            // since is pushed again against that base and merged again.
            let base = match op {
                Operation::Put { base, .. } => base.unwrap_or(head.revision),
                Operation::Delete { base, .. } => *base,
            };
            state.entries.insert(
                op.entry(),
                Known {
                    path: p.local.clone(),
                    revision: base,
                    blob: p.blob,
                    size: p.size,
                    stamp: Some(p.stamp),
                    resend: false,
                },
            );
            apply(host, remote, state, &head, opts, report).await?;
        }
        (_, Some(p), _) => {
            if head.entry != op.entry() {
                // Unified with an identical entry pushed by another device.
                state.entries.remove(&op.entry());
            }
            let stamp = if head.path == p.local {
                Some(p.stamp)
            } else {
                // The server suffixed or re-cased the name: follow it. A file
                // edited since the scan gets no stamp, so the next run rehashes it.
                let moved = Stamp::from(&relocate(host, state, None, &p.local, &head.path).await?);
                (moved == p.stamp).then_some(p.stamp)
            };
            state.entries.insert(
                head.entry,
                Known {
                    path: head.path.clone(),
                    revision: head.revision,
                    blob: p.blob,
                    size: head.size,
                    stamp,
                    resend: false,
                },
            );
        }
        (_, None, _) => {}
    }
    Ok(())
}

async fn pull<H: Host, R: Remote>(
    host: &H,
    remote: &R,
    state: &mut LocalState,
    opts: &Options,
    report: &mut Report,
) -> Result<(), SyncError> {
    // Pages are read from `pos`; the saved cursor stops before the first item
    // this device did not write (a failure, or a local edit in the way), so
    // the next run fetches it again, while later pages still arrive (review
    // 3.6 S2, S3).
    let mut pos = state.cursor;
    let start = state.cursor;
    let mut held = false;
    // RFC 0002: the content of the next items is fetched a window at a time.
    let pre = Prefetch::new(remote);
    loop {
        let page = remote.changes(pos, state.epoch, start).await?;
        state.epoch = Some(page.epoch);
        let before = pos;
        let mut window_end = 0;
        for (i, item) in page.items.iter().enumerate() {
            if i >= window_end {
                window_end = pre.fill(&page.items, i, state).await?;
            }
            let current = state.entries.get(&item.entry).map(|k| k.revision);
            if current != Some(item.revision) {
                match apply(host, &pre, state, item, opts, report).await {
                    Ok(Applied::Done) => {}
                    Ok(Applied::Deferred) => held = true,
                    Err(e @ (SyncError::Host(_) | SyncError::Corrupt)) => {
                        report.failed.push((item.path.clone(), e.to_string()));
                        held = true;
                    }
                    Err(e) => return Err(e),
                }
            }
            pos = pos.max(item.seq);
            if !held {
                state.cursor = pos;
            }
        }
        // The last page, nothing held: everything up to the head this page was
        // read at is applied, so the cursor reaches it. A view that rarely
        // changes (a prefix, a configuration space) would otherwise stay below
        // a horizon moved by others' purged deletions, and 410 forever.
        if page.next.is_none() && !held {
            state.cursor = state.cursor.max(page.head_seq);
        }
        // The cursor advances once the page is applied (§6.3).
        host.save(state).await?;
        if page.next.is_none() {
            return Ok(());
        }
        if pos <= before {
            return Err(
                RemoteError::Unavailable("the server's changes do not advance".into()).into(),
            );
        }
    }
}

/// Moves a file. A change of case alone goes through a temporary name: on a
/// case-insensitive disk the target is the file itself, so a host refuses to
/// move onto it. The known entry, if any, points at the temporary name while
/// it is there, so a crash in between leaves a tracked file.
async fn relocate<H: Host>(
    host: &H,
    state: &mut LocalState,
    entry: Option<EntryId>,
    from: &VaultPath,
    to: &VaultPath,
) -> Result<FileMeta, SyncError> {
    if !from.same_but_case(to) {
        return Ok(host.rename(from, to).await?);
    }
    let tmp = to
        .suffixed("kura-move")
        .map_err(|e| HostError(format!("no temporary name for {to}: {e}")))?;
    host.rename(from, &tmp).await?;
    let track = |state: &mut LocalState, path: &VaultPath| {
        if let Some(k) = entry.and_then(|e| state.entries.get_mut(&e)) {
            k.path = path.clone();
        }
    };
    track(state, &tmp);
    host.save(state).await?;
    match host.rename(&tmp, to).await {
        Ok(meta) => Ok(meta),
        // Another file at `to` (a case-sensitive disk): back where it was.
        Err(e) => {
            host.rename(&tmp, from).await?;
            track(state, from);
            host.save(state).await?;
            Err(e.into())
        }
    }
}

/// Is the local file still the version this device last synced?
async fn untouched<H: Host>(host: &H, k: &Known) -> Result<bool, SyncError> {
    Ok(match host.stat(&k.path).await? {
        None => true,
        Some(m) => k.stamp == Some(Stamp::from(&m)),
    })
}

/// What [`apply`] did with one remote state.
enum Applied {
    Done,
    /// Not written: a local file in the way was changed here.
    Deferred,
}

/// Brings one remote entry state onto the disk, never over a local edit.
async fn apply<H: Host, R: Remote>(
    host: &H,
    remote: &R,
    state: &mut LocalState,
    item: &EntryState,
    opts: &Options,
    report: &mut Report,
) -> Result<Applied, SyncError> {
    let mut known = state.entries.get(&item.entry).cloned();
    if let Some(k) = &known
        && (k.resend || !untouched(host, k).await?)
    {
        // Changed here since: keep the old base, so the next push conflicts
        // instead of overwriting either side.
        report.deferred.push(k.path.clone());
        return Ok(Applied::Deferred);
    }
    let Some(blob) = item.blob.filter(|_| !item.deleted) else {
        if let Some(k) = known {
            if host.stat(&k.path).await?.is_some() {
                host.trash(&k.path).await?;
                report.trashed = report.trashed.saturating_add(1);
            }
            state.entries.remove(&item.entry);
        }
        return Ok(Applied::Done);
    };
    if item.size > opts.max_file_size {
        report.unsyncable.push((
            item.path.to_string(),
            format!("larger than {} bytes", opts.max_file_size),
        ));
        return Ok(Applied::Done);
    }
    // The path is held here by another entry: if that file is untouched, the
    // server holds its content, and its own change (later in the log) brings
    // it back where it now lives; move it out of the way (review 3.6 F1).
    if let Some(other) = state.entry_at(&item.path).filter(|e| *e != item.entry) {
        if let Some(o) = state.entries.get(&other).cloned() {
            if !untouched(host, &o).await? {
                report.deferred.push(item.path.clone());
                return Ok(Applied::Deferred);
            }
            if host.stat(&o.path).await?.is_some() {
                host.trash(&o.path).await?;
            }
            state.entries.remove(&other);
        }
    } else if known
        .as_ref()
        .is_none_or(|k| k.path != item.path && !k.path.same_but_case(&item.path))
        && host.stat(&item.path).await?.is_some()
    {
        if !opts.prefer_remote {
            // A local file this device never synced: never written over.
            report.deferred.push(item.path.clone());
            return Ok(Applied::Deferred);
        }
        // Configuration: the server's copy is the user's; this one is kept
        // in `.trash`.
        host.trash(&item.path).await?;
    }
    // A change of case first moves the file, which a case-insensitive disk
    // sees at both paths (review 2026-11-15 #11); the content follows below.
    let mut moved = None;
    if let Some(k) = known.as_mut().filter(|k| k.path.same_but_case(&item.path))
        && host.stat(&k.path).await?.is_some()
    {
        let meta = relocate(host, state, Some(item.entry), &k.path, &item.path).await?;
        k.path = item.path.clone();
        k.stamp = Some(Stamp::from(&meta));
        state.entries.insert(item.entry, k.clone());
        moved = Some(meta);
    }
    let only_moved = match &known {
        Some(k) if k.blob == blob => host.stat(&k.path).await?.is_some(),
        _ => false,
    };
    let meta = match (&known, moved) {
        (Some(_), Some(meta)) if only_moved => meta,
        (Some(k), _) if only_moved => host.rename(&k.path, &item.path).await?,
        _ => {
            let data = download(remote, blob, item.size).await?;
            state.pending = Some(Pending {
                state: item.clone(),
            });
            host.save(state).await?;
            let meta = host.write(&item.path, &data).await?;
            if let Some(k) = known.as_ref().filter(|k| k.path != item.path)
                && host.stat(&k.path).await?.is_some()
            {
                host.trash(&k.path).await?;
            }
            state.pending = None;
            meta
        }
    };
    state.entries.insert(
        item.entry,
        Known {
            path: item.path.clone(),
            revision: item.revision,
            blob,
            size: item.size,
            stamp: Some(Stamp::from(&meta)),
            resend: false,
        },
    );
    report.pulled = report.pulled.saturating_add(1);
    Ok(Applied::Done)
}

/// A blob's content, every chunk and the whole checked against their addresses.
async fn download<R: Remote>(remote: &R, blob: ChunkHash, size: u64) -> Result<Vec<u8>, SyncError> {
    let (stated, hashes) = remote.get_blob(&blob).await?;
    if stated != size || blob_hash(size, &hashes) != blob {
        return Err(SyncError::Corrupt);
    }
    // The size is the server's word until the bytes arrive: reserve at most one chunk.
    let mut out = Vec::with_capacity(
        usize::try_from(size.min(crate::chunk::MAX_CHUNK as u64)).unwrap_or_default(),
    );
    for h in &hashes {
        let part = remote.get_chunk(h).await?;
        // The stated size bounds memory before the bytes are kept (review 4 L6).
        if ChunkHash::of(&part) != *h || (out.len() as u64).saturating_add(part.len() as u64) > size
        {
            return Err(SyncError::Corrupt);
        }
        out.extend_from_slice(&part);
    }
    if out.len() as u64 != size {
        return Err(SyncError::Corrupt);
    }
    Ok(out)
}

/// Stated bytes of content fetched ahead per window (RFC 0002 decision 4).
pub const WINDOW: u64 = 16 << 20;

/// A [`Remote`] that serves manifests and chunks fetched ahead in batches,
/// and asks the real one for anything else (RFC 0002 §5.1). What it serves
/// is checked by [`download`] exactly as single reads are.
struct Prefetch<'a, R: Remote> {
    remote: &'a R,
    /// Off for the rest of the run once the server has no batch route.
    batching: std::sync::atomic::AtomicBool,
    blobs: std::sync::Mutex<HashMap<ChunkHash, (u64, Vec<ChunkHash>)>>,
    chunks: std::sync::Mutex<HashMap<ChunkHash, Vec<u8>>>,
}

impl<'a, R: Remote> Prefetch<'a, R> {
    fn new(remote: &'a R) -> Self {
        Self {
            remote,
            batching: std::sync::atomic::AtomicBool::new(true),
            blobs: std::sync::Mutex::default(),
            chunks: std::sync::Mutex::default(),
        }
    }

    fn batching(&self) -> bool {
        self.batching.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn stop_batching(&self) {
        self.batching
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }

    /// Drops what the last window held, then fetches the content of the
    /// items from `from` on, up to [`WINDOW`] of stated size (a larger file
    /// alone); returns where the window ends. Memory is bounded by what
    /// arrives, never by the stated sizes: past `WINDOW` plus one chunk of
    /// received bytes, the rest is left to single reads.
    async fn fill(
        &self,
        items: &[EntryState],
        from: usize,
        state: &LocalState,
    ) -> Result<usize, SyncError> {
        lock(&self.blobs).clear();
        lock(&self.chunks).clear();
        let mut end = from;
        let mut stated = 0u64;
        let mut wanted = Vec::new();
        for item in items.iter().skip(from) {
            let needed = !item.deleted
                && state.entries.get(&item.entry).map(|k| k.revision) != Some(item.revision);
            if let (true, Some(blob)) = (needed, item.blob) {
                if stated > 0 && stated.saturating_add(item.size) > WINDOW {
                    break;
                }
                stated = stated.saturating_add(item.size);
                wanted.push(blob);
            }
            end = end.saturating_add(1);
        }
        wanted.sort();
        wanted.dedup();
        if wanted.is_empty() || !self.batching() {
            return Ok(end.max(from.saturating_add(1)));
        }
        let Some(manifests) = self.remote.get_blobs(&wanted).await? else {
            self.stop_batching();
            return Ok(end.max(from.saturating_add(1)));
        };
        let mut want: Vec<ChunkHash> = Vec::new();
        {
            let mut blobs = lock(&self.blobs);
            for (hash, size, list) in manifests {
                if wanted.binary_search(&hash).is_ok() {
                    want.extend(list.iter().copied());
                    blobs.insert(hash, (size, list));
                }
            }
        }
        want.sort();
        want.dedup();
        let mut received = 0usize;
        let cap = usize::try_from(WINDOW)
            .unwrap_or(usize::MAX)
            .saturating_add(crate::chunk::MAX_CHUNK);
        while !want.is_empty() && received < cap {
            let ask: Vec<ChunkHash> = want.iter().take(BATCH).copied().collect();
            let Some(body) = self.remote.get_chunk_records(&ask).await? else {
                self.stop_batching();
                break;
            };
            // A truncated or garbled answer ends the window: the rest is read
            // one by one (review 2026-11-15 #17).
            let Ok(records) = crate::wire::parse_records(&body) else {
                break;
            };
            let before = want.len();
            let mut chunks = lock(&self.chunks);
            for (hash, data) in records {
                want.retain(|h| *h != hash);
                // Only what was asked and matches its address is kept.
                if let Some(data) = data
                    && ask.contains(&hash)
                    && ChunkHash::of(data) == hash
                {
                    received = received.saturating_add(data.len());
                    chunks.insert(hash, data.to_vec());
                }
            }
            // An answer that settles none of what was asked (empty, or only
            // other hashes) ends the window: the rest is read one by one.
            if want.len() == before {
                break;
            }
        }
        Ok(end.max(from.saturating_add(1)))
    }
}

fn lock<T>(m: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl<R: Remote> Remote for Prefetch<'_, R> {
    async fn missing(&self, hashes: &[ChunkHash]) -> Result<Vec<ChunkHash>, RemoteError> {
        self.remote.missing(hashes).await
    }
    async fn put_chunk(&self, hash: &ChunkHash, data: &[u8]) -> Result<(), RemoteError> {
        self.remote.put_chunk(hash, data).await
    }
    async fn get_chunk(&self, hash: &ChunkHash) -> Result<Vec<u8>, RemoteError> {
        // Kept for the window: identical files, or a chunk repeated in one,
        // ask for it again (review 2026-11-15 #16). `fill` clears the window.
        let held = lock(&self.chunks).get(hash).cloned();
        match held {
            Some(data) => Ok(data),
            None => self.remote.get_chunk(hash).await,
        }
    }
    async fn put_blob(
        &self,
        hash: &ChunkHash,
        size: u64,
        chunks: &[ChunkHash],
    ) -> Result<(), RemoteError> {
        self.remote.put_blob(hash, size, chunks).await
    }
    async fn get_blob(&self, hash: &ChunkHash) -> Result<(u64, Vec<ChunkHash>), RemoteError> {
        let held = lock(&self.blobs).get(hash).cloned();
        match held {
            Some(m) => Ok(m),
            None => self.remote.get_blob(hash).await,
        }
    }
    async fn commit(&self, req: &CommitRequest) -> Result<CommitResponse, RemoteError> {
        self.remote.commit(req).await
    }
    async fn changes(
        &self,
        since: u64,
        epoch: Option<Uuid>,
        start: u64,
    ) -> Result<ChangesResponse, RemoteError> {
        self.remote.changes(since, epoch, start).await
    }
}
