//! The vault folder on disk (RFC 0001 §6.8): writes go through a temporary
//! file renamed into place, and no symbolic link is ever followed, listed or
//! written through, so a path received from the server cannot reach outside
//! the folder (T14).

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use kura_core::VaultPath;
use kura_core::sync::{FileMeta, Host, HostError, LocalState};

/// Where the local state lives, inside the vault folder; a dot-folder, so it never syncs.
pub const STATE_DIR: &str = ".kura";
const TRASH_DIR: &str = ".trash";

pub struct FsHost {
    root: PathBuf,
}

fn err(context: &str, e: impl std::fmt::Display) -> HostError {
    HostError(format!("{context}: {e}"))
}

fn mtime(m: &std::fs::Metadata) -> i64 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| i64::try_from(d.as_millis()).ok())
        .unwrap_or_default()
}

impl FsHost {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn state_file(&self) -> PathBuf {
        self.root.join(STATE_DIR).join("state.json")
    }

    pub fn load_state(&self) -> Option<LocalState> {
        let bytes = std::fs::read(self.state_file()).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    /// `path` under the root, refusing any existing component that is a link.
    fn resolve(&self, path: &VaultPath) -> Result<PathBuf, HostError> {
        let mut p = self.root.clone();
        for seg in path.segments() {
            p.push(seg);
            match std::fs::symlink_metadata(&p) {
                Ok(m) if m.file_type().is_symlink() => {
                    return Err(HostError(format!("{path}: a symbolic link is in the way")));
                }
                _ => {}
            }
        }
        Ok(p)
    }

    /// Creates `root/<segments>` folder by folder, refusing a link or a
    /// file anywhere on the way (also for `.kura` and `.trash`).
    fn ensure_dir<'a>(
        &self,
        segments: impl IntoIterator<Item = &'a str>,
    ) -> Result<PathBuf, HostError> {
        let mut p = self.root.clone();
        for seg in segments {
            p.push(seg);
            match std::fs::symlink_metadata(&p) {
                Ok(m) if m.is_dir() => {}
                Ok(_) => return Err(HostError(format!("{}: not a folder", p.display()))),
                Err(_) => std::fs::create_dir(&p).map_err(|e| err("creating a folder", e))?,
            }
        }
        Ok(p)
    }

    fn create_parents(&self, path: &VaultPath) -> Result<(), HostError> {
        let segs: Vec<&str> = path.segments().collect();
        self.ensure_dir(segs.iter().take(segs.len().saturating_sub(1)).copied())
            .map(drop)
    }

    fn meta(path: &VaultPath, full: &Path) -> Result<FileMeta, HostError> {
        let m = std::fs::symlink_metadata(full).map_err(|e| err(path.as_str(), e))?;
        Ok(FileMeta {
            path: path.to_string(),
            size: m.len(),
            mtime: mtime(&m),
        })
    }

    /// Atomic write of `data` to `target`, through a new temporary file in
    /// `.kura`: a short name, whatever the target's (review 3.6 F2), never an
    /// existing file or link.
    fn replace(&self, target: &Path, data: &[u8]) -> Result<(), HostError> {
        let dir = self.ensure_dir([STATE_DIR])?;
        let tmp = dir.join(format!("tmp-{}", uuid::Uuid::new_v4().simple()));
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|e| err("writing", e))?;
        let written = f.write_all(data).and_then(|()| f.sync_all());
        let renamed = written.and_then(|()| std::fs::rename(&tmp, target));
        if renamed.is_err() {
            drop(std::fs::remove_file(&tmp));
        }
        renamed.map_err(|e| err("writing", e))
    }

    fn walk(&self, dir: &Path, rel: &str, out: &mut Vec<FileMeta>) -> Result<(), HostError> {
        let entries = std::fs::read_dir(dir).map_err(|e| err("listing", e))?;
        for entry in entries {
            let entry = entry.map_err(|e| err("listing", e))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            let m = std::fs::symlink_metadata(entry.path()).map_err(|e| err("listing", e))?;
            if m.file_type().is_symlink() {
                continue;
            }
            if m.is_dir() {
                // Dot-folders are not notes (decision 28): not even walked.
                if !name.starts_with('.') {
                    self.walk(&entry.path(), &path, out)?;
                }
            } else if m.is_file() {
                out.push(FileMeta {
                    path,
                    size: m.len(),
                    mtime: mtime(&m),
                });
            }
        }
        Ok(())
    }
}

impl Host for FsHost {
    async fn list(&self) -> Result<Vec<FileMeta>, HostError> {
        let mut out = Vec::new();
        self.walk(&self.root, "", &mut out)?;
        Ok(out)
    }

    async fn read(&self, path: &VaultPath) -> Result<Vec<u8>, HostError> {
        std::fs::read(self.resolve(path)?).map_err(|e| err(path.as_str(), e))
    }

