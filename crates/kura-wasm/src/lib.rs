//! `kura-core`'s sync engine for JavaScript (RFC 0001 §6.8, phase 4): the
//! Obsidian plugin passes a host (its vault adapter) and a remote (`fetch` to
//! the server) as objects whose methods return promises; values cross as
//! JSON strings and byte arrays. No panic crosses the boundary: every error
//! comes back as a rejected promise.

use kura_core::sync::{
    FileMeta, Host, HostError, LocalState, Options, Remote, RemoteError, Report,
};
use kura_core::wire::{ChangesResponse, CommitRequest, CommitResponse};
use kura_core::{ChunkHash, ChunkParams, VaultPath};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const TS: &'static str = r#"
/** A file of the vault: relative `/`-separated path, size, mtime in ms. */
export interface KuraFile { path: string; size: number; mtime: number; }
/** The vault folder, through the Obsidian adapter. Rejections are error strings. */
export interface KuraHost {
  list(): Promise<KuraFile[]>;
  read(path: string): Promise<Uint8Array>;
  stat(path: string): Promise<KuraFile | null>;
  write(path: string, data: Uint8Array): Promise<KuraFile>;
  rename(from: string, to: string): Promise<KuraFile>;
  trash(path: string): Promise<void>;
  save(state: string): Promise<void>;
  nowMs(): number;
  random(): Uint8Array;
}
/** The server, one vault. Rejections are `{status, kind, detail}` for a problem document, anything else when unreachable. */
export interface KuraRemote {
  missing(hashes: string[]): Promise<string[]>;
  putChunk(hash: string, data: Uint8Array): Promise<void>;
  getChunk(hash: string): Promise<Uint8Array>;
  putBlob(hash: string, size: number, chunks: string[]): Promise<void>;
  getBlob(hash: string): Promise<{ size: number; chunks: string[] }>;
  /** RFC 0002: manifests of many blobs, or null when the server has no batch route (405). */
  getBlobs(hashes: string[]): Promise<{ hash: string; size: number; chunks: string[] }[] | null>;
  /** RFC 0002: the raw chunks/batch body, unparsed, or null as for getBlobs. */
  getChunkRecords(hashes: string[]): Promise<Uint8Array | null>;
  commit(request: unknown): Promise<unknown>;
  changes(since: number, epoch: string | null, start: number): Promise<unknown>;
}
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "KuraHost")]
    pub type JsHost;
    #[wasm_bindgen(method, catch)]
    async fn list(this: &JsHost) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(method, catch)]
    async fn read(this: &JsHost, path: &str) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(method, catch)]
    async fn stat(this: &JsHost, path: &str) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(method, catch)]
    async fn write(this: &JsHost, path: &str, data: &[u8]) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(method, catch)]
    async fn rename(this: &JsHost, from: &str, to: &str) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(method, catch)]
    async fn trash(this: &JsHost, path: &str) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(method, catch)]
    async fn save(this: &JsHost, state: &str) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(method, js_name = nowMs)]
    fn now_ms(this: &JsHost) -> f64;
    #[wasm_bindgen(method)]
    fn random(this: &JsHost) -> Vec<u8>;

    #[wasm_bindgen(typescript_type = "KuraRemote")]
    pub type JsRemote;
    #[wasm_bindgen(method, catch)]
    async fn missing(this: &JsRemote, hashes: JsValue) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(method, catch, js_name = putChunk)]
    async fn put_chunk(this: &JsRemote, hash: &str, data: &[u8]) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(method, catch, js_name = getChunk)]
    async fn get_chunk(this: &JsRemote, hash: &str) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(method, catch, js_name = putBlob)]
    async fn put_blob(
        this: &JsRemote,
        hash: &str,
        size: f64,
        chunks: JsValue,
    ) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(method, catch, js_name = getBlob)]
    async fn get_blob(this: &JsRemote, hash: &str) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(method, catch, js_name = getBlobs)]
    async fn get_blobs(this: &JsRemote, hashes: JsValue) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(method, catch, js_name = getChunkRecords)]
    async fn get_chunk_records(this: &JsRemote, hashes: JsValue) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(method, catch)]
    async fn commit(this: &JsRemote, request: JsValue) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(method, catch)]
    async fn changes(
        this: &JsRemote,
        since: f64,
        epoch: Option<String>,
        start: f64,
    ) -> Result<JsValue, JsValue>;
}

