use std::{
    collections::{HashSet, VecDeque},
    io::{BufRead, BufReader, Read},
};

use crate::{
    models::{Event, EventStore, ExecArgsKind},
    writers::EventWrite,
};
use anyhow::{anyhow, Context};
use regex_lite::Regex;

type Error = anyhow::Error;

#[derive(Debug)]
pub struct EventParser {
    fork: Regex,
    exec: Regex,
    badexec: Regex,
    exec_args: Regex,
    exec_filename: Regex,
    exit: Regex,
    setsid: Regex,
    setpgid: Regex,
}

impl Default for EventParser {
    fn default() -> Self {
        EventParser::new()
    }
}

impl EventParser {
    pub fn new() -> Self {
        let fork_regex = Regex::new(
        r"FORK: seq=(?<seq>\d+),ts=(?<ts>\d+),parent_pid=(?<ppid>[\-\d]+),child_pid=(?<cpid>[\-\d]+),parent_pgid=(?<pgid>[\-\d]+)",
    ).unwrap();
        let exec_regex = Regex::new(
            r"EXEC: seq=(?<seq>\d+),ts=(?<ts>\d+),pid=(?<pid>[\-\d]+),ppid=(?<ppid>[\-\d]+),pgid=(?<pgid>[\-\d]+)",
        )
        .unwrap();
        let badexec_regex =
            Regex::new(r"BADEXEC: seq=(?<seq>\d+),ts=(?<ts>\d+),pid=(?<pid>[\-\d]+)").unwrap();
        let exec_filename_regex = Regex::new(
            r"EXEC_FILENAME: seq=(?<seq>\d+),ts=(?<ts>\d+),pid=(?<pid>[\-\d]+),filename=(?<filename>.*)",
        )
        .unwrap();
        let exec_args_regex = Regex::new(
            r"EXEC_ARGS: seq=(?<seq>\d+),ts=(?<ts>\d+),pid=(?<pid>[\-\d]+),(?<exec_args>.*)",
        )
        .unwrap();
        let exit_regex = Regex::new(
            r"EXIT: seq=(?<seq>\d+),ts=(?<ts>\d+),pid=(?<pid>[\-\d]+),ppid=(?<ppid>[\-\d]+),pgid=(?<pgid>[\-\d]+)",
        )
        .unwrap();
        let setsid_regex = Regex::new(
        r"SETSID: seq=(?<seq>\d+),ts=(?<ts>\d+),pid=(?<pid>[\-\d]+),ppid=(?<ppid>[\-\d]+),pgid=(?<pgid>[\-\d]+),sid=(?<sid>[\-\d]+)",
    ).unwrap();
        let setpgid_regex = Regex::new(
        r"SETPGID: seq=(?<seq>\d+),ts=(?<ts>\d+),pid=(?<pid>[\-\d]+),ppid=(?<ppid>[\-\d]+),pgid=(?<pgid>[\-\d]+)",
    )
    .unwrap();
        Self {
            fork: fork_regex,
            exec: exec_regex,
            badexec: badexec_regex,
            exec_filename: exec_filename_regex,
            exec_args: exec_args_regex,
            exit: exit_regex,
            setsid: setsid_regex,
            setpgid: setpgid_regex,
        }
    }

