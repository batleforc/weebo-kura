//! RFC 0010 §7.3: the share item grammar and the home of a path never
//! panic, and an accepted item is one of the closed forms.
#![no_main]

use kura_api::spec::ConfigSync;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };
    if ConfigSync::item_category(s).is_some() {
        let id = s.strip_prefix("plugin:");
        assert!(id.map_or(matches!(s, "snippets" | "themes" | "hotkeys"), kura_api::spec::plugin_id_ok));
    }
    let all = ["plugins", "snippets", "themes", "hotkeys"].map(String::from);
    let home = kura_core::config::home(s, &all, &[s.to_owned()], &[s.to_owned()]);
    if home.is_some() {
        assert!(kura_core::config::synced(s, &all));
    }
});
