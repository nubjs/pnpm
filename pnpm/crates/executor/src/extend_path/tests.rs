use super::{ScriptsPrependNodePath, extend_path};
use pretty_assertions::assert_eq;
use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
};

#[cfg(unix)]
const SEP: char = ':';
#[cfg(windows)]
const SEP: char = ';';

fn segments(path: &OsString) -> Vec<String> {
    env::split_paths(path).map(|path| path.to_string_lossy().into_owned()).collect()
}

#[test]
fn node_gyp_comes_after_node_modules_dot_bin() {
    let wd = Path::new("/Users/x/project");
    let node_gyp = PathBuf::from("/lib/node-gyp-bin");
    let extra: Vec<PathBuf> = vec![];
    let path =
        extend_path(wd, None, Some(&node_gyp), &extra, ScriptsPrependNodePath::Never, None, None);
    let parts = segments(&path);
    let bin_idx = parts
        .iter()
        .position(|p| {
            p.ends_with(&format!(
                "project{}node_modules{}.bin",
                std::path::MAIN_SEPARATOR,
                std::path::MAIN_SEPARATOR,
            ))
        })
        .unwrap_or_else(|| panic!("missing node_modules/.bin in {parts:?}"));
    let gyp_idx = parts
        .iter()
        .position(|p| p.contains("node-gyp-bin"))
        .unwrap_or_else(|| panic!("missing node-gyp-bin in {parts:?}"));
    assert!(
        bin_idx < gyp_idx,
        ".bin must precede node-gyp; got bin@{bin_idx}, gyp@{gyp_idx} in {parts:?}",
    );
}

/// When `wd` has no `/node_modules/` segment the split yields a single
/// element and no ancestor `.bin` directories are produced.
#[test]
fn no_ancestors_when_wd_has_no_node_modules_segment() {
    let wd = Path::new("/home/me/project");
    let extra: Vec<PathBuf> = vec![];
    let path = extend_path(wd, None, None, &extra, ScriptsPrependNodePath::Never, None, None);
    let parts = segments(&path);
    assert_eq!(parts.len(), 1, "expected exactly one .bin entry, got {parts:?}");
    assert!(parts[0].ends_with(".bin"), "must be a .bin path: {:?}", parts[0]);
}

/// Unix-only because `path::absolute("/proj")` on Windows resolves
/// against the current drive (`C:\proj`), which makes the hard-coded
/// expected values racy. The structural invariants (count + deepest-
/// first ordering) are covered platform-neutrally in
/// [`virtual_store_walk_orders_deepest_first`] below.
#[cfg(unix)]
#[test]
fn pnpm_virtual_store_layout_yields_three_bins_deepest_first() {
    let wd = Path::new("/proj/node_modules/.pnpm/foo@1.0.0/node_modules/foo");
    let extra: Vec<PathBuf> = vec![];
    let path = extend_path(wd, None, None, &extra, ScriptsPrependNodePath::Never, None, None);
    let parts = segments(&path);
    assert_eq!(
        parts,
        vec![
            "/proj/node_modules/.pnpm/foo@1.0.0/node_modules/foo/node_modules/.bin".to_string(),
            "/proj/node_modules/.pnpm/foo@1.0.0/node_modules/.bin".to_string(),
            "/proj/node_modules/.bin".to_string(),
        ],
    );
}

/// Platform-neutral counterpart to
/// [`pnpm_virtual_store_layout_yields_three_bins_deepest_first`],
/// anchoring to no absolute root.
#[test]
fn virtual_store_walk_orders_deepest_first() {
    let wd = Path::new("proj")
        .join("node_modules")
        .join(".pnpm")
        .join("foo@1.0.0")
        .join("node_modules")
        .join("foo");
    let extra: Vec<PathBuf> = vec![];
    let path = extend_path(&wd, None, None, &extra, ScriptsPrependNodePath::Never, None, None);
    let parts = segments(&path);
    assert_eq!(parts.len(), 3, "expected three bin paths, got {parts:?}");
    for window in parts.windows(2) {
        let deeper = &window[0];
        let shallower = &window[1];
        assert!(deeper.len() > shallower.len(), "{deeper:?} must be deeper than {shallower:?}");
        assert!(deeper.ends_with(".bin") && shallower.ends_with(".bin"));
    }
}

/// Final PATH order is `[bins..., nodeGyp, ...extraBinPaths]`: the
/// `.bin` directories come first, then the bundled node-gyp dir, then
/// the caller-supplied extra paths.
#[test]
fn extra_bin_paths_come_after_bins_and_node_gyp() {
    let wd = Path::new("/proj");
    let node_gyp = PathBuf::from("/bundled/node-gyp-bin");
    let extra: Vec<PathBuf> = vec![PathBuf::from("/extra/one"), PathBuf::from("/extra/two")];
    let path =
        extend_path(wd, None, Some(&node_gyp), &extra, ScriptsPrependNodePath::Never, None, None);
    let parts = segments(&path);
    let bin_idx =
        parts.iter().position(|part| part.contains("proj") && part.ends_with(".bin")).unwrap();
    let gyp_idx = parts.iter().position(|part| part.contains("node-gyp-bin")).unwrap();
    let extra1_idx =
        parts.iter().position(|part| part == "/extra/one" || part == r"\extra\one").unwrap();
    let extra2_idx =
        parts.iter().position(|part| part == "/extra/two" || part == r"\extra\two").unwrap();
    assert!(
        bin_idx < gyp_idx && gyp_idx < extra1_idx && extra1_idx < extra2_idx,
        "expected order .bin < nodeGyp < extra1 < extra2; got {parts:?}",
    );
}

