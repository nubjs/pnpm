//! Notification of a package's arrival in the content-addressed store.
//!
//! pacquet is consumed as a library by hosts other than the `pnpm` binary,
//! and a host may need to inspect a package's contents. The end of
//! extraction is the one point where every file of a package is known and
//! already written: the contents are on disk, addressed by hash, and the
//! host learns of them once rather than re-walking the store afterwards.
//!
//! pnpm itself registers no observer, so the notification is a `None` check
//! per extracted package.
//!
//! It lives beside the store rather than with the tarball reader because the
//! fetcher that fires it and the config that carries it both already depend
//! on the store, and neither depends on the other.

use crate::store_index::PackageFilesIndex;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};

/// What one extraction wrote into the store.
pub struct ExtractedPackage<'a> {
    /// Each file's path inside the package, mapped to the
    /// content-addressed file the extraction wrote it to.
    pub cas_paths: &'a HashMap<String, PathBuf>,

    /// The store-index row for the package: the per-file hashes and sizes,
    /// and the bundled manifest that names it.
    pub files: &'a PackageFilesIndex,
}

/// Notified once per package extracted into the store.
///
/// Called on the extracting thread, after the files are written and before
/// the store-index row is queued, so an implementation should be cheap or
/// hand the work off itself. It reports nothing back: extraction has
/// already succeeded by this point, and an observer must not be able to
/// fail an install.
///
/// A cache hit extracts nothing and so notifies nothing. An observer that
/// has to account for every installed package needs its own read path for
/// packages already in the store.
pub trait ExtractObserver: std::fmt::Debug + Send + Sync {
    fn package_extracted(&self, extracted: ExtractedPackage<'_>);
}

/// An observer an install carries, if its host registered one.
pub type SharedExtractObserver = Option<Arc<dyn ExtractObserver>>;

/// One package an install resolved, as a policy sees it.
pub struct ResolvedPackage<'a> {
    /// The install's `"{name}@{version}"` identifier.
    pub id: &'a str,

    /// The identifiers of the packages this one imports, spelled the same
    /// way. A dependency that resolves to a link rather than to a package
    /// of its own is absent, having no identifier to name.
    pub dependencies: &'a [String],

    /// The key this package's row takes in the store index, for a policy
    /// that decides by looking at what the package contains.
    ///
    /// A package already in the store when the install began is never
    /// extracted, so a host watching extractions does not hear about it.
    /// Reading the store is how such a host reaches it anyway. `None` for
    /// a resolution with no store row to name — a directory or a link.
    pub index_key: Option<&'a str>,
}

/// Decides which packages must be materialized in the project rather than
/// in a store shared across projects.
///
/// Under the global virtual store a package directory lives outside the
/// project and is shared by every project resolving the same content, so
/// nothing project-specific can be written inside it. A host that needs
/// per-project content in a package — a resolution shim, a repair, an
/// analysis artifact — names that package here and the install gives it a
/// project-local directory instead.
///
/// Asked ONCE, with everything the install resolved and who imports whom,
/// because keeping a package out of the shared store is only sound if
/// everything that imports it is kept out too: a store-resident importer
/// would go on resolving the shared copy, and the package would exist
/// twice at two real paths. A policy that has to reason about that needs
/// the graph, and asking package by package cannot give it one.
///
/// pnpm sets no policy, and without one every package takes the shared
/// layout, which is the behavior this replaces nothing of.
pub trait MaterializePolicy: std::fmt::Debug + Send + Sync {
    /// The identifiers, among `resolved`, that must not be shared between
    /// projects. An identifier that names nothing in `resolved` is ignored.
    fn materialize_locally(&self, resolved: &[ResolvedPackage<'_>]) -> HashSet<String>;
}

/// A materialization policy an install carries, if its host set one.
pub type SharedMaterializePolicy = Option<Arc<dyn MaterializePolicy>>;