fn text(v: &JsValue) -> String {
    // An `Error` serializes to `{}`: its message first.
    v.as_string()
        .or_else(|| {
            js_sys::Reflect::get(v, &JsValue::from_str("message"))
                .ok()
                .and_then(|m| m.as_string())
        })
        .or_else(|| js_sys::JSON::stringify(v).ok().and_then(|s| s.as_string()))
        .unwrap_or_else(|| "error".into())
}

fn host_err(v: JsValue) -> HostError {
    HostError(text(&v))
}

/// A JS rejection: a problem document `{status, kind, detail}`, or unreachable.
fn remote_err(v: JsValue) -> RemoteError {
    #[derive(Deserialize)]
    struct Problem {
        status: u16,
        kind: String,
        detail: Option<String>,
    }
    match serde_wasm(&v) {
        Ok(Problem {
            status,
            kind,
            detail,
        }) => RemoteError::Problem {
            status,
            kind,
            detail,
        },
        Err(_) => RemoteError::Unavailable(text(&v)),
    }
}

/// A JS value through JSON, the one serialization both sides share.
fn serde_wasm<T: serde::de::DeserializeOwned>(v: &JsValue) -> Result<T, String> {
    let s = js_sys::JSON::stringify(v)
        .map_err(|e| text(&e))?
        .as_string()
        .unwrap_or_default();
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

fn to_js<T: Serialize>(v: &T) -> Result<JsValue, String> {
    let s = serde_json::to_string(v).map_err(|e| e.to_string())?;
    js_sys::JSON::parse(&s).map_err(|e| text(&e))
}

fn bytes(v: &JsValue) -> Vec<u8> {
    js_sys::Uint8Array::new(v).to_vec()
}

#[derive(Deserialize)]
struct File {
    path: String,
    size: f64,
    mtime: f64,
}

fn meta(v: &JsValue) -> Result<FileMeta, HostError> {
    let f: File = serde_wasm(v).map_err(HostError)?;
    // JS numbers: sizes and times are integers below 2^53.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "integral JS numbers below 2^53"
    )]
    Ok(FileMeta {
        path: f.path,
        size: f.size.max(0.0) as u64,
        mtime: f.mtime as i64,
    })
}

struct HostAdapter(JsHost);

impl Host for HostAdapter {
    async fn list(&self) -> Result<Vec<FileMeta>, HostError> {
        let v = self.0.list().await.map_err(host_err)?;
        let files: Vec<File> = serde_wasm(&v).map_err(HostError)?;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "integral JS numbers below 2^53"
        )]
        Ok(files
            .into_iter()
            .map(|f| FileMeta {
                path: f.path,
                size: f.size.max(0.0) as u64,
                mtime: f.mtime as i64,
            })
            .collect())
    }
    async fn read(&self, path: &VaultPath) -> Result<Vec<u8>, HostError> {
        Ok(bytes(&self.0.read(path.as_str()).await.map_err(host_err)?))
    }
    async fn stat(&self, path: &VaultPath) -> Result<Option<FileMeta>, HostError> {
        let v = self.0.stat(path.as_str()).await.map_err(host_err)?;
        if v.is_null() || v.is_undefined() {
            Ok(None)
        } else {
            meta(&v).map(Some)
        }
    }
    async fn write(&self, path: &VaultPath, data: &[u8]) -> Result<FileMeta, HostError> {
        meta(&self.0.write(path.as_str(), data).await.map_err(host_err)?)
    }
    async fn rename(&self, from: &VaultPath, to: &VaultPath) -> Result<FileMeta, HostError> {
        meta(
            &self
                .0
                .rename(from.as_str(), to.as_str())
                .await
                .map_err(host_err)?,
        )
    }
    async fn trash(&self, path: &VaultPath) -> Result<(), HostError> {
        self.0
            .trash(path.as_str())
            .await
            .map(drop)
            .map_err(host_err)
    }
    async fn save(&self, state: &LocalState) -> Result<(), HostError> {
        let s = serde_json::to_string(state).map_err(|e| HostError(e.to_string()))?;
        self.0.save(&s).await.map(drop).map_err(host_err)
    }
    fn now_ms(&self) -> u64 {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "Date.now() is a positive integer below 2^53"
        )]
        let ms = self.0.now_ms().max(0.0) as u64;
        ms
    }
    fn random(&self) -> [u8; 16] {
        let mut out = [0u8; 16];
        for (o, b) in out.iter_mut().zip(self.0.random()) {
            *o = b;
        }
        out
    }
}

