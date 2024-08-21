use crate::cli::Cli;
use clap::Parser;
use cli::Command;
use models::Event;
use record::record;
use render::render;
use serde_json::Deserializer;
use std::{
    fs::OpenOptions,
    io::{stdin, stdout, BufReader, BufWriter, Write},
    path::Path,
    sync::{atomic::AtomicBool, Arc},
};
use utils::make_path_absolute;

use anyhow::Context;

type Error = anyhow::Error;

const SCRIPT: &'static str = include_str!("../assets/proctrace.bt");

mod cli;
mod models;
mod record;
mod render;
mod utils;

fn main() -> Result<(), Error> {
    let args = Cli::parse();

    match args.command {
        Command::Record(args) => {
            if args.cmd.is_empty() {
                anyhow::bail!("must provide a command to run");
            }
            let shutdown_flag = Arc::new(AtomicBool::new(false));
            let _ = signal_hook::flag::register(nix::libc::SIGINT, Arc::clone(&shutdown_flag))
                .context("failed to install signal handler")?;
            let mut user_cmd = std::process::Command::new(&args.cmd[0]);
            user_cmd.args(&args.cmd[1..]);

            if let Some(path) = args.output_path {
                let real_path = make_path_absolute(&path)?;
                let file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(real_path)
                    .context("failed to open output file")?;
                let writer = BufWriter::new(file);
                let (_, root_pid) = record(
                    user_cmd,
                    args.bpftrace_path,
                    shutdown_flag.clone(),
                    args.debug,
                    args.raw,
                    writer,
                )
                .context("failed while recording events")?;
                if args.raw {
                    eprintln!("Process tree root was PID {root_pid}");
                }
            } else {
                let stdout = stdout().lock();
                let writer = BufWriter::new(stdout);
                let (_, root_pid) = record(
                    user_cmd,
                    args.bpftrace_path,
                    shutdown_flag.clone(),
                    args.debug,
                    args.raw,
                    writer,
                )
                .context("failed while recording events")?;
                if args.raw {
                    eprintln!("Process tree root was PID {root_pid}");
                }
            }
        }
        Command::Sort(args) => {
            let input_path = make_path_absolute(&args.input_path)?;
            let file = std::fs::File::open(&input_path).context("failed to open input file")?;
            let reader = BufReader::new(file);
            let de = Deserializer::from_reader(reader);
            let mut events = Vec::new();
            for maybe_event in de.into_iter::<Event>() {
                let event = maybe_event.context("failed to deserialize event")?;
                events.push(event);
            }
            events.sort_by_key(|e| e.timestamp());
            if let Some(path) = args.output_path {
                let real_path = make_path_absolute(&path)?;
                let file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(real_path)
                    .context("failed to open output file")?;
                let mut writer = BufWriter::new(file);
                for event in events.into_iter() {
                    serde_json::to_writer(&mut writer, &event)
                        .context("failed to write sorted event")?;
                    writer.write(b"\n").context("writer failed")?;
                }
            } else {
                let stdout = stdout().lock();
                let mut writer = BufWriter::new(stdout);
                for event in events.into_iter() {
                    serde_json::to_writer(&mut writer, &event)
                        .context("failed to write sorted event")?;
                    writer.write(b"\n").context("writer failed")?;
                }
            }
        }
        Command::Render(args) => {
            if &args.input_path == Path::new("-") {
                let stdin = stdin().lock();
                let reader = BufReader::new(stdin);
                render(reader, args.display_mode)?;
            } else {
                let real_path = make_path_absolute(&args.input_path)?;
                let file = std::fs::File::open(real_path).context("failed to open output file")?;
                let reader = BufReader::new(file);
                render(reader, args.display_mode)?;
            }
        }
    }

    Ok(())
}
