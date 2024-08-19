use crate::cli::Cli;
use clap::Parser;
use cli::Command;
use record::record;
use render::render;
use std::{
    fs::OpenOptions,
    io::{stdin, stdout, BufReader, BufWriter},
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
                    .open(real_path)
                    .context("failed to open output file")?;
                let writer = BufWriter::new(file);
                let _ = record(
                    user_cmd,
                    args.bpftrace_path,
                    shutdown_flag.clone(),
                    args.debug,
                    writer,
                )
                .context("failed while recording events")?;
            } else {
                let stdout = stdout().lock();
                let writer = BufWriter::new(stdout);
                let _ = record(
                    user_cmd,
                    args.bpftrace_path,
                    shutdown_flag.clone(),
                    args.debug,
                    writer,
                )
                .context("failed while recording events")?;
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
