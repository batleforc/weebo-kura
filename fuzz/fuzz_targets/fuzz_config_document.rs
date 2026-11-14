//! Resource documents are input the server did not produce (RFC 0001 §14).
//! Property: an accepted document renders back to YAML that parses to the
//! same value, and validation never panics on anything the parser accepts.
#![no_main]

use kura_api::{PreviousState, Resource, parse_file, validate};
use libfuzzer_sys::fuzz_target;

/// Renders one document alone, so it parses back as `#1` of its file.
fn render(r: &Resource) -> (Resource, String) {
    let (kind, yaml, alone) = match r {
        Resource::Tenant(s, d) => ("KuraTenant", serde_norway::to_string(d), Resource::Tenant(first(s), d.clone())),
        Resource::Vault(s, d) => ("KuraVault", serde_norway::to_string(d), Resource::Vault(first(s), d.clone())),
    };
    let yaml = yaml.expect("an accepted document renders");
    (alone, format!("apiVersion: kura.weebo.io/v1alpha1\nkind: {kind}\n{yaml}"))
}

fn first(s: &kura_api::Source) -> kura_api::Source {
    kura_api::Source { file: s.file.clone(), index: 1 }
}

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(resources) = parse_file("fuzz.yaml", text) else {
        return;
    };
    for r in &resources {
        let (alone, yaml) = render(r);
        let reparsed = parse_file("fuzz.yaml", &yaml).expect("a rendered document parses");
        assert_eq!(reparsed, vec![alone], "round trip changed the document: {yaml}");
    }
    // Only "no panic" here: rejection is a normal outcome.
    if let Err(errors) = validate(resources, &PreviousState::default()) {
        assert!(!errors.is_empty());
    }
});
