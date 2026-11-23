//! One case per row of RFC 0001 §4.3 that concerns documents, plus defaults.

use super::*;
use crate::parse::parse_file;
use crate::spec::DeletionPolicy;
use crate::{DnsLabel, Quantity};

const TENANT: &str = r#"
apiVersion: kura.weebo.io/v1alpha1
kind: KuraTenant
metadata: { name: maxime }
spec:
  claimMappings:
    claim: groups
    roles: { owner: [kura-maxime-owners], admin: [], member: [kura-maxime] }
  vaultNamespaces: [maxime]
  retention: { maxRevisionAge: 180d, maxRevisionsPerEntry: 200, deletedEntryTtl: 90d }
  quota: { storage: 20Gi, maxFileSize: 500Mi }
  search: { enabled: true }
  deletionPolicy: Orphan
"#;

const VAULT: &str = r#"
apiVersion: kura.weebo.io/v1alpha1
kind: KuraVault
metadata: { name: knowledge, namespace: maxime }
spec:
  tenantRef: { name: maxime }
  chunking: { min: 256Ki, avg: 1Mi, max: 4Mi }
  configSync: { categories: [plugins, snippets, themes, hotkeys] }
  bindings:
    - { role: editor, claimValue: kura-maxime }
"#;

fn tenant(name: &str, member: &str) -> String {
    format!(
        "apiVersion: kura.weebo.io/v1alpha1\nkind: KuraTenant\nmetadata: {{ name: {name} }}\nspec:\n  claimMappings: {{ claim: groups, roles: {{ member: [{member}] }} }}\n  vaultNamespaces: [{name}]\n"
    )
}

fn parse(files: &[(&str, &str)]) -> Vec<Resource> {
    files
        .iter()
        .flat_map(|(f, c)| parse_file(f, c).unwrap())
        .collect()
}

fn errors(files: &[(&str, &str)], previous: &PreviousState) -> Vec<String> {
    validate(parse(files), previous)
        .unwrap_err()
        .iter()
        .map(ToString::to_string)
        .collect()
}

fn key(ns: &str, name: &str) -> VaultKey {
    VaultKey {
        namespace: DnsLabel::try_from(ns).unwrap(),
        name: DnsLabel::try_from(name).unwrap(),
    }
}

fn previous_tenants(names: &[&str]) -> PreviousState {
    PreviousState {
        tenants: names
            .iter()
            .map(|n| DnsLabel::try_from(*n).unwrap())
            .collect(),
        ..PreviousState::default()
    }
}

#[test]
fn the_rfc_example_is_valid() {
    let config = validate(
        parse(&[("tenants.yaml", TENANT), ("vaults.yaml", VAULT)]),
        &PreviousState::default(),
    )
    .unwrap();
    let t = &config.tenants["maxime"].doc.spec;
    assert_eq!(t.quota.storage, Some(Quantity::from_bytes(20 << 30)));
    assert_eq!(t.retention.max_revision_age.unwrap().hours(), 180 * 24);
    let v = &config.vaults[&key("maxime", "knowledge")];
    assert_eq!(v.source.to_string(), "vaults.yaml#1");
    assert_eq!(v.doc.spec.chunking, Some(Chunking::DEFAULT));
}

