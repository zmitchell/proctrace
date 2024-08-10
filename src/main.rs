use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use anyhow::{anyhow, Context};
use clap::{Parser, ValueEnum};
use procfs::process::Process;
use regex_lite::Regex;

type Error = anyhow::Error;

const SCRIPT: &'static str = r#"
BEGIN {}

tracepoint:syscalls:sys_enter_clone
{
	$task = (struct task_struct *)curtask;
	// Ensures that we don't record threads exiting
	if ($task->pid == $task->tgid) {
	    // Ensures that we don't process forks of threads
	    if (args.clone_flags & 0x00010000 == 0) {
			// Store the PID to be looked up later
			@clones[$task->tgid] = 1;
		}
    }
}

tracepoint:syscalls:sys_exit_clone
{
	$task = (struct task_struct *)curtask;
	// Ensures that we don't record threads exiting
	if ($task->pid == $task->tgid) {
	    // Don't process this clone unless we've recorded the `enter` side of it
		if (@clones[tid] != 0) {
			@clones[tid] = 0;
			$child_pid = args.ret;
			printf("FORK: ts=%u,parent_pid=%d,child_pid=%d,parent_pgid=%d,\n", elapsed, $task->tgid, $child_pid, $task->real_parent->tgid);
		}
    }
}

tracepoint:syscalls:sys_enter_clone3
{
	$task = (struct task_struct *)curtask;
	// Ensures that we don't record a clone unless it's a process
	if ($task->pid == $task->tgid) {
		// Ensures that we don't record a fork of a thread
		if (args.uargs->flags & 0x00010000 == 0) {
			@clones[tid] = 1;
		}
    }
}

tracepoint:syscalls:sys_exit_clone3
{
	$task = (struct task_struct *)curtask;
	// Ensures that we don't record a clone unless it's a process
	if ($task->pid == $task->tgid) {
		// Don't process this clone unless we've seen the `enter` side of it
		if (@clones[tid] != 0) {
			@clones[tid] = 0;
			$child_pid = args.ret;
			printf("FORK: ts=%u,parent_pid=%d,child_pid=%d,parent_pgid=%d,\n", elapsed, $task->tgid, $child_pid, $task->real_parent->tgid);
		}
    }
}


tracepoint:syscalls:sys_enter_execve
{
	$task = (struct task_struct *)curtask;
	printf("EXEC: ts=%u,pid=%d,ppid=%d,pgid=%d\n", elapsed, $task->tgid, $task->real_parent->tgid, $task->group_leader->tgid);
}

tracepoint:sched:sched_process_exit
{
	$task = (struct task_struct *)curtask;
	// Ensures that we don't record threads exiting
	if ($task->pid == $task->tgid) {
    	printf("EXIT: ts=%u,pid=%d,ppid=%d,pgid=%d\n", elapsed, $task->tgid, $task->real_parent->tgid, $task->group_leader->tgid);
    }
}

// tracepoint:syscalls:sys_enter_exit_group
// {
// 	$task = (struct task_struct *)curtask;
// 	// Ensures that we don't record threads exiting
// 	if ($task->pid == $task->tgid) {
//     	printf("EXIT: ts=%u,pid=%d,ppid=%d,pgid=%d,GROUP\n", elapsed, $task->tgid, $task->real_parent->tgid, $task->group_leader->tgid);
//     }
// }

uretprobe:libc:setsid
{
	$task = (struct task_struct *)curtask;
	$session = retval;
	printf("SETSID: ts=%u,pid=%d,ppid=%d,pgid=%d,sid=%d\n", elapsed, $task->tgid, $task->real_parent->tgid, $task->group_leader->tgid,$session);
}

uretprobe:libc:setpgid
{
	$task = (struct task_struct *)curtask;
	printf("SETPGID: ts=%u,pid=%d,ppid=%d,pgid=%d\n", elapsed, $task->tgid, $task->real_parent->tgid, $task->group_leader->tgid);
}
"#;

#[derive(Debug, Parser)]
#[command(author, version)]
struct Cli {
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
enum DisplayMode {
    Multiplexed,
    ByProcess,
    Mermaid,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum Event {
    Fork {
        timestamp: u128,
        parent_pid: i32,
        child_pid: i32,
        parent_pgid: i32,
    },
    Exec {
        timestamp: u128,
        pid: i32,
        ppid: i32,
        pgid: i32,
        cmdline: Option<Vec<String>>,
    },
    Exit {
        timestamp: u128,
        pid: i32,
        ppid: i32,
        pgid: i32,
    },
    SetSID {
        timestamp: u128,
        pid: i32,
        ppid: i32,
        pgid: i32,
        sid: i32,
    },
    SetPGID {
        timestamp: u128,
        pid: i32,
        ppid: i32,
        pgid: i32,
    },
}

impl Event {
    fn timestamp(&self) -> u128 {
        match self {
            Event::Fork { timestamp, .. } => *timestamp,
            Event::Exec { timestamp, .. } => *timestamp,
            Event::Exit { timestamp, .. } => *timestamp,
            Event::SetSID { timestamp, .. } => *timestamp,
            Event::SetPGID { timestamp, .. } => *timestamp,
        }
    }

