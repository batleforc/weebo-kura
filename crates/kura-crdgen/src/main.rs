//! Writes `deploy/crd/<kind, lowercase>.yaml`, one file per CRD, from the
//! `kura-api` specs (`CustomResourceExt::crd()`), and `deploy/crd/links.json`,
//! the reference fields of each kind (`weebo_kit_api::links`) with the kit's
//! schematic kinds, for the docs' CRD reference and IDE. The kit's operator
//! template, for the operator a later RFC adds here. Run by `task recu`; the
//! result is committed.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;
use kube::CustomResourceExt;
use kura_api::{KuraTenantResource, KuraVaultResource};
use schemars::JsonSchema;
use serde_json::Value;
use weebo_kit_api::links::{self, GroupKind, KindLinks, Roles, Schematic};

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "deploy/crd")]
    out: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    fs::create_dir_all(&args.out)?;
    // Kura has no instance kinds: no field of its CRDs names a role, and the
    // kit's schematic kinds need one, so the tenant stands for both.
    let tenant = GroupKind::new("kura.weebo.io", "KuraTenant");
    let roles = Roles {
        instance: tenant.clone(),
        local_instance: tenant,
    };
    let mut kinds = BTreeMap::new();
    write::<KuraTenantResource>(&args.out, &roles, &mut kinds)?;
    write::<KuraVaultResource>(&args.out, &roles, &mut kinds)?;
    let schematic: BTreeMap<String, Schematic> = weebo_kit_api::schematic::shared(&roles)?;
    let links = serde_json::json!({ "version": 1, "kinds": kinds, "schematic": schematic });
    fs::write(
        args.out.join("links.json"),
        serde_json::to_string_pretty(&links)? + "\n",
    )?;
    println!("wrote CRDs and links.json to {}", args.out.display());
    Ok(())
}

fn write<K: CustomResourceExt + JsonSchema>(
    out: &Path,
    roles: &Roles,
    kinds: &mut BTreeMap<String, KindLinks>,
) -> anyhow::Result<()> {
    let crd = K::crd();
    let kind = crd.spec.names.kind.clone();
    let mut doc = serde_json::to_value(&crd)?;
    open_objects(&mut doc);
    fs::write(
        out.join(format!("{}.yaml", kind.to_lowercase())),
        serde_norway::to_string(&doc)?,
    )?;
    kinds.insert(kind, links::of::<K>(roles)?);
    Ok(())
}

/// Drops `additionalProperties: false`, which the specs' `deny_unknown_fields`
/// gives and apiextensions/v1 refuses: the apiserver prunes unknown fields
/// instead, and the file loader still refuses them.
fn open_objects(v: &mut Value) {
    match v {
        Value::Object(map) => {
            if map.get("additionalProperties") == Some(&Value::Bool(false)) {
                map.remove("additionalProperties");
            }
            map.values_mut().for_each(open_objects);
        }
        Value::Array(items) => items.iter_mut().for_each(open_objects),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The committed CRDs are what the specs give: `task recu` was run.
    #[test]
    fn committed_crds_match_the_specs() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../deploy/crd");
        for (crd, file) in [
            (KuraTenantResource::crd(), "kuratenant.yaml"),
            (KuraVaultResource::crd(), "kuravault.yaml"),
        ] {
            let mut want = serde_json::to_value(&crd).unwrap();
            open_objects(&mut want);
            let text = fs::read_to_string(dir.join(file)).unwrap();
            let got: Value = serde_norway::from_str(&text).unwrap();
            assert_eq!(got, want, "{file} is stale: run `task recu`");
        }
    }

    #[test]
    fn no_closed_object_reaches_a_crd() {
        let mut v = serde_json::json!({ "a": { "additionalProperties": false, "b": [{ "additionalProperties": false }] }, "additionalProperties": true });
        open_objects(&mut v);
        assert_eq!(
            v,
            serde_json::json!({ "a": { "b": [{}] }, "additionalProperties": true })
        );
    }
}
