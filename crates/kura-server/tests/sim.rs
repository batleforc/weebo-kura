//! Device simulation (RFC 0001 §14): N devices running the real `kura-core`
//! engine against the real server, with random edits, creations, renames and
//! deletes and random offline rounds. The vaults must converge and no pushed
//! content may be lost. Opt-in: PostgreSQL, Vault (or OpenBao) and S3, as
//! `tests/chunks.rs`.

#![expect(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "test fakes: counters and fixed seeds, failing loudly on purpose"
)]

mod common;

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashSet};

use axum::body::Body;
use axum::http::Request;
use common::app::{Env, env_with, storage, vault_id};
use kura_core::sync::{FileMeta, Host, HostError, LocalState, Options, Remote, RemoteError, sync};
use kura_core::wire::{ChangesResponse, CommitRequest, CommitResponse};
use kura_core::{ChunkHash, ChunkParams, VaultPath, blob_hash, chunks};
use kura_server::app::app;
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

/// Deterministic randomness, no dependency.
struct Rng(Cell<u64>);

impl Rng {
    fn next(&self) -> u64 {
        let mut x = self.0.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0.set(x);
        x
    }
    fn below(&self, n: usize) -> usize {
        usize::try_from(self.next() % (n as u64)).unwrap()
    }
}

/// A vault folder in memory.
struct MemHost {
    files: RefCell<BTreeMap<String, (Vec<u8>, i64)>>,
    tick: Cell<i64>,
    rng: Rng,
    saved: RefCell<Option<LocalState>>,
}

impl MemHost {
    fn new(seed: u64) -> Self {
        Self {
            files: RefCell::default(),
            tick: Cell::new(1),
            rng: Rng(Cell::new(seed)),
            saved: RefCell::default(),
        }
    }
    fn stamp(&self) -> i64 {
        let t = self.tick.get() + 1;
        self.tick.set(t);
        t
    }
    fn put(&self, path: &str, data: &[u8]) {
        let t = self.stamp();
        self.files
            .borrow_mut()
            .insert(path.to_owned(), (data.to_vec(), t));
    }
    fn meta(path: &str, f: &(Vec<u8>, i64)) -> FileMeta {
        FileMeta {
            path: path.to_owned(),
            size: f.0.len() as u64,
            mtime: f.1,
        }
    }
    /// Notes only: what every device must agree on.
    fn notes(&self) -> BTreeMap<String, Vec<u8>> {
        self.files
            .borrow()
            .iter()
            .filter(|(p, _)| !p.starts_with(".trash/"))
            .map(|(p, f)| (p.clone(), f.0.clone()))
            .collect()
    }
}

impl Host for MemHost {
    async fn list(&self) -> Result<Vec<FileMeta>, HostError> {
        Ok(self
            .files
            .borrow()
            .iter()
            .map(|(p, f)| Self::meta(p, f))
            .collect())
    }
    async fn read(&self, path: &VaultPath) -> Result<Vec<u8>, HostError> {
        self.files
            .borrow()
            .get(path.as_str())
            .map(|f| f.0.clone())
            .ok_or_else(|| HostError(format!("no {path}")))
    }
    async fn stat(&self, path: &VaultPath) -> Result<Option<FileMeta>, HostError> {
        Ok(self
            .files
            .borrow()
            .get(path.as_str())
            .map(|f| Self::meta(path.as_str(), f)))
    }
    async fn write(&self, path: &VaultPath, data: &[u8]) -> Result<FileMeta, HostError> {
        self.put(path.as_str(), data);
        Ok(self.stat(path).await?.unwrap())
    }
    async fn rename(&self, from: &VaultPath, to: &VaultPath) -> Result<FileMeta, HostError> {
        // As FsHost: never onto an existing file (review 3.6 F1).
        if self.files.borrow().contains_key(to.as_str()) {
            return Err(HostError(format!("{to} already exists")));
        }
        let f = self
            .files
            .borrow_mut()
            .remove(from.as_str())
            .ok_or_else(|| HostError(format!("no {from}")))?;
        self.files.borrow_mut().insert(to.as_str().to_owned(), f);
        Ok(self.stat(to).await?.unwrap())
    }
    async fn trash(&self, path: &VaultPath) -> Result<(), HostError> {
        let f = self
            .files
            .borrow_mut()
            .remove(path.as_str())
            .ok_or_else(|| HostError(format!("no {path}")))?;
        self.files.borrow_mut().insert(format!(".trash/{path}"), f);
        Ok(())
    }
    async fn save(&self, state: &LocalState) -> Result<(), HostError> {
        *self.saved.borrow_mut() = Some(state.clone());
        Ok(())
    }
    fn now_ms(&self) -> u64 {
        1_763_000_000_000 + self.rng.next() % 1000
    }
    fn random(&self) -> [u8; 16] {
        let mut out = [0u8; 16];
        out[..8].copy_from_slice(&self.rng.next().to_le_bytes());
        out[8..].copy_from_slice(&self.rng.next().to_le_bytes());
        out
    }
}

