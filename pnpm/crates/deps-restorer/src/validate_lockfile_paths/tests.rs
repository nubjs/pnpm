use super::validate_virtual_store_slot_containment;
use crate::VirtualStoreLayout;
use miette::Diagnostic;
use pnpm_lockfile::{PackageKey, SnapshotEntry};
use std::{collections::HashMap, path::PathBuf};

fn assert_invalid_dependency_name_code(err: &pnpm_lockfile_verification::VerifyError) {
    let code = err.code().map(|code| code.to_string());
    assert_eq!(code.as_deref(), Some("ERR_PNPM_INVALID_DEPENDENCY_NAME"));
}

#[test]
fn accepts_snapshots_whose_slots_stay_in_the_store() {
    let layout = VirtualStoreLayout::legacy(
        PathBuf::from("/project/node_modules/.pnpm"),
        pnpm_config::default_virtual_store_dir_max_length() as usize,
    );
    let mut snapshots = HashMap::new();
    snapshots.insert("@scope/foo@1.2.3".parse::<PackageKey>().unwrap(), SnapshotEntry::default());
    snapshots.insert("bar@4.5.6".parse::<PackageKey>().unwrap(), SnapshotEntry::default());
    validate_virtual_store_slot_containment(Some(&snapshots), &layout)
        .expect("contained slots must pass");
}

#[test]
fn rejects_a_global_virtual_store_version_escape() {
    // Under the global virtual store the slot path inserts the version
    // segment as a raw path component (unlike the legacy flat name, which
    // escapes `/`). A traversal-bearing version escapes the store root
    // even though the package name itself is valid, so the containment
    // check — not the name check — is what rejects it.
    let key: PackageKey = "evil@../../../escaped".parse().expect("parse escaping version key");

    let mut config = pnpm_config::Config::new();
    config.enable_global_virtual_store = true;
    config.global_virtual_store_dir = PathBuf::from("/store/links");

    let mut snapshots = HashMap::new();
    snapshots.insert(key.clone(), SnapshotEntry::default());
    let layout = VirtualStoreLayout::new(&config, None, Some(&snapshots), None, None, None, None);
    assert!(
        !pnpm_fs::is_subdir(layout.package_store_dir(), &layout.slot_dir(&key)),
        "the crafted slot must actually escape the store for this test to be meaningful",
    );

    let err = validate_virtual_store_slot_containment(Some(&snapshots), &layout)
        .expect_err("a slot that escapes the store root must be rejected");
    assert_invalid_dependency_name_code(&err);
    assert!(err.to_string().contains("evil@"), "offender listed: {err}");
}

/// Keeps the one package it names out of the shared store.
#[derive(Debug)]
struct KeepsInProject(&'static str);

impl pnpm_store_dir::MaterializePolicy for KeepsInProject {
    fn materialize_locally(
        &self,
        resolved: &[pnpm_store_dir::ResolvedPackage<'_>],
    ) -> std::collections::HashSet<String> {
        resolved
            .iter()
            .filter(|package| package.id == self.0)
            .map(|package| package.id.to_owned())
            .collect()
    }
}

#[test]
fn accepts_a_slot_the_materialize_policy_keeps_in_the_project() {
    // A package the host's policy keeps out of the shared store is placed in
    // the project's own virtual store, which is not under the global one.
    // Checking every slot against the global root rejected each such package
    // as an escape as soon as an install ran from the lockfile.
    let key: PackageKey = "kept@1.0.0".parse().expect("parse kept key");

    let mut config = pnpm_config::Config::new();
    config.enable_global_virtual_store = true;
    config.global_virtual_store_dir = PathBuf::from("/store/links");
    config.virtual_store_dir = PathBuf::from("/project/node_modules/.pnpm");
    config.materialize_policy = Some(std::sync::Arc::new(KeepsInProject("kept@1.0.0")));

    let mut snapshots = HashMap::new();
    snapshots.insert(key.clone(), SnapshotEntry::default());
    snapshots.insert("shared@1.0.0".parse::<PackageKey>().unwrap(), SnapshotEntry::default());
    let layout = VirtualStoreLayout::new(&config, None, Some(&snapshots), None, None, None, None);
    assert!(
        !pnpm_fs::is_subdir(layout.package_store_dir(), &layout.slot_dir(&key)),
        "the kept package must sit outside the shared store for this test to be meaningful",
    );

    validate_virtual_store_slot_containment(Some(&snapshots), &layout)
        .expect("a slot in the project's own virtual store is contained");
}