#[test]
fn defaults_are_on_the_closed_side() {
    let minimal =
        "apiVersion: kura.weebo.io/v1alpha1\nkind: KuraTenant\nmetadata: { name: t }\nspec: {}\n";
    let vault = "apiVersion: kura.weebo.io/v1alpha1\nkind: KuraVault\nmetadata: { name: v, namespace: t }\nspec: { tenantRef: { name: t } }\n";
    let resources = parse(&[("t.yaml", minimal), ("v.yaml", vault)]);
    let Resource::Tenant(_, t) = &resources[0] else {
        panic!()
    };
    assert_eq!(t.spec.claim_mappings, None, "no members");
    assert!(
        t.spec.vault_namespaces.is_empty(),
        "no namespace may declare a vault"
    );
    assert!(!t.spec.search.enabled, "search is opt-in");
    assert_eq!(t.spec.deletion_policy, DeletionPolicy::Orphan);
    let Resource::Vault(_, v) = &resources[1] else {
        panic!()
    };
    assert!(v.spec.bindings.is_empty(), "only tenant owner and admin");
    assert_eq!(v.spec.config_sync, None, ".obsidian does not sync");
    assert_eq!(v.spec.chunking, None, "the stored value applies");
    assert_eq!(v.spec.deletion_policy, DeletionPolicy::Orphan);
    // Absent vaultNamespaces: the vault is refused.
    let e = validate(resources, &PreviousState::default()).unwrap_err();
    assert!(
        e[0].to_string().contains("not in the vaultNamespaces"),
        "{e:?}"
    );
}

// Row 1: schema, unknown kind, unknown role.
#[test]
fn unknown_role_is_refused_with_its_path() {
    let doc = TENANT.replace("owner: [kura-maxime-owners]", "ownr: [kura-maxime-owners]");
    let doc = format!("{TENANT}---\n{doc}");
    let e = parse_file("tenants.yaml", &doc).unwrap_err();
    assert_eq!(e.len(), 1);
    let s = e[0].to_string();
    assert!(
        s.starts_with("tenants.yaml#2 spec.claimMappings.roles"),
        "{s}"
    );
    assert!(s.contains("unknown field `ownr`"), "{s}");
}

#[test]
fn unknown_kind_and_version_and_fields_are_refused() {
    let kind = TENANT.replace("kind: KuraTenant", "kind: KuraTenent");
    assert!(
        parse_file("a.yaml", &kind).unwrap_err()[0]
            .to_string()
            .contains("a.yaml#1 kind: unknown kind")
    );
    let version = TENANT.replace("v1alpha1", "v1");
    assert!(
        parse_file("a.yaml", &version).unwrap_err()[0]
            .to_string()
            .contains("apiVersion")
    );
    let field = TENANT.replace("search:", "serch:");
    assert!(
        parse_file("a.yaml", &field).unwrap_err()[0]
            .to_string()
            .contains("unknown field `serch`")
    );
    let role = VAULT.replace("role: editor", "role: editr");
    let e = parse_file("v.yaml", &role).unwrap_err()[0].to_string();
    assert!(
        e.contains("spec.bindings[0].role") && e.contains("unknown variant `editr`"),
        "{e}"
    );
    let size = TENANT.replace("20Gi", "20GB");
    assert!(
        parse_file("a.yaml", &size).unwrap_err()[0]
            .to_string()
            .contains("spec.quota.storage")
    );
    let name = TENANT.replace("name: maxime", "name: Maxime");
    assert!(
        parse_file("a.yaml", &name).unwrap_err()[0]
            .to_string()
            .contains("metadata.name")
    );
    assert!(parse_file("a.yaml", "[1, 2]").is_err());
    assert!(parse_file("a.yaml", "a: [").is_err());
}

#[test]
fn every_error_of_a_file_is_reported() {
    let bad = TENANT.replace("search:", "serch:");
    let e = parse_file("a.yaml", &format!("{bad}---\n{bad}")).unwrap_err();
    assert_eq!(e.len(), 2);
}

#[test]
fn empty_documents_are_skipped() {
    assert_eq!(
        parse_file("a.yaml", &format!("---\n{TENANT}---\n"))
            .unwrap()
            .len(),
        1
    );
}

// Row 2: duplicate names.
#[test]
fn duplicate_tenant_or_vault_is_refused() {
    let e = errors(
        &[("a.yaml", TENANT), ("b.yaml", TENANT)],
        &PreviousState::default(),
    );
    assert!(
        e[0].starts_with("b.yaml#1 metadata.name: tenant `maxime` is already declared at a.yaml#1"),
        "{e:?}"
    );
    let e = errors(
        &[("a.yaml", TENANT), ("v.yaml", VAULT), ("w.yaml", VAULT)],
        &PreviousState::default(),
    );
    assert!(
        e[0].contains("vault `maxime/knowledge` is already declared"),
        "{e:?}"
    );
}

