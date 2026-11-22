//! The body of `PUT /v1/admin/documents` (RFC 0009 §14): JSON from the one
//! workload allowed to push, still parsed as untrusted. Property: a name is
//! given only to a document of a known kind, made of labels, and every named
//! document goes through the file parser without panicking.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(body) = serde_json::from_slice::<serde_json::Value>(data) else {
        return;
    };
    let Some(documents) = body.get("documents").and_then(serde_json::Value::as_array) else {
        return;
    };
    for doc in documents {
        let Ok(name) = kura_api::pushed_name(doc) else {
            continue;
        };
        let parts: Vec<&str> = name.split('/').collect();
        assert!(matches!(parts.as_slice(), ["KuraTenant", _] | ["KuraVault", _, _]), "{name}");
        assert!(!name.contains(['\n', '\r', ' ']), "{name:?}");
        let text = serde_json::to_string(doc).expect("a value renders");
        // Rejection is a normal outcome; only a panic is a finding.
        drop(kura_api::parse_file(&name, &text));
    }
});
