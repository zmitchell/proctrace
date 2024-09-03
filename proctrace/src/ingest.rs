use std::{
    collections::HashSet,
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
    exec_args: Regex,
    exit: Regex,
    setsid: Regex,
    setpgid: Regex,
}

impl EventParser {
    pub fn new() -> Self {
        let fork_regex = Regex::new(
        r"FORK: ts=(?<ts>\d+),parent_pid=(?<ppid>[\-\d]+),child_pid=(?<cpid>[\-\d]+),parent_pgid=(?<pgid>[\-\d]+)",
    ).unwrap();
        let exec_regex = Regex::new(
            r"EXEC: ts=(?<ts>\d+),pid=(?<pid>[\-\d]+),ppid=(?<ppid>[\-\d]+),pgid=(?<pgid>[\-\d]+)",
        )
        .unwrap();
        let exec_args_regex =
            Regex::new(r"EXEC_ARGS: ts=(?<ts>\d+),pid=(?<pid>[\-\d]+),(?<exec_args>.*)").unwrap();
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
        Self {
            fork: fork_regex,
            exec: exec_regex,
            exec_args: exec_args_regex,
            exit: exit_regex,
            setsid: setsid_regex,
            setpgid: setpgid_regex,
        }
    }

    pub fn parse_line(&self, line: impl AsRef<str>) -> Result<Event, Error> {
        let line = line.as_ref();
        if let Some(caps) = self.fork.captures(line) {
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
        } else if let Some(caps) = self.exec.captures(line) {
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
        } else if let Some(caps) = self.exec_args.captures(line) {
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
                args: ExecArgsKind::Joined(args.parse().context("failed to parse exec args")?),
            };
            Ok(event)
        } else if let Some(caps) = self.exit.captures(line) {
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
        } else if let Some(caps) = self.setsid.captures(line) {
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
        } else if let Some(caps) = self.setpgid.captures(line) {
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
}

#[derive(Debug)]
pub struct EventIngester<T> {
    /// The PID that will be the root of the process tree.
    root_pid: Option<i32>,
    /// Event store for events that are part of the process tree.
    events: EventStore,
    /// Events that we are unsure about being part of the process tree.
    buffered: EventStore,
    /// The writer for events and raw output.
    pub(crate) writer: Option<T>,
    /// Whether the writer should write processed events or only raw output
    /// on demand.
    raw: bool,
}

impl<T> EventIngester<T> {
    /// Returns a reference to the store of tracked events.
    pub fn tracked_events(&self) -> &EventStore {
        &self.events
    }

    /// Consumes the ingester and returns the store of tracked events.
    pub fn into_tracked_events(self) -> EventStore {
        self.events
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
        !self.events.is_empty()
    }

    /// Adds the event to the backlog of outstanding events that we've seen and
    /// might want to keep.
    fn buffer_event(&mut self, event: &Event) {
        self.buffered.add(event.pid(), event);
    }

    /// Adds the event to the tracked process tree.
    fn store_event(&mut self, event: &Event) {
        self.events.add(event.pid(), event);
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn prepare_for_rendering(&mut self) {
        self.events.collapse_execs();
    }
}

impl<T: EventWrite> EventIngester<T> {
    /// Create a new ingester.
    ///
    /// If initialized without a `root_pid` it will buffer events until one is set.
    /// If initialized with a writer, events will be written to it as they are identified
    /// to be part of the process tree rooted at `root_pid`.
    pub fn new(root_pid: Option<i32>, writer: Option<T>, raw: bool) -> Self {
        Self {
            root_pid,
            events: EventStore::new(),
            buffered: EventStore::new(),
            writer,
            raw,
        }
    }

    /// Write a line of raw output from the script.
    #[allow(dead_code)]
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
        let currently_tracked = self.events.currently_tracked();
        let currently_buffered = self.buffered.currently_tracked();

        let mut pids_to_unbuffer = HashSet::new();
        for pid in currently_buffered.iter() {
            if let Some(parent_pid) = self.buffered.parent(*pid) {
                if currently_tracked.contains(&parent_pid) {
                    pids_to_unbuffer.insert(pid);
                }
            } else if currently_tracked.contains(pid) {
                pids_to_unbuffer.insert(pid);
            }
        }