// Row 3: namespace.
#[test]
fn vault_without_namespace_or_outside_vault_namespaces_is_refused() {
    let no_ns = VAULT.replace(", namespace: maxime", "");
    let e = errors(
        &[("a.yaml", TENANT), ("v.yaml", &no_ns)],
        &PreviousState::default(),
    );
    assert!(
        e[0].contains("metadata.namespace: a KuraVault must have a namespace"),
        "{e:?}"
    );
    let other = VAULT.replace("namespace: maxime", "namespace: other");
    let e = errors(
        &[("a.yaml", TENANT), ("v.yaml", &other)],
        &PreviousState::default(),
    );
    assert!(
        e[0].contains("namespace `other` is not in the vaultNamespaces"),
        "{e:?}"
    );
    let ghost = VAULT.replace("tenantRef: { name: maxime }", "tenantRef: { name: ghost }");
    let e = errors(
        &[("a.yaml", TENANT), ("v.yaml", &ghost)],
        &PreviousState::default(),
    );
    assert!(e[0].contains("no tenant `ghost`"), "{e:?}");
}

// Row 4: chunking bounds.
#[test]
fn chunking_must_be_ordered_and_at_most_4_mib() {
    let unordered = VAULT.replace("avg: 1Mi", "avg: 128Ki");
    let e = errors(
        &[("a.yaml", TENANT), ("v.yaml", &unordered)],
        &PreviousState::default(),
    );
    assert!(
        e[0].contains("spec.chunking: expected min < avg < max"),
        "{e:?}"
    );
    let big = VAULT.replace("max: 4Mi", "max: 8Mi");
    let e = errors(
        &[("a.yaml", TENANT), ("v.yaml", &big)],
        &PreviousState::default(),
    );
    assert!(e[0].contains("spec.chunking.max: at most 4Mi"), "{e:?}");
}

#[test]
fn chunking_must_fit_the_algorithms_bounds() {
    for bad in [
        "{ min: 0, avg: 1Mi, max: 4Mi }",
        "{ min: 2Mi, avg: 3Mi, max: 4Mi }",
        "{ min: 101, avg: 1Mi, max: 4Mi }",
    ] {
        let doc = VAULT.replace("{ min: 256Ki, avg: 1Mi, max: 4Mi }", bad);
        let e = errors(
            &[("a.yaml", TENANT), ("v.yaml", &doc)],
            &PreviousState::default(),
        );
        assert!(e.iter().any(|e| e.contains("all even")), "{bad}: {e:?}");
    }
}

// Row 5: chunking changed since creation.
#[test]
fn chunking_cannot_change_after_creation() {
    let mut previous = previous_tenants(&["maxime"]);
    let stored = Chunking {
        max: Quantity::from_bytes(2 << 20),
        ..Chunking::DEFAULT
    };
    previous.vaults.insert(
        key("maxime", "knowledge"),
        StoredVault {
            chunking: stored,
            managed_by: ManagedBy::Config,
        },
    );
    let e = errors(&[("a.yaml", TENANT), ("v.yaml", VAULT)], &previous);
    assert!(
        e[0].contains("differs from the value the vault was created with"),
        "{e:?}"
    );
    // Absent chunking keeps the stored value: accepted.
    let absent = VAULT.replace("  chunking: { min: 256Ki, avg: 1Mi, max: 4Mi }\n", "");
    assert!(validate(parse(&[("a.yaml", TENANT), ("v.yaml", &absent)]), &previous).is_ok());
}