/// The real application, in process.
struct AppRemote<'a> {
    e: &'a Env,
    tok: String,
    device: String,
    vault: String,
}

impl AppRemote<'_> {
    async fn send(
        &self,
        method: &str,
        path: &str,
        body: Option<(Vec<u8>, &str)>,
    ) -> Result<Vec<u8>, RemoteError> {
        let mut req = Request::builder()
            .method(method)
            .uri(format!("/v1/vaults/{}{path}", self.vault))
            .header("authorization", format!("Bearer {}", self.tok))
            .header("x-kura-device", &self.device);
        let req = match body {
            Some((b, ct)) => req.header("content-type", ct).body(Body::from(b)),
            None => {
                req = req.header("x-kura-client", "kura-cli/0.1.0");
                req.body(Body::empty())
            }
        };
        let r = app(self.e.state.clone())
            .oneshot(req.unwrap())
            .await
            .unwrap();
        let status = r.status();
        let bytes = axum::body::to_bytes(r.into_body(), 8 << 20)
            .await
            .unwrap()
            .to_vec();
        if status.is_success() {
            return Ok(bytes);
        }
        let v: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        Err(RemoteError::Problem {
            status: status.as_u16(),
            kind: v
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim_start_matches("/v1/problems/")
                .to_owned(),
            detail: v.get("detail").and_then(Value::as_str).map(str::to_owned),
        })
    }
    async fn json<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> Result<T, RemoteError> {
        let b = body.map(|v| (v.to_string().into_bytes(), "application/json"));
        let bytes = self.send(method, path, b).await?;
        serde_json::from_slice(&bytes).map_err(|e| RemoteError::Unavailable(e.to_string()))
    }
}

impl Remote for AppRemote<'_> {
    async fn missing(&self, hashes: &[ChunkHash]) -> Result<Vec<ChunkHash>, RemoteError> {
        let v: Value = self
            .json("POST", "/chunks/missing", Some(json!({ "hashes": hashes })))
            .await?;
        Ok(serde_json::from_value(v["missing"].clone()).unwrap())
    }
    async fn put_chunk(&self, hash: &ChunkHash, data: &[u8]) -> Result<(), RemoteError> {
        self.send(
            "PUT",
            &format!("/chunks/{hash}"),
            Some((data.to_vec(), "application/octet-stream")),
        )
        .await
        .map(drop)
    }
    async fn get_chunk(&self, hash: &ChunkHash) -> Result<Vec<u8>, RemoteError> {
        self.send("GET", &format!("/chunks/{hash}"), None).await
    }
    async fn put_blob(
        &self,
        hash: &ChunkHash,
        size: u64,
        chunks: &[ChunkHash],
    ) -> Result<(), RemoteError> {
        let b = json!({ "size": size, "chunks": chunks })
            .to_string()
            .into_bytes();
        self.send(
            "PUT",
            &format!("/blobs/{hash}"),
            Some((b, "application/json")),
        )
        .await
        .map(drop)
    }
    async fn get_blob(&self, hash: &ChunkHash) -> Result<(u64, Vec<ChunkHash>), RemoteError> {
        let v: Value = self.json("GET", &format!("/blobs/{hash}"), None).await?;
        Ok((
            v["size"].as_u64().unwrap(),
            serde_json::from_value(v["chunks"].clone()).unwrap(),
        ))
    }
    async fn commit(&self, req: &CommitRequest) -> Result<CommitResponse, RemoteError> {
        self.json("POST", "/commits", Some(serde_json::to_value(req).unwrap()))
            .await
    }
    async fn changes(
        &self,
        since: u64,
        epoch: Option<Uuid>,
    ) -> Result<ChangesResponse, RemoteError> {
        let e = epoch.map(|e| format!("&epoch={e}")).unwrap_or_default();
        self.json("GET", &format!("/changes?since={since}{e}"), None)
            .await
    }
}

struct Device<'a> {
    host: MemHost,
    remote: AppRemote<'a>,
    state: LocalState,
}

#[tokio::test]
async fn devices_converge_and_lose_nothing() {
    let mut conflicts = 0;
    for seed in [42, 7, 1234, 99, 2026] {
        conflicts += run(seed).await;
    }
    eprintln!("conflict files over all seeds: {conflicts}");
    if std::env::var("KURA_TEST_S3_ENDPOINT").is_ok() {
        assert!(conflicts > 0, "the runs never produced a conflict");
    }
}

