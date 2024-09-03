use crate::cli::Cli;
use clap::Parser;
use cli::Command;
use ingest::ingest_raw;
use models::Event;
#[cfg(target_os = "linux")]
use record::record;
use render::render;
use serde_json::Deserializer;

#[cfg(target_os = "linux")]
use std::sync::{atomic::AtomicBool, Arc};

use utils::{new_buffered_input_stream, new_buffered_output_stream};
use writers::{EventWrite, JsonWriter};

use anyhow::Context;

type Error = anyhow::Error;

#[cfg(target_os = "linux")]
const SCRIPT: &'static str = include_str!("../assets/proctrace.bt");

mod cli;
mod ingest;
mod models;
mod record;
mod render;
mod utils;
mod writers;

fn main() -> Result<(), Error> {
    let args = Cli::parse();

    match args.command {
        #[cfg(target_os = "linux")]
        Command::Record(args) => {
            if args.cmd.is_empty() {
                anyhow::bail!("must provide a command to run");
            }
            let shutdown_flag = Arc::new(AtomicBool::new(false));
            let _ = signal_hook::flag::register(nix::libc::SIGINT, Arc::clone(&shutdown_flag))
                .context("failed to install signal handler")?;
            let mut user_cmd = std::process::Command::new(&args.cmd[0]);
            user_cmd.args(&args.cmd[1..]);

            let writer = new_buffered_output_stream(&args.output_path)?;
            let ingester = record(
                user_cmd,
                args.bpftrace_path,
                shutdown_flag.clone(),
                args.debug,
                args.raw,
                writer,
            )
            .context("failed while recording events")?;
            if args.raw {
                eprintln!(
                    "Process tree root was PID {}",
                    ingester
                        .root_pid()
                        .map(|pid| format!("{pid}"))
                        .unwrap_or("UNSET".to_string())
                );
            }
        }
        Command::Sort(args) => {
            let reader = new_buffered_input_stream(&args.input_path)?;
            let de = Deserializer::from_reader(reader);
            let write_stream = new_buffered_output_stream(&args.output_path)?;
            let mut writer = JsonWriter::new(write_stream);
            let mut events = Vec::new();
            for maybe_event in de.into_iter::<Event>() {
                let event = maybe_event.context("failed to deserialize event")?;
                events.push(event);
            }
            events.sort_by_key(|e| e.timestamp());
            for event in events.into_iter() {
                writer.write_event(&event)?;
            }
        }
        Command::Render(args) => {
            let reader = new_buffered_input_stream(&args.input_path)?;
            let writer = new_buffered_output_stream(&args.output_path)?;
            render(reader, writer, args.display_mode)?;
        }
        Command::Ingest(args) => {
            let reader = new_buffered_input_stream(&args.input_path)?;
            let write_stream = new_buffered_output_stream(&args.output_path)?;
            let writer = JsonWriter::new(write_stream);
            ingest_raw(args.debug, args.root_pid, reader, writer)?;
        }
    }

    Ok(())
}