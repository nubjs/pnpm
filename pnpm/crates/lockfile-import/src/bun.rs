//! Version extraction from bun's text lockfile.

use serde_json::{Map, Value};

use crate::{VersionsByPackageName, add_version};

/// Collect every version a `bun.lock` pins.
///
/// Entries live under `packages`, keyed by the path the package occupies in
/// bun's tree — a bare name at the root, `parent/child` for a nested copy —
/// and each value is a tuple whose first element is the resolved
/// `name@version`. That element is what this reads, so the key's shape never
/// has to be parsed and a nested duplicate contributes its own version.
///
/// The root workspace's declared ranges are collected too, matching how the
/// npm reader treats a flat entry's `dependencies`: bun writes an exact
/// version there whenever the manifest pinned one.
pub fn collect_bun_lockfile_versions(lockfile: &Value, versions: &mut VersionsByPackageName) {
    if let Some(packages) = lockfile.get("packages").and_then(Value::as_object) {
        collect_from_packages(packages, versions);
    }
    if let Some(workspaces) = lockfile.get("workspaces").and_then(Value::as_object) {
        for workspace in workspaces.values() {
            collect_declared_ranges(workspace, versions);
        }
    }
}

fn collect_from_packages(packages: &Map<String, Value>, versions: &mut VersionsByPackageName) {
    for entry in packages.values() {
        let Some(descriptor) = entry.get(0).and_then(Value::as_str) else {
            continue;
        };
        if let Some((name, version)) = split_descriptor(descriptor) {
            add_version(versions, name, version);
        }
    }
}

/// The declared ranges of one workspace's manifest, from whichever of bun's
/// dependency fields are present.
fn collect_declared_ranges(workspace: &Value, versions: &mut VersionsByPackageName) {
    for field in ["dependencies", "devDependencies", "optionalDependencies", "peerDependencies"] {
        let Some(declared) = workspace.get(field).and_then(Value::as_object) else {
            continue;
        };
        for (name, range) in declared {
            if let Some(range) = range.as_str().filter(|range| is_plain_version(range)) {
                add_version(versions, name, range);
            }
        }
    }
}

/// Split `name@version`, tolerating the leading `@` of a scoped name.
///
/// Anything whose version half names a protocol — `workspace:`, `link:`,
/// `file:`, a git or tarball URL — is dropped rather than offered as a
/// preference, because the resolver would only discard it and a bare `:` is
/// never part of a semver version.
fn split_descriptor(descriptor: &str) -> Option<(&str, &str)> {
    let at = descriptor.rfind('@').filter(|at| *at > 0)?;
    let (name, version) = (&descriptor[..at], &descriptor[at + 1..]);
    is_plain_version(version).then_some((name, version))
}

fn is_plain_version(version: &str) -> bool {
    !version.is_empty() && !version.contains(':')
}

/// Strip the trailing commas `bun.lock` writes before a `}` or `]`.
///
/// The file is JSONC, which `serde_json` rejects, and the workspace carries no
/// JSONC parser. Only trailing commas are handled because they are the only
/// extension bun actually emits; anything else stays a parse error rather than
/// being silently tolerated. The scan tracks string state so a comma inside a
/// value — an integrity hash, a description — is never touched.
#[must_use]
pub fn strip_trailing_commas(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut in_string = false;
    let mut escaped = false;
    let mut pending_comma = false;
    // Whitespace seen since that comma, held back so it can follow the comma
    // if one is emitted and vanish with it if not. Without this the pass also
    // reflows the file, which a function by this name has no business doing.
    let mut pending_space = String::new();

    for character in source.chars() {
        if in_string {
            out.push(character);
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        match character {
            ',' => {
                settle(&mut out, &mut pending_comma, &mut pending_space);
                pending_comma = true;
            }
            '}' | ']' => {
                pending_comma = false;
                pending_space.clear();
                out.push(character);
            }
            _ if character.is_whitespace() => {
                if pending_comma {
                    pending_space.push(character);
                } else {
                    out.push(character);
                }
            }
            _ => {
                settle(&mut out, &mut pending_comma, &mut pending_space);
                if character == '"' {
                    in_string = true;
                    escaped = false;
                }
                out.push(character);
            }
        }
    }
    settle(&mut out, &mut pending_comma, &mut pending_space);
    out
}

/// Emit a comma that turned out not to be trailing, and the whitespace that
/// followed it.
fn settle(out: &mut String, pending_comma: &mut bool, pending_space: &mut String) {
    if std::mem::take(pending_comma) {
        out.push(',');
    }
    out.push_str(pending_space);
    pending_space.clear();
}

#[cfg(test)]
mod tests;
