use std::io::Write;

type Error = anyhow::Error;

pub trait EventWrite {
    fn write_raw(&mut self, line: impl AsRef<[u8]>) -> Result<(), Error>;
}

#[derive(Debug)]
pub struct JsonWriter<T> {
    inner: T,
}

impl<T> JsonWriter<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
}

impl<T: Write> EventWrite for JsonWriter<T> {
    fn write_raw(&mut self, line: impl AsRef<[u8]>) -> Result<(), Error> {
        if let Err(err) = self.inner.write_all(line.as_ref()) {
            eprintln!("failed to write raw event: {err}");
        }
        let _ = self.inner.write(b"\n");
        Ok(())
    }
}

#[derive(Debug)]
pub struct NoOpWriter;

impl EventWrite for NoOpWriter {
    fn write_raw(&mut self, _line: impl AsRef<[u8]>) -> Result<(), Error> {
        Ok(())
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::models::Event;

    #[derive(Debug)]
    pub(crate) struct MockWriter {
        pub(crate) events: Vec<Event>,
        pub(crate) raw: Vec<u8>,
    }

    impl MockWriter {
        pub fn new() -> Self {
            Self {
                events: vec![],
                raw: vec![],
            }
        }
    }

    impl EventWrite for MockWriter {
        fn write_raw(&mut self, line: impl AsRef<[u8]>) -> Result<(), Error> {
            self.raw.write_all(line.as_ref())?;
            Ok(())
        }
    }
}
