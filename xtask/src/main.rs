use std::{
    io::{stderr, Write},
    path::{Path, PathBuf},
};

use anyhow::Context;
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};

type Error = anyhow::Error;

const DOCS_DIR: &str = "docs/src/content/docs/reference";

#[derive(Debug, Parser)]
#[command(author, version)]
#[command(max_term_width = 80)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum Command {
    #[command(name = "manpages")]
    #[command(about = "Generate the manpages.")]
    GenManpages,
    #[command(name = "docs")]
    BuildDocs,
}

fn main() -> Result<(), Error> {
    let args = Cli::parse();
    match args.command {
        Command::GenManpages => generate_manpages(),
        Command::BuildDocs => todo!(),
    }
}

fn generate_manpages() -> Result<(), Error> {
    let cmd = proctrace::cli::Cli::command();
    let tempdir = tempfile::tempdir().context("failed to create tempdir")?;
    let output_dir = std::env::current_dir().unwrap().join(DOCS_DIR);
    clap_mangen::generate_to(cmd, tempdir.path()).context("failed to generate manpages")?;
    for dir_entry in std::fs::read_dir(tempdir.path()).context("couldn't read tempdir")? {
        let dir_entry = dir_entry.context("couldn't access dir entry")?;
        let mut cmd = std::process::Command::new("pandoc");
        cmd.args(["--standalone", "--from", "man", "--to", "markdown"]);
        cmd.arg(dir_entry.path());
        cmd.arg("-o");
        let mut manpage_path_as_md = dir_entry.path();
        manpage_path_as_md.set_extension("md");
        let mut page_path = output_dir.join("dummy");
        page_path.set_file_name(manpage_path_as_md.file_name().unwrap());
        cmd.arg(page_path);
        let output = cmd.output().context("failed to run pandoc command")?;
        if !output.status.success() {
            let mut stderr = stderr().lock();
            stderr
                .write_all(&output.stderr)
                .context("failed to write pandoc stderr")?;
            return Err(anyhow::anyhow!("failed to convert manpage with pandoc"));
        }
    }
    Ok(())
}
