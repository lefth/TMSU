use std::ops;
use std::path::{Path, PathBuf};

use crate::errors::*;

#[derive(Debug, Clone)]
pub struct AbsPath(PathBuf);

impl AbsPath {
    pub fn from_unchecked(abs: PathBuf) -> Self {
        assert!(
            abs.is_absolute(),
            "Expected an absolute path, but got '{}'",
            abs.display()
        );
        Self { 0: abs }
    }
}

// Make all the `Path` methods available on AbsPath
impl ops::Deref for AbsPath {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for AbsPath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

/// Simple wrapper around PathBuf to enforce stronger typing.
///
/// At creation time, the file/directory is guaranteed to exist and its path to be canonical.
#[derive(Debug, Clone)]
pub struct CanonicalPath(AbsPath);

impl CanonicalPath {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        Ok(Self {
            0: AbsPath::from_unchecked(path.as_ref().canonicalize()?),
        })
    }
}

// Make all the `AbsPath` methods available on CanonicalPath
impl ops::Deref for CanonicalPath {
    type Target = AbsPath;

    fn deref(&self) -> &AbsPath {
        &self.0
    }
}

impl AsRef<Path> for CanonicalPath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}