#[test]
fn original_path_is_appended_last() {
    let wd = Path::new("/proj");
    let extra: Vec<PathBuf> = vec![];
    let sys_path = {
        let mut text = OsString::new();
        text.push("/usr/local/bin");
        text.push(SEP.to_string());
        text.push("/usr/bin");
        text
    };
    let path =
        extend_path(wd, Some(&sys_path), None, &extra, ScriptsPrependNodePath::Never, None, None);
    let parts = segments(&path);
    assert_eq!(parts.len(), 3, "1 bin + 2 sys = 3 entries, got {parts:?}");
    assert_eq!(parts[1], "/usr/local/bin");
    assert_eq!(parts[2], "/usr/bin");
}

#[test]
fn scripts_prepend_node_path_always_appends_dirname_of_node() {
    let wd = Path::new("/proj");
    let node = PathBuf::from("/opt/node/bin/node");
    let extra: Vec<PathBuf> = vec![];
    let path =
        extend_path(wd, None, None, &extra, ScriptsPrependNodePath::Always, Some(&node), None);
    let parts = segments(&path);
    assert!(
        parts.iter().any(|part| part == "/opt/node/bin"),
        "expected dirname(node) in PATH, got {parts:?}",
    );
}

/// Regression: a path component containing the platform separator
/// must not cause `extend_path` to drop the computed entries. The
/// plain string join embeds the separator verbatim. Skipping on
/// Windows where `;` is far less likely to appear in real paths, but
/// the invariant holds there too.
#[cfg(unix)]
#[test]
fn separator_in_path_component_does_not_drop_other_entries() {
    // A bin path that itself contains a colon — exotic, but valid
    // on POSIX. `env::join_paths` would reject it; the naive join
    // embeds it verbatim.
    let wd = Path::new("/proj");
    let weird = PathBuf::from("/tmp/a:b/.bin");
    let path = extend_path(
        wd,
        None,
        None,
        std::slice::from_ref(&weird),
        ScriptsPrependNodePath::Never,
        None,
        None,
    );
    let text = path.to_string_lossy();
    assert!(text.contains("/proj/node_modules/.bin"), "wd .bin must survive: {text:?}");
    assert!(text.contains("/tmp/a:b/.bin"), "the weird extra path must survive verbatim: {text:?}");
}

/// The host bin dir occupies the slot a host would otherwise buy by
/// prepending the directory to the process `PATH`: below everything the
/// engine computes, above everything it inherited.
#[test]
fn script_bin_dir_sits_between_the_node_dir_and_the_inherited_path() {
    let wd = Path::new("/proj");
    let node = PathBuf::from("/opt/node/bin/node");
    let shims = PathBuf::from("/tmp/host-shim-4171-9c2a");
    let extra: Vec<PathBuf> = vec![PathBuf::from("/extra/one")];
    let sys_path = {
        let mut text = OsString::new();
        text.push("/usr/local/bin");
        text.push(SEP.to_string());
        text.push("/usr/bin");
        text
    };
    let path = extend_path(
        wd,
        Some(&sys_path),
        None,
        &extra,
        ScriptsPrependNodePath::Always,
        Some(&node),
        Some(&shims),
    );
    let parts = segments(&path);
    let index = |needle: &str| {
        parts
            .iter()
            .position(|part| part.contains(needle))
            .unwrap_or_else(|| panic!("missing {needle} in {parts:?}"))
    };
    let (extra_idx, node_idx, shim_idx, sys_idx) =
        (index("extra"), index("opt"), index("host-shim-4171-9c2a"), index("usr"));
    assert!(
        extra_idx < node_idx && node_idx < shim_idx && shim_idx < sys_idx,
        "expected order extra < dirname(node) < shims < inherited; got {parts:?}",
    );
}

/// The slot is unconditional: a host that sets no `scriptsPrependNodePath`
/// still gets its shims ahead of the inherited `PATH`, and `None` adds
/// nothing at all.
#[test]
fn script_bin_dir_needs_no_other_setting_and_none_adds_nothing() {
    let wd = Path::new("/proj");
    let shims = PathBuf::from("/tmp/host-shim-4171-9c2a");
    let extra: Vec<PathBuf> = vec![];
    let sys_path = OsString::from("/usr/bin");

    let with = extend_path(
        wd,
        Some(&sys_path),
        None,
        &extra,
        ScriptsPrependNodePath::Never,
        None,
        Some(&shims),
    );
    assert_eq!(
        segments(&with),
        vec![
            Path::new("/proj").join("node_modules").join(".bin").to_string_lossy().into_owned(),
            shims.to_string_lossy().into_owned(),
            "/usr/bin".to_string(),
        ],
    );

    let without =
        extend_path(wd, Some(&sys_path), None, &extra, ScriptsPrependNodePath::Never, None, None);
    let parts = segments(&without);
    assert_eq!(parts.len(), 2, "no host dir means bin + inherited only, got {parts:?}");
    assert_eq!(parts[1], "/usr/bin");
}

/// `WarnOnly` would emit a warning; that reporter-side emission is
/// decoupled from `extend_path`, so this function just skips the
/// prepend like `Never`.
#[test]
fn scripts_prepend_node_path_never_and_warn_only_do_not_prepend() {
    let wd = Path::new("/proj");
    let node = PathBuf::from("/opt/node/bin/node");
    let extra: Vec<PathBuf> = vec![];
    for variant in [ScriptsPrependNodePath::Never, ScriptsPrependNodePath::WarnOnly] {
        let path = extend_path(wd, None, None, &extra, variant, Some(&node), None);
        let parts = segments(&path);
        assert!(
            !parts.iter().any(|part| part == "/opt/node/bin"),
            "variant {variant:?} must not prepend dirname(node), got {parts:?}",
        );
    }
}
