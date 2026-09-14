use pnpm_reporter::SilentReporter;

use super::{
    ApprovalDecision, ApproveBuildsArgs, ApproveBuildsError, all_denied_notice, partition_params,
    sort_unique, write_approval_settings,
};

fn pending(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_string()).collect()
}

fn params(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_string()).collect()
}

fn args(packages: &[&str]) -> ApproveBuildsArgs {
    ApproveBuildsArgs { packages: params(packages), all: false, global: false }
}

fn approve_builds_error(report: miette::Report) -> ApproveBuildsError {
    report.downcast::<ApproveBuildsError>().expect("an approve-builds error")
}

#[test]
fn splits_approved_and_denied() {
    let partition = partition_params(&params(&["foo", "!bar"]), &pending(&["foo", "bar"]));
    assert_eq!(partition.approved, vec!["foo".to_string()]);
    assert_eq!(partition.denied, vec!["bar".to_string()]);
    assert!(partition.unknown.is_empty());
}

#[test]
fn reports_an_unknown_approved_package_as_pre_emptive() {
    let partition = partition_params(&params(&["nope"]), &pending(&["foo"]));
    assert_eq!(partition.approved, vec!["nope".to_string()]);
    assert!(partition.denied.is_empty());
    assert_eq!(partition.unknown, vec!["nope".to_string()]);
}

#[test]
fn reports_an_unknown_denied_package_as_pre_emptive() {
    let partition = partition_params(&params(&["!nope"]), &pending(&["foo"]));
    assert!(partition.approved.is_empty());
    assert_eq!(partition.denied, vec!["nope".to_string()]);
    assert_eq!(partition.unknown, vec!["nope".to_string()]);
}

// Ports pnpm's `contradictory arguments throw error`.
#[test]
fn rejects_contradictory_arguments() {
    let err = args(&["foo", "!foo"])
        .decide::<SilentReporter>(&pending(&["foo"]), "allowBuilds")
        .err()
        .expect("contradicting arguments are rejected");
    let ApproveBuildsError::ContradictingArgs(names) = approve_builds_error(err) else {
        panic!("expected ContradictingArgs");
    };
    assert_eq!(names, vec!["foo".to_string()]);
}

/// Denying every build interactively reports the allow-list under the name
/// the running program reads it back by. pnpm's wording is its own profile's.
#[test]
fn the_all_denied_notice_names_the_profiles_allow_list() {
    assert_eq!(
        all_denied_notice(pnpm_config::Embedder::PNPM.allow_builds_display_name),
        "All packages were added to allowBuilds with value false.",
    );
    assert_eq!(
        all_denied_notice("allowScripts"),
        "All packages were added to allowScripts with value false.",
    );
}

#[test]
fn rejects_an_argument_that_names_no_package() {
    for packages in [&["!"][..], &[""][..], &["foo", "!"][..]] {
        let err = args(packages).validate().unwrap_err();
        assert!(
            matches!(approve_builds_error(err), ApproveBuildsError::MissingPackage),
            "expected MissingPackage for {packages:?}",
        );
    }
}

#[test]
fn rejects_positional_arguments_with_all() {
    let err = ApproveBuildsArgs { packages: params(&["foo"]), all: true, global: false }
        .validate()
        .unwrap_err();
    assert!(matches!(approve_builds_error(err), ApproveBuildsError::AllWithArgs));
}

#[test]
fn sort_unique_dedupes_and_sorts() {
    assert_eq!(sort_unique(params(&["b", "a", "b"])), vec!["a".to_string(), "b".to_string()]);
}

/// A host that reads no workspace manifest records the decision itself, and
/// the engine writes no `pnpm-workspace.yaml` behind its back — a file the
/// host would never read, and which may mean something else to it entirely.
#[test]
fn a_host_writer_takes_the_approval_instead_of_the_workspace_manifest() {
    fn record(dir: &std::path::Path, entries: &[(&str, bool)]) -> std::io::Result<()> {
        let body: Vec<String> =
            entries.iter().map(|(pkg, allowed)| format!("{pkg}={allowed}")).collect();
        std::fs::write(dir.join("host-approvals"), body.join("\n"))
    }

    let dir = tempfile::tempdir().expect("a temp dir");
    let embedder =
        pnpm_config::Embedder { allow_builds_writer: Some(record), ..pnpm_config::Embedder::PNPM };
    let decision = ApprovalDecision {
        decisions: [("esbuild".to_string(), true), ("sharp".to_string(), false)].into(),
        build_packages: vec!["esbuild".to_string()],
        clear_all: false,
    };

    write_approval_settings(dir.path(), &decision, embedder).expect("the host writer runs");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("host-approvals")).expect("the host's own file"),
        "esbuild=true\nsharp=false"
    );
    assert!(
        !dir.path().join("pnpm-workspace.yaml").exists(),
        "the engine must not write its own manifest for a host that supplied a writer"
    );

    // pnpm itself is the control: with no writer, the manifest is where the
    // decision goes.
    write_approval_settings(dir.path(), &decision, pnpm_config::Embedder::PNPM)
        .expect("pnpm's own path runs");
    let manifest = std::fs::read_to_string(dir.path().join("pnpm-workspace.yaml"))
        .expect("pnpm writes its workspace manifest");
    assert!(manifest.contains("esbuild: true"), "{manifest}");
}