    pub fn parse_line(&self, line: impl AsRef<str>) -> Result<Event, Error> {
        let line = line.as_ref();
        if let Some(caps) = self.fork.captures(line) {
            let seq = caps
                .name("seq")
                .ok_or(anyhow!("FORK line had no seq: {}", line))?
                .as_str();
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
                seq: seq.parse().context("failed to parse fork seq")?,
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
        } else if let Some(caps) = self.exec.captures(line) {
            let seq = caps
                .name("seq")
                .ok_or(anyhow!("EXEC line had no seq: {}", line))?
                .as_str();
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
                seq: seq.parse().context("failed to parse exec seq")?,
                timestamp: ts.parse().context("failed to parse exec timestamp")?,
                pid: pid.parse().context("failed to parse exec pid")?,
                ppid: ppid.parse().context("failed to parse exec ppid")?,
                pgid: pgid.parse().context("failed to parse exec pgid")?,
                cmdline: None,
            };
            Ok(event)
        } else if let Some(caps) = self.badexec.captures(line) {
            let seq = caps
                .name("seq")
                .ok_or(anyhow!("BADEXEC line had no seq: {}", line))?
                .as_str();
            let ts = caps
                .name("ts")
                .ok_or(anyhow!("BADEXEC line had no timestamp: {}", line))?
                .as_str();
            let pid = caps
                .name("pid")
                .ok_or(anyhow!("BADEXEC line had no pid: {}", line))?
                .as_str();
            let event = Event::BadExec {
                seq: seq.parse().context("failed to parse badexec seq")?,
                timestamp: ts.parse().context("failed to parse badexec timestamp")?,
                pid: pid.parse().context("failed to parse badexec pid")?,
            };
            Ok(event)
        } else if let Some(caps) = self.exec_filename.captures(line) {
            let seq = caps
                .name("seq")
                .ok_or(anyhow!("EXEC_FILENAME line had no seq: {}", line))?
                .as_str();
            let ts = caps
                .name("ts")
                .ok_or(anyhow!("EXEC_FILENAME line had no timestamp: {}", line))?
                .as_str();
            let pid = caps
                .name("pid")
                .ok_or(anyhow!("EXEC_FILENAME line had no pid: {}", line))?
                .as_str();
            let filename = caps
                .name("filename")
                .ok_or(anyhow!("EXEC_FILENAME had no filename: {}", line))?
                .as_str();
            let event = Event::ExecFilename {
                seq: seq.parse().context("failed to parse exec_filename seq")?,
                timestamp: ts.parse().context("failed to parse badexec timestamp")?,
                pid: pid.parse().context("failed to parse badexec pid")?,
                filename: filename.to_string(),
            };
            Ok(event)
        } else if let Some(caps) = self.exec_args.captures(line) {
            let seq = caps
                .name("seq")
                .ok_or(anyhow!("EXEC_ARGS line had no seq: {}", line))?
                .as_str();
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
                seq: seq.parse().context("failed to parse exec seq")?,
                timestamp: ts.parse().context("failed to parse exec timestamp")?,
                pid: pid.parse().context("failed to parse exec pid")?,
                args: ExecArgsKind::Joined(args.parse().context("failed to parse exec args")?),
            };
            Ok(event)
        } else if let Some(caps) = self.exit.captures(line) {
            let seq = caps
                .name("seq")
                .ok_or(anyhow!("EXIT line had no seq: {}", line))?
                .as_str();
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
                seq: seq.parse().context("failed to parse exit seq")?,
                timestamp: ts.parse().context("failed to parse exit timestamp")?,
                pid: pid.parse().context("failed to parse exit pid")?,
                ppid: ppid.parse().context("failed to parse exit ppid")?,
                pgid: pgid.parse().context("failed to parse exit pgid")?,
            };
            Ok(event)
        } else if let Some(caps) = self.setsid.captures(line) {
            let seq = caps
                .name("seq")
                .ok_or(anyhow!("SETSID line had no seq: {}", line))?
                .as_str();
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
                seq: seq.parse().context("failed to parse setsid seq")?,
                timestamp: ts.parse().context("failed to parse setsid timestamp")?,
                pid: pid.parse().context("failed to parse setsid pid")?,
                ppid: ppid.parse().context("failed to parse setsid ppid")?,
                pgid: pgid.parse().context("failed to parse setsid pgid")?,
                sid: sid.parse().context("failed to parse setsid sid")?,
            };
            Ok(event)
        } else if let Some(caps) = self.setpgid.captures(line) {
            let seq = caps
                .name("seq")
                .ok_or(anyhow!("SETPGID line had no seq: {}", line))?
                .as_str();
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
                seq: seq.parse().context("failed to parse setpgid seq")?,
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
}

