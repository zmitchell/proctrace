use std::io::Write;

use anyhow::Context;

use crate::models::Event;

type Error = anyhow::Error;

pub trait EventWrite {
    fn write_event(&mut self, event: &Event) -> Result<(), Error>;
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
    fn write_event(&mut self, event: &Event) -> Result<(), Error> {
        serde_json::to_writer(&mut self.inner, event).context("failed to write json event")?;
        let _ = self.inner.write(b"\n")?;
        Ok(())
    }

    fn write_raw(&mut self, line: impl AsRef<[u8]>) -> Result<(), Error> {
        if let Err(err) = self.inner.write_all(line.as_ref()) {
            eprintln!("failed to write raw event: {err}");
        }
        let _ = self.inner.write(b"\n");
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct NoOpWriter;

impl EventWrite for NoOpWriter {
    fn write_event(&mut self, _event: &Event) -> Result<(), Error> {
        Ok(())
    }

    fn write_raw(&mut self, _line: impl AsRef<[u8]>) -> Result<(), Error> {
        Ok(())
    }
}

#[cfg(test)]
pub mod test {
    use super::*;

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
        fn write_event(&mut self, event: &Event) -> Result<(), Error> {
            self.events.push(event.clone());
            Ok(())
        }

        fn write_raw(&mut self, line: impl AsRef<[u8]>) -> Result<(), Error> {
            self.raw.write_all(line.as_ref())?;
            Ok(())
        }
    }
}