struct RemoteAdapter(JsRemote);

fn hashes(v: &[ChunkHash]) -> Result<JsValue, RemoteError> {
    to_js(&v).map_err(RemoteError::Unavailable)
}

fn decode<T: serde::de::DeserializeOwned>(v: &JsValue) -> Result<T, RemoteError> {
    serde_wasm(v).map_err(|e| RemoteError::Unavailable(format!("unexpected answer: {e}")))
}

impl Remote for RemoteAdapter {
    async fn missing(&self, h: &[ChunkHash]) -> Result<Vec<ChunkHash>, RemoteError> {
        decode(&self.0.missing(hashes(h)?).await.map_err(remote_err)?)
    }
    async fn put_chunk(&self, hash: &ChunkHash, data: &[u8]) -> Result<(), RemoteError> {
        self.0
            .put_chunk(&hash.to_string(), data)
            .await
            .map(drop)
            .map_err(remote_err)
    }
    async fn get_chunk(&self, hash: &ChunkHash) -> Result<Vec<u8>, RemoteError> {
        Ok(bytes(
            &self
                .0
                .get_chunk(&hash.to_string())
                .await
                .map_err(remote_err)?,
        ))
    }
    async fn put_blob(
        &self,
        hash: &ChunkHash,
        size: u64,
        chunks: &[ChunkHash],
    ) -> Result<(), RemoteError> {
        #[expect(clippy::cast_precision_loss, reason = "file sizes are below 2^53")]
        let size = size as f64;
        self.0
            .put_blob(&hash.to_string(), size, hashes(chunks)?)
            .await
            .map(drop)
            .map_err(remote_err)
    }
    async fn get_blob(&self, hash: &ChunkHash) -> Result<(u64, Vec<ChunkHash>), RemoteError> {
        #[derive(Deserialize)]
        struct Manifest {
            size: u64,
            chunks: Vec<ChunkHash>,
        }
        let m: Manifest = decode(
            &self
                .0
                .get_blob(&hash.to_string())
                .await
                .map_err(remote_err)?,
        )?;
        Ok((m.size, m.chunks))
    }
    async fn get_blobs(
        &self,
        h: &[ChunkHash],
    ) -> Result<Option<Vec<(ChunkHash, u64, Vec<ChunkHash>)>>, RemoteError> {
        #[derive(Deserialize)]
        struct Manifest {
            hash: ChunkHash,
            size: u64,
            chunks: Vec<ChunkHash>,
        }
        let v = self.0.get_blobs(hashes(h)?).await.map_err(remote_err)?;
        if v.is_null() {
            return Ok(None);
        }
        let list: Vec<Manifest> = decode(&v)?;
        Ok(Some(
            list.into_iter()
                .map(|m| (m.hash, m.size, m.chunks))
                .collect(),
        ))
    }
    async fn get_chunk_records(&self, h: &[ChunkHash]) -> Result<Option<Vec<u8>>, RemoteError> {
        let v = self
            .0
            .get_chunk_records(hashes(h)?)
            .await
            .map_err(remote_err)?;
        Ok((!v.is_null()).then(|| bytes(&v)))
    }
    async fn commit(&self, req: &CommitRequest) -> Result<CommitResponse, RemoteError> {
        let body = to_js(req).map_err(RemoteError::Unavailable)?;
        decode(&self.0.commit(body).await.map_err(remote_err)?)
    }
    async fn changes(
        &self,
        since: u64,
        epoch: Option<Uuid>,
        start: u64,
    ) -> Result<ChangesResponse, RemoteError> {
        #[expect(clippy::cast_precision_loss, reason = "seq values are below 2^53")]
        let (since, start) = (since as f64, start as f64);
        decode(
            &self
                .0
                .changes(since, epoch.map(|e| e.to_string()), start)
                .await
                .map_err(remote_err)?,
        )
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncOptions {
    device: Uuid,
    vault: Uuid,
    min: u64,
    avg: u64,
    max: u64,
    max_file_size: u64,
    #[serde(default)]
    dry_run: bool,
    /// The user agreed to a full resync above the deletion guard (§6.3).
    #[serde(default)]
    allow_deletions: bool,
    /// The configuration space: the server's copy wins on first contact (§6.8).
    #[serde(default)]
    prefer_remote: bool,
}

fn report(r: &Report) -> Value {
    let problem = |e: &RemoteError| match e {
        RemoteError::Problem {
            status,
            kind,
            detail,
        } => json!({ "status": status, "kind": kind, "detail": detail }),
        RemoteError::Unavailable(m) => json!({ "status": 0, "kind": "unavailable", "detail": m }),
    };
    json!({
        "pushed": r.pushed,
        "pulled": r.pulled,
        "trashed": r.trashed,
        "conflicts": r.conflicts,
        "unsyncable": r.unsyncable,
        "deferred": r.deferred,
        "failed": r.failed,
        "refused": r.refused.as_ref().map(problem),
        "resynced": r.resynced,
    })
}

/// One sync run. `state` is the JSON the host last saved (or empty);
/// resolves to `{ state, report }`, rejects with an error message, or with a
/// problem document `{status, kind, detail}` when the server refused.
#[wasm_bindgen]
pub async fn sync(
    host: JsHost,
    remote: JsRemote,
    state: String,
    options: String,
) -> Result<JsValue, JsValue> {
    let o: SyncOptions =
        serde_json::from_str(&options).map_err(|e| JsValue::from_str(&format!("options: {e}")))?;
    let params =
        ChunkParams::new(o.min, o.avg, o.max).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let stored = serde_json::from_str::<LocalState>(&state).ok();
    let mut st = LocalState::adopt(stored, o.device, o.vault);
    let opts = Options {
        params,
        max_file_size: o.max_file_size,
        dry_run: o.dry_run,
        allow_deletions: o.allow_deletions,
        prefer_remote: o.prefer_remote,
    };
    let r = kura_core::sync::sync(&HostAdapter(host), &RemoteAdapter(remote), &mut st, &opts).await;
    match r {
        Ok(r) => {
            to_js(&json!({ "state": st, "report": report(&r) })).map_err(|e| JsValue::from_str(&e))
        }
        Err(kura_core::sync::SyncError::Remote(RemoteError::Problem {
            status,
            kind,
            detail,
        })) => Err(
            to_js(&json!({ "status": status, "kind": kind, "detail": detail }))
                .unwrap_or_else(|e| JsValue::from_str(&e)),
        ),
        // The shell asks the user, then runs again with `allowDeletions`.
        Err(e @ kura_core::sync::SyncError::TooManyDeletions { deletions, files }) => Err(
            to_js(&json!({ "status": 0, "kind": "too_many_deletions", "detail": e.to_string(), "deletions": deletions, "files": files }))
                .unwrap_or_else(|e| JsValue::from_str(&e)),
        ),
        Err(e) => Err(JsValue::from_str(&e.to_string())),
    }
}

/// Checks a local path the way the server will (§6.3): `null` when valid,
/// else the rule it breaks, for the unsyncable list.
#[wasm_bindgen(js_name = pathProblem)]
pub fn path_problem(path: &str) -> Option<String> {
    VaultPath::from_local(path).err().map(|e| e.to_string())
}

/// Whether a path under `.obsidian` syncs with these categories (§6.8): the
/// rule the server applies, so the plugin never offers a refused file.
#[wasm_bindgen(js_name = configSynced)]
pub fn config_synced(path: &str, categories: Vec<String>) -> bool {
    kura_core::config::synced(path, &categories)
}