/// One simulated history; returns how many conflict files it produced.
async fn run(seed: u64) -> usize {
    let name = format!("{}::seed{seed}", module_path!());
    let Some(st) = storage(&name).await else {
        return 0;
    };
    let Some(e) = env_with(&name, Some(st)).await else {
        return 0;
    };
    let tok = e.key.token("u1", "s1", &["maxime-members"]);
    let v = vault_id(&e, "knowledge").await;
    let vault = Uuid::parse_str(&v).unwrap();
    // Small chunks, so multi-chunk files and partial re-uploads happen.
    let params = ChunkParams::new(64, 256, 1024).unwrap();
    let opts = Options {
        params,
        max_file_size: 1 << 20,
        dry_run: false,
    };
    let mut devices = Vec::new();
    for i in 0..4u64 {
        let d = e.register(&tok, &format!("dev{i}")).await;
        let device = Uuid::parse_str(&d).unwrap();
        devices.push(Device {
            host: MemHost::new(0x9e37_79b9_7f4a_7c15 ^ (i + 1)),
            remote: AppRemote {
                e: &e,
                tok: tok.clone(),
                device: d,
                vault: v.clone(),
            },
            state: LocalState::new(device, vault),
        });
    }
    let rng = Rng(Cell::new(seed));
    let mut conflicts = 0;
    // Every content a device held when it synced; each must survive somewhere.
    let mut pushed: HashSet<Vec<u8>> = HashSet::new();
    let mut n = 0u32;
    for _round in 0..20 {
        for (i, d) in devices.iter_mut().enumerate() {
            for _ in 0..rng.below(3) {
                n += 1;
                let names: Vec<String> = d.host.notes().into_keys().collect();
                let body = format!("device {i} edit {n}\n")
                    .repeat(1 + rng.below(40))
                    .into_bytes();
                match (rng.below(5), names.is_empty()) {
                    (0 | 1, false) => {
                        let p = &names[rng.below(names.len())];
                        d.host.put(p, &body);
                    }
                    (2, false) => {
                        let p = names[rng.below(names.len())].clone();
                        let f = d.host.files.borrow_mut().remove(&p).unwrap();
                        d.host
                            .files
                            .borrow_mut()
                            .insert(format!("moved/{n}-{}", p.rsplit('/').next().unwrap()), f);
                    }
                    (3, false) => {
                        let p = &names[rng.below(names.len())];
                        d.host.files.borrow_mut().remove(p);
                    }
                    _ => d.host.put(&format!("notes/n{}.md", rng.below(12)), &body),
                }
            }
            // Offline about one round in three.
            if rng.below(3) == 0 {
                continue;
            }
            pushed.extend(d.host.notes().into_values());
            let r = sync(&d.host, &d.remote, &mut d.state, &opts).await.unwrap();
            assert!(r.refused.is_none() && r.failed.is_empty(), "{r:?}");
            conflicts += r.conflicts.len();
        }
    }
    // Everyone online: the rounds settle every deferred change.
    for round in 0..3 {
        for d in &mut devices {
            pushed.extend(d.host.notes().into_values());
            let r = sync(&d.host, &d.remote, &mut d.state, &opts).await.unwrap();
            assert!(r.failed.is_empty(), "seed {seed}: {r:?}");
            if round == 2 {
                assert!(r.deferred.is_empty(), "seed {seed}: still deferred: {r:?}");
            }
            conflicts += r.conflicts.len();
        }
    }
    let first = devices[0].host.notes();
    assert!(first.len() > 5, "the run produced files: {}", first.len());
    for (i, d) in devices.iter().enumerate() {
        assert_eq!(d.host.notes(), first, "seed {seed}: device {i} diverged");
    }
    // The server's live entries are the same set.
    let live: i64 = sqlx::query_scalar("SELECT count(*) FROM entry WHERE NOT deleted")
        .fetch_one(&e.t.owner)
        .await
        .unwrap();
    assert_eq!(usize::try_from(live).unwrap(), first.len());
    // No content lost: every version a device synced is in the history.
    let stored: HashSet<Vec<u8>> =
        sqlx::query_scalar("SELECT blob_hash FROM revision WHERE blob_hash IS NOT NULL")
            .fetch_all(&e.t.owner)
            .await
            .unwrap()
            .into_iter()
            .collect();
    for content in &pushed {
        let hashes: Vec<ChunkHash> = chunks(content, params).iter().map(|c| c.hash).collect();
        let b = blob_hash(content.len() as u64, &hashes);
        assert!(
            stored.contains(b.as_bytes().as_slice()),
            "seed {seed} lost: {:?}",
            String::from_utf8_lossy(content)
        );
    }
    conflicts
}