// Row 7: one claim value, two tenants.
#[test]
fn claim_value_mapped_by_two_tenants_is_refused() {
    let e = errors(
        &[
            ("a.yaml", &tenant("a", "shared")),
            ("b.yaml", &tenant("b", "shared")),
        ],
        &PreviousState::default(),
    );
    assert!(
        e[0].contains("claim value `shared` is already mapped by tenant `a`"),
        "{e:?}"
    );
    // The same value under two roles of one tenant is not ambiguous.
    let one = TENANT.replace("admin: []", "admin: [kura-maxime]");
    assert!(validate(parse(&[("a.yaml", &one)]), &PreviousState::default()).is_ok());
    let empty = TENANT.replace("admin: []", "admin: ['']");
    assert!(
        errors(&[("a.yaml", &empty)], &PreviousState::default())[0].contains("cannot be empty")
    );
}

// Row 8: a file vault colliding with an API-managed one.
#[test]
fn file_vault_cannot_take_over_an_api_managed_vault() {
    let mut previous = previous_tenants(&["maxime"]);
    previous.vaults.insert(
        key("maxime", "knowledge"),
        StoredVault {
            chunking: Chunking::DEFAULT,
            managed_by: ManagedBy::Api,
        },
    );
    let e = errors(&[("a.yaml", TENANT), ("v.yaml", VAULT)], &previous);
    assert!(e[0].contains("managed through the API"), "{e:?}");
}

// Row 9: mass removal.
#[test]
fn removing_two_tenants_or_all_of_them_is_refused() {
    let previous = previous_tenants(&["a", "b", "c"]);
    let e = errors(&[("a.yaml", &tenant("a", "x"))], &previous);
    assert_eq!(
        e,
        ["the change removes 2 tenants (b, c); remove them one at a time"]
    );
    let e = validate(Vec::new(), &previous).unwrap_err();
    assert!(e[0].to_string().starts_with("no KuraTenant is declared"));
    // One at a time is accepted.
    assert!(
        validate(
            parse(&[("a.yaml", &tenant("a", "x")), ("b.yaml", &tenant("b", "y"))]),
            &previous
        )
        .is_ok()
    );
}

#[test]
fn binding_names_exactly_one_subject() {
    let both = VAULT.replace(
        "claimValue: kura-maxime",
        "claimValue: kura-maxime, subject: abc",
    );
    assert!(
        errors(
            &[("a.yaml", TENANT), ("v.yaml", &both)],
            &PreviousState::default()
        )[0]
        .contains("spec.bindings[0]: set exactly one")
    );
    let none = VAULT.replace(
        "{ role: editor, claimValue: kura-maxime }",
        "{ role: editor }",
    );
    assert!(
        errors(
            &[("a.yaml", TENANT), ("v.yaml", &none)],
            &PreviousState::default()
        )[0]
        .contains("spec.bindings[0]")
    );
    let subject = VAULT.replace("claimValue: kura-maxime", "subject: 8f1c");
    assert!(
        validate(
            parse(&[("a.yaml", TENANT), ("v.yaml", &subject)]),
            &PreviousState::default()
        )
        .is_ok()
    );
}

#[test]
fn force_orphan_annotation_is_read_under_kura_domain() {
    let doc = TENANT.replace(
        "metadata: { name: maxime }",
        "metadata: { name: maxime, annotations: { kura.weebo.io/force-orphan: \"true\" } }",
    );
    let r = parse_file("a.yaml", &doc).unwrap();
    let Resource::Tenant(_, t) = &r[0] else {
        panic!()
    };
    assert!(t.metadata.force_orphan());
}

#[test]
fn documents_round_trip_through_yaml() {
    for r in parse(&[("a.yaml", TENANT), ("v.yaml", VAULT)]) {
        let (kind, yaml) = match &r {
            Resource::Tenant(_, d) => ("KuraTenant", serde_norway::to_string(d).unwrap()),
            Resource::Vault(_, d) => ("KuraVault", serde_norway::to_string(d).unwrap()),
        };
        let again = format!("apiVersion: kura.weebo.io/v1alpha1\nkind: {kind}\n{yaml}");
        assert_eq!(parse_file(&r.source().file, &again).unwrap(), vec![r]);
    }
}

