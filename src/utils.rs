use std::{
    fs::{File, OpenOptions},
    io::{stdin, stdout, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
};

use anyhow::Context;

type Error = anyhow::Error;

/// Returns an absolute path from a path that may not be absolute.
///
/// Relative paths are resolved relative to the current directory.
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

/// Opens a new file for output with common options.
pub fn new_output_file(path: impl AsRef<Path>) -> Result<File, Error> {
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .context("failed to open output file")
}

/// Returns a generic buffered output stream, either `stdout` or a file.
pub fn new_buffered_output_stream<T: AsRef<Path>>(
    path: &Option<T>,
) -> Result<Box<dyn Write>, Error> {
    if let Some(path) = path {
        let path = path.as_ref();
        let real_path = make_path_absolute(path)?;
        let file = new_output_file(real_path)?;
        let writer = BufWriter::new(file);
        Ok(Box::new(writer))
    } else {
        let stdout = stdout().lock();
        let writer = BufWriter::new(stdout);
        Ok(Box::new(writer))
    }
}

/// Returns a generic buffered input stream, either `stdin` or a file.
pub fn new_buffered_input_stream(path: impl AsRef<Path>) -> Result<Box<dyn Read>, Error> {
    let path = path.as_ref();
    if path == Path::new("-") {
        let stdin = stdin();
        let reader = BufReader::new(stdin);
        Ok(Box::new(reader))
    } else {
        let real_path = make_path_absolute(path)?;
        let file = std::fs::File::open(real_path).context("failed to open input file")?;
        let reader = BufReader::new(file);
        Ok(Box::new(reader))
    }
}