    async fn stat(&self, path: &VaultPath) -> Result<Option<FileMeta>, HostError> {
        let full = self.resolve(path)?;
        if std::fs::symlink_metadata(&full).is_err() {
            return Ok(None);
        }
        Self::meta(path, &full).map(Some)
    }

    async fn write(&self, path: &VaultPath, data: &[u8]) -> Result<FileMeta, HostError> {
        self.create_parents(path)?;
        let full = self.resolve(path)?;
        self.replace(&full, data)?;
        Self::meta(path, &full)
    }

    async fn rename(&self, from: &VaultPath, to: &VaultPath) -> Result<FileMeta, HostError> {
        let src = self.resolve(from)?;
        self.create_parents(to)?;
        let dst = self.resolve(to)?;
        if std::fs::symlink_metadata(&dst).is_ok() {
            return Err(HostError(format!("{to} already exists")));
        }
        std::fs::rename(&src, &dst).map_err(|e| err(from.as_str(), e))?;
        Self::meta(to, &dst)
    }

    async fn trash(&self, path: &VaultPath) -> Result<(), HostError> {
        let src = self.resolve(path)?;
        let segs: Vec<&str> = path.segments().collect();
        let dir = self.ensure_dir(
            std::iter::once(TRASH_DIR)
                .chain(segs.iter().take(segs.len().saturating_sub(1)).copied()),
        )?;
        let name = path.name();
        let mut dst = dir.join(name);
        let mut n = 1u32;
        while std::fs::symlink_metadata(&dst).is_ok() {
            dst = dir.join(format!("{name}.{n}"));
            n = n.saturating_add(1);
        }
        std::fs::rename(&src, &dst).map_err(|e| err("trash", e))
    }

    async fn save(&self, state: &LocalState) -> Result<(), HostError> {
        let bytes = serde_json::to_vec(state).map_err(|e| err("saving state", e))?;
        let dir = self.ensure_dir([STATE_DIR])?;
        self.replace(&dir.join("state.json"), &bytes)
    }

    fn now_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|d| u64::try_from(d.as_millis()).ok())
            .unwrap_or_default()
    }

    fn random(&self) -> [u8; 16] {
        uuid::Uuid::new_v4().into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kura-fs-{name}-{}", std::process::id()));
        drop(std::fs::remove_dir_all(&d));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn p(s: &str) -> VaultPath {
        VaultPath::parse(s).unwrap()
    }

    #[tokio::test]
    async fn writes_atomically_and_lists_notes_only() {
        let root = tmp("write");
        let h = FsHost::new(root.clone());
        h.write(&p("a/b/c.md"), b"hello").await.unwrap();
        std::fs::create_dir_all(root.join(".obsidian")).unwrap();
        std::fs::write(root.join(".obsidian/app.json"), "{}").unwrap();
        let list = h.list().await.unwrap();
        assert_eq!(
            list.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(),
            ["a/b/c.md"]
        );
        assert_eq!(h.read(&p("a/b/c.md")).await.unwrap(), b"hello");
        h.trash(&p("a/b/c.md")).await.unwrap();
        assert!(root.join(".trash/a/b/c.md").exists());
        assert!(h.stat(&p("a/b/c.md")).await.unwrap().is_none());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn never_follows_a_symbolic_link() {
        let root = tmp("link");
        let outside = tmp("outside");
        std::os::unix::fs::symlink(&outside, root.join("escape")).unwrap();
        std::os::unix::fs::symlink(outside.join("x.md"), root.join("x.md")).unwrap();
        let h = FsHost::new(root.clone());
        assert!(h.write(&p("escape/pwned.md"), b"x").await.is_err());
        assert!(h.write(&p("x.md"), b"x").await.is_err());
        assert!(!outside.join("pwned.md").exists());
        assert!(!outside.join("x.md").exists());
        assert!(h.list().await.unwrap().is_empty(), "links are not listed");
        // Planted links on the client's own folders are refused too.
        std::fs::remove_file(root.join("x.md")).unwrap();
        std::fs::write(root.join("x.md"), "x").unwrap();
        std::os::unix::fs::symlink(&outside, root.join(".trash")).unwrap();
        assert!(h.trash(&p("x.md")).await.is_err());
        std::os::unix::fs::symlink(&outside, root.join(".kura")).unwrap();
        assert!(h.write(&p("y.md"), b"y").await.is_err());
        assert!(
            std::fs::read_dir(&outside).unwrap().next().is_none(),
            "nothing written outside"
        );
    }

    #[tokio::test]
    async fn a_255_byte_name_is_written() {
        let root = tmp("long");
        let h = FsHost::new(root);
        let name = format!("{}.md", "n".repeat(252));
        h.write(&p(&name), b"x").await.unwrap();
        assert_eq!(h.read(&p(&name)).await.unwrap(), b"x");
    }
}
