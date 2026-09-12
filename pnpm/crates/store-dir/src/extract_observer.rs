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
use std::{collections::HashMap, path::PathBuf, sync::Arc};

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