#[derive(Debug)]
pub struct EventIngester<T> {
    /// The PID that will be the root of the process tree.
    root_pid: Option<i32>,
    /// Event store for events that are part of the process tree.
    tracked_events: EventStore,
    /// Events that we are unsure about being part of the process tree.
    buffered_events: EventStore,
    /// The writer for events and raw output.
    pub(crate) writer: Option<T>,
}

impl<T> EventIngester<T> {
    /// Returns a reference to the store of tracked events.
    pub fn tracked_events(&self) -> &EventStore {
        &self.tracked_events
    }

    /// Consumes the ingester and returns the store of tracked events.
    pub fn into_tracked_events(self) -> EventStore {
        self.tracked_events
    }

    /// Set the root PID after the ingester has been created.
    ///
    /// Returns an error if the root PID has already been set.
    #[allow(dead_code)]
    pub fn set_root_pid(&mut self, pid: i32) -> Result<(), Error> {
        let existing = self.root_pid.take();
        if existing.is_some() {
            Err(anyhow!("tried to set root PID when one existed"))
        } else {
            self.root_pid = Some(pid);
            Ok(())
        }
    }

    /// Returns the configured `root_pid` if one has been set.
    #[allow(dead_code)]
    pub fn root_pid(&self) -> Option<i32> {
        self.root_pid
    }

    /// Returns `Some(true)` if the event is the initial fork of the process at the root
    /// of the process tree or `Some(false)` if it isn't. Returns `None` if the root pid
    /// has not yet been set.
    fn is_initial_fork(&self, event: &Event) -> Option<bool> {
        self.root_pid
            .as_ref()
            .map(|pid| (event.pid() == *pid) && event.is_fork())
    }

    /// Returns `true` if we've seen the initial fork of the process at the root
    /// of the process tree. Returns `false` if the root pid is unset or it's set
    /// and we haven't been the fork.
    #[allow(dead_code)]
    fn have_seen_initial_fork(&self) -> bool {
        !self.tracked_events.is_empty()
    }

    /// Adds the event to the backlog of outstanding events that we've seen and
    /// might want to keep.
    fn buffer_event(&mut self, event: &Event) {
        self.buffered_events.add(event.pid(), event);
    }

    /// Adds the event to the tracked process tree.
    fn store_event(&mut self, event: &Event) {
        self.tracked_events.add(event.pid(), event);
    }

    pub fn is_empty(&self) -> bool {
        self.tracked_events.is_empty()
    }

    pub fn prepare_for_rendering(&mut self) {
        self.tracked_events.collapse_execs();
    }

    pub fn post_process_buffers(&mut self) {
        self.tracked_events.post_process_buffers();
    }
}

impl<T: EventWrite> EventIngester<T> {
    /// Create a new ingester.
    ///
    /// If initialized without a `root_pid` it will buffer events until one is set.
    /// If initialized with a writer, events will be written to it as they are identified
    /// to be part of the process tree rooted at `root_pid`.
    pub fn new(root_pid: Option<i32>, writer: Option<T>) -> Self {
        Self {
            root_pid,
            tracked_events: EventStore::new(),
            buffered_events: EventStore::new(),
            writer,
        }
    }

    /// Write a line of raw output from the script.
    pub fn write_raw(&mut self, line: &str) -> Result<(), Error> {
        if let Some(ref mut writer) = self.writer {
            writer.write_raw(line)?;
        }
        Ok(())
    }

