#[cfg(record)]
pub use has_record_support::*;

#[cfg(record)]
mod has_record_support {

    use std::{
        collections::{BTreeMap, VecDeque},
        io::{BufRead, BufReader, Write},
        path::PathBuf,
        process::{Command, Stdio},
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
    };

    use anyhow::{anyhow, Context};
    use procfs::process::Process;
    use regex_lite::Regex;

    use crate::{
        ingest::EventParser,
        models::{Event, ProcEvents},
        SCRIPT,
    };

    type Error = anyhow::Error;

    // target/debug/proctrace record -o dbg.log -- flox activate -- sleep 1

    pub fn record(
        mut user_cmd: Command,
        bpftrace_path: PathBuf,
        shutdown_flag: Arc<AtomicBool>,
        debug: bool,
        record_raw: bool,
        mut output: impl Write,
    ) -> Result<(ProcEvents, i32), Error> {
        let mut proc_events = BTreeMap::new();
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

        let mut user_cmd_started = false;
        #[allow(unused_assignments)]
        let mut user_cmd_pid = 0;
        let mut child = None;

        // Forks may appear out of order from execs due to how they're recorded,
        // so we need to keep a backlog of events that _may_ contain the exec of
        // the user's command. We keep putting events into this backlog until we
        // see the fork, then we drain it and never put anything back into it.
        let mut backlog = vec![];
        let mut seen_fork = false;

        for line in reader.lines() {
            if shutdown_flag.load(Ordering::SeqCst) {
                break;
            }
            // We need the reader started before the process, otherwise we might not catch it starting
            if !user_cmd_started {
                let proc = user_cmd.spawn().context("failed to spawn user command")?;
                user_cmd_pid = proc.id() as i32; // it should fit
                child = Some(proc);
                let mut cmdline = vec![user_cmd.get_program().to_string_lossy().to_string()];
                let cmd_args: Vec<String> = user_cmd
                    .get_args()
                    .map(|s| s.to_string_lossy().to_string())
                    .collect();
                cmdline.extend_from_slice(&cmd_args);
                proc_events.insert(user_cmd_pid, VecDeque::new());
                user_cmd_started = true;
                continue;
            }
            if line.is_err() {
                eprintln!("failed to parse line");
                continue;
            }
            let line = line.unwrap();
            if debug {
                eprintln!("RX: {}", line);
            }
            match event_parser.parse_line(&line) {
                Ok(event) => {
                    let (is_initial_fork, should_write) =
                        ingest_event(&event, &mut proc_events, user_cmd_pid, true);
                    if is_initial_fork {
                        // Write to the output
                        if record_raw {
                            write_raw(&mut output, &line)?;
                        } else {
                            write_event(&mut output, &event)?;
                        }
                        // Now go through the backlog
                        for ev in backlog.drain(..) {
                            ingest_event(&ev, &mut proc_events, user_cmd_pid, true);
                            // Only write backlog events if we aren't in raw mode,
                            // we write event lines before putting them in the backlog
                            // in raw mode.
                            if !record_raw {
                                write_event(&mut output, &ev)?;
                            }
                            seen_fork = true;
                        }
                    } else if !seen_fork {
                        if record_raw {
                            write_raw(&mut output, &line)?;
                        }
                        backlog.push(event);
                    } else if should_write {
                        if record_raw {
                            write_raw(&mut output, &line)?;
                        } else {
                            write_event(&mut output, &event)?;
                        }
                    } else if record_raw {
                        write_raw(&mut output, &line)?;
                    }
                }
                Err(err) => {
                    eprintln!("{}", err);
                }
            }
            // Reap the child process if possible
            if let Some(ref mut proc) = child {
                if let Ok(Some(_status)) = proc.try_wait() {
                    child = None;
                }
            }

            // Print the outstanding processes
            if debug {
                let remaining_pids = proc_events
                    .values()
                    .filter_map(|events| match events.back() {
                        Some(ev) => match ev {
                            Event::Fork { child_pid, .. } => Some(child_pid),
                            Event::Exec { pid, .. } => Some(pid),
                            Event::ExecArgs { .. } => None,
                            Event::Exit { .. } => None,
                            Event::SetSID { pid, .. } => Some(pid),
                            Event::SetPGID { pid, .. } => Some(pid),
                        },
                        None => None,
                    })
                    .collect::<Vec<_>>();
                eprintln!("STILL_RUNNING: {remaining_pids:?}");
            }
            if !proc_events.is_empty()
                && proc_events
                    .values()
                    .all(|events| matches!(events.back().unwrap(), Event::Exit { .. }))
            {
                break;
            }
        }

        Ok((proc_events, user_cmd_pid))
    }

