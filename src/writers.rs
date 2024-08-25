use std::io::Write;

use anyhow::Context;

use crate::models::Event;

type Error = anyhow::Error;

pub trait EventWrite {
    fn write_event(&mut self, event: &Event) -> Result<(), Error>;
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
        self.inner.write(b"\n")?;
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct MockWriter {
    pub(crate) events: Vec<Event>,
}

impl MockWriter {
    pub fn new() -> Self {
        Self { events: vec![] }
    }
}

impl EventWrite for MockWriter {
    fn write_event(&mut self, event: &Event) -> Result<(), Error> {
        self.events.push(event.clone());
        Ok(())
    }
}
