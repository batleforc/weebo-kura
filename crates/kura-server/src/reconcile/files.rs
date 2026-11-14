//! Reading `KURA_CONFIG_DIR`, as Kubernetes mounts a ConfigMap: each entry is
//! a symlink into `..data`, which is swapped atomically. Names starting with
//! `.` (`..data`, `..2026_11_14…`) are Kubernetes' own and skipped.

use std::path::Path;

use sha2::{Digest, Sha256};

/// Bounds on what is read (§6.11): a bad mount must not exhaust memory.
pub const MAX_FILES: usize = 256;
pub const MAX_FILE_BYTES: u64 = 1 << 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigFiles {
    /// `(file name, content)`, sorted by name.
    pub files: Vec<(String, String)>,
    /// SHA-256 over every name and content: the configuration generation.
    pub generation: [u8; 32],
}

#[derive(Debug, thiserror::Error)]
pub enum FilesError {
    #[error("cannot read {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("{0}: more than {MAX_FILES} configuration files")]
    TooMany(String),
    #[error("{0}: larger than {MAX_FILE_BYTES} bytes")]
    TooLarge(String),
}

/// Blocking: call it from `spawn_blocking`.
pub fn read_dir(dir: &Path) -> Result<ConfigFiles, FilesError> {
    let io = |path: &Path| {
        let path = path.display().to_string();
        move |source| FilesError::Io { path, source }
    };
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(io(dir))? {
        let entry = entry.map_err(io(dir))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || !(name.ends_with(".yaml") || name.ends_with(".yml")) {
            continue;
        }
        // metadata() follows the ConfigMap's symlink; symlink_metadata() would not.
        let meta = std::fs::metadata(entry.path()).map_err(io(&entry.path()))?;
        if !meta.is_file() {
            continue;
        }
        if meta.len() > MAX_FILE_BYTES {
            return Err(FilesError::TooLarge(name));
        }
        names.push(name);
        if names.len() > MAX_FILES {
            return Err(FilesError::TooMany(dir.display().to_string()));
        }
    }
    names.sort();
    let mut hasher = Sha256::new();
    let mut files = Vec::with_capacity(names.len());
    for name in names {
        let path = dir.join(&name);
        let content = std::fs::read_to_string(&path).map_err(io(&path))?;
        hasher.update(name.as_bytes());
        hasher.update([0]);
        hasher.update((content.len() as u64).to_be_bytes());
        hasher.update(content.as_bytes());
        files.push((name, content));
    }
    Ok(ConfigFiles {
        files,
        generation: hasher.finalize().into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("kura-files-{}-{name}", std::process::id()));
        drop(std::fs::remove_dir_all(&d));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// The layout kubelet writes for a ConfigMap volume.
    #[cfg(unix)]
    #[test]
    fn reads_a_configmap_mount_through_its_symlinks() {
        let d = tmp("configmap");
        let real = d.join("..2026_11_14_00_00_00.1");
        std::fs::create_dir(&real).unwrap();
        std::fs::write(real.join("tenants.yaml"), "a: 1\n").unwrap();
        std::fs::write(real.join("vaults.yaml"), "b: 2\n").unwrap();
        std::os::unix::fs::symlink("..2026_11_14_00_00_00.1", d.join("..data")).unwrap();
        std::os::unix::fs::symlink("..data/vaults.yaml", d.join("vaults.yaml")).unwrap();
        std::os::unix::fs::symlink("..data/tenants.yaml", d.join("tenants.yaml")).unwrap();
        let f = read_dir(&d).unwrap();
        let names: Vec<&str> = f.files.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["tenants.yaml", "vaults.yaml"]);

        // The atomic swap: a new directory, ..data repointed. The generation follows.
        let before = f.generation;
        let next = d.join("..2026_11_14_00_01_00.2");
        std::fs::create_dir(&next).unwrap();
        std::fs::write(next.join("tenants.yaml"), "a: 2\n").unwrap();
        std::fs::write(next.join("vaults.yaml"), "b: 2\n").unwrap();
        std::os::unix::fs::symlink("..2026_11_14_00_01_00.2", d.join("..data_tmp")).unwrap();
        std::fs::rename(d.join("..data_tmp"), d.join("..data")).unwrap();
        let after = read_dir(&d).unwrap();
        assert_ne!(after.generation, before);
        assert_eq!(after.files[0].1, "a: 2\n");
    }

    #[test]
    fn skips_other_files_and_bounds_sizes() {
        let d = tmp("bounds");
        std::fs::write(d.join("notes.txt"), "x").unwrap();
        std::fs::write(d.join(".hidden.yaml"), "x").unwrap();
        std::fs::create_dir(d.join("dir.yaml")).unwrap();
        assert!(read_dir(&d).unwrap().files.is_empty());
        std::fs::write(
            d.join("big.yaml"),
            vec![b'a'; usize::try_from(MAX_FILE_BYTES).unwrap() + 1],
        )
        .unwrap();
        assert!(matches!(read_dir(&d), Err(FilesError::TooLarge(_))));
        assert!(matches!(
            read_dir(&d.join("missing")),
            Err(FilesError::Io { .. })
        ));
    }

    #[test]
    fn generation_depends_on_names_and_contents() {
        let (a, b) = (tmp("gen-a"), tmp("gen-b"));
        std::fs::write(a.join("x.yaml"), "1").unwrap();
        std::fs::write(b.join("y.yaml"), "1").unwrap();
        assert_ne!(
            read_dir(&a).unwrap().generation,
            read_dir(&b).unwrap().generation
        );
        std::fs::write(b.join("y.yaml"), "1").unwrap();
        assert_eq!(
            read_dir(&b).unwrap().generation,
            read_dir(&b).unwrap().generation
        );
    }
}
