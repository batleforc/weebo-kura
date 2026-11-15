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
use kura_core::wire::{
    ChangesResponse, CommitRequest, CommitResponse, EntryState, OpResult, Outcome,
};
use kura_core::{ChunkHash, ChunkParams, EntryId, VaultPath, blob_hash};
use uuid::Uuid;

#[derive(Default)]
struct Mem {
    files: RefCell<BTreeMap<String, Vec<u8>>>,
    tick: Cell<i64>,
    /// Writes to this path fail, as a name the disk refuses would.
    unwritable: Option<String>,
    /// This file cannot be moved (an open note).
    pinned: Option<String>,
    /// A case-insensitive disk (macOS, Windows, Android): a path names the
    /// file stored under any case.
    fold: bool,
}

impl Mem {
    /// The stored name a path reaches.
    fn key(&self, path: &str) -> Option<String> {
        let files = self.files.borrow();
        if files.contains_key(path) {
            return Some(path.to_owned());
        }
        let lower = path.to_lowercase();
        files
            .keys()
            .find(|k| self.fold && k.to_lowercase() == lower)
            .cloned()
    }
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
        let key = self.key(path.as_str()).unwrap_or_default();
        self.files
            .borrow()
            .get(&key)
            .cloned()
            .ok_or_else(|| HostError("missing".into()))
    }
    async fn stat(&self, path: &VaultPath) -> Result<Option<FileMeta>, HostError> {
        let key = self.key(path.as_str()).unwrap_or_default();
        Ok(self.files.borrow().get(&key).map(|d| FileMeta {
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
        // An existing file keeps its stored case, as on a real disk.
        let key = self.key(path.as_str()).unwrap_or_else(|| path.to_string());
        self.files.borrow_mut().insert(key, data.to_vec());
        Ok(self.stat(path).await?.unwrap())
    }
    async fn rename(&self, from: &VaultPath, to: &VaultPath) -> Result<FileMeta, HostError> {
        if self.pinned.as_deref() == Some(from.as_str()) {
            return Err(HostError("open in the editor".into()));
        }
        // As FsHost: never onto an existing file.
        if self.key(to.as_str()).is_some() {
            return Err(HostError("exists".into()));
        }
        let from = self.key(from.as_str()).unwrap();
        let d = self.files.borrow_mut().remove(&from).unwrap();
        self.files.borrow_mut().insert(to.to_string(), d);
        Ok(self.stat(to).await?.unwrap())
    }
    async fn trash(&self, path: &VaultPath) -> Result<(), HostError> {
        let key = self.key(path.as_str()).unwrap_or_default();
        self.files.borrow_mut().remove(&key);
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
    /// The answer to a commit; unreachable when absent.
    reply: Option<CommitResponse>,
    /// Restored from a backup: a cursor from another epoch than nil is 410.
    rewound: bool,
    /// The vault's head, above every item served (changes elsewhere).
    head: u64,
    /// Serves the batch routes of RFC 0002; an older server when false.
    batch: bool,
    /// Batch records carry other bytes than their hash names (T5).
    forge: bool,
    /// Records per `chunks/batch` answer; 0 for all asked.
    per_call: usize,
    /// `chunks/batch` answers a hash nobody asked for, and nothing else.
    foreign: bool,
    /// `blobs/batch` works but `chunks/batch` answers as an older server.
    no_chunk_batch: bool,
    /// Hashes asked of each `blobs/batch` call.
    blob_asks: std::cell::RefCell<Vec<usize>>,
    /// Single `get_blob` and `get_chunk` calls, and batch calls.
    singles: Cell<usize>,
    batches: Cell<usize>,
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
        self.singles.set(self.singles.get() + 1);
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
    async fn get_blobs(
        &self,
        hashes: &[ChunkHash],
    ) -> Result<Option<Vec<(ChunkHash, u64, Vec<ChunkHash>)>>, RemoteError> {
        if !self.batch {
            return Ok(None);
        }
        self.batches.set(self.batches.get() + 1);
        self.blob_asks.borrow_mut().push(hashes.len());
        Ok(Some(
            self.serve
                .iter()
                .filter_map(|(s, d)| {
                    s.blob
                        .filter(|b| hashes.contains(b))
                        .map(|b| (b, d.len() as u64, vec![ChunkHash::of(d)]))
                })
                .collect(),
        ))
    }
    async fn get_chunk_records(
        &self,
        hashes: &[ChunkHash],
    ) -> Result<Option<Vec<u8>>, RemoteError> {
        if !self.batch || self.no_chunk_batch {
            return Ok(None);
        }
        self.batches.set(self.batches.get() + 1);
        let mut out = Vec::new();
        if self.foreign {
            out.extend_from_slice(ChunkHash::of(b"never asked").as_bytes());
            out.extend_from_slice(&[0, 0, 0, 0, 1, b'x']);
            return Ok(Some(out));
        }
        let n = if self.per_call == 0 {
            hashes.len()
        } else {
            self.per_call
        };
        for h in hashes.iter().take(n) {
            let data = self
                .serve
                .iter()
                .find(|(_, d)| ChunkHash::of(d) == *h)
                .map(|(_, d)| d.clone());
            let data = if self.forge {
                data.map(|_| b"forged".to_vec())
            } else {
                data
            };
            out.extend_from_slice(h.as_bytes());
            match data {
                Some(d) => {
                    out.push(0);
                    out.extend_from_slice(&u32::try_from(d.len()).unwrap().to_be_bytes());
                    out.extend_from_slice(&d);
                }
                None => out.extend_from_slice(&[1, 0, 0, 0, 0]),
            }
        }
        Ok(Some(out))
    }
    async fn get_blob(&self, b: &ChunkHash) -> Result<(u64, Vec<ChunkHash>), RemoteError> {
        self.singles.set(self.singles.get() + 1);
        let data = &self
            .serve
            .iter()
            .find(|(s, _)| s.blob == Some(*b))
            .unwrap()
            .1;
        Ok((data.len() as u64, vec![ChunkHash::of(data)]))
    }
    async fn commit(&self, _: &CommitRequest) -> Result<CommitResponse, RemoteError> {
        self.reply
            .clone()
            .ok_or_else(|| RemoteError::Unavailable("scripted".into()))
    }
    async fn changes(
        &self,
        since: u64,
        epoch: Option<Uuid>,
        _start: u64,
    ) -> Result<ChangesResponse, RemoteError> {
        if self.rewound && epoch.is_some_and(|e| !e.is_nil()) {
            return Err(RemoteError::Problem {
                status: 410,
                kind: "history_rewound".into(),
                detail: None,
            });
        }
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
            head_seq: self
                .serve
                .iter()
                .map(|(s, _)| s.seq)
                .max()
                .unwrap_or(0)
                .max(self.head),
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
        allow_deletions: false,
        prefer_remote: false,
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
    assert_eq!((r.pushed, r.pushed_bytes), (1, 1), "only ok.md");
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
                resend: false,
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

/// Phase 4: a push answered `merged` ends with the merged head on disk,
/// known at its new revision, and nothing left to push.
#[tokio::test]
async fn a_merged_push_writes_the_merged_head() {
    let (base, base_data) = served_as("plan.md", b"one\ntwo\n", 1, 1);
    let host = Mem::default();
    let meta = host.write(&base.path, &base_data).await.unwrap();
    let mut s = state();
    s.entries.insert(
        base.entry,
        kura_core::sync::Known {
            path: base.path.clone(),
            revision: base.revision,
            blob: base.blob.unwrap(),
            size: base.size,
            stamp: Some(kura_core::sync::Stamp::from(&meta)),
            resend: false,
        },
    );
    s.cursor = 1;
    // Edited here (the fake host's mtime is fixed, so the size tells).
    host.files
        .borrow_mut()
        .insert("plan.md".into(), b"one, here\ntwo\n".to_vec());
    let (merged, merged_data) = served_as("plan.md", b"one, here\ntwo, there\n", 1, 3);
    let remote = Scripted {
        serve: vec![(merged.clone(), merged_data.clone())],
        reply: Some(CommitResponse {
            head_seq: 3,
            results: vec![OpResult {
                outcome: Outcome::Merged,
                head: merged.clone(),
                conflict: None,
            }],
        }),
        ..Scripted::default()
    };
    let r = sync(&host, &remote, &mut s, &opts(false)).await.unwrap();
    assert!(r.deferred.is_empty() && r.failed.is_empty(), "{r:?}");
    assert_eq!(host.files.borrow()["plan.md"], merged_data);
    assert_eq!(s.entries[&merged.entry].revision, merged.revision);
    let again = sync(&host, &remote, &mut s, &opts(true)).await.unwrap();
    assert_eq!(again.pushed, 0, "nothing left to push");
}

/// Review 4 H2: a conflict whose local file cannot be moved (an open note)
/// fails that file only; the run completes and keeps the old base.
#[tokio::test]
async fn a_conflict_on_an_open_note_fails_that_file_only() {
    let (base, base_data) = served_as("open.md", b"base\n", 1, 1);
    let host = Mem {
        pinned: Some("open.md".into()),
        ..Mem::default()
    };
    let meta = host.write(&base.path, &base_data).await.unwrap();
    let mut s = state();
    s.entries.insert(
        base.entry,
        kura_core::sync::Known {
            path: base.path.clone(),
            revision: base.revision,
            blob: base.blob.unwrap(),
            size: base.size,
            stamp: Some(kura_core::sync::Stamp::from(&meta)),
            resend: false,
        },
    );
    s.cursor = 1;
    host.files
        .borrow_mut()
        .insert("open.md".into(), b"typed here\n".to_vec());
    let (head, head_data) = served_as("open.md", b"edited elsewhere\n", 1, 2);
    let (conflict, _) = served_as("open (conflict x).md", b"typed here\n", 9, 3);
    let remote = Scripted {
        serve: vec![(head.clone(), head_data)],
        reply: Some(CommitResponse {
            head_seq: 3,
            results: vec![OpResult {
                outcome: Outcome::Conflicted,
                head: head.clone(),
                conflict: Some(conflict),
            }],
        }),
        ..Scripted::default()
    };
    let r = sync(&host, &remote, &mut s, &opts(false)).await.unwrap();
    assert_eq!(r.failed.len(), 1, "{r:?}");
    assert_eq!(
        host.files.borrow()["open.md"],
        b"typed here\n",
        "the open note is untouched"
    );
    assert_eq!(
        s.entries[&base.entry].revision, base.revision,
        "the old base is kept for the retry"
    );
}

/// Review 5: after `history_rewound`, a file the server holds in another
/// version is resent; a viewer's refused push forgets it rather than letting
/// the pull write over it or a later resync trash it.
#[tokio::test]
async fn a_refused_resend_keeps_the_local_version() {
    let host = Mem::default();
    let (mine, data) = served_as("n.md", b"mine, lost by the server", 1, 7);
    let meta = host.write(&mine.path, &data).await.unwrap();
    let mut s = state();
    s.epoch = Some(Uuid::from_u128(9));
    s.cursor = 7;
    s.entries.insert(
        mine.entry,
        kura_core::sync::Known {
            path: mine.path.clone(),
            revision: mine.revision,
            blob: mine.blob.unwrap(),
            size: mine.size,
            stamp: Some(kura_core::sync::Stamp::from(&meta)),
            resend: false,
        },
    );
    let remote = Scripted {
        refuse: true,
        rewound: true,
        serve: vec![served_as("n.md", b"older", 1, 3)],
        ..Scripted::default()
    };
    let r = sync(&host, &remote, &mut s, &opts(false)).await.unwrap();
    assert_eq!(r.resynced.as_deref(), Some("history_rewound"));
    assert!(r.refused.is_some(), "{r:?}");
    assert_eq!(host.files.borrow()["n.md"], b"mine, lost by the server");
    assert!(s.entries.is_empty(), "forgotten, not resent forever");
    assert_eq!(r.trashed, 0);
}

/// A view that saw nothing new still reaches the head it was read at: a
/// configuration space or a prefix must not stay below the horizon that
/// others' purged deletions move (phase 8 review).
#[tokio::test]
async fn a_quiet_view_moves_its_cursor_to_the_head() {
    let host = Mem::default();
    let remote = Scripted {
        head: 50,
        ..Scripted::default()
    };
    let mut s = state();
    sync(&host, &remote, &mut s, &opts(false)).await.unwrap();
    assert_eq!(s.cursor, 50);
}

/// Phase 8 review H1: in the configuration space a new device's local copy
/// of a file the server already holds gives way, instead of being pushed
/// over the user's settings.
#[tokio::test]
async fn the_configuration_takes_the_servers_copy_on_first_contact() {
    let host = Mem::default();
    host.files
        .borrow_mut()
        .insert("hotkeys.json".into(), b"{\"defaults\": true}".to_vec());
    let remote = Scripted {
        serve: vec![served("hotkeys.json", b"{\"mine\": true}")],
        ..Scripted::default()
    };
    let mut s = state();
    let mut o = opts(false);
    o.prefer_remote = true;
    let r = sync(&host, &remote, &mut s, &o).await.unwrap();
    assert_eq!(host.files.borrow()["hotkeys.json"], b"{\"mine\": true}");
    assert_eq!((r.pulled, r.pushed), (1, 0), "{r:?}");
}

/// RFC 0002: a server with the batch routes is read in batches, with no
/// single read; a forged record is dropped and the chunk read again (T5).
#[tokio::test]
async fn a_batched_pull_makes_no_single_reads_and_drops_forged_records() {
    let files = [
        ("a.md", &b"first"[..]),
        ("b.md", b"second"),
        ("c/d.md", b"third"),
    ];
    for forge in [false, true] {
        let host = Mem::default();
        let remote = Scripted {
            serve: files
                .iter()
                .zip(1u8..)
                .map(|((p, d), n)| served_as(p, d, n, u64::from(n)))
                .collect(),
            batch: true,
            forge,
            ..Scripted::default()
        };
        let r = sync(&host, &remote, &mut state(), &opts(false))
            .await
            .unwrap();
        assert_eq!(r.pulled, 3, "{r:?}");
        for (p, d) in files {
            assert_eq!(host.files.borrow()[p], d, "{p}");
        }
        assert_eq!(
            remote.batches.get(),
            2,
            "one call for manifests, one for chunks"
        );
        assert_eq!(
            remote.singles.get(),
            if forge { 3 } else { 0 },
            "forge {forge}"
        );
    }
}

/// RFC 0002: the window asks again for what an answer left out, stops on an
/// answer that settles nothing, reads one by one when chunk batches are
/// missing, and closes at 16 MiB of stated size.
#[tokio::test]
async fn the_window_asks_again_stops_without_progress_and_closes_at_16_mib() {
    let files: Vec<(String, Vec<u8>)> = (0..6u8)
        .map(|i| (format!("big-{i}.bin"), vec![i; 3_500_000]))
        .collect();
    let serve = || -> Vec<(EntryState, Vec<u8>)> {
        files
            .iter()
            .zip(1u8..)
            .map(|((p, d), n)| served_as(p, d, n, u64::from(n)))
            .collect()
    };
    let check = |host: &Mem| {
        for (p, d) in &files {
            assert_eq!(&host.files.borrow()[p.as_str()], d, "{p}");
        }
    };
    let mut o = opts(false);
    o.max_file_size = 1 << 30;
    for (remote, singles, asks) in [
        // One record per answer: asked again until all six are held.
        (
            Scripted {
                serve: serve(),
                batch: true,
                per_call: 1,
                ..Scripted::default()
            },
            0,
            vec![4, 2],
        ),
        // Only a foreign hash: no progress, the window ends, chunks read singly.
        (
            Scripted {
                serve: serve(),
                batch: true,
                foreign: true,
                ..Scripted::default()
            },
            6,
            vec![4, 2],
        ),
        // Manifests in a batch, chunks without one: batching stops for the
        // run, so the second window reads its manifests singly too (2 + 6).
        (
            Scripted {
                serve: serve(),
                batch: true,
                no_chunk_batch: true,
                ..Scripted::default()
            },
            8,
            vec![4],
        ),
    ] {
        let host = Mem::default();
        let r = sync(&host, &remote, &mut state(), &o).await.unwrap();
        assert_eq!(r.pulled, 6, "{r:?}");
        check(&host);
        assert_eq!(remote.singles.get(), singles);
        // 4 × 3.5 MB fit 16 MiB; the fifth opens a second window.
        assert_eq!(*remote.blob_asks.borrow(), asks);
    }
}

/// Review 2026-11-15 #1: a synced file that grows past the size limit is
/// reported as unsyncable and stays known; it is never pushed as a deletion
/// (which would trash it on every other device).
#[tokio::test]
async fn a_known_file_grown_past_the_limit_is_not_deleted() {
    let host = Mem::default();
    host.files
        .borrow_mut()
        .insert("talk.pdf".into(), b"small".to_vec());
    let mut s = state();
    let path = VaultPath::parse("talk.pdf").unwrap();
    let meta = host.stat(&path).await.unwrap().unwrap();
    let entry = EntryId::new(1, &[7; 10]);
    s.entries.insert(
        entry,
        kura_core::sync::Known {
            path: path.clone(),
            revision: Uuid::from_u128(9),
            blob: blob_hash(5, &[ChunkHash::of(b"small")]),
            size: 5,
            stamp: Some(kura_core::sync::Stamp::from(&meta)),
            resend: false,
        },
    );
    // Now larger than opts' limit; any commit would fail (no reply scripted).
    host.files
        .borrow_mut()
        .insert("talk.pdf".into(), vec![b'x'; 101]);
    let remote = Scripted::default();
    let r = sync(&host, &remote, &mut s, &opts(false)).await.unwrap();
    assert_eq!(r.pushed, 0, "no deletion pushed: {r:?}");
    assert_eq!(r.unsyncable.len(), 1);
    assert!(s.entries.contains_key(&entry), "still known");
}

/// Review 2026-11-15 #5: the host killed between removing the old file and
/// moving the new one into place. The next push sends a delete against the
/// old revision, the server drops it (edited since) and the file comes back.
#[tokio::test]
async fn a_write_killed_after_the_old_file_went_brings_it_back() {
    let (old, old_data) = served_as("note.md", b"old\n", 1, 1);
    let (new, new_data) = served_as("note.md", b"new\n", 1, 2);
    let host = Mem::default();
    let mut s = state();
    s.cursor = 1;
    s.entries.insert(
        old.entry,
        kura_core::sync::Known {
            path: old.path.clone(),
            revision: old.revision,
            blob: old.blob.unwrap(),
            size: old_data.len() as u64,
            stamp: None,
            resend: false,
        },
    );
    s.pending = Some(Pending { state: new.clone() });
    let remote = Scripted {
        serve: vec![(new.clone(), new_data.clone())],
        reply: Some(CommitResponse {
            head_seq: 2,
            results: vec![OpResult {
                outcome: Outcome::Conflicted,
                head: new.clone(),
                conflict: None,
            }],
        }),
        ..Scripted::default()
    };
    sync(&host, &remote, &mut s, &opts(false)).await.unwrap();
    assert_eq!(host.files.borrow()["note.md"], new_data);
    assert_eq!(s.entries[&new.entry].revision, new.revision);
}

/// Review 2026-11-15 #10: killed after writing the new path of a remote move
/// with an edit, before trashing the old one. The old file goes on restart
/// instead of being pushed as a new note.
#[tokio::test]
async fn a_move_killed_before_the_old_file_went_trashes_it_on_restart() {
    let (old, old_data) = served_as("a.md", b"old\n", 1, 1);
    let (new, new_data) = served_as("b.md", b"new\n", 1, 2);
    let host = Mem::default();
    host.files
        .borrow_mut()
        .insert("a.md".into(), old_data.clone());
    host.files
        .borrow_mut()
        .insert("b.md".into(), new_data.clone());
    let meta = host.stat(&old.path).await.unwrap().unwrap();
    let mut s = state();
    s.cursor = 1;
    s.entries.insert(
        old.entry,
        kura_core::sync::Known {
            path: old.path.clone(),
            revision: old.revision,
            blob: old.blob.unwrap(),
            size: old_data.len() as u64,
            stamp: Some(kura_core::sync::Stamp::from(&meta)),
            resend: false,
        },
    );
    s.pending = Some(Pending { state: new.clone() });
    let remote = Scripted {
        serve: vec![(new.clone(), new_data)],
        ..Scripted::default()
    };
    let r = sync(&host, &remote, &mut s, &opts(false)).await.unwrap();
    assert_eq!(r.pushed, 0, "nothing pushed as new: {r:?}");
    assert!(
        !host.files.borrow().contains_key("a.md"),
        "old file trashed"
    );
    assert_eq!(s.entries[&new.entry].path, new.path);
}

/// Review 2026-11-15 #11: a remote change of case only (Note.md → note.md),
/// with and without an edit, on a case-insensitive disk: applied, not held.
#[tokio::test]
async fn a_change_of_case_lands_on_a_case_insensitive_disk() {
    for edited in [false, true] {
        let (old, old_data) = served_as("Note.md", b"text\n", 1, 1);
        let body: &[u8] = if edited { b"text, edited\n" } else { b"text\n" };
        let (new, new_data) = served_as("note.md", body, 1, 2);
        let host = Mem {
            fold: true,
            ..Mem::default()
        };
        host.files
            .borrow_mut()
            .insert("Note.md".into(), old_data.clone());
        let meta = host.stat(&old.path).await.unwrap().unwrap();
        let mut s = state();
        s.cursor = 1;
        s.entries.insert(
            old.entry,
            kura_core::sync::Known {
                path: old.path.clone(),
                revision: old.revision,
                blob: old.blob.unwrap(),
                size: old_data.len() as u64,
                stamp: Some(kura_core::sync::Stamp::from(&meta)),
                resend: false,
            },
        );
        let remote = Scripted {
            serve: vec![(new.clone(), new_data.clone())],
            ..Scripted::default()
        };
        let r = sync(&host, &remote, &mut s, &opts(false)).await.unwrap();
        assert!(r.deferred.is_empty() && r.failed.is_empty(), "{r:?}");
        assert_eq!(
            host.files.borrow().clone().into_iter().collect::<Vec<_>>(),
            vec![("note.md".to_owned(), new_data)],
            "edited: {edited}"
        );
        assert_eq!(s.entries[&new.entry].path, new.path);
        assert_eq!(s.cursor, 2, "the cursor moves on");
    }
}
