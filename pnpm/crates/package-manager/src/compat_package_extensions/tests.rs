use super::{COMPAT_PACKAGE_EXTENSIONS, compat_package_extender};
use pnpm_config::{Config, Embedder, PackageExtension};
use serde_json::json;

/// Rules a host keeps for the rest of its run.
fn host_rules(rules: serde_json::Value) -> &'static indexmap::IndexMap<String, PackageExtension> {
    Box::leak(Box::new(serde_json::from_value(rules).expect("parse the host rules")))
}

/// A host's rules repair a package the built-in database does not name, and
/// only under a config that carries them.
#[test]
fn host_rules_extend_the_built_in_database() {
    let manifest = json!({ "name": "host-only-fixture", "version": "1.0.0" });
    let built_in = Config::default();
    assert!(!compat_package_extender(&built_in).expect("on by default").matches(&manifest));

    let rules =
        host_rules(json!({ "host-only-fixture@*": { "dependencies": { "left-pad": "^1.0.0" } } }));
    let embedder = Embedder { compat_package_extensions: Some(rules), ..Embedder::PNPM };
    let config = Config { embedder, ..Config::default() };
    let mut extended = manifest;
    compat_package_extender(&config).expect("on by default").apply(&mut extended);
    assert_eq!(extended.pointer("/dependencies/left-pad"), Some(&json!("^1.0.0")));
}

/// Where a host rule sets a field a built-in rule already sets, the built-in
/// value stands, and `ignoreCompatibilityDb` declines the host's layer too.
#[test]
fn built_in_rules_win_and_the_opt_out_declines_host_rules_too() {
    let rules =
        host_rules(json!({ "@angular/build@*": { "dependencies": { "tslib": "^9.9.9" } } }));
    let embedder = Embedder { compat_package_extensions: Some(rules), ..Embedder::PNPM };
    let config = Config { embedder, ..Config::default() };
    let mut manifest = json!({ "name": "@angular/build", "version": "20.0.0" });
    compat_package_extender(&config).expect("on by default").apply(&mut manifest);
    assert_eq!(manifest.pointer("/dependencies/tslib"), Some(&json!("^2.3.0")));

    let declined = Config { embedder, ignore_compatibility_db: true, ..Config::default() };
    assert!(compat_package_extender(&declined).is_none());
}

#[test]
fn includes_pnpm_specific_compat_entries() {
    let angular_build = COMPAT_PACKAGE_EXTENSIONS
        .get("@angular/build@*")
        .expect("@angular/build compat entry present");
    assert_eq!(
        angular_build.dependencies.as_ref().and_then(|deps| deps.get("tslib")),
        Some(&"^2.3.0".to_string()),
    );
    let legacy_nuxt_vite_builder = COMPAT_PACKAGE_EXTENSIONS
        .get("@nuxt/vite-builder@>=4.0.0 <4.5.0")
        .expect("legacy @nuxt/vite-builder compat entry present");
    assert_eq!(
        legacy_nuxt_vite_builder.dependencies.as_ref().and_then(|deps| deps.get("unplugin")),
        Some(&"^2.3.5".to_string()),
    );
    let nuxt_vite_builder = COMPAT_PACKAGE_EXTENSIONS
        .get("@nuxt/vite-builder@>=4.5.0")
        .expect("@nuxt/vite-builder compat entry present");
    assert_eq!(
        nuxt_vite_builder.dependencies.as_ref().and_then(|deps| deps.get("unplugin")),
        Some(&"^3.3.0".to_string()),
    );
}

/// Compat entries must not inject `estree` — no such npm package exists,
/// the import that names it is type-only and satisfied by `@types/estree` —
/// nor a single-instance runtime like `typescript`, `react`, or `eslint`,
/// where a second copy in the graph breaks the tools that load it.
#[test]
fn compat_entries_never_inject_type_only_or_singleton_packages() {
    for target in ["estree", "typescript", "react", "eslint"] {
        for (selector, extension) in COMPAT_PACKAGE_EXTENSIONS.iter() {
            assert!(
                extension.dependencies.as_ref().is_none_or(|deps| !deps.contains_key(target)),
                "{selector} must not inject a {target} dependency",
            );
        }
    }
}
