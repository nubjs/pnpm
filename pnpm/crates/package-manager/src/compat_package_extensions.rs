// `compat_package_extensions.json` is a copy of the compatibility database in
// `@yarnpkg/extensions`, which is distributed under the following license:
//
//     BSD 2-Clause License
//
//     Copyright (c) 2016-present, Yarn Contributors.
//     All rights reserved.
//
//     Redistribution and use in source and binary forms, with or without
//     modification, are permitted provided that the following conditions are met:
//
//     1. Redistributions of source code must retain the above copyright notice, this
//        list of conditions and the following disclaimer.
//
//     2. Redistributions in binary form must reproduce the above copyright notice,
//        this list of conditions and the following disclaimer in the documentation
//        and/or other materials provided with the distribution.
//
//     THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
//     AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//     IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
//     DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
//     FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
//     DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
//     SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
//     CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
//     OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
//     OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use crate::PackageExtender;
use indexmap::IndexMap;
use pnpm_config::{Config, PackageExtension, PeerDependencyMeta};
use std::{
    collections::BTreeMap,
    sync::{LazyLock, Mutex, PoisonError},
};

// `pnpm_compat_package_extensions.json` holds pnpm-specific entries not in
// `@yarnpkg/extensions`. It must stay identical to the TypeScript CLI's
// `pnpmCompatPackageExtensions`.
static COMPAT_PACKAGE_EXTENSIONS: LazyLock<IndexMap<String, PackageExtension>> =
    LazyLock::new(|| {
        let mut extensions = IndexMap::new();
        for (source, name) in [
            (include_str!("compat_package_extensions.json"), "@yarnpkg/extensions"),
            (include_str!("pnpm_compat_package_extensions.json"), "pnpm"),
        ] {
            let entries: Vec<(String, PackageExtension)> = serde_json::from_str(source)
                .unwrap_or_else(|error| {
                    panic!("failed to parse {name} compatibility DB JSON: {error}")
                });
            for (selector, extension) in entries {
                merge_package_extension_entry(&mut extensions, selector, extension);
            }
        }
        extensions
    });

static COMPAT_PACKAGE_EXTENDER: LazyLock<PackageExtender> = LazyLock::new(|| {
    PackageExtender::new(&COMPAT_PACKAGE_EXTENSIONS)
        .expect("@yarnpkg/extensions compatibility DB selectors are valid")
});

/// The compatibility extender an install applies: the built-in database, with
/// any rules the embedding host adds merged beneath it. `None` when
/// `ignoreCompatibilityDb` declines the whole thing.
pub(crate) fn compat_package_extender(config: &Config) -> Option<&'static PackageExtender> {
    if config.ignore_compatibility_db {
        return None;
    }
    Some(match config.embedder.compat_package_extensions {
        None => &COMPAT_PACKAGE_EXTENDER,
        Some(host) => extender_with_host_rules(host),
    })
}

/// The built-in database with `host`'s rules merged beneath it, built once per
/// host database. A host's rules are `'static`, so the same address always
/// names the same rules and the built extender can be kept for the process.
fn extender_with_host_rules(
    host: &'static IndexMap<String, PackageExtension>,
) -> &'static PackageExtender {
    static BUILT: Mutex<Vec<(usize, &'static PackageExtender)>> = Mutex::new(Vec::new());
    let address = std::ptr::from_ref(host) as usize;
    let mut built = BUILT.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((_, extender)) = built.iter().find(|(key, _)| *key == address) {
        return extender;
    }
    let mut extensions = COMPAT_PACKAGE_EXTENSIONS.clone();
    for (selector, extension) in host {
        merge_package_extension_entry(&mut extensions, selector.clone(), extension.clone());
    }
    let extender: &'static PackageExtender = match PackageExtender::new(&extensions) {
        Ok(extender) => Box::leak(Box::new(extender)),
        Err(error) => {
            tracing::warn!(%error, "a host compatibility rule has an invalid selector; using the built-in database alone");
            &COMPAT_PACKAGE_EXTENDER
        }
    };
    built.push((address, extender));
    extender
}

fn merge_package_extension_entry(
    extensions: &mut IndexMap<String, PackageExtension>,
    selector: String,
    extension: PackageExtension,
) {
    match extensions.get_mut(&selector) {
        Some(previous) => merge_package_extension(previous, &extension),
        None => {
            extensions.insert(selector, extension);
        }
    }
}

fn merge_package_extension(previous: &mut PackageExtension, next: &PackageExtension) {
    merge_string_map(&mut previous.dependencies, next.dependencies.as_ref());
    merge_string_map(&mut previous.optional_dependencies, next.optional_dependencies.as_ref());
    merge_string_map(&mut previous.peer_dependencies, next.peer_dependencies.as_ref());
    merge_peer_meta_map(&mut previous.peer_dependencies_meta, next.peer_dependencies_meta.as_ref());
}

fn merge_string_map(
    previous: &mut Option<BTreeMap<String, String>>,
    next: Option<&BTreeMap<String, String>>,
) {
    let Some(next) = next else { return };
    let mut merged = next.clone();
    if let Some(previous) = previous.take() {
        merged.extend(previous);
    }
    *previous = Some(merged);
}

fn merge_peer_meta_map(
    previous: &mut Option<BTreeMap<String, PeerDependencyMeta>>,
    next: Option<&BTreeMap<String, PeerDependencyMeta>>,
) {
    let Some(next) = next else { return };
    let mut merged = next.clone();
    if let Some(previous) = previous.take() {
        merged.extend(previous);
    }
    *previous = Some(merged);
}

#[cfg(test)]
mod tests;
