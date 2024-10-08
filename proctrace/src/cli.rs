use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(author, version)]
#[command(max_term_width = 80)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Default, ValueEnum, Clone, PartialEq, Eq)]
pub enum DisplayMode {
    #[default]
    Sequential,
    ByProcess,
    Mermaid,
}

impl std::fmt::Display for DisplayMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DisplayMode::Sequential => write!(f, "sequential"),
            DisplayMode::ByProcess => write!(f, "by-process"),
            DisplayMode::Mermaid => write!(f, "mermaid"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum Command {
    /// Record the process lifecycle events from a command.
    ///
    /// Note that this uses `bpftrace` under the hood, and will try to run it
    /// with superuser priviledges (but will not run any other commands with
    /// elevated priviledges). Depending on how you've installed `bpftrace` it
    /// may not be in the PATH of the superuser. If this is the case then you
    /// can use the `--bpftrace-path` flag to specify it manually. This is likely
    /// the case if you've installed `bpftrace` via `flox` or `nix profile install`.
    #[cfg(target_os = "linux")]
    Record(RecordArgs),

    /// Convert a raw recording into a processed recording that can be rendered.
    ///
    /// A recording produced in "raw" mode cannot be rendered directly, so it must first
    /// be processed into a render-ready form. This subcommand does that processing.
    Ingest(IngestArgs),

    /// Render a recording in the specified display format.
    Render(RenderArgs),
}

#[derive(Debug, Clone, Args, PartialEq, Eq)]
#[cfg(target_os = "linux")]
pub struct RecordArgs {
    /// The path to a `bpftrace` executable.
    ///
    /// Since `bpftrace` needs to be run as root, it's possible that the root
    /// user may not have `bpftrace` in their path. In that case you'll need
    /// to pass in an explicit path. This is the case if you've installed
    /// `bpftrace` via `flox` or `nix profile`.
    #[arg(
        short,
        long,
        help = "Path to a bpftrace executable",
        value_name = "PATH",
        default_value = "bpftrace"
    )]
    pub bpftrace_path: PathBuf,

    /// Show each line of output from `bpftrace` before it goes through filtering.
    ///
    /// This also displays which PIDs are being tracked but have not yet exited.
    #[arg(long, help = "Show debug output")]
    pub debug: bool,

    /// Write the raw events from the `bpftrace` script instead of the processed
    /// events.
    ///
    /// This will write events from processes outside of the target process
    /// tree, but recording this raw output allows you to rerun analysis without
    /// needing to collect another recording.
    #[arg(short, long, help = "Record all of the raw events from bpftrace")]
    pub raw: bool,

    /// Where to write the output (default: stdout).
    #[arg(
        short,
        long = "output",
        help = "Where to write the output (printed to stdout if omitted).",
        value_name = "PATH"
    )]
    pub output_path: Option<PathBuf>,

    /// The user-provided command that should be recorded.
    ///
    /// Note that this will print to the terminal if it has output. `proctrace`
    /// does its best to not meddle with the environment of the command so that
    /// it behaves as you expect.
    #[arg(last = true, value_name = "CMD")]
    pub cmd: Vec<String>,
}

#[derive(Debug, Clone, Args, PartialEq, Eq)]
pub struct RenderArgs {
    /// How should the output be rendered.
    ///
    /// For "sequential" events will be shown in the order that they were received.
    /// For "by-process" events are shown in order for each process,
    /// and processes are separated by a blank line. For "mermaid" the output is the
    /// syntax for a Mermaid.js Gantt chart.
    #[arg(short, long, help = "The output format")]
    #[arg(default_value_t = DisplayMode::Sequential)]
    pub display_mode: DisplayMode,

    /// The location where an event recording should be read from.
    ///
    /// Must either be a path to a file or '-' to read from stdin.
    #[arg(short, long = "input", help = "The path to the event data file")]
    pub input_path: PathBuf,

    /// Where to write the rendered output.
    #[arg(
        short,
        long = "output",
        help = "Where to write the output (printed to stdout if omitted).",
        value_name = "PATH"
    )]
    pub output_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Args, PartialEq, Eq)]
pub struct IngestArgs {
    /// The path to the raw recording to be processed.
    ///
    /// Must either be a path to a file or '-' to read from stdin.
    #[arg(short, long = "input", help = "The path to the event data file")]
    pub input_path: PathBuf,

    /// Where to write the processed recording.
    #[arg(
        short,
        long = "output",
        help = "Where to write the output (printed to stdout if omitted).",
        value_name = "PATH"
    )]
    pub output_path: Option<PathBuf>,

    /// Which PID to use as the root of the process tree.
    ///
    /// A raw recording contains events from the entire system,
    /// so the user must supply a PID from which to begin tracing
    /// a process tree.
    #[arg(short = 'p', long, value_name = "PID")]
    pub root_pid: i32,

    /// Whether to display debug output while ingesting.
    #[arg(short, long)]
    pub debug: bool,
}