    /// Walk the buffer collecting any new PIDs to track and writing out any buffered
    /// events that belong to new PIDs to track.
    ///
    /// If this ingester has not been configured with a writer, the events will be stored
    /// internally but they won't be written anywhere.
    fn drain_buffer(&mut self) -> Result<(), Error> {
        // Grab any PIDs that are already tracked or that are direct children of PIDs that are already
        // tracked.
        let pids_currently_tracked = self.tracked_events.pids();
        let pids_currently_buffered = self.buffered_events.pids();

        let mut pids_to_unbuffer = HashSet::new();
        for pid in pids_currently_buffered.iter() {
            // Mark this PID to unbuffer if it's the child of a currently tracked PID,
            // or if the PID is already tracked.
            if let Some(parent_pid) = self.buffered_events.parent_of_pid_if_stored(*pid) {
                if pids_currently_tracked.contains(&parent_pid) {
                    pids_to_unbuffer.insert(pid);
                }
            } else if pids_currently_tracked.contains(pid) {
                pids_to_unbuffer.insert(pid);
            }
        }

        // At this point we've marked buffered PIDs that are children of already tracked PIDs,
        // but the buffer may also contain children of those children, etc, so we need to iteratively
        // mark PIDs that form a parent-child relationship with other marked PIDs.

        loop {
            let more_to_unbuffer = {
                let mut more = HashSet::new();
                for pid in pids_currently_buffered.iter() {
                    if pids_to_unbuffer.contains(pid) {
                        // We've already recorded this PID
                        continue;
                    }
                    // If the parent is already tracked or has been recorded, record the child.
                    if let Some(parent_pid) = self.buffered_events.parent_of_pid_if_stored(*pid) {
                        if pids_currently_tracked.contains(&parent_pid)
                            || pids_to_unbuffer.contains(&parent_pid)
                        {
                            more.insert(pid);
                        }
                    } else if pids_currently_tracked.contains(pid) {
                        more.insert(pid);
                    }
                }
                more
            };
            if more_to_unbuffer.is_empty() {
                break;
            } else {
                for pid in more_to_unbuffer.iter() {
                    pids_to_unbuffer.insert(*pid);
                }
            }
        }

        // Now that we know which PIDs to drain from the store, remove those individual
        // event buffers so we can write out their events.
        let mut drained_events = vec![];
        for pid in pids_to_unbuffer.iter() {
            let buffer = self
                .buffered_events
                .remove(**pid)
                .ok_or(anyhow!("buffered PID {pid} not found"))?;
            drained_events.push((*pid, buffer));
        }
        drained_events.sort_by_key(|(_, events)| {
            events
                .front()
                .expect("expected events but found none")
                .timestamp()
        });
        // Track this pid from now on
        for (pid, events) in drained_events.iter() {
            self.tracked_events.add_many(**pid, events.iter());
        }

        Ok(())
    }

    pub fn observe_event(&mut self, event: &Event) -> Result<(), Error> {
        if self.tracked_events.pid_is_tracked(event.pid()) {
            // We're already tracking this PID, so just store the latest event
            self.store_event(event);
        } else if self.is_initial_fork(event).unwrap_or(false) {
            // We aren't tracking any PIDs yet, and this will be the first
            self.store_event(event);
        } else {
            // We can't tell if we need this event yet, so buffer it and maybe
            // it will get drained later.
            // TODO: decide on a garbage collection scheme for these events
            self.buffer_event(event);
        }
        self.drain_buffer()?;
        Ok(())
    }
}

#[derive(Debug, Default)]
struct ExecState {
    exec_filename: Option<Event>,
    exec_args: Option<Event>,
    exec: Option<Event>,
}

impl std::fmt::Display for ExecState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let filename = if self.exec_filename.is_some() {
            "some"
        } else {
            "none"
        };
        let args = if self.exec_args.is_some() {
            "some"
        } else {
            "none"
        };
        let exec = if self.exec.is_some() { "some" } else { "none" };
        write!(f, "ExecState(filename:{filename},args:{args},exec:{exec})")
    }
}

impl ExecState {
    fn is_empty(&self) -> bool {
        self.exec_filename.is_none() && self.exec_args.is_none() && self.exec.is_none()
    }

