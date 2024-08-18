use std::path::PathBuf;

use clap::{Parser, ValueEnum};

#[derive(Debug, Parser)]
#[command(author, version)]
pub struct Cli {
    #[arg(
        short = 'b',
        long,
        help = "Path to a bpftrace executable",
        value_name = "PATH"
    )]
    pub bpftrace_path: PathBuf,

    #[arg(short = 'd', long, help = "How to sort the event output")]
    pub display_mode: DisplayMode,

    #[arg(long, help = "Show debug output")]
    pub debug: bool,

    #[arg(last = true, value_name = "CMD")]
    pub cmd: Vec<String>,
}

#[derive(Debug, ValueEnum, Clone, PartialEq, Eq)]
pub enum DisplayMode {
    Multiplexed,
    ByProcess,
    Mermaid,
}
