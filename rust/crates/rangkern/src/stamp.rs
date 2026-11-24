//! What tells `rangkern serve` that a file changed between two requests.
//!
//! A stamp is the part of the file system metadata that every write, rename, `chmod` and `utime`
//! moves: on Unix the device, the inode, the size, the modification time and the change time
//! (`ctime`). `ctime` is the one that matters: a program can reset `mtime` (`os.utime`, `touch -r`,
//! an archive tool), but it cannot reset `ctime`, so a rewrite of the same size with an old `mtime`
//! still shows. Size and `mtime` alone are not enough.
//!
//! Two limits of a stamp, and what the server does about each:
//!
//! - **Coarse clocks.** Two writes within one tick of the file system clock can carry the same
//!   `ctime`. A file whose newest timestamp lies less than [`RACY_NS`] before the moment of the
//!   stamp is *racy*: its stamp proves nothing yet, and the server compares the content the next
//!   time it looks (the technique git uses for "racily clean" index entries). After that window a
//!   later change cannot share the timestamp any more.
//! - **No `ctime`.** Windows exposes no change time through safe `std` (`unsafe` is forbidden in
//!   this workspace), so a timestamp could be reset without a trace. There every stamp is racy:
//!   the server compares the content on every request ([`Detect::Content`]). Reading a file closes
//!   it again at once, so no handle stays open between two requests.

use std::fs::Metadata;
use std::time::{SystemTime, UNIX_EPOCH};

/// How long after its last change a file stays racy: longer than the coarsest timestamp
/// granularity of the file systems in use (FAT keeps two seconds; the others are finer).
pub const RACY_NS: i128 = 2_000_000_000;

/// How changes are detected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detect {
    /// By the stamp, with the content as a check for racy files.
    Metadata,
    /// By the content on every request: for platforms without a change time.
    Content,
}

impl Detect {
    /// The method the platform allows: the stamp on Unix, the content elsewhere.
    #[must_use]
    pub const fn native() -> Self {
        if cfg!(unix) {
            Self::Metadata
        } else {
            Self::Content
        }
    }
}

/// The metadata that identifies one state of a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    pub(crate) dev: u64,
    pub(crate) ino: u64,
    pub(crate) size: u64,
    pub(crate) mtime: i128,
    pub(crate) ctime: i128,
}

/// The state of a file at the moment it was looked at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub(crate) key: Key,
    pub(crate) racy: bool,
}

/// Nanoseconds since the Unix epoch (negative before it).
pub(crate) fn system_ns(time: SystemTime) -> i128 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => i128::try_from(after.as_nanos()).unwrap_or(i128::MAX),
        Err(before) => -i128::try_from(before.duration().as_nanos()).unwrap_or(i128::MAX),
    }
}

/// The current time in nanoseconds since the Unix epoch.
#[must_use]
pub fn now_ns() -> i128 {
    system_ns(SystemTime::now())
}

#[cfg(unix)]
fn key_of(meta: &Metadata) -> Key {
    use std::os::unix::fs::MetadataExt;
    let ns = |seconds: i64, nanos: i64| i128::from(seconds) * 1_000_000_000 + i128::from(nanos);
    Key {
        dev: meta.dev(),
        ino: meta.ino(),
        size: meta.size(),
        mtime: ns(meta.mtime(), meta.mtime_nsec()),
        ctime: ns(meta.ctime(), meta.ctime_nsec()),
    }
}

/// The key where the platform has no inode and no change time.
#[cfg(not(unix))]
fn portable_key(meta: &Metadata) -> Key {
    Key {
        dev: 0,
        ino: 0,
        size: meta.len(),
        mtime: meta.modified().map_or(0, system_ns),
        ctime: 0,
    }
}

impl Stamp {
    /// The stamp of a file with the given metadata, looked at `now` (nanoseconds since the epoch).
    #[must_use]
    pub fn new(meta: &Metadata, now: i128, detect: Detect) -> Self {
        #[cfg(unix)]
        let key = key_of(meta);
        #[cfg(not(unix))]
        let key = portable_key(meta);
        Self {
            key,
            racy: Self::is_racy(&key, now, detect),
        }
    }

    /// Whether a stamp taken `now` proves nothing yet.
    fn is_racy(key: &Key, now: i128, detect: Detect) -> bool {
        detect == Detect::Content || now < key.mtime.max(key.ctime).saturating_add(RACY_NS)
    }

    /// Whether the file is still the one this stamp described, judging by the metadata alone:
    /// the same key and no doubt about the clock. `false` means: look at the content.
    #[must_use]
    pub fn unchanged(&self, newer: &Self) -> bool {
        !self.racy && self.key == newer.key
    }

    /// Whether the stamp leaves room for doubt, so that the content has to be compared.
    #[must_use]
    pub fn racy(&self) -> bool {
        self.racy
    }
}
