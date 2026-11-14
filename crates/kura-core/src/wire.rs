//! The sync protocol's wire types (RFC 0001 §4.4, §6.3), shared by the server
//! and every client so both sides parse the same way. Deserializing parses:
//! a path, an entry id or a hash that breaks a rule is refused here.

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::chunk::ChunkHash;
use crate::path::VaultPath;

/// A stable entry id, chosen by the client: a UUIDv7 (§6.3).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema), schema(value_type = String, format = Uuid))]
pub struct EntryId(Uuid);

impl EntryId {
    /// From a millisecond timestamp and 10 random bytes; the caller supplies
    /// both, so this crate needs no clock or random source (it runs in WASM).
    pub fn new(unix_millis: u64, random: &[u8; 10]) -> Self {
        Self(uuid::Builder::from_unix_timestamp_millis(unix_millis, random).into_uuid())
    }

    pub fn parse(s: &str) -> Option<Self> {
        Uuid::parse_str(s).ok().and_then(Self::from_uuid)
    }

    pub fn from_uuid(u: Uuid) -> Option<Self> {
        (u.get_version_num() == 7).then_some(Self(u))
    }

    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl fmt::Display for EntryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Debug for EntryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EntryId({})", self.0)
    }
}

impl<'de> Deserialize<'de> for EntryId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let u = Uuid::deserialize(d)?;
        Self::from_uuid(u).ok_or_else(|| serde::de::Error::custom("an entry id is a UUIDv7"))
    }
}

/// A revision id, chosen by the server. Fields spell it `Uuid` so the
/// OpenAPI derive recognizes the type.
pub type RevisionId = Uuid;

/// One operation of a commit. A rename is a `put` with another path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "op",
    rename_all = "lowercase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum Operation {
    /// Create (no `base`) or update an entry: its content, its path, or both.
    Put {
        entry: EntryId,
        #[cfg_attr(feature = "openapi", schema(value_type = String))]
        path: VaultPath,
        /// The revision this edit started from; absent for a new entry.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base: Option<Uuid>,
        /// The blob address (`blob_hash`) of the content, already uploaded.
        #[cfg_attr(feature = "openapi", schema(value_type = String))]
        blob: ChunkHash,
        size: u64,
    },
    /// Delete an entry, as of the revision the client last saw.
    Delete { entry: EntryId, base: Uuid },
}

impl Operation {
    pub const fn entry(&self) -> EntryId {
        match self {
            Self::Put { entry, .. } | Self::Delete { entry, .. } => *entry,
        }
    }
}

/// `POST /vaults/{v}/commits`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct CommitRequest {
    /// Unique per device for 7 days; a retry reuses it with the same body.
    pub idempotency_key: String,
    /// At most 1,000.
    pub operations: Vec<Operation>,
    /// The epoch this device's state was built under: another one answers
    /// `410 history_rewound` (§6.3). Absent on a first push.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epoch: Option<Uuid>,
    /// `config`: the caller's own Obsidian configuration for its device's
    /// profile (§6.8); absent: the notes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum Outcome {
    /// Applied as sent, or already the head.
    Accepted,
    /// Merged with edits made since its base (from phase 4).
    Merged,
    /// The head was kept; the push is stored as a conflict file, or, for a
    /// delete of an entry edited since, dropped.
    Conflicted,
}

/// An entry's current state, as `changes` and commit results return it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct EntryState {
    pub entry: EntryId,
    pub revision: Uuid,
    pub seq: u64,
    #[cfg_attr(feature = "openapi", schema(value_type = String))]
    pub path: VaultPath,
    pub deleted: bool,
    /// Absent when deleted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>))]
    pub blob: Option<ChunkHash>,
    pub size: u64,
}

/// The result of one operation, in request order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct OpResult {
    pub outcome: Outcome,
    /// The pushed entry as it now stands. For a new entry unified with an
    /// identical one at the same path, that other entry.
    pub head: EntryState,
    /// The conflict file holding the pushed content, when one was created.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict: Option<EntryState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct CommitResponse {
    pub head_seq: u64,
    pub results: Vec<OpResult>,
}

/// `GET /vaults/{v}/changes` and `GET /vaults/{v}/entries`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ChangesResponse {
    /// Changes when the server loses history (§6.3); send it back with `since`.
    pub epoch: Uuid,
    /// The head this page was read against, in the same snapshot.
    pub head_seq: u64,
    pub items: Vec<EntryState>,
    /// Set when more pages follow: the next `since` (changes) or `after` (entries).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
}

/// One stored revision of an entry (`GET …/entries/{e}/revisions`, §4.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RevisionInfo {
    pub revision: Uuid,
    pub seq: u64,
    #[cfg_attr(feature = "openapi", schema(value_type = String))]
    pub path: VaultPath,
    pub deleted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<String>))]
    pub blob: Option<ChunkHash>,
    pub size: u64,
    /// RFC 3339, UTC.
    pub created_at: String,
}

/// An entry's history, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RevisionsResponse {
    pub items: Vec<RevisionInfo>,
    /// Set when older revisions follow: the next `before`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<u64>,
}

/// `POST /vaults/{v}/entries/{e}/restore`: a new revision with this one's content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RestoreRequest {
    pub revision: Uuid,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eid() -> EntryId {
        EntryId::new(1_763_000_000_000, &[7; 10])
    }

    #[test]
    fn entry_ids_are_uuid_v7() {
        let e = eid();
        assert_eq!(e.as_uuid().get_version_num(), 7);
        assert_eq!(EntryId::parse(&e.to_string()), Some(e));
        assert_eq!(
            EntryId::parse("67e55044-10b1-426f-9247-bb680e5fe0c8"),
            None,
            "v4"
        );
        assert_eq!(EntryId::parse("nope"), None);
        assert!(
            serde_json::from_str::<EntryId>("\"67e55044-10b1-426f-9247-bb680e5fe0c8\"").is_err()
        );
        assert!(format!("{e:?}").starts_with("EntryId("));
    }

    #[test]
    fn commit_bodies_parse_strictly() {
        let e = eid();
        let h = ChunkHash::of(b"x");
        let body = format!(
            r#"{{"idempotencyKey":"k1","operations":[
                {{"op":"put","entry":"{e}","path":"a.md","blob":"{h}","size":1}},
                {{"op":"delete","entry":"{e}","base":"67e55044-10b1-426f-9247-bb680e5fe0c8"}}]}}"#
        );
        let c: CommitRequest = serde_json::from_str(&body).unwrap();
        assert_eq!(c.operations.len(), 2);
        assert_eq!(c.operations[0].entry(), e);
        let back: CommitRequest =
            serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back, c);
        for bad in [
            body.replace("a.md", "../a.md"),
            body.replace("a.md", ".obsidian/x"),
            body.replace(r#""size":1"#, r#""size":1,"extra":1"#),
            body.replace("\"put\"", "\"move\""),
            body.replace(&h.to_string(), "00"),
            body.replace("idempotencyKey", "key"),
        ] {
            assert!(
                serde_json::from_str::<CommitRequest>(&bad).is_err(),
                "{bad}"
            );
        }
    }
}
