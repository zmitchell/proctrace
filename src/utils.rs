use std::path::{Path, PathBuf};

use anyhow::Context;

type Error = anyhow::Error;

pub fn make_path_absolute(path: impl AsRef<Path>) -> Result<PathBuf, Error> {
    let path = path.as_ref();
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()
            .context("failed to get current directory")?
            .join(path))
    }
}
