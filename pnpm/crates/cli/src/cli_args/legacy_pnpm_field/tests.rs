use super::{ignored_field_warning, ignored_pnpm_field_keys};
use crate::cli_args::package_manager::read_root_manifest_json;
use pnpm_config::Embedder;
use std::{fs, path::Path};

fn write_manifest(dir: &Path, contents: &str) {
    fs::write(dir.join("package.json"), contents).expect("write package.json");
}

fn keys_in(dir: &Path) -> Vec<String> {
    ignored_pnpm_field_keys(read_root_manifest_json(dir).as_ref())
}

#[test]
fn reports_migrated_keys_in_declaration_order() {
    let dir = tempfile::tempdir().expect("create temp dir");
    write_manifest(
        dir.path(),
        r#"{"pnpm":{"onlyBuiltDependencies":["a"],"app":{},"overrides":{"x":"1"}}}"#,
    );
    assert_eq!(
        keys_in(dir.path()),
        vec!["onlyBuiltDependencies".to_string(), "overrides".to_string()],
    );
}

#[test]
fn ignores_manifests_without_a_migrated_key() {
    let dir = tempfile::tempdir().expect("create temp dir");
    write_manifest(dir.path(), r#"{"pnpm":{"app":{}},"name":"x"}"#);
    assert!(keys_in(dir.path()).is_empty());
}

/// An editor-written manifest may open with a UTF-8 BOM, which every
/// other manifest read in the CLI strips. The warning has to see the
/// same keys those reads do.
#[test]
fn reports_migrated_keys_through_a_utf8_bom() {
    let dir = tempfile::tempdir().expect("create temp dir");
    write_manifest(dir.path(), "\u{feff}{\"pnpm\":{\"overrides\":{\"x\":\"1\"}}}");
    assert_eq!(keys_in(dir.path()), vec!["overrides".to_string()]);
}

/// A non-object `pnpm` field belongs to some other tool; there is
/// nothing to warn about, and a malformed or missing manifest is
/// reported by the install path with far better context.
#[test]
fn tolerates_absent_malformed_and_non_object_manifests() {
    let dir = tempfile::tempdir().expect("create temp dir");
    assert!(keys_in(dir.path()).is_empty(), "no manifest");

    write_manifest(dir.path(), "{ not json");
    assert!(keys_in(dir.path()).is_empty(), "malformed manifest");

    write_manifest(dir.path(), r#"{"pnpm":"11.0.0"}"#);
    assert!(keys_in(dir.path()).is_empty(), "non-object pnpm field");
}

/// pnpm's warning keeps its wording. A host's names the host, and leaves out
/// the pointer to pnpm's settings page when the host reads none of pnpm's
/// configuration — that page sends the reader to `pnpm-workspace.yaml`.
#[test]
fn the_warning_names_the_program_that_ignored_the_keys() {
    let ignored = ["overrides".to_string(), "allowBuilds".to_string()];
    assert_eq!(
        ignored_field_warning(&ignored, Embedder::PNPM),
        r#"The "pnpm" field in package.json is no longer read by pnpm. The following keys were ignored: "pnpm.overrides", "pnpm.allowBuilds". See https://pnpm.io/settings for the new home of each setting."#,
    );
    let host = Embedder { program_name: "host", reads_pnpm_config: false, ..Embedder::PNPM };
    assert_eq!(
        ignored_field_warning(&ignored, host),
        r#"The "pnpm" field in package.json is not read by host. The following keys were ignored: "pnpm.overrides", "pnpm.allowBuilds"."#,
    );
}