    fn set_timestamp(&mut self, new_ts: u128) {
        match self {
            Event::Fork { timestamp, .. } => *timestamp = new_ts,
            Event::Exec { timestamp, .. } => *timestamp = new_ts,
            Event::Exit { timestamp, .. } => *timestamp = new_ts,
            Event::SetSID { timestamp, .. } => *timestamp = new_ts,
            Event::SetPGID { timestamp, .. } => *timestamp = new_ts,
        }
    }

    fn is_fork(&self) -> bool {
        matches!(self, Event::Fork { .. })
    }

    fn is_exec(&self) -> bool {
        matches!(self, Event::Exec { .. })
    }

    #[allow(dead_code)]
    fn is_exit(&self) -> bool {
        matches!(self, Event::Exit { .. })
    }
}

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
    let mut proc_events: BTreeMap<i32, Vec<Event>> = BTreeMap::new();
    let mut bpf_cmd = Command::new("sudo")
        .arg(&args.bpftrace_path)
        .arg("-e")
        .arg(SCRIPT)
        .stdout(Stdio::piped())
        .spawn()
        .context("failed to spawn bpftrace")?;
    let bpf_stdout = bpf_cmd.stdout.take().unwrap();
    let reader = BufReader::new(bpf_stdout);

    let fork_regex = Regex::new(
        r"FORK: ts=(?<ts>\d+),parent_pid=(?<ppid>[\-\d]+),child_pid=(?<cpid>[\-\d]+),parent_pgid=(?<pgid>[\-\d]+)",
    ).unwrap();
    let exec_regex = Regex::new(
        r"EXEC: ts=(?<ts>\d+),pid=(?<pid>[\-\d]+),ppid=(?<ppid>[\-\d]+),pgid=(?<pgid>[\-\d]+)",
    )
    .unwrap();
    let exit_regex = Regex::new(
        r"EXIT: ts=(?<ts>\d+),pid=(?<pid>[\-\d]+),ppid=(?<ppid>[\-\d]+),pgid=(?<pgid>[\-\d]+)",
    )
    .unwrap();
    let setsid_regex = Regex::new(
        r"SETSID: ts=(?<ts>\d+),pid=(?<pid>[\-\d]+),ppid=(?<ppid>[\-\d]+),pgid=(?<pgid>[\-\d]+),sid=(?<sid>[\-\d]+)",
    ).unwrap();
    let setpgid_regex = Regex::new(
        r"SETPGID: ts=(?<ts>\d+),pid=(?<pid>[\-\d]+),ppid=(?<ppid>[\-\d]+),pgid=(?<pgid>[\-\d]+)",
    )
    .unwrap();

