use super::render_ignored_builds;
use pnpm_config::{Config, Embedder};
use std::{fs, path::Path};
use tempfile::tempdir;

const HINTS: &str = "\nhint: To allow the execution of build scripts for a package, add its name to \"allowBuilds\" and set to \"true\", then run \"pnpm rebuild\".\nhint: For example:\nhint: allowBuilds:\nhint:   esbuild: true\nhint: If you don't want to build a package, set it to \"false\" instead.";

/// Build a `Config` whose `modules_dir` is a `node_modules` under `dir`
/// and whose `allow_builds` records each `disallowed` name as `false`.
/// When `ignored_builds` is `Some`, a `.modules.yaml` recording those
/// depPaths is written; `None` leaves `node_modules` absent (the
/// "cannot identify" case).
fn config_with(dir: &Path, ignored_builds: Option<&[&str]>, disallowed: &[&str]) -> Config {
    let modules_dir = dir.join("node_modules");
    if let Some(ignored) = ignored_builds {
        fs::create_dir_all(&modules_dir).expect("create node_modules");
        let manifest = serde_json::json!({
            "layoutVersion": 5,
            "packageManager": "pacquet@test",
            "ignoredBuilds": ignored,
            "storeDir": "/store",
            "virtualStoreDir": ".pnpm",
        });
        fs::write(modules_dir.join(".modules.yaml"), manifest.to_string())
            .expect("write .modules.yaml");
    }
    let mut config = Config::new();
    config.modules_dir = modules_dir;
    for name in disallowed {
        config.allow_builds.insert((*name).to_string(), false);
    }
    config
}

// Ports pnpm's `ignoredBuilds lists automatically ignored dependencies`.
#[test]
fn lists_automatically_ignored_dependencies() {
    let dir = tempdir().unwrap();
    let config = config_with(dir.path(), Some(&["foo@1.0.0"]), &[]);
    let output = render_ignored_builds(&config).unwrap();
    assert_eq!(
        output,
        format!("Automatically ignored builds during installation:\n  foo{HINTS}\n"),
    );
}

// Ports pnpm's `ignoredBuilds lists explicitly ignored dependencies`.
#[test]
fn lists_explicitly_ignored_dependencies() {
    let dir = tempdir().unwrap();
    let config = config_with(dir.path(), Some(&[]), &["bar"]);
    let output = render_ignored_builds(&config).unwrap();
    assert_eq!(
        output,
        "Automatically ignored builds during installation:\n  None\n\nExplicitly ignored package builds (via allowBuilds):\n  bar\n",
    );
}

// Ports pnpm's `ignoredBuilds lists both automatically and explicitly
// ignored dependencies`.
#[test]
fn lists_both_automatically_and_explicitly_ignored() {
    let dir = tempdir().unwrap();
    let config = config_with(dir.path(), Some(&["foo@1.0.0", "bar@1.0.0"]), &["qar", "zoo"]);
    let output = render_ignored_builds(&config).unwrap();
    assert_eq!(
        output,
        format!(
            "Automatically ignored builds during installation:\n  foo\n  bar{HINTS}\n\nExplicitly ignored package builds (via allowBuilds):\n  qar\n  zoo\n",
        ),
    );
}

// Ports pnpm's `ignoredBuilds prints an info message when there is no
// node_modules`.
#[test]
fn reports_cannot_identify_when_no_node_modules() {
    let dir = tempdir().unwrap();
    let config = config_with(dir.path(), None, &["qar", "zoo"]);
    let output = render_ignored_builds(&config).unwrap();
    assert_eq!(
        output,
        "Automatically ignored builds during installation:\n  Cannot identify as no node_modules found\n\nExplicitly ignored package builds (via allowBuilds):\n  qar\n  zoo\n",
    );
}

/// A host reads its approvals back under a name of its own and runs
/// `rebuild` itself, so both halves of the advice move with the profile. The
/// tests above pin pnpm's own wording through the default profile.
#[test]
fn a_hosts_advice_names_its_own_allow_list_and_program() {
    let dir = tempdir().unwrap();
    let mut config = config_with(dir.path(), Some(&["foo@1.0.0"]), &["bar"]);
    config.embedder = Embedder {
        program_name: "nub",
        allow_builds_display_name: "allowScripts",
        ..Embedder::PNPM
    };
    let output = render_ignored_builds(&config).unwrap();
    assert_eq!(
        output,
        "Automatically ignored builds during installation:\n  foo\nhint: To allow the execution of build scripts for a package, add its name to \"allowScripts\" and set to \"true\", then run \"nub rebuild\".\nhint: For example:\nhint: allowScripts:\nhint:   esbuild: true\nhint: If you don't want to build a package, set it to \"false\" instead.\n\nExplicitly ignored package builds (via allowScripts):\n  bar\n",
    );
}
