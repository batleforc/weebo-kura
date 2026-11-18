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
use kura_core::{ChunkHash, ChunkParams, VaultPath};
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
    /// Reads and writes through the batch routes (RFC 0002); one by one
    /// otherwise, as against an older server.
    batch: bool,
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
    async fn get_blobs(
        &self,
        hashes: &[ChunkHash],
    ) -> Result<Option<Vec<(ChunkHash, u64, Vec<ChunkHash>)>>, RemoteError> {
        if !self.batch {
            return Ok(None);
        }
        let v: Value = self
            .json("POST", "/blobs/batch", Some(json!({ "blobs": hashes })))
            .await?;
        Ok(Some(
            v["manifests"]
                .as_array()
                .unwrap()
                .iter()
                .map(|m| {
                    (
                        m["hash"].as_str().unwrap().parse().unwrap(),
                        m["size"].as_u64().unwrap(),
                        serde_json::from_value(m["chunks"].clone()).unwrap(),
                    )
                })
                .collect(),
        ))
    }
    async fn get_chunk_records(
        &self,
        hashes: &[ChunkHash],
    ) -> Result<Option<Vec<u8>>, RemoteError> {
        if !self.batch {
            return Ok(None);
        }
        let b = json!({ "chunks": hashes }).to_string().into_bytes();
        self.send("POST", "/chunks/batch", Some((b, "application/json")))
            .await
            .map(Some)
    }
    async fn put_chunks(&self, records: &[u8]) -> Result<bool, RemoteError> {
        if !self.batch {
            return Ok(false);
        }
        self.send(
            "POST",
            "/chunks/upload",
            Some((records.to_vec(), "application/octet-stream")),
        )
        .await
        .map(|_| true)
    }
    async fn put_blobs(
        &self,
        manifests: &[kura_core::sync::Manifest],
    ) -> Result<bool, RemoteError> {
        if !self.batch {
            return Ok(false);
        }
        let list: Vec<Value> = manifests
            .iter()
            .map(|(h, size, chunks)| json!({ "hash": h, "size": size, "chunks": chunks }))
            .collect();
        let b = json!({ "manifests": list }).to_string().into_bytes();
        self.send("POST", "/blobs/upload", Some((b, "application/json")))
            .await
            .map(|_| true)
    }
    async fn commit(&self, req: &CommitRequest) -> Result<CommitResponse, RemoteError> {
        self.json("POST", "/commits", Some(serde_json::to_value(req).unwrap()))
            .await
    }
    async fn changes(
        &self,
        since: u64,
        epoch: Option<Uuid>,
        start: u64,
    ) -> Result<ChangesResponse, RemoteError> {
        let e = epoch.map(|e| format!("&epoch={e}")).unwrap_or_default();
        self.json(
            "GET",
            &format!("/changes?since={since}&start={start}{e}"),
            None,
        )
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
    let (mut conflicts, mut merged) = (0, 0);
    for seed in [42, 7, 1234, 99, 2026] {
        let (c, m) = run(seed).await;
        conflicts += c;
        merged += m;
    }
    eprintln!("conflict files over all seeds: {conflicts}");
    if std::env::var("KURA_TEST_S3_ENDPOINT").is_ok() {
        assert!(conflicts > 0, "the runs never produced a conflict");
        assert!(merged > 0, "the runs never merged");
    }
}

/// One simulated history; returns its conflict files and merges.
async fn run(seed: u64) -> (usize, u64) {
    let name = format!("{}::seed{seed}", module_path!());
    let Some(st) = storage(&name).await else {
        return (0, 0);
    };
    let Some(e) = env_with(&name, Some(st)).await else {
        return (0, 0);
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
        allow_deletions: false,
        prefer_remote: false,
        writable: None,
        folders: false,
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
                batch: i % 2 == 0,
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
                // Unique lines, so a merge can be told from a loss.
                let body: Vec<u8> = (0..1 + rng.below(40))
                    .map(|j| format!("device {i} edit {n} line {j}\n"))
                    .collect::<String>()
                    .into_bytes();
                match (rng.below(5), names.is_empty()) {
                    (0 | 1, false) => {
                        // One line replaced or inserted: what merges (phase 4).
                        let p = names[rng.below(names.len())].clone();
                        let old = d.host.files.borrow()[&p].0.clone();
                        let mut lines: Vec<String> = String::from_utf8(old)
                            .unwrap()
                            .lines()
                            .map(str::to_owned)
                            .collect();
                        let k = rng.below(lines.len() + 1);
                        let line = format!("device {i} edit {n} line 0");
                        if k < lines.len() && rng.below(2) == 0 {
                            lines[k] = line;
                        } else {
                            lines.insert(k, line);
                        }
                        d.host.put(&p, (lines.join("\n") + "\n").as_bytes());
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
    // No line lost: every line of every version a device synced is in some
    // stored revision (merged heads do not store the pushed version itself).
    let blobs: Vec<Vec<u8>> =
        sqlx::query_scalar("SELECT DISTINCT blob_hash FROM revision WHERE blob_hash IS NOT NULL")
            .fetch_all(&e.t.owner)
            .await
            .unwrap();
    let mut stored: HashSet<String> = HashSet::new();
    for b in blobs {
        let hash = ChunkHash::from_bytes(b.try_into().unwrap());
        let (_, parts) = devices[0].remote.get_blob(&hash).await.unwrap();
        let mut text = Vec::new();
        for c in parts {
            text.extend(devices[0].remote.get_chunk(&c).await.unwrap());
        }
        stored.extend(String::from_utf8(text).unwrap().lines().map(str::to_owned));
    }
    for content in &pushed {
        for line in String::from_utf8_lossy(content).lines() {
            assert!(stored.contains(line), "seed {seed} lost the line {line:?}");
        }
    }
    let merged = e
        .state
        .metrics
        .merge_total
        .with_label_values(&["merged"])
        .get();
    let attempted = e
        .state
        .metrics
        .merge_total
        .with_label_values(&["conflict"])
        .get();
    eprintln!(
        "seed {seed}: {merged} merged, {attempted} merge conflicts, {conflicts} conflict files"
    );
    // Half the devices read through the batch routes, half one by one.
    let batched = e
        .state
        .metrics
        .batch_items_total
        .with_label_values(&["chunks", "found"])
        .get();
    assert!(batched > 0, "no chunk came through chunks/batch");
    let uploaded = e
        .state
        .metrics
        .batch_items_total
        .with_label_values(&["upload", "stored"])
        .get();
    assert!(uploaded > 0, "no chunk came through chunks/upload");
    (conflicts, merged)
}