/// RFC 0005 §4.3: folders on a viewer only, 1 to 16, valid, not repeated.
#[test]
fn binding_paths_are_checked() {
    let with = |role: &str, paths: &str| {
        VAULT.replace(
            "{ role: editor, claimValue: kura-maxime }",
            &format!("{{ role: {role}, claimValue: kura-maxime, paths: {paths} }}"),
        )
    };
    let ok = with("viewer", "[Sessions/, Resumes/]");
    let config = validate(
        parse(&[("a.yaml", TENANT), ("v.yaml", &ok)]),
        &PreviousState::default(),
    )
    .unwrap();
    let b = &config.vaults.values().next().unwrap().doc.spec.bindings[0];
    assert_eq!(
        b.paths.as_deref(),
        Some(&["Sessions/".to_owned(), "Resumes/".to_owned()][..])
    );
    for (role, paths, why) in [
        ("editor", "[Sessions/]", "only a viewer"),
        ("viewer", "[]", "1 to 16"),
        ("viewer", "[Sessions]", "ending with /"),
        ("viewer", "['../x/']", "not a folder"),
        ("viewer", "['/x/']", "not a folder"),
        ("viewer", "[.obsidian/]", "not a folder"),
        ("viewer", "[a/, A/]", "twice"),
    ] {
        let e = errors(
            &[("a.yaml", TENANT), ("v.yaml", &with(role, paths))],
            &PreviousState::default(),
        );
        assert!(
            e[0].contains("spec.bindings[0].paths") && e[0].contains(why),
            "{role} {paths}: {e:?}"
        );
    }
    let many = format!(
        "[{}]",
        (0..17)
            .map(|i| format!("f{i}/"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    assert!(
        errors(
            &[("a.yaml", TENANT), ("v.yaml", &with("viewer", &many))],
            &PreviousState::default()
        )[0]
        .contains("1 to 16")
    );
}

/// RFC 0010 §4.3: shared defaults name well-formed items of a synced
/// category; vault plugins need the plugins category.
#[test]
fn shared_defaults_and_vault_plugins_are_checked() {
    let ok = VAULT.replace(
        "configSync: { categories: [plugins, snippets, themes, hotkeys] }",
        "configSync: { categories: [plugins, snippets], sharedByDefault: [snippets, plugin:dataview], vaultPlugins: true }",
    );
    let c = validate(
        parse(&[("t.yaml", TENANT), ("v.yaml", &ok)]),
        &PreviousState::default(),
    )
    .unwrap();
    let sync = c
        .vaults
        .values()
        .next()
        .unwrap()
        .doc
        .spec
        .config_sync
        .clone()
        .unwrap();
    assert_eq!(sync.shared_by_default, ["snippets", "plugin:dataview"]);
    assert!(sync.vault_plugins);
    for (spec, needle) in [
        (
            "{ categories: [snippets], sharedByDefault: [themes] }",
            "sharedByDefault[0]",
        ),
        (
            "{ categories: [plugins], sharedByDefault: [plugin:weebo-kura] }",
            "sharedByDefault[0]",
        ),
        (
            "{ categories: [plugins], sharedByDefault: [plugin:Bad] }",
            "sharedByDefault[0]",
        ),
        (
            "{ categories: [plugins], sharedByDefault: [plugins] }",
            "sharedByDefault[0]",
        ),
        (
            "{ categories: [snippets], vaultPlugins: true }",
            "vaultPlugins",
        ),
    ] {
        let bad = VAULT.replace("{ categories: [plugins, snippets, themes, hotkeys] }", spec);
        let e = errors(
            &[("t.yaml", TENANT), ("v.yaml", &bad)],
            &PreviousState::default(),
        );
        assert!(e.iter().any(|m| m.contains(needle)), "{spec}: {e:?}");
    }
}
