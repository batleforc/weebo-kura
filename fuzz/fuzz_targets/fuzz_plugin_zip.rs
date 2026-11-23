//! The CLI's reader of Obsidian plugin release zips (RFC 0010 §7.3), included
//! as is from kura-cli. Property: any bytes are refused or give one plugin
//! whose id the server accepts, within the bounds; never a panic.
#![no_main]

use libfuzzer_sys::fuzz_target;

#[path = "../../crates/kura-cli/src/plugin_zip.rs"]
#[allow(dead_code)]
mod plugin_zip;

fuzz_target!(|data: &[u8]| {
    let Ok(p) = plugin_zip::read(data) else {
        return;
    };
    assert!(kura_api::spec::plugin_id_ok(&p.id), "{}", p.id);
    assert!(p.manifest.len() <= plugin_zip::MAX_MANIFEST);
    let total = p.manifest.len() + p.main.len() + p.styles.as_ref().map_or(0, Vec::len);
    assert!(total as u64 <= plugin_zip::MAX_TOTAL);
});
