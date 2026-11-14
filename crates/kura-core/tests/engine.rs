//! The sync engine against a scripted server: the paths the simulation in
//! `kura-server/tests/sim.rs` rarely takes.

#![expect(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    reason = "test fakes: counters and fixed seeds, failing loudly on purpose"
)]

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

use kura_core::sync::{
    FileMeta, Host, HostError, LocalState, Options, Pending, Remote, RemoteError, sync,
};
use kura_core::wire::{ChangesResponse, CommitRequest, CommitResponse, EntryState};
use kura_core::{ChunkHash, ChunkParams, EntryId, VaultPath, blob_hash};
use uuid::Uuid;

#[derive(Default)]
struct Mem {
    files: RefCell<BTreeMap<String, Vec<u8>>>,
    tick: Cell<i64>,
    /// Writes to this path fail, as a name the disk refuses would.
    unwritable: Option<String>,
}

impl Host for Mem {
    async fn list(&self) -> Result<Vec<FileMeta>, HostError> {
        Ok(self
            .files
            .borrow()
            .iter()
            .map(|(p, d)| FileMeta {
                path: p.clone(),
                size: d.len() as u64,
                mtime: 1,
            })
            .collect())
    }
    async fn read(&self, path: &VaultPath) -> Result<Vec<u8>, HostError> {
        self.files
            .borrow()
            .get(path.as_str())
            .cloned()
            .ok_or_else(|| HostError("missing".into()))
    }
    async fn stat(&self, path: &VaultPath) -> Result<Option<FileMeta>, HostError> {
        Ok(self.files.borrow().get(path.as_str()).map(|d| FileMeta {
            path: path.to_string(),
            size: d.len() as u64,
            mtime: 1,
        }))
    }
    async fn write(&self, path: &VaultPath, data: &[u8]) -> Result<FileMeta, HostError> {
        if self.unwritable.as_deref() == Some(path.as_str()) {
            return Err(HostError("the disk refuses this name".into()));
        }
        self.tick.set(self.tick.get() + 1);
        self.files
            .borrow_mut()
            .insert(path.to_string(), data.to_vec());
        Ok(self.stat(path).await?.unwrap())
    }
    async fn rename(&self, from: &VaultPath, to: &VaultPath) -> Result<FileMeta, HostError> {
        // As FsHost: never onto an existing file.
        if self.files.borrow().contains_key(to.as_str()) {
            return Err(HostError("exists".into()));
        }
        let d = self.files.borrow_mut().remove(from.as_str()).unwrap();
        self.files.borrow_mut().insert(to.to_string(), d);
        Ok(self.stat(to).await?.unwrap())
    }
    async fn trash(&self, path: &VaultPath) -> Result<(), HostError> {
        self.files.borrow_mut().remove(path.as_str());
        Ok(())
    }
    async fn save(&self, _: &LocalState) -> Result<(), HostError> {
        Ok(())
    }
    fn now_ms(&self) -> u64 {
        1_763_000_000_000
    }
    fn random(&self) -> [u8; 16] {
        [9; 16]
    }
}

/// A server that refuses uploads, or serves one entry, possibly corrupt.
#[derive(Default)]
struct Scripted {
    refuse: bool,
    serve: Vec<(EntryState, Vec<u8>)>,
    corrupt: bool,
    calls: Cell<usize>,
    /// Items per `changes` page; 0 for all in one.
    page: usize,
}

impl Remote for Scripted {
    async fn missing(&self, hashes: &[ChunkHash]) -> Result<Vec<ChunkHash>, RemoteError> {
        self.calls.set(self.calls.get() + 1);
        if self.refuse {
            return Err(RemoteError::Problem {
                status: 403,
                kind: "insufficient_role".into(),
                detail: None,
            });
        }
        Ok(hashes.to_vec())
    }
    async fn put_chunk(&self, _: &ChunkHash, _: &[u8]) -> Result<(), RemoteError> {
        Ok(())
    }
    async fn get_chunk(&self, h: &ChunkHash) -> Result<Vec<u8>, RemoteError> {
        let data = self
            .serve
            .iter()
            .find(|(_, d)| ChunkHash::of(d) == *h)
            .unwrap()
            .1
            .clone();
        Ok(if self.corrupt {
            b"tampered".to_vec()
        } else {
            data
        })
    }
    async fn put_blob(&self, _: &ChunkHash, _: u64, _: &[ChunkHash]) -> Result<(), RemoteError> {
        Ok(())
    }
    async fn get_blob(&self, b: &ChunkHash) -> Result<(u64, Vec<ChunkHash>), RemoteError> {
        let data = &self
            .serve
            .iter()
            .find(|(s, _)| s.blob == Some(*b))
            .unwrap()
            .1;
        Ok((data.len() as u64, vec![ChunkHash::of(data)]))
    }
    async fn commit(&self, _: &CommitRequest) -> Result<CommitResponse, RemoteError> {
        Err(RemoteError::Unavailable("scripted".into()))
    }
    async fn changes(&self, since: u64, _: Option<Uuid>) -> Result<ChangesResponse, RemoteError> {
        let all: Vec<EntryState> = self
            .serve
            .iter()
            .filter(|(s, _)| s.seq > since)
            .map(|(s, _)| s.clone())
            .collect();
        let n = if self.page == 0 {
            all.len()
        } else {
            self.page.min(all.len())
        };
        Ok(ChangesResponse {
            epoch: Uuid::nil(),
            head_seq: 1,
            next: (n < all.len())
                .then(|| all.get(n - 1).map(|i| i.seq.to_string()))
                .flatten(),
            items: all.get(..n).unwrap_or_default().to_vec(),
        })
    }
}

