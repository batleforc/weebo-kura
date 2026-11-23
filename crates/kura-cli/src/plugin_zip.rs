//! An Obsidian plugin's release zip, read for `kura vault plugin install`
//! (RFC 0010 §7.3): at most 64 entries, 64 MiB uncompressed and a ratio of
//! 100 per entry read; only `manifest.json`, `main.js` and `styles.css` at the
//! root or under one top folder are taken, the rest ignored, and no name is
//! ever used as a path. Depends on `zip` and `serde_json` only: the fuzz
//! target `fuzz_plugin_zip` includes this file as is.

use std::io::Read as _;

pub const MAX_ENTRIES: usize = 64;
pub const MAX_TOTAL: u64 = 64 << 20;
pub const MAX_RATIO: u64 = 100;
/// The server's limit for `manifest.json`.
pub const MAX_MANIFEST: usize = 64 << 10;

const FILES: [&str; 3] = ["manifest.json", "main.js", "styles.css"];

/// The three code files of one plugin, and what its manifest says.
#[derive(Debug)]
pub struct Plugin {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub manifest: Vec<u8>,
    pub main: Vec<u8>,
    pub styles: Option<Vec<u8>>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ZipError {
    NoPlugin,
    TooLarge(&'static str),
}

impl std::fmt::Display for ZipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoPlugin => f.write_str("this zip holds no Obsidian plugin"),
            Self::TooLarge(why) => write!(f, "this zip is refused: {why}"),
        }
    }
}