    pub fn write_event(mut writer: &mut impl Write, event: &Event) -> Result<(), Error> {
        // Write to the output
        if let Err(err) = serde_json::to_writer(&mut writer, event) {
            eprintln!("failed to write event: {err}");
        }
        let _ = writer.write(b"\n");
        Ok(())
    }

    pub fn write_raw(writer: &mut impl Write, line: impl AsRef<[u8]>) -> Result<(), Error> {
        if let Err(err) = writer.write_all(line.as_ref()) {
            eprintln!("failed to write raw event: {err}");
        }
        let _ = writer.write(b"\n");
        Ok(())
    }

    pub fn parse_line(
        line: &str,
        fork_regex: &Regex,
        exec_regex: &Regex,
        exec_args_regex: &Regex,
        exit_regex: &Regex,
        setsid_regex: &Regex,
        setpgid_regex: &Regex,
    ) -> Result<Event, Error> {
        if let Some(caps) = fork_regex.captures(line) {
            let ts = caps
                .name("ts")
                .ok_or(anyhow!("FORK line had no timestamp: {}", line))?
                .as_str();
            let parent_pid = caps
                .name("ppid")
                .ok_or(anyhow!("FORK line had no parent_pid: {}", line))?
                .as_str();
            let child_pid = caps
                .name("cpid")
                .ok_or(anyhow!("FORK line had no child_pid: {}", line))?
                .as_str();
            let parent_pgid = caps
                .name("pgid")
                .ok_or(anyhow!("FORK line had no parent_pgid: {}", line))?
                .as_str();
            let event = Event::Fork {
                timestamp: ts.parse().context("failed to parse fork timestamp")?,
                parent_pid: parent_pid
                    .parse()
                    .context("failed to parse fork parent_pid")?,
                child_pid: child_pid
                    .parse()
                    .context("failed to parse fork child_pid")?,
                parent_pgid: parent_pgid
                    .parse()
                    .context("failed to parse fork parent_pgid")?,
            };
            Ok(event)
        } else if let Some(caps) = exec_regex.captures(line) {
            let ts = caps
                .name("ts")
                .ok_or(anyhow!("EXEC line had no timestamp: {}", line))?
                .as_str();
            let pid = caps
                .name("pid")
                .ok_or(anyhow!("EXEC line had no pid: {}", line))?
                .as_str();
            let ppid = caps
                .name("ppid")
                .ok_or(anyhow!("EXEC line had no ppid: {}", line))?
                .as_str();
            let pgid = caps
                .name("pgid")
                .ok_or(anyhow!("EXEC line had no pgid: {}", line))?
                .as_str();
            let event = Event::Exec {
                timestamp: ts.parse().context("failed to parse exec timestamp")?,
                pid: pid.parse().context("failed to parse exec pid")?,
                ppid: ppid.parse().context("failed to parse exec ppid")?,
                pgid: pgid.parse().context("failed to parse exec pgid")?,
                cmdline: None,
            };
            Ok(event)
        } else if let Some(caps) = exec_args_regex.captures(line) {
            let ts = caps
                .name("ts")
                .ok_or(anyhow!("EXEC_ARGS line had no timestamp: {line}"))?
                .as_str();
            let pid = caps
                .name("pid")
                .ok_or(anyhow!("EXEC_ARGS line had no pid: {line}"))?
                .as_str();
            let args = caps
                .name("exec_args")
                .ok_or(anyhow!("EXEC_ARGS line had no args: {line}"))?
                .as_str();
            let event = Event::ExecArgs {
                timestamp: ts.parse().context("failed to parse exec timestamp")?,
                pid: pid.parse().context("failed to parse exec pid")?,
                args: args.parse().context("failed to parse exec args")?,
            };
            Ok(event)
        } else if let Some(caps) = exit_regex.captures(line) {
            let ts = caps
                .name("ts")
                .ok_or(anyhow!("EXIT line had no timestamp: {}", line))?
                .as_str();
            let pid = caps
                .name("pid")
                .ok_or(anyhow!("EXIT line had no pid: {}", line))?
                .as_str();
            let ppid = caps
                .name("ppid")
                .ok_or(anyhow!("EXIT line had no ppid: {}", line))?
                .as_str();
            let pgid = caps
                .name("pgid")
                .ok_or(anyhow!("EXIT line had no pgid: {}", line))?
                .as_str();
            let event = Event::Exit {
                timestamp: ts.parse().context("failed to parse exit timestamp")?,
                pid: pid.parse().context("failed to parse exit pid")?,
                ppid: ppid.parse().context("failed to parse exit ppid")?,
                pgid: pgid.parse().context("failed to parse exit pgid")?,
            };
            Ok(event)
        } else if let Some(caps) = setsid_regex.captures(line) {
            let ts = caps
                .name("ts")
                .ok_or(anyhow!("SETSID line had no timestamp: {}", line))?
                .as_str();
            let pid = caps
                .name("pid")
                .ok_or(anyhow!("SETSID line had no pid: {}", line))?
                .as_str();
            let ppid = caps
                .name("ppid")
                .ok_or(anyhow!("SETSID line had no ppid: {}", line))?
                .as_str();
            let pgid = caps
                .name("pgid")
                .ok_or(anyhow!("SETSID line had no pgid: {}", line))?
                .as_str();
            let sid = caps
                .name("sid")
                .ok_or(anyhow!("SETSID line had no sid: {}", line))?
                .as_str();
            let event = Event::SetSID {
                timestamp: ts.parse().context("failed to parse setsid timestamp")?,
                pid: pid.parse().context("failed to parse setsid pid")?,
                ppid: ppid.parse().context("failed to parse setsid ppid")?,
                pgid: pgid.parse().context("failed to parse setsid pgid")?,
                sid: sid.parse().context("failed to parse setsid sid")?,
            };
            Ok(event)
        } else if let Some(caps) = setpgid_regex.captures(line) {
            let ts = caps
                .name("ts")
                .ok_or(anyhow!("SETPGID line had no timestamp: {}", line))?
                .as_str();
            let pid = caps
                .name("pid")
                .ok_or(anyhow!("SETPGID line had no pid: {}", line))?
                .as_str();
            let ppid = caps
                .name("ppid")
                .ok_or(anyhow!("SETPGID line had no ppid: {}", line))?
                .as_str();
            let pgid = caps
                .name("pgid")
                .ok_or(anyhow!("SETPGID line had no pgid: {}", line))?
                .as_str();
            let event = Event::SetPGID {
                timestamp: ts.parse().context("failed to parse setpgid timestamp")?,
                pid: pid.parse().context("failed to parse setpgid pid")?,
                ppid: ppid.parse().context("failed to parse setpgid ppid")?,
                pgid: pgid.parse().context("failed to parse setpgid pgid")?,
            };
            Ok(event)
        } else {
            Err(anyhow!("line did not match any regexes: {}", line))
        }
    }

