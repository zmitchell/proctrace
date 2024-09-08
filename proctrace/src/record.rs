#[cfg(target_os = "linux")]
pub use has_record_support::*;

#[cfg(target_os = "linux")]
mod has_record_support {

    use std::{
        io::{BufRead, BufReader, Write},
        path::PathBuf,
        process::{Command, Stdio},
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
    };

    use anyhow::Context;

    use crate::{
        ingest::{EventIngester, EventParser},
        writers::JsonWriter,
        SCRIPT,
    };

    type Error = anyhow::Error;

    pub fn record(
        mut user_cmd: Command,
        bpftrace_path: PathBuf,
        shutdown_flag: Arc<AtomicBool>,
        debug: bool,
        record_raw: bool,
        output: impl Write,
    ) -> Result<EventIngester<JsonWriter<impl Write>>, Error> {
        let mut bpf_cmd = Command::new("sudo")
            .arg(&bpftrace_path)
            .arg("-e")
            .arg(SCRIPT)
            .stdout(Stdio::piped())
            .spawn()
            .context("failed to spawn bpftrace")?;
        let bpf_stdout = bpf_cmd.stdout.take().unwrap();
        // Sleep for just a bit to let bpftrace start up
        std::thread::sleep(std::time::Duration::from_millis(1000));

        let reader = BufReader::new(bpf_stdout);
        let event_parser = EventParser::new();
        let mut ingester = EventIngester::new(None, Some(JsonWriter::new(output)));

        let mut user_cmd_started = false;
        let mut child = None;

        for line in reader.lines() {
            // TODO: we can probably merge this implementation with `ingest_raw` if
            // we create a wrapper around the reader that checks this shutdown flag.
            if shutdown_flag.load(Ordering::SeqCst) {
                break;
            }
            // We need the reader started before the process, otherwise we might not catch it starting
            if !user_cmd_started {
                let proc = user_cmd.spawn().context("failed to spawn user command")?;
                let user_cmd_pid = proc.id() as i32; // it should fit
                child = Some(proc);
                ingester.set_root_pid(user_cmd_pid)?;
                user_cmd_started = true;
                continue;
            }
            if line.is_err() {
                eprintln!("failed to read line");
                continue;
            }
            let line = line.unwrap();
            if debug {
                eprintln!("RX: {}", line);
            }
            match event_parser.parse_line(&line) {
                Ok(event) => {
                    if record_raw {
                        ingester
                            .write_raw(&line)
                            .context("failed to write raw output")?;
                    }
                    ingester
                        .observe_event(&event)
                        .with_context(|| format!("failed to ingest event: {event:?}"))?;
                }
                Err(err) => {
                    eprintln!("failed to parse line: {}", err);
                }
            }

            // Reap the child process if possible
            if let Some(ref mut proc) = child {
                if let Ok(Some(_status)) = proc.try_wait() {
                    child = None;
                }
            }

            let unfinished = ingester
                .tracked_events()
                .unfinished_pids()
                .collect::<Vec<_>>();
            if debug {
                eprintln!("STILL_RUNNING: {unfinished:?}");
            }
            if !ingester.is_empty() && unfinished.is_empty() {
                break;
            }
        }

        Ok(ingester)
    }
}