    fn is_full(&self) -> bool {
        self.exec_filename.is_some() && self.exec_args.is_some() && self.exec.is_some()
    }

    fn clear(&mut self) {
        self.exec_filename = None;
        self.exec_args = None;
        self.exec = None;
    }

    fn ready_for_args(&self) -> bool {
        self.exec_filename.is_some() && self.exec_args.is_none() && self.exec.is_none()
    }

    fn ready_for_exec(&self) -> bool {
        self.exec_filename.is_some() && self.exec_args.is_some() && self.exec.is_none()
    }

    fn to_exec_full(&mut self) -> Event {
        let Event::ExecFilename { filename, .. } = self.exec_filename.take().unwrap() else {
            panic!("expected exec_filename event");
        };
        let Event::ExecArgs { args, .. } = self.exec_args.take().unwrap() else {
            panic!("expected exec_args event");
        };
        let Event::Exec {
            seq,
            timestamp,
            pid,
            ppid,
            pgid,
            ..
        } = self.exec.take().unwrap()
        else {
            panic!("expected exec event");
        };
        let event = Event::ExecFull {
            seq,
            timestamp,
            pid,
            ppid,
            pgid,
            filename,
            args,
        };
        self.clear();
        event
    }
}

pub(crate) fn clean_exec_sequences(events: &[Event]) -> VecDeque<Event> {
    let mut cleaned = VecDeque::new();
    let mut state = ExecState::default();
    for event in events.iter() {
        match event {
            Event::ExecFilename { .. } => {
                if state.is_full() {
                    cleaned.push_back(state.to_exec_full());
                } else if !state.is_empty() {
                    state.clear();
                }
                state.exec_filename = Some(event.clone());
            }
            Event::ExecArgs { .. } => {
                if state.ready_for_args() {
                    state.exec_args = Some(event.clone());
                } else {
                    state.clear();
                }
            }
            Event::Exec { .. } => {
                if state.ready_for_exec() {
                    state.exec = Some(event.clone());
                }
                if state.is_full() {
                    cleaned.push_back(state.to_exec_full());
                }
            }
            Event::BadExec { .. } => {
                state.clear();
            }
            _ => {
                if state.is_full() {
                    cleaned.push_back(state.to_exec_full());
                }
                cleaned.push_back(event.clone());
            }
        }
    }
    cleaned
}

pub fn ingest_raw<W: EventWrite>(
    debug: bool,
    root_pid: i32,
    input: impl Read,
    writer: W,
) -> Result<EventIngester<W>, Error> {
    let reader = BufReader::new(input);
    let event_parser = EventParser::new();
    let mut ingester = EventIngester::new(Some(root_pid), Some(writer));

    for line in reader.lines() {
        if line.is_err() {
            if debug {
                eprintln!("failed to parse line");
            }
            continue;
        }
        let line = line.unwrap();
        match event_parser.parse_line(&line) {
            Ok(event) => {
                ingester
                    .observe_event(&event)
                    .context("failed to ingest event")?;
            }
            Err(err) => {
                eprintln!("{}", err);
            }
        }

        let unfinished = ingester
            .tracked_events()
            .unfinished_pids()
            .collect::<Vec<_>>();

        // Print the outstanding processes in debug mode
        if debug {
            let list = unfinished
                .iter()
                .map(|pid| format!("{pid}"))
                .collect::<Vec<_>>()
                .join(", ");
            eprintln!("[UNFINISHED]: {}", list);
        }

        // Break if all the processes we're tracking are done, but don't get
        // fooled by the beginning of execution where the ingester will be
        // empty as well.
        if unfinished.is_empty() && !ingester.is_empty() {
            break;
        }
    }

    ingester.post_process_buffers();

    Ok(ingester)
}

// Bugs
// - Doesn't seem to be tracking forks properly

// ProcEvents ideas
// - parent_for_pid(pid)
// - pid_is_finished(pid)