    pub fn ingest_event<'a>(
        event: &'a Event,
        procs: &mut ProcEvents,
        root_pid: i32,
        lookup_args: bool,
    ) -> (bool, bool) {
        let mut is_initial_fork = false;
        let mut should_write = false;
        match event {
            Event::Fork {
                parent_pid,
                child_pid,
                ..
            } => {
                if child_pid == &root_pid {
                    procs
                        .entry(root_pid)
                        .and_modify(|events| events.push_back(event.clone()));
                    is_initial_fork = true;
                    should_write = true;
                } else if procs.contains_key(parent_pid) && !procs.contains_key(child_pid) {
                    let mut events = VecDeque::new();
                    events.push_back(event.clone());
                    procs.insert(*child_pid, events);
                    should_write = true;
                }
            }
            Event::Exec {
                timestamp,
                pid,
                ppid,
                pgid,
                cmdline,
            } => {
                if procs.contains_key(pid) {
                    let cmdline = if lookup_args {
                        match Process::new(*pid).and_then(|p| p.cmdline()) {
                            Ok(cmd) => Some(cmd),
                            Err(e) => {
                                eprintln!("failed to get cmd for PID {}: {}", pid, e.to_string());
                                None
                            }
                        }
                    } else {
                        cmdline.clone()
                    };
                    let event = Event::Exec {
                        timestamp: *timestamp,
                        pid: *pid,
                        ppid: *ppid,
                        pgid: *pgid,
                        cmdline,
                    };
                    procs
                        .entry(*pid)
                        .and_modify(|events| events.push_back(event.clone()));
                    should_write = true;
                }
            }
            Event::ExecArgs { pid, .. } => {
                if procs.contains_key(pid) {
                    procs
                        .entry(*pid)
                        .and_modify(|events| events.push_back(event.clone()));
                    should_write = true;
                }
            }
            Event::Exit { pid, .. } => {
                if procs.contains_key(pid) {
                    procs
                        .entry(*pid)
                        .and_modify(|events| events.push_back(event.clone()));
                    should_write = true;
                }
            }
            Event::SetSID { pid, .. } => {
                if procs.contains_key(pid) {
                    procs
                        .entry(*pid)
                        .and_modify(|events| events.push_back(event.clone()));
                    should_write = true;
                }
            }
            Event::SetPGID { pid, .. } => {
                if procs.contains_key(pid) {
                    procs
                        .entry(*pid)
                        .and_modify(|events| events.push_back(event.clone()));
                    should_write = true;
                }
            }
        }
        (is_initial_fork, should_write)
    }
}
