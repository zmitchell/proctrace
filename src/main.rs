use crate::cli::Cli;
use clap::Parser;
use record::record;
use render::render;
use std::{
    process::Command,
    sync::{atomic::AtomicBool, Arc},
};

use anyhow::Context;

type Error = anyhow::Error;

const SCRIPT: &'static str = include_str!("../assets/proctrace.bt");

mod cli;
mod models;
mod record;
mod render;

fn main() -> Result<(), Error> {
    let args = Cli::parse();
    if args.cmd.is_empty() {
        anyhow::bail!("must provide a command to run");
    }
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let _ = signal_hook::flag::register(nix::libc::SIGINT, Arc::clone(&shutdown_flag))
        .context("failed to install signal handler")?;
    let mut user_cmd = Command::new(&args.cmd[0]);
    user_cmd.args(&args.cmd[1..]);

    let (proc_events, user_cmd_pid) = record(
        user_cmd,
        args.bpftrace_path,
        shutdown_flag.clone(),
        args.debug,
    )
    .context("failed while recording events")?;

    render(proc_events, user_cmd_pid, args.display_mode);

    Ok(())
}