#[cfg(test)]
pub(crate) mod test {
    use crate::writers::test::MockWriter;

    use super::*;

    /// Make a sequence of events from a shorthand:
    /// ("<lowercase event name>,<pid>,<parent_pid>")
    pub(crate) fn make_simple_events(
        initial_timestamp: u128,
        initial_seq: u128,
        protos: &[(&str, i32, i32)],
    ) -> Vec<Event> {
        let mut events = vec![];
        let mut timestamp = initial_timestamp;
        let mut seq = initial_seq;
        for (name, pid, ppid) in protos {
            match *name {
                "fork" => {
                    let event = Event::Fork {
                        seq,
                        timestamp,
                        parent_pid: *ppid,
                        child_pid: *pid,
                        parent_pgid: *ppid,
                    };
                    seq += 1;
                    timestamp += 1;
                    events.push(event);
                }
                "exec" => {
                    let event = Event::Exec {
                        seq,
                        timestamp,
                        pid: *pid,
                        ppid: *ppid,
                        pgid: *pid,
                        cmdline: None,
                    };
                    seq += 1;
                    timestamp += 1;
                    events.push(event);
                }
                "exec_args" => {
                    let event = Event::ExecArgs {
                        seq,
                        timestamp,
                        pid: *pid,
                        args: ExecArgsKind::Joined("".to_string()),
                    };
                    seq += 1;
                    timestamp += 1;
                    events.push(event);
                }
                "exec_filename" => {
                    let event = Event::ExecFilename {
                        seq,
                        timestamp,
                        pid: *pid,
                        filename: "/foo/bar".to_string(),
                    };
                    seq += 1;
                    timestamp += 1;
                    events.push(event);
                }
                "badexec" => {
                    let event = Event::BadExec {
                        seq,
                        timestamp,
                        pid: *pid,
                    };
                    seq += 1;
                    timestamp += 1;
                    events.push(event);
                }
                "exit" => {
                    let event = Event::Exit {
                        seq,
                        timestamp,
                        pid: *pid,
                        ppid: *ppid,
                        pgid: *pid,
                    };
                    seq += 1;
                    timestamp += 1;
                    events.push(event);
                }
                "setpgid" => {
                    let event = Event::SetPGID {
                        seq,
                        timestamp,
                        pid: *pid,
                        ppid: *ppid,
                        pgid: *pid,
                    };
                    seq += 1;
                    timestamp += 1;
                    events.push(event);
                }
                "setsid" => {
                    let event = Event::SetSID {
                        seq,
                        timestamp,
                        pid: *pid,
                        ppid: *ppid,
                        pgid: *pid,
                        sid: *pid,
                    };
                    seq += 1;
                    timestamp += 1;
                    events.push(event);
                }
                _ => {}
            }
        }
        events
    }

    /// Returns a new [EventIngester] for use in tests
    fn mock_ingester(root_pid: Option<i32>) -> EventIngester<MockWriter> {
        let writer = MockWriter::new();
        EventIngester::new(root_pid, Some(writer))
    }

    #[test]
    fn it_works() {}

