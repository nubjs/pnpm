use serde_json::json;

use super::{collect_bun_lockfile_versions, strip_trailing_commas};
use crate::VersionsByPackageName;

fn collect(lockfile: &str) -> VersionsByPackageName {
    let value = serde_json::from_str(&strip_trailing_commas(lockfile))
        .expect("the fixture must parse once its trailing commas are stripped");
    let mut versions = VersionsByPackageName::new();
    collect_bun_lockfile_versions(&value, &mut versions);
    versions
}

fn versions_of<'a>(versions: &'a VersionsByPackageName, name: &str) -> Vec<&'a str> {
    versions.get(name).map(|set| set.iter().map(String::as_str).collect()).unwrap_or_default()
}

/// A real `bun.lock`, trailing commas and all. The resolved version comes from
/// the descriptor rather than the key, and a transitive package contributes
/// its version even though nothing declares it.
#[test]
fn a_bun_lockfile_yields_every_resolved_version() {
    let versions = collect(
        r#"{
  "lockfileVersion": 1,
  "workspaces": {
    "": {
      "name": "fixture",
      "dependencies": { "ajv": "6.12.6", "ajv-keywords": "3.5.2", },
    },
  },
  "packages": {
    "ajv": ["ajv@6.12.6", "", { "dependencies": { "uri-js": "^4.2.2" } }, "sha512-j3fVLg=="],
    "ajv-keywords": ["ajv-keywords@3.5.2", "", { "peerDependencies": { "ajv": "^6.9.1" } }, "sha512-5p6WTN=="],
    "uri-js": ["uri-js@4.4.1", "", {}, "sha512-7rKUyy=="],
  }
}"#,
    );
    assert_eq!(versions_of(&versions, "ajv"), ["6.12.6"]);
    assert_eq!(versions_of(&versions, "ajv-keywords"), ["3.5.2"]);
    assert_eq!(versions_of(&versions, "uri-js"), ["4.4.1"], "a transitive package still counts");
}

/// bun keys a nested copy by its path, so two entries can name one package at
/// different versions. Reading the descriptor rather than the key is what
/// keeps both.
#[test]
fn a_nested_duplicate_contributes_its_own_version() {
    let versions = collect(
        r#"{"packages": {
            "debug": ["debug@4.3.4", "", {}, ""],
            "send/debug": ["debug@2.6.9", "", {}, ""]
        }}"#,
    );
    assert_eq!(versions_of(&versions, "debug"), ["2.6.9", "4.3.4"]);
}

/// A scoped name carries its own `@`, so the split has to take the LAST one.
#[test]
fn a_scoped_name_keeps_its_leading_at() {
    let versions = collect(r#"{"packages": {"@scope/pkg": ["@scope/pkg@1.2.3", "", {}, ""]}}"#);
    assert_eq!(versions_of(&versions, "@scope/pkg"), ["1.2.3"]);
}

/// A protocol is not a version. Offering `workspace:*` as a preference would
/// only be discarded downstream, and passing it on as if it were a version
/// invites a resolver to try to match it.
#[test]
fn a_protocol_descriptor_is_not_offered_as_a_version() {
    let versions = collect(
        r#"{
            "workspaces": { "": { "dependencies": { "local": "link:../local" } } },
            "packages": {
                "pkg": ["pkg@workspace:packages/pkg", "", {}, ""],
                "tar": ["tar@https://example.test/tar.tgz", "", {}, ""],
                "real": ["real@1.0.0", "", {}, ""]
            }
        }"#,
    );
    assert!(versions_of(&versions, "pkg").is_empty());
    assert!(versions_of(&versions, "tar").is_empty());
    assert!(versions_of(&versions, "local").is_empty());
    assert_eq!(versions_of(&versions, "real"), ["1.0.0"], "the probe still finds a real version");
}

/// The stripper must not reach inside a string. An integrity hash or a
/// description can hold a comma, a brace, or an escaped quote, and corrupting
/// one would turn a valid lockfile into a parse error.
///
/// Asserted as exact output rather than "still parses", because a stripper
/// that deleted the whole string would also still parse.
#[test]
fn only_a_trailing_comma_is_removed() {
    for (jsonc, expected) in [
        (r#"{"a": [1, 2,],}"#, r#"{"a": [1, 2]}"#),
        // A comma, a brace and an escaped quote inside a string: a naive
        // scanner ends the string at the `\"` and then treats what follows as
        // structural.
        (r#"{"a": "x,y}z \" , w",}"#, r#"{"a": "x,y}z \" , w"}"#),
        // Nothing to strip, and the spacing survives untouched.
        (r#"{"a": [1, 2]}"#, r#"{"a": [1, 2]}"#),
    ] {
        assert_eq!(strip_trailing_commas(jsonc), expected, "input: {jsonc}");
        serde_json::from_str::<serde_json::Value>(&strip_trailing_commas(jsonc))
            .expect("the stripped form must be valid JSON");
    }
    let stripped = strip_trailing_commas(r#"{"note": "a \" b", "n": 1,}"#);
    let parsed: serde_json::Value = serde_json::from_str(&stripped).expect("must parse");
    assert_eq!(parsed, json!({"note": "a \" b", "n": 1}));
}