fn opts(dry_run: bool) -> Options {
    Options {
        params: ChunkParams::new(256 << 10, 1 << 20, 4 << 20).unwrap(),
        max_file_size: 100,
        dry_run,
    }
}

fn served(path: &str, data: &[u8]) -> (EntryState, Vec<u8>) {
    served_as(path, data, 1, 1)
}

/// Entry `n` at `seq`, its revision derived from both.
fn served_as(path: &str, data: &[u8], n: u8, seq: u64) -> (EntryState, Vec<u8>) {
    let blob = blob_hash(data.len() as u64, &[ChunkHash::of(data)]);
    let state = EntryState {
        entry: EntryId::new(1, &[n; 10]),
        revision: Uuid::from_u128(u128::from(seq) << 8 | u128::from(n)),
        seq,
        path: VaultPath::parse(path).unwrap(),
        deleted: false,
        blob: Some(blob),
        size: data.len() as u64,
    };
    (state, data.to_vec())
}

fn state() -> LocalState {
    LocalState::new(Uuid::from_u128(1), Uuid::from_u128(2))
}

#[tokio::test]
async fn unsyncable_and_dot_files_are_reported_or_skipped() {
    let host = Mem::default();
    for (p, d) in [
        ("ok.md", "x"),
        ("Q3: budget.md", "x"),
        (".obsidian/app.json", "{}"),
        ("big.bin", &"y".repeat(101)),
    ] {
        host.files
            .borrow_mut()
            .insert(p.into(), d.as_bytes().to_vec());
    }
    let remote = Scripted::default();
    let r = sync(&host, &remote, &mut state(), &opts(true))
        .await
        .unwrap();
    assert_eq!(r.pushed, 1, "only ok.md");
    let names: Vec<&str> = r.unsyncable.iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(names, ["Q3: budget.md", "big.bin"]);
    assert_eq!(remote.calls.get(), 0, "a dry run sends nothing");
}

#[tokio::test]
async fn a_refused_push_keeps_local_changes_and_still_pulls() {
    let host = Mem::default();
    host.files
        .borrow_mut()
        .insert("mine.md".into(), b"local".to_vec());
    let remote = Scripted {
        refuse: true,
        serve: vec![served("theirs.md", b"remote")],
        ..Scripted::default()
    };
    let mut s = state();
    let r = sync(&host, &remote, &mut s, &opts(false)).await.unwrap();
    assert_eq!(
        r.refused.as_ref().map(RemoteError::kind),
        Some("insufficient_role")
    );
    assert_eq!(host.files.borrow()["mine.md"], b"local");
    assert_eq!(host.files.borrow()["theirs.md"], b"remote");
    assert_eq!(s.entries.len(), 1, "only the pulled entry is known");
}

#[tokio::test]
async fn corrupt_content_is_never_written() {
    let host = Mem::default();
    let remote = Scripted {
        serve: vec![served("a.md", b"genuine")],
        corrupt: true,
        ..Scripted::default()
    };
    let mut s = state();
    let r = sync(&host, &remote, &mut s, &opts(false)).await.unwrap();
    assert_eq!(r.failed.len(), 1);
    assert_eq!(
        r.failed[0].1,
        "the server sent content that does not match its address"
    );
    assert!(host.files.borrow().is_empty());
    assert_eq!(s.cursor, 0, "fetched again next run");
}

#[tokio::test]
async fn a_remote_change_never_overwrites_an_unknown_local_file() {
    let host = Mem::default();
    host.files
        .borrow_mut()
        .insert("a.md".into(), b"mine, not pushed".to_vec());
    let remote = Scripted {
        refuse: true,
        serve: vec![served("a.md", b"theirs")],
        ..Scripted::default()
    };
    let r = sync(&host, &remote, &mut state(), &opts(false))
        .await
        .unwrap();
    assert_eq!(host.files.borrow()["a.md"], b"mine, not pushed");
    assert_eq!(r.deferred.len(), 1);
}

#[tokio::test]
async fn an_interrupted_write_is_adopted_on_restart() {
    let (entry, data) = served("a.md", b"new content");
    let host = Mem::default();
    host.files.borrow_mut().insert("a.md".into(), data.clone());
    let mut s = state();
    s.pending = Some(Pending {
        state: entry.clone(),
    });
    let remote = Scripted {
        serve: vec![(entry.clone(), data)],
        ..Scripted::default()
    };
    sync(&host, &remote, &mut s, &opts(false)).await.unwrap();
    assert!(s.pending.is_none());
    assert_eq!(s.entries[&entry.entry].revision, entry.revision);
}

