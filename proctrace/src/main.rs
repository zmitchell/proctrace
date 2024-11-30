use crate::cli::Cli;
use clap::Parser;
use cli::Command;
use ingest::ingest_raw;
#[cfg(target_os = "linux")]
use record::record;
use render::{render, render_sequential};

#[cfg(target_os = "linux")]
use std::sync::{atomic::AtomicBool, Arc};

use utils::{new_buffered_input_stream, new_buffered_output_stream};
use writers::NoOpWriter;

#[cfg(target_os = "linux")]
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
            let mut ingester = record(
                user_cmd,
                args.bpftrace_path,
                shutdown_flag.clone(),
                args.debug,
                args.raw,
                writer,
            )
            .context("failed while recording events")?;
            ingester.post_process_buffers();
            if args.raw {
                eprintln!(
                    "Process tree root was PID {}",
                    ingester
                        .root_pid()
                        .map(|pid| format!("{pid}"))
                        .unwrap_or("UNSET".to_string())
                );
            } else {
                let writer = new_buffered_output_stream(&args.output_path)?;
                render_sequential(ingester, writer)?;
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
            let dummy_writer = NoOpWriter;
            let mut ingester = ingest_raw(args.debug, args.root_pid, reader, dummy_writer)?;
            ingester.post_process_buffers();
            render_sequential(ingester, write_stream)?;
        }
    }

    Ok(())
}