    let mut user_cmd_started = false;
    #[allow(unused_assignments)]
    let mut user_cmd_pid = 0;
    let mut child = None;
    for line in reader.lines() {
        if shutdown_flag.load(Ordering::SeqCst) {
            break;
        }
        // We need the reader started before the process, otherwise we might not catch it starting
        if !user_cmd_started {
            let proc = user_cmd.spawn().context("failed to spawn user command")?;
            user_cmd_pid = proc.id() as i32; // it should fit
            child = Some(proc);
            proc_events.insert(
                user_cmd_pid,
                vec![Event::Exec {
                    timestamp: 0,
                    pid: user_cmd_pid,
                    ppid: 0,
                    pgid: 0,
                    cmdline: Some(args.cmd.clone()),
                }],
            );
            user_cmd_started = true;
            continue;
        }
        if line.is_err() {
            eprintln!("failed to parse line");
            continue;
        }
        let line = line.unwrap();
        if args.debug {
            eprintln!("RX: {}", line);
        }
        match parse_line(
            &line,
            &fork_regex,
            &exec_regex,
            &exit_regex,
            &setsid_regex,
            &setpgid_regex,
        ) {
            Ok(event) => {
                handle_event(&event, &mut proc_events);
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
        if args.debug {
            let remaining_pids = proc_events
                .values()
                .filter_map(|events| match events.last() {
                    Some(ev) => match ev {
                        Event::Fork { child_pid, .. } => Some(child_pid),
                        Event::Exec { pid, .. } => Some(pid),
                        Event::Exit { .. } => None,
                        Event::SetSID { pid, .. } => Some(pid),
                        Event::SetPGID { pid, .. } => Some(pid),
                    },
                    None => None,
                })
                .collect::<Vec<_>>();
            eprintln!("{remaining_pids:?}");
        }
        if !proc_events.is_empty()
            && proc_events
                .values()
                .all(|events| matches!(events.last().unwrap(), Event::Exit { .. }))
        {
            break;
        }
    }

    match args.display_mode {
        DisplayMode::Multiplexed => {
            let mut sorted_events = proc_events.into_values().flatten().collect::<Vec<_>>();
            sorted_events.sort_by_key(|e| e.timestamp());
            if sorted_events.len() > 1 {
                let next_ts = sorted_events[1].timestamp();
                sorted_events.get_mut(0).unwrap().set_timestamp(next_ts);
            }
            println!("EVENTS");
            let mut prev_ts = 0;
            for event in sorted_events.into_iter() {
                let ellapsed_us = (event.timestamp() - prev_ts) / 1_000;
                prev_ts = event.timestamp();
                println!("({}us): {:?}", ellapsed_us, event);
            }
        }
        DisplayMode::ByProcess => {
            println!("EVENTS");
            let mut sorted = proc_events.into_values().collect::<Vec<_>>();
            sorted.sort_by_key(|events| events.first().unwrap().timestamp());
            for events in sorted.iter() {
                for event in events.iter() {
                    println!("{:?}", event);
                }
                println!();
            }
        }
        DisplayMode::Mermaid => {
            print_mermaid_output(user_cmd_pid, proc_events);
        }
    }

    Ok(())
}

fn parse_line(
    line: &str,
    fork_regex: &Regex,
    exec_regex: &Regex,
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

fn handle_event(event: &Event, procs: &mut BTreeMap<i32, Vec<Event>>) {
    match event {
        Event::Fork {
            parent_pid,
            child_pid,
            ..
        } => {
            if procs.contains_key(parent_pid) && !procs.contains_key(child_pid) {
                procs.insert(*child_pid, vec![event.clone()]);
            }
        }
        Event::Exec {
            timestamp,
            pid,
            ppid,
            pgid,
            ..
        } => {
            if procs.contains_key(pid) {
                let cmdline = match Process::new(*pid).and_then(|p| p.cmdline()) {
                    Ok(cmd) => Some(cmd),
                    Err(e) => {
                        eprintln!("failed to get cmd for PID {}: {}", pid, e.to_string());
                        None
                    }
                };
                let event = Event::Exec {
                    timestamp: *timestamp,
                    pid: *pid,
                    ppid: *ppid,
                    pgid: *pgid,
                    cmdline,
                };
                procs.entry(*pid).and_modify(|events| events.push(event));
            }
        }
        Event::Exit { pid, .. } => {
            if procs.contains_key(pid) {
                procs
                    .entry(*pid)
                    .and_modify(|events| events.push(event.clone()));
            }
        }
        Event::SetSID { pid, .. } => {
            if procs.contains_key(pid) {
                procs
                    .entry(*pid)
                    .and_modify(|events| events.push(event.clone()));
            }
        }
        Event::SetPGID { pid, .. } => {
            if procs.contains_key(pid) {
                procs
                    .entry(*pid)
                    .and_modify(|events| events.push(event.clone()));
            }
        }
    }
}

fn print_mermaid_output(root_pid: i32, mut events: BTreeMap<i32, Vec<Event>>) {
    // We inject a timestamp of 0 for the first event (the user's command starting)
    // and that will fuck up the Gantt chart, so we need to patch it. I've arbitrarily
    // chosen the timestamp of the second event.
    let mut sorted = events
        .clone()
        .into_iter()
        .map(|(_pid, proc_events)| proc_events.into_iter())
        .flatten()
        .collect::<Vec<Event>>();
    sorted.sort_by_key(Event::timestamp);
    let second_ts = sorted[1].timestamp();
    events
        .get_mut(&root_pid)
        .unwrap()
        .first_mut()
        .unwrap()
        .set_timestamp(second_ts);

    // There's a bug that catches a bunch of Fork events with no exit right
    // now. I have no idea what those forks are or why they don't show up
    // with an exit.
    events.retain(|_k, v| !matches!(v.last().unwrap(), Event::Fork { .. }));
    let mut buf = String::new();
    buf.push_str("gantt\n");
    buf.push_str("    title Process Trace\n");
    buf.push_str("    dateFormat x\n"); // pretend like our timestamps are seconds
    buf.push_str("    axisFormat %S.%L\n"); // put "seconds" on the x-axis
    buf.push_str("    todayMarker off\n\n"); // time has no meaning
    recurse_children(root_pid, events, &mut buf, second_ts);
    println!("{}", buf);
}

fn recurse_children(
    parent: i32,
    mut events: BTreeMap<i32, Vec<Event>>,
    buf: &mut String,
    initial_time: u128,
) {
    print_spans_for_process(events[&parent].as_slice(), buf, initial_time);
    if let Some(child) = next_child_pid(parent, &events) {
        recurse_children(child, events, buf, initial_time);
    } else {
        events.remove(&parent);
    }
}

fn next_child_pid(parent: i32, events: &BTreeMap<i32, Vec<Event>>) -> Option<i32> {
    let mut pid_starts = events
        .iter()
        .filter(|(pid, _)| **pid != parent)
        .filter_map(|(pid, proc_events)| {
            proc_events.first().and_then(|e| {
                if let Event::Fork {
                    timestamp,
                    parent_pid,
                    ..
                } = e
                {
                    if *parent_pid == parent {
                        Some((*pid, timestamp))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
        })
        .collect::<Vec<_>>();
    pid_starts.sort_by_key(|(_, ts)| **ts);
    pid_starts.first().map(|(pid, _)| *pid).clone()
}

fn print_spans_for_process(proc_events: &[Event], buf: &mut String, initial_time: u128) {
    let default_length_limit = 200;
    let num_execs = num_execs(proc_events);
    if num_execs > 1 {
        let first_exec = proc_events.iter().position(|e| e.is_exec()).unwrap();
        buf.push_str(
            format!(
                "    section {} execs\n",
                exec_command(proc_events.get(first_exec).unwrap(), 10)
            )
            .as_str(),
        );
        if first_exec != 0 {
            // Must have started with a `fork`
            let start = proc_events.get(0).unwrap();
            let stop = proc_events.get(first_exec).unwrap();
            single_exec_span(
                start,
                stop,
                1_000_000,
                initial_time,
                buf,
                default_length_limit,
                None,
            );
        }
        for i in 0..num_execs {
            let idx = i + first_exec;
            let start = proc_events.get(idx).unwrap();
            let stop = proc_events.get(idx + 1).unwrap();
            single_exec_span(
                start,
                stop,
                1_000_000,
                initial_time,
                buf,
                default_length_limit,
                None,
            );
        }
        buf.push_str("    section other\n");
    } else {
        let start = proc_events.first().unwrap();
        let label = if proc_events.get(1).unwrap().is_exec() {
            exec_command(proc_events.get(1).unwrap(), default_length_limit)
        } else {
            "fork".to_string()
        };
        let stop = proc_events.last().unwrap();
        single_exec_span(
            start,
            stop,
            1_000_000,
            initial_time,
            buf,
            default_length_limit,
            Some(label),
        );
    }
}

fn single_exec_span(
    start: &Event,
    stop: &Event,
    scale: u128,
    initial_time: u128,
    buf: &mut String,
    length_limit: usize,
    label_override: Option<String>,
) {
    let duration = (stop.timestamp() - start.timestamp()) / scale;
    let duration = duration.max(1);
    let shifted_start = (start.timestamp() - initial_time) / scale;
    let label = if let Some(label) = label_override {
        label
    } else if start.is_fork() {
        "fork".to_string()
    } else {
        exec_command(start, length_limit)
    };
    buf.push_str(format!("    {} :active, {}, {}ms\n", label, shifted_start, duration).as_str());
}

fn num_execs(events: &[Event]) -> usize {
    events.iter().filter(|e| e.is_exec()).count()
}

fn exec_command(event: &Event, limit: usize) -> String {
    let regex = Regex::new(r"\/nix\/store\/.*\/bin\/").unwrap();
    let Event::Exec { ref cmdline, .. } = event else {
        unreachable!("we reached it");
    };
    cmdline
        .clone()
        .map(|cmds| {
            let joined = cmds.join(" ");
            let denixified = regex.replace_all(&joined, "<store>/");
            if denixified.len() > limit {
                printable_cmd(&cmds[0])
            } else {
                denixified.to_string()
            }
        })
        .unwrap_or("proc".to_string())
}

// Store paths and long argument lists don't work so well
fn printable_cmd(cmd: &str) -> String {
    let path = Path::new(cmd);
    path.file_name().unwrap().to_string_lossy().to_string()
}