/// Review 3.6 F1: entry B moves onto x, where this device still holds A,
/// whose own move (x → t) comes later in the log.
#[tokio::test]
async fn a_path_held_by_another_known_entry_is_handed_over() {
    let host = Mem::default();
    let (a_old, a_data) = served_as("x.md", b"note A", 1, 1);
    let (b_old, b_data) = served_as("y.md", b"note B", 2, 2);
    let mut s = state();
    for (st, data) in [(&a_old, &a_data), (&b_old, &b_data)] {
        let meta = host.write(&st.path, data).await.unwrap();
        s.entries.insert(
            st.entry,
            kura_core::sync::Known {
                path: st.path.clone(),
                revision: st.revision,
                blob: st.blob.unwrap(),
                size: st.size,
                stamp: Some(kura_core::sync::Stamp::from(&meta)),
            },
        );
    }
    s.cursor = 2;
    let (b_new, b_new_data) = served_as("x.md", b"note B, edited", 2, 11);
    let (a_new, _) = served_as("t.md", b"note A", 1, 12);
    let remote = Scripted {
        serve: vec![(b_new, b_new_data), (a_new, a_data)],
        ..Scripted::default()
    };
    let r = sync(&host, &remote, &mut s, &opts(false)).await.unwrap();
    assert!(r.failed.is_empty() && r.deferred.is_empty(), "{r:?}");
    let files = host.files.borrow();
    assert_eq!(
        files.get("x.md").map(Vec::as_slice),
        Some(&b"note B, edited"[..])
    );
    assert_eq!(files.get("t.md").map(Vec::as_slice), Some(&b"note A"[..]));
    assert!(!files.contains_key("y.md"));
    assert_eq!(s.cursor, 12);
}

/// Review 3.6 F2, F3: an item that cannot be written, or that waits behind a
/// local file, is reported and fetched again: the cursor stops before it.
#[tokio::test]
async fn an_unwritten_item_holds_the_cursor() {
    let host = Mem {
        unwritable: Some("bad.md".into()),
        ..Mem::default()
    };
    let remote = Scripted {
        serve: vec![
            served_as("ok.md", b"fine", 1, 1),
            served_as("bad.md", b"refused", 2, 2),
            served_as("later.md", b"later", 3, 3),
        ],
        ..Scripted::default()
    };
    let mut s = state();
    let r = sync(&host, &remote, &mut s, &opts(false)).await.unwrap();
    assert_eq!(r.failed.len(), 1, "{r:?}");
    assert_eq!(s.cursor, 1, "stops before the failed item");
    assert!(
        host.files.borrow().contains_key("later.md"),
        "later items are still applied"
    );
    // A local file that was never synced, at a remote entry's path, holds it too.
    let host = Mem::default();
    host.files
        .borrow_mut()
        .insert("mine.md".into(), b"local only".to_vec());
    let remote = Scripted {
        refuse: true,
        serve: vec![served_as("mine.md", b"theirs", 4, 1)],
        ..Scripted::default()
    };
    let mut s = state();
    let r = sync(&host, &remote, &mut s, &opts(false)).await.unwrap();
    assert_eq!(r.deferred.len(), 1);
    assert_eq!(s.cursor, 0, "the unseen entry will be fetched again");
}

/// A held item on page 1 does not stop page 2 from arriving.
#[tokio::test]
async fn paging_continues_past_a_held_item() {
    let host = Mem {
        unwritable: Some("bad.md".into()),
        ..Mem::default()
    };
    let remote = Scripted {
        serve: vec![
            served_as("bad.md", b"refused", 1, 1),
            served_as("next.md", b"page two", 2, 2),
        ],
        page: 1,
        ..Scripted::default()
    };
    let mut s = state();
    let r = sync(&host, &remote, &mut s, &opts(false)).await.unwrap();
    assert_eq!(r.failed.len(), 1);
    assert!(
        host.files.borrow().contains_key("next.md"),
        "page 2 arrived"
    );
    assert_eq!(s.cursor, 0);
}

#[test]
fn a_copied_folder_starts_fresh() {
    let mut s = state();
    s.cursor = 9;
    assert_eq!(
        LocalState::adopt(Some(s.clone()), s.device, s.vault).cursor,
        9
    );
    assert_eq!(
        LocalState::adopt(Some(s.clone()), Uuid::from_u128(7), s.vault).cursor,
        0
    );
    let mut old = s.clone();
    old.version = 0;
    assert_eq!(LocalState::adopt(Some(old), s.device, s.vault).cursor, 0);
    let json = serde_json::to_string(&s).unwrap();
    assert_eq!(serde_json::from_str::<LocalState>(&json).unwrap(), s);
}