    #[test]
    fn parses_fork_line() {
        let parser = EventParser::new();
        let parsed = parser
            .parse_line("FORK: seq=0,ts=0,parent_pid=1,child_pid=2,parent_pgid=1")
            .unwrap();
        let expected = Event::Fork {
            seq: 0,
            timestamp: 0,
            parent_pid: 1,
            child_pid: 2,
            parent_pgid: 1,
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_exec_line() {
        let parser = EventParser::new();
        let parsed = parser
            .parse_line("EXEC: seq=0,ts=0,pid=2,ppid=1,pgid=1")
            .unwrap();
        let expected = Event::Exec {
            seq: 0,
            timestamp: 0,
            pid: 2,
            ppid: 1,
            pgid: 1,
            cmdline: None,
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_exec_args_line() {
        let parser = EventParser::new();
        let parsed = parser
            .parse_line("EXEC_ARGS: seq=0,ts=0,pid=1,foo")
            .unwrap();
        let expected = Event::ExecArgs {
            seq: 0,
            timestamp: 0,
            pid: 1,
            args: ExecArgsKind::Joined("foo".to_string()),
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_setsid_line() {
        let parser = EventParser::new();
        let parsed = parser
            .parse_line("SETSID: seq=0,ts=0,pid=1,ppid=0,pgid=1,sid=1")
            .unwrap();
        let expected = Event::SetSID {
            seq: 0,
            timestamp: 0,
            pid: 1,
            pgid: 1,
            sid: 1,
            ppid: 0,
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_setpgid_line() {
        let parser = EventParser::new();
        let parsed = parser
            .parse_line("SETPGID: seq=0,ts=0,pid=1,ppid=0,pgid=1")
            .unwrap();
        let expected = Event::SetPGID {
            seq: 0,
            timestamp: 0,
            pid: 1,
            ppid: 0,
            pgid: 1,
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn detects_initial_fork() {
        let root_pid = 3;
        // The event that _would_ be detected as the first fork is:
        // ("fork", <root_pid>, <anything>)
        let dummy_events =
            make_simple_events(0, 0, &[("exec", 1, 0), ("fork", 2, 1), ("fork", 4, 2)]);
        let mut ingester = mock_ingester(Some(root_pid));
        for event in dummy_events.iter() {
            ingester.observe_event(event).unwrap();
        }

        // All of the previous events should have been buffered since we haven't seen
        // the root pid yet, which means that no events should have been written yet
        // either
        assert!(ingester.writer.as_ref().unwrap().events.is_empty());
        assert!(!ingester.have_seen_initial_fork());

        assert!(ingester
            .is_initial_fork(&Event::Fork {
                seq: 1,
                timestamp: 10,
                parent_pid: 2,
                child_pid: root_pid,
                parent_pgid: 2,
            })
            .unwrap())
    }

    #[test]
    fn drains_buffered_events_from_initial_fork() {
        let root_pid = 1; // This is the child PID of the fork
        let dummy_events = make_simple_events(
            1,
            1,
            &[
                ("exec_filename", root_pid, 0),
                ("exec_args", root_pid, 0),
                ("exec", root_pid, 0),
            ],
        );

        let mut ingester = mock_ingester(Some(root_pid));
        for event in dummy_events.iter() {
            ingester.observe_event(event).unwrap();
        }

        // All of the previous events should have been buffered since we haven't seen
        // the root pid yet, which means that no events should have been written yet
        // either
        assert!(!ingester.have_seen_initial_fork());

        let fork = Event::Fork {
            seq: 0,
            timestamp: 0,
            parent_pid: 0,
            child_pid: root_pid,
            parent_pgid: 0,
        };
        ingester.observe_event(&fork).unwrap();

        // Assert that the PID is now being tracked
        let root_events = ingester.tracked_events.remove(root_pid).unwrap();
        assert_eq!(root_events.len(), 4);
        assert!(matches!(
            root_events.front().unwrap(),
            Event::Fork { child_pid: 1, .. }
        ));
    }

    #[test]
    fn stores_events_from_tracked_pid() {
        let root_pid = 1;
        let events = make_simple_events(
            1,
            0,
            &[
                ("setsid", root_pid, 0),
                ("setsid", root_pid, 0),
                ("setsid", root_pid, 0),
            ],
        );
        let mut ingester = mock_ingester(Some(root_pid));
        // Ensure that the root PID is tracked
        ingester.tracked_events.register_root(root_pid);

        for event in events.iter() {
            ingester.observe_event(event).unwrap();
        }

        // Assert that the PID is now being tracked
        let root_events = ingester.tracked_events.remove(root_pid).unwrap();
        assert_eq!(root_events.len(), 3);
    }

    #[test]
    fn stores_events_from_initial_fork() {
        let root_pid = 1; // This is the child PID of the fork
        let events = make_simple_events(
            1,
            1,
            &[
                ("fork", root_pid, 0),
                ("setsid", root_pid, 0),
                ("setsid", root_pid, 0),
            ],
        );

        let mut ingester = mock_ingester(Some(root_pid));
        for event in events.iter() {
            ingester.observe_event(event).unwrap();
        }

        // Assert that the PID is now being tracked
        let root_events = ingester.tracked_events.remove(root_pid).unwrap();
        assert_eq!(root_events.len(), 3);
        assert!(matches!(
            root_events.front().unwrap(),
            Event::Fork { child_pid: 1, .. }
        ));
    }

    #[test]
    fn follows_new_forks() {
        let root_pid = 1;
        let events = make_simple_events(
            0,
            0,
            &[
                ("fork", root_pid, 0),
                ("exec", root_pid, 0),
                ("exec_args", root_pid, 0),
            ],
        );

        let mut ingester = mock_ingester(Some(root_pid));
        for event in events.iter() {
            ingester.observe_event(event).unwrap();
        }

        let new_events = make_simple_events(
            3,
            3,
            &[
                ("fork", 2, root_pid),
                ("exec", 2, root_pid),
                ("exec_args", 2, root_pid),
            ],
        );
        for event in new_events.iter() {
            ingester.observe_event(event).unwrap();
        }

        let recorded_new_events = ingester.tracked_events.remove(2).unwrap();
        assert_eq!(recorded_new_events.len(), 3);
    }

    #[test]
    fn cleans_simple_exec_seq() {
        let ppid = 1;
        let pid = 2;
        let events = make_simple_events(
            1,
            1,
            &[
                ("exec_filename", pid, ppid),
                ("exec_args", pid, ppid),
                ("exec", pid, ppid),
            ],
        );
        let mut cleaned = clean_exec_sequences(&events);
        assert_eq!(cleaned.len(), 1);
        assert!(matches!(
            cleaned.pop_front().unwrap(),
            Event::ExecFull { .. }
        ));
    }

    #[test]
    fn cleans_prefixed_exec_seq() {
        let ppid = 1;
        let pid = 2;
        let events = make_simple_events(
            1,
            1,
            &[
                ("fork", pid, ppid),
                ("exec_filename", pid, ppid),
                ("exec_args", pid, ppid),
                ("exec", pid, ppid),
            ],
        );
        let mut cleaned = clean_exec_sequences(&events);
        assert_eq!(cleaned.len(), 2);
        assert!(matches!(cleaned.pop_front().unwrap(), Event::Fork { .. }));
        assert!(matches!(
            cleaned.pop_front().unwrap(),
            Event::ExecFull { .. }
        ));
    }

    #[test]
    fn cleans_bad_execs() {
        let ppid = 1;
        let pid = 2;
        let events = make_simple_events(
            1,
            1,
            &[
                ("fork", pid, ppid),
                ("exec_filename", pid, ppid),
                ("exec_args", pid, ppid),
                ("badexec", pid, ppid),
            ],
        );
        let mut cleaned = clean_exec_sequences(&events);
        assert_eq!(cleaned.len(), 1);
        assert!(matches!(cleaned.pop_front().unwrap(), Event::Fork { .. }));
    }

    #[test]
    fn resumes_exec_state() {
        let ppid = 1;
        let pid = 2;
        let events = make_simple_events(
            1,
            1,
            &[
                ("exec_filename", pid, ppid),
                ("exec_args", pid, ppid),
                ("fork", pid, ppid),
                ("exec", pid, ppid),
            ],
        );
        let mut cleaned = clean_exec_sequences(&events);
        assert_eq!(cleaned.len(), 2);
        assert!(matches!(cleaned.pop_front().unwrap(), Event::Fork { .. }));
        assert!(matches!(
            cleaned.pop_front().unwrap(),
            Event::ExecFull { .. }
        ));
    }
}
