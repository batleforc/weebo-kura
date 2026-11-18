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
    /// Who wrote it, by the name readers may see (RFC 0005 §4.5); absent
    /// from servers before it, or when the device was purged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<Author>,
}

/// The device or scoped token that wrote a revision, never the person's
/// identity provider subject.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Author {
    /// `device` or `key`.
    pub kind: String,
    pub name: String,
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

/// Bytes before a record's payload: hash, status, big-endian length.
pub const RECORD_HEADER: usize = 37;

/// A malformed `chunks/batch` answer (RFC 0002 §4.4).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RecordError {
    #[error("a chunk record is cut short")]
    Truncated,
    #[error("a chunk record of {0} bytes is larger than a chunk")]
    TooLarge(usize),
    #[error("a chunk record has an unknown status {0}")]
    Status(u8),
}

/// The records of a `chunks/batch` answer, in order: a hash with its bytes,
/// or with `None` when the server has nothing it may send. Lengths are
/// checked against the rest of the body and the largest chunk before any
/// slice is taken; the bytes are not checked against their hash here.
/// A hash with its bytes, or `None` for a chunk the server may not send.
pub type Record<'a> = (ChunkHash, Option<&'a [u8]>);

/// Appends one record: found (`Some`, status 0) or missing (status 1). The
/// one writer of the framing, for `chunks/batch` answers and `chunks/upload`
/// bodies alike (RFC 0002 §4.4). A chunk is at most 4 MiB, so its length
/// fits the 4-byte field.
pub fn write_record(out: &mut Vec<u8>, hash: &ChunkHash, data: Option<&[u8]>) {
    out.extend_from_slice(hash.as_bytes());
    let (status, data) = match data {
        Some(d) => (0, d),
        None => (1, &[][..]),
    };
    out.push(status);
    out.extend_from_slice(&u32::try_from(data.len()).unwrap_or(u32::MAX).to_be_bytes());
    out.extend_from_slice(data);
}

pub fn parse_records(mut body: &[u8]) -> Result<Vec<Record<'_>>, RecordError> {
    let mut out = Vec::new();
    while !body.is_empty() {
        let (head, rest) = body
            .split_at_checked(RECORD_HEADER)
            .ok_or(RecordError::Truncated)?;
        let (hash, tail) = head.split_at(32);
        let hash = <[u8; 32]>::try_from(hash).map_err(|_| RecordError::Truncated)?;
        let (status, len) = tail.split_first().ok_or(RecordError::Truncated)?;
        let len = <[u8; 4]>::try_from(len).map_err(|_| RecordError::Truncated)?;
        let len = usize::try_from(u32::from_be_bytes(len)).map_err(|_| RecordError::Truncated)?;
        if len > crate::chunk::MAX_CHUNK {
            return Err(RecordError::TooLarge(len));
        }
        let (data, rest) = rest.split_at_checked(len).ok_or(RecordError::Truncated)?;
        let item = match status {
            0 => Some(data),
            1 if len == 0 => None,
            s => return Err(RecordError::Status(*s)),
        };
        out.push((ChunkHash::from_bytes(hash), item));
        body = rest;
    }
    Ok(out)
}

#[cfg(test)]
mod record_tests {
    use super::*;

    fn record(h: ChunkHash, status: u8, data: &[u8]) -> Vec<u8> {
        let mut out = h.as_bytes().to_vec();
        out.push(status);
        out.extend_from_slice(&u32::try_from(data.len()).unwrap_or(u32::MAX).to_be_bytes());
        out.extend_from_slice(data);
        out
    }

    #[test]
    fn the_writer_writes_what_the_parser_reads() {
        let (a, b) = (ChunkHash::of(b"a"), ChunkHash::of(b"b"));
        let mut body = Vec::new();
        write_record(&mut body, &a, Some(b"a"));
        write_record(&mut body, &b, None);
        write_record(&mut body, &b, Some(b""));
        assert_eq!(
            parse_records(&body),
            Ok(vec![(a, Some(&b"a"[..])), (b, None), (b, Some(&b""[..]))])
        );
    }

    #[test]
    fn records_round_trip_and_refuse_bad_lengths() {
        let (a, b) = (ChunkHash::of(b"a"), ChunkHash::of(b"b"));
        let mut body = record(a, 0, b"a");
        body.extend(record(b, 1, b""));
        assert_eq!(
            parse_records(&body),
            Ok(vec![(a, Some(&b"a"[..])), (b, None)])
        );
        assert_eq!(parse_records(&[]), Ok(vec![]));
        assert_eq!(parse_records(&body[..40]), Err(RecordError::Truncated));
        assert_eq!(parse_records(&body[..10]), Err(RecordError::Truncated));
        let mut huge = a.as_bytes().to_vec();
        huge.push(0);
        huge.extend_from_slice(&u32::MAX.to_be_bytes());
        assert!(matches!(
            parse_records(&huge),
            Err(RecordError::TooLarge(_))
        ));
        assert_eq!(
            parse_records(&record(a, 7, b"")),
            Err(RecordError::Status(7))
        );
        assert_eq!(
            parse_records(&record(a, 1, b"x")),
            Err(RecordError::Status(1))
        );
    }
}