        // Iteratively grab any PIDs that are children of other PIDs in the buffer that we've decided
        // we can remove. Do this until there are no PIDs that can be removed.
        loop {
            let more_to_unbuffer = {
                let mut more = HashSet::new();
                for pid in currently_buffered.iter() {
                    if pids_to_unbuffer.contains(pid) {
                        // We've already recorded this PID
                        continue;
                    }
                    // If the parent is already tracked or has been recorded, record the child.
                    if let Some(parent_pid) = self.buffered.parent(*pid) {
                        if currently_tracked.contains(&parent_pid)
                            || pids_to_unbuffer.contains(&parent_pid)
                        {
                            more.insert(pid);
                        }
                    } else if currently_tracked.contains(pid) {
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
            drained_events.push((
                *pid,
                self.buffered
                    .remove(**pid)
                    .ok_or(anyhow!("buffered PID {pid} not found"))?,
            ));
        }
        drained_events.sort_by_key(|(_, events)| {
            events
                .front()
                .expect("expected events but found none")
                .timestamp()
        });
        // Track this pid from now on
        for (pid, events) in drained_events.iter() {
            self.events.add_many(**pid, events.iter());
        }
        // Write out the previously buffered events
        for (_pid, events) in drained_events.drain(..) {
            if !self.raw {
                self.maybe_write_events(events.iter())?;
            }
        }

        Ok(())
    }

    fn maybe_write_event(&mut self, event: &Event) -> Result<(), Error> {
        if let Some(ref mut writer) = self.writer {
            writer.write_event(event)
        } else {
            Ok(())
        }
    }

    fn maybe_write_events<'a>(
        &mut self,
        events: impl Iterator<Item = &'a Event>,
    ) -> Result<(), Error> {
        if let Some(ref mut writer) = self.writer {
            for event in events {
                writer.write_event(event)?;
            }
        }
        Ok(())
    }

    pub fn observe_event(&mut self, event: &Event) -> Result<(), Error> {
        if self.events.pid_is_tracked(event.pid()) {
            // We're already tracking this PID, so just store the latest event
            self.store_event(event);
            if !self.raw {
                self.maybe_write_event(event)?;
            }
        } else if self.is_initial_fork(event).unwrap_or(false) {
            // We aren't tracking any PIDs yet, and this will be the first
            self.store_event(event);
            if !self.raw {
                self.maybe_write_event(event)?;
            }
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

pub fn ingest_raw<W: EventWrite>(
    debug: bool,
    root_pid: i32,
    input: impl Read,
    writer: W,
) -> Result<EventIngester<W>, Error> {
    let reader = BufReader::new(input);
    let event_parser = EventParser::new();
    let mut ingester = EventIngester::new(Some(root_pid), Some(writer), false);

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
        protos: &[(&str, i32, i32)],
    ) -> Vec<Event> {
        let mut events = vec![];
        let mut timestamp = initial_timestamp;
        for (name, pid, ppid) in protos {
            match *name {
                "fork" => {
                    let event = Event::Fork {
                        timestamp,
                        parent_pid: *ppid,
                        child_pid: *pid,
                        parent_pgid: *ppid,
                    };
                    timestamp += 1;
                    events.push(event);
                }
                "exec" => {
                    let event = Event::Exec {
                        timestamp,
                        pid: *pid,
                        ppid: *ppid,
                        pgid: *pid,
                        cmdline: None,
                    };
                    timestamp += 1;
                    events.push(event);
                }
                "exec_args" => {
                    let event = Event::ExecArgs {
                        timestamp,
                        pid: *pid,
                        args: ExecArgsKind::Joined("".to_string()),
                    };
                    timestamp += 1;
                    events.push(event);
                }
                "exit" => {
                    let event = Event::Exit {
                        timestamp,
                        pid: *pid,
                        ppid: *ppid,
                        pgid: *pid,
                    };
                    timestamp += 1;
                    events.push(event);
                }
                "setpgid" => {
                    let event = Event::SetPGID {
                        timestamp,
                        pid: *pid,
                        ppid: *ppid,
                        pgid: *pid,
                    };
                    timestamp += 1;
                    events.push(event);
                }
                "setsid" => {
                    let event = Event::SetSID {
                        timestamp,
                        pid: *pid,
                        ppid: *ppid,
                        pgid: *pid,
                        sid: *pid,
                    };
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
        EventIngester::new(root_pid, Some(writer), false)
    }

    #[test]
    fn it_works() {}

    #[test]
    fn parses_fork_line() {
        let parser = EventParser::new();
        let parsed = parser
            .parse_line("FORK: ts=0,parent_pid=1,child_pid=2,parent_pgid=1")
            .unwrap();
        let expected = Event::Fork {
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
        let parsed = parser.parse_line("EXEC: ts=0,pid=2,ppid=1,pgid=1").unwrap();
        let expected = Event::Exec {
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
        let parsed = parser.parse_line("EXEC_ARGS: ts=0,pid=1,foo").unwrap();
        let expected = Event::ExecArgs {
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
            .parse_line("SETSID: ts=0,pid=1,ppid=0,pgid=1,sid=1")
            .unwrap();
        let expected = Event::SetSID {
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
            .parse_line("SETPGID: ts=0,pid=1,ppid=0,pgid=1")
            .unwrap();
        let expected = Event::SetPGID {
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
        let dummy_events = make_simple_events(0, &[("exec", 1, 0), ("fork", 2, 1), ("fork", 4, 2)]);
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
        let dummy_events =
            make_simple_events(1, &[("exec", root_pid, 0), ("exec_args", root_pid, 0)]);

        let mut ingester = mock_ingester(Some(root_pid));
        for event in dummy_events.iter() {
            ingester.observe_event(event).unwrap();
        }

        // All of the previous events should have been buffered since we haven't seen
        // the root pid yet, which means that no events should have been written yet
        // either
        assert!(ingester.writer.as_ref().unwrap().events.is_empty());
        assert!(!ingester.have_seen_initial_fork());

        let fork = Event::Fork {
            timestamp: 0,
            parent_pid: 0,
            child_pid: root_pid,
            parent_pgid: 0,
        };
        ingester.observe_event(&fork).unwrap();

        // Assert that the written events are in the correct order
        let written_events = ingester.writer.as_ref().unwrap().events.clone();
        assert_eq!(written_events.len(), 3);
        assert!(matches!(written_events[0], Event::Fork { .. }));

        // Assert that the PID is now being tracked
        let root_events = ingester.events.remove(root_pid).unwrap();
        assert_eq!(root_events.len(), 3);
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
            &[
                ("exec", root_pid, 0),
                ("exec", root_pid, 0),
                ("exec", root_pid, 0),
            ],
        );
        let mut ingester = mock_ingester(Some(root_pid));
        // Ensure that the root PID is tracked
        ingester.events.register_root(root_pid);

        for event in events.iter() {
            ingester.observe_event(event).unwrap();
        }

        // Assert that all of the events were written
        let written_events = ingester.writer.as_ref().unwrap().events.clone();
        assert_eq!(written_events.len(), 3);

        // Assert that the PID is now being tracked
        let root_events = ingester.events.remove(root_pid).unwrap();
        assert_eq!(root_events.len(), 3);
    }

    #[test]
    fn stores_events_from_initial_fork() {
        let root_pid = 1; // This is the child PID of the fork
        let events = make_simple_events(
            1,
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

        // Assert that the written events are in the correct order
        let written_events = ingester.writer.as_ref().unwrap().events.clone();
        assert_eq!(written_events.len(), 3);
        assert!(matches!(written_events[0], Event::Fork { .. }));

        // Assert that the PID is now being tracked
        let root_events = ingester.events.remove(root_pid).unwrap();
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
            &[
                ("fork", 2, root_pid),
                ("exec", 2, root_pid),
                ("exec_args", 2, root_pid),
            ],
        );
        for event in new_events.iter() {
            ingester.observe_event(event).unwrap();
        }

        let recorded_new_events = ingester.events.remove(2).unwrap();
        assert_eq!(recorded_new_events.len(), 3);
    }
}