/// `kura_api::spec::plugin_id_ok`, the same rule (the fuzz target checks
/// they agree): 1–64 of `a-z0-9-`, never the Kura plugin.
pub fn id_ok(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id != "weebo-kura"
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// An entry name as (its top folder, `""` at the root; which of the three
/// files), when it is one of them at the root or one folder down.
fn place(name: &str) -> Option<(&str, usize)> {
    if name.starts_with('/') {
        return None;
    }
    let (top, file) = name.split_once('/').unwrap_or(("", name));
    if file.contains('/') {
        return None;
    }
    FILES.iter().position(|f| *f == file).map(|k| (top, k))
}

pub fn read(bytes: &[u8]) -> Result<Plugin, ZipError> {
    let mut zip =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| ZipError::NoPlugin)?;
    if zip.len() > MAX_ENTRIES {
        return Err(ZipError::TooLarge("more than 64 entries"));
    }
    // (top folder, which file, entry index) of every candidate.
    let mut found: Vec<(String, usize, usize)> = Vec::new();
    for i in 0..zip.len() {
        let f = zip.by_index_raw(i).map_err(|_| ZipError::NoPlugin)?;
        // A name that does not decode cannot be one of the three.
        let Ok(name) = f.name() else { continue };
        if let Some((top, k)) = place(&name).filter(|_| !f.is_dir()) {
            found.push((top.to_owned(), k, i));
        }
    }
    // Exactly one folder holds a manifest: the plugin's.
    let tops: Vec<&str> = found
        .iter()
        .filter(|(_, k, _)| *k == 0)
        .map(|(t, _, _)| t.as_str())
        .collect();
    let [top] = tops.as_slice() else {
        return Err(ZipError::NoPlugin);
    };
    let top = (*top).to_owned();
    let mut files: [Option<Vec<u8>>; 3] = Default::default();
    let mut total = 0u64;
    for (_, k, i) in found.iter().filter(|(t, _, _)| *t == top) {
        let f = zip.by_index(*i).map_err(|_| ZipError::NoPlugin)?;
        let size = f.size();
        if size > f.compressed_size().saturating_mul(MAX_RATIO) {
            return Err(ZipError::TooLarge(
                "an entry is compressed more than 100 times",
            ));
        }
        total = total.saturating_add(size);
        if total > MAX_TOTAL {
            return Err(ZipError::TooLarge("more than 64 MiB uncompressed"));
        }
        // A declared size can lie: never read past it.
        let mut out = Vec::new();
        f.take(size.saturating_add(1))
            .read_to_end(&mut out)
            .map_err(|_| ZipError::NoPlugin)?;
        if u64::try_from(out.len()).ok() != Some(size) {
            return Err(ZipError::NoPlugin);
        }
        if let Some(slot) = files.get_mut(*k) {
            *slot = Some(out);
        }
    }
    let [Some(manifest), Some(main), styles] = files else {
        return Err(ZipError::NoPlugin);
    };
    if manifest.len() > MAX_MANIFEST {
        return Err(ZipError::NoPlugin);
    }
    let v: serde_json::Value = serde_json::from_slice(&manifest).map_err(|_| ZipError::NoPlugin)?;
    let text = |k: &str| v.get(k).and_then(serde_json::Value::as_str);
    let id = text("id")
        .filter(|id| id_ok(id))
        .ok_or(ZipError::NoPlugin)?
        .to_owned();
    Ok(Plugin {
        name: text("name").unwrap_or(&id).to_owned(),
        version: text("version").map(str::to_owned),
        id,
        manifest,
        main,
        styles,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    const MANIFEST: &str = r#"{"id":"excalidraw","name":"Excalidraw","version":"2.6.1"}"#;

    /// A zip of `(name, bytes)`, deflated.
    fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, bytes) in files {
            assert!(w.start_file(*name, opts).is_ok());
            assert!(w.write_all(bytes).is_ok());
        }
        w.finish().map(|c| c.into_inner()).unwrap_or_default()
    }

    #[test]
    fn a_release_zip_at_the_root_or_under_one_folder_is_read() {
        for top in ["", "excalidraw-2.6.1/"] {
            let m = format!("{top}manifest.json");
            let j = format!("{top}main.js");
            let z = zip_of(&[
                (&m, MANIFEST.as_bytes()),
                (&j, b"module.exports = {};"),
                ("README.md", b"ignored"),
            ]);
            let p = read(&z);
            assert!(p.is_ok(), "{top}: {p:?}");
            if let Ok(p) = p {
                assert_eq!(
                    (p.id.as_str(), p.name.as_str(), p.version.as_deref()),
                    ("excalidraw", "Excalidraw", Some("2.6.1"))
                );
                assert_eq!(p.main, b"module.exports = {};");
                assert!(p.styles.is_none());
            }
        }
    }

    #[test]
    fn what_is_not_one_plugin_is_refused() {
        let main: (&str, &[u8]) = ("main.js", b"x");
        let cases: [(&str, Vec<u8>); 6] = [
            ("not a zip", b"PK nothing".to_vec()),
            ("no manifest", zip_of(&[main])),
            (
                "nested two folders",
                zip_of(&[
                    ("a/b/manifest.json", MANIFEST.as_bytes()),
                    ("a/b/main.js", b"x"),
                ]),
            ),
            (
                "the Kura plugin",
                zip_of(&[
                    (
                        "manifest.json",
                        br#"{"id":"weebo-kura","name":"Kura"}"#.as_slice(),
                    ),
                    main,
                ]),
            ),
            (
                "two plugins",
                zip_of(&[
                    ("a/manifest.json", MANIFEST.as_bytes()),
                    ("a/main.js", b"x"),
                    ("b/manifest.json", MANIFEST.as_bytes()),
                    ("b/main.js", b"x"),
                ]),
            ),
            (
                "no main.js",
                zip_of(&[("manifest.json", MANIFEST.as_bytes())]),
            ),
        ];
        for (why, z) in cases {
            assert_eq!(read(&z).err(), Some(ZipError::NoPlugin), "{why}");
        }
    }

    #[test]
    fn bombs_and_crowds_are_refused() {
        let zeros = vec![0u8; 1 << 20];
        let bomb = zip_of(&[("manifest.json", MANIFEST.as_bytes()), ("main.js", &zeros)]);
        assert!(matches!(read(&bomb), Err(ZipError::TooLarge(_))));
        let names: Vec<String> = (0..65).map(|i| format!("f{i}")).collect();
        let crowd: Vec<(&str, &[u8])> = names
            .iter()
            .map(|n| (n.as_str(), b"x".as_slice()))
            .collect();
        assert_eq!(
            read(&zip_of(&crowd)).err(),
            Some(ZipError::TooLarge("more than 64 entries"))
        );
    }
}
