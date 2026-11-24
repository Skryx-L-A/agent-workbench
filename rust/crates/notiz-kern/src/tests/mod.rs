//! Unit tests. They pin the behaviour of the Python model with exact values, so that a mutation
//! of any rule changes a result.

mod blocks;
mod cli;
mod corpus;
mod frontmatter;
mod note;
mod schema;
mod section;
mod text;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A temporary directory that is removed when it goes out of scope.
pub(crate) struct TempDir(PathBuf);

impl TempDir {
    pub(crate) fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("notiz-kern-unit-{}-{n}", std::process::id()));
        fs::create_dir_all(&path).expect("temporary directory is created");
        Self(path)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }

    /// Writes a file below the directory, creating parents, and returns its path.
    pub(crate) fn write(&self, rel: &str, content: &[u8]) -> PathBuf {
        let path = self.0.join(rel);
        fs::create_dir_all(path.parent().expect("file has a parent")).expect("parent is created");
        fs::write(&path, content).expect("file is written");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
