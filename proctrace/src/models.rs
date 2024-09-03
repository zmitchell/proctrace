use std::{
    collections::{BTreeMap, HashSet, VecDeque},
    fmt::Display,
};

use serde::{Deserialize, Serialize};

type Error = anyhow::Error;

/// Represents the arguments for an `exec` call.
///
/// Depending on where we get the arguments from, we will either get them as a single
/// string or as an array of strings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExecArgsKind {
    Joined(String),
    Args(Vec<String>),
}

impl Display for ExecArgsKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecArgsKind::Joined(joined) => joined.fmt(f),
            ExecArgsKind::Args(args) => args.join(" ").fmt(f),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum Event {
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
        cmdline: Option<ExecArgsKind>,
    },
    ExecArgs {
        timestamp: u128,
        pid: i32,
        args: ExecArgsKind,
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

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Event {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let timestamp = self.timestamp();
        let other_timestamp = other.timestamp();
        timestamp.cmp(&other_timestamp)
    }
}

impl Event {
    pub fn timestamp(&self) -> u128 {
        match self {
            Event::Fork { timestamp, .. } => *timestamp,
            Event::Exec { timestamp, .. } => *timestamp,
            Event::ExecArgs { timestamp, .. } => *timestamp,
            Event::Exit { timestamp, .. } => *timestamp,
            Event::SetSID { timestamp, .. } => *timestamp,
            Event::SetPGID { timestamp, .. } => *timestamp,
        }
    }

    pub fn pid(&self) -> i32 {
        match self {
            Event::Fork { child_pid, .. } => *child_pid,
            Event::Exec { pid, .. } => *pid,
            Event::ExecArgs { pid, .. } => *pid,
            Event::Exit { pid, .. } => *pid,
            Event::SetSID { pid, .. } => *pid,
            Event::SetPGID { pid, .. } => *pid,
        }
    }

    pub fn is_fork(&self) -> bool {
        matches!(self, Event::Fork { .. })
    }

    pub fn fork_parent(&self) -> Option<i32> {
        if let Event::Fork { parent_pid, .. } = self {
            Some(*parent_pid)
        } else {
            None
        }
    }

    pub fn is_exec(&self) -> bool {
        matches!(self, Event::Exec { .. })
    }

    #[allow(dead_code)]
    pub fn is_exit(&self) -> bool {
        matches!(self, Event::Exit { .. })
    }
}

/// A store for events received while recording or ingesting
/// a trace.
#[derive(Debug, Default)]
pub struct EventStore {
    // TODO: add initialization typestate?
    // We could parameterize this struct with a typestate representing whether the
    // root PID has been initialized or not. Afterwards we could make the `add` method
    // only available on the initialized variant. Not sure if that's worth the effort
    // or if it would just make things more complicated at the call sites in `record`.
    inner: BTreeMap<i32, VecDeque<Event>>,
}

impl EventStore {
    /// Creates a new event store.
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    /// Store a new event for a given PID.
    pub fn add(&mut self, pid: i32, event: &Event) {
        let events = self.inner.entry(pid).or_default();
        // Events are stored in timestamp-sorted order
        let insert_point =
            match events.binary_search_by_key(&event.timestamp(), |event| event.timestamp()) {
                Ok(found_idx) => found_idx + 1,
                Err(candidate_idx) => candidate_idx,
            };
        events.insert(insert_point, event.clone());
    }

    /// Add several events from the same PID.
    pub fn add_many<'a>(&mut self, pid: i32, new_events: impl Iterator<Item = &'a Event>) {
        for event in new_events {
            self.add(pid, event);
        }
    }

    /// Remove and return the buffer of events for this PID.
    pub fn remove(&mut self, pid: i32) -> Option<VecDeque<Event>> {
        self.inner.remove(&pid)
    }

    /// Initializes a PID as the root PID for the store.
    #[allow(dead_code)]
    pub fn register_root(&mut self, pid: i32) {
        eprintln!("root was registered");
        debug_assert!(self.inner.is_empty());
        self.inner.insert(pid, VecDeque::new());
    }

    /// Returns `true` if the provided PID is being tracked by this event store.
    pub fn pid_is_tracked(&self, pid: i32) -> bool {
        self.inner.contains_key(&pid)
    }

    /// Returns an iterator over the PIDs of processes that haven't yet finished.
    #[allow(clippy::needless_lifetimes)]
    pub fn unfinished_pids<'a>(&'a self) -> impl Iterator<Item = i32> + 'a {
        self.inner
            .iter()
            .filter_map(|(pid, events)| match events.back() {
                Some(Event::Exit { .. }) => None,
                Some(event) => Some(event.pid()),
                None => Some(*pid),
            })
    }

    /// Returns `true` if no PIDs have been registered.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns the set of currently tracked PIDs.
    pub fn currently_tracked(&self) -> HashSet<i32> {
        self.inner.keys().cloned().collect::<HashSet<_>>()
    }

    /// Returns the PID of the process that this PID was forked from,
    /// if it's known.
    pub fn parent(&self, child_pid: i32) -> Option<i32> {
        self.inner
            .get(&child_pid)
            .and_then(|events| events.front())
            .and_then(|event| event.fork_parent())
    }

    /// Returns an iterator over stored events in order.
    pub fn events_ordered(self) -> impl Iterator<Item = Event> {
        let mut all_events = self
            .inner
            .into_values()
            .flat_map(|buffer| buffer.into_iter())
            .collect::<Vec<_>>();
        all_events.sort();
        all_events.into_iter()
    }

    /// Returns an iterator over the PID and buffer for each tracked PID
    /// in order of the timestamp of the earliest event for each PID.
    pub fn pid_buffers_ordered(mut self) -> impl Iterator<Item = (i32, VecDeque<Event>)> {
        let mut pid_to_ts = self
            .inner
            .iter()
            .filter_map(|(&pid, buffer)| {
                // It shouldn't be possible for a buffer to be here and be empty,
                // so if we find an empty buffer we just drop it for now.
                // TODO: write some kind of log about the bad PID
                buffer.front().map(|event| (pid, event.timestamp()))
            })
            .collect::<Vec<_>>();
        pid_to_ts.sort_by_key(|(_, ts)| *ts);
        let mut pids_and_buffers = vec![];
        for (pid, _) in pid_to_ts.into_iter() {
            pids_and_buffers.push((pid, self.inner.remove(&pid).unwrap()));
        }
        pids_and_buffers.into_iter()
    }

    /// Returns an iterator over the buffers in depth-first fork order.
    pub fn buffers_depth_first_fork_order(
        mut self,
        root_pid: i32,
    ) -> Result<impl Iterator<Item = (i32, VecDeque<Event>)>, Error> {
        let mut pids_ordered = vec![];
        pids_ordered.push(root_pid);
        pids_ordered.extend_from_slice(&self.find_child_pids(root_pid));
        let pids_and_buffers = pids_ordered
            .into_iter()
            .map(|pid| {
                (
                    pid,
                    self.inner.remove(&pid).expect("failed to remove buffer"),
                )
            })
            .collect::<Vec<_>>();
        Ok(pids_and_buffers.into_iter())
    }

    fn find_child_pids(&self, parent_pid: i32) -> Vec<i32> {
        let mut direct_children = self
            .inner
            .keys()
            .filter(|pid| {
                self.parent(**pid)
                    .is_some_and(|parent| parent == parent_pid)
            })
            .copied()
            .collect::<Vec<_>>();
        direct_children.sort_by_key(|pid| self.pid_start_time(*pid));
        let mut all_children = vec![];
        for pid in direct_children.into_iter() {
            all_children.push(pid);
            all_children.extend_from_slice(&self.find_child_pids(pid));
        }
        all_children
    }

    /// Returns the timestamp of the first even tracked for this PID.
    pub fn pid_start_time(&self, pid: i32) -> Option<u128> {
        self.inner
            .get(&pid)
            .and_then(|buffer| buffer.front())
            .map(|event| event.timestamp())
    }

    /// Fills out the `cmdline` field of all `Exec` events from `ExecArgs` events,
    /// removing the `ExecArgs` events in the process.
    pub(crate) fn collapse_execs(&mut self) {
        let collapsed = BTreeMap::new();
        let original = std::mem::replace(&mut self.inner, collapsed);
        for (pid, buffer) in original.into_iter() {
            let new_buffer = collapse_buffer_execs(buffer.iter());
            self.inner.insert(pid, new_buffer);
        }
    }
}

fn collapse_buffer_execs<'a>(events: impl Iterator<Item = &'a Event>) -> VecDeque<Event> {
    use Event::*;

    let mut buffer = VecDeque::new();
    let mut execs = vec![];
    for event in events {
        match event {
            Exec { .. } => {
                if execs.is_empty() {
                    // Not currently buffering exec events, so start
                    execs.push(event);
                } else {
                    // We're currently buffering exec events and have seen another exec,
                    // so we need to unbuffer the existing events and start buffering again.

                    // Unbuffer the existing events.
                    let exec = fill_in_exec_args(&execs);
                    execs.clear();

                    // Store the exec that was previously buffered.
                    if let Some(exec) = exec {
                        buffer.push_back(exec);
                    }
                    // Start buffering again.
                    execs.push(event);
                }
            }
            ExecArgs { .. } => {
                if !execs.is_empty() && execs[0].is_exec() && (execs[0].pid() == event.pid()) {
                    execs.push(event);
                }
                // If we didn't trigger that branch, then we either got an EXEC_ARGS event without
                // a preceding EXEC event, or we got an EXEC_ARGS event that doesn't match the existing
                // buffered EXEC(_ARGS) events in the buffer. Something must have gone wrong for that
                // to happen, so just drop this event?
            }
            _ => {
                if !execs.is_empty() {
                    // We're currently buffering exec events and have seen a different kind of event,
                    // so we need to unbuffer the existing events.

                    // Unbuffer the existing events.
                    let exec = fill_in_exec_args(&execs);
                    execs.clear();
                    if let Some(exec) = exec {
                        // Store the exec that was previously buffered.
                        buffer.push_back(exec);
                    }

                    buffer.push_back(event.clone());
                } else {
                    buffer.push_back(event.clone());
                }
            }
        }
    }

    // The last few events may have been execs, so we need to unbuffer them before returning.
    if !execs.is_empty() {
        let exec = fill_in_exec_args(&execs);
        if let Some(exec) = exec {
            buffer.push_back(exec);
        }
    }

    buffer
}

/// Try to fill in the `cmdline` field on an `Event::Exec` from `Event::ExecArgs` events.
///
/// Note that because the exec args come from two different sources, sometimes you get more
/// information from one vs. the other. When they don't match we just take the longer of
/// the two since it probably has more information.
fn fill_in_exec_args(execs: &[&Event]) -> Option<Event> {
    use Event::*;

    match execs {
        [] => None,
        [event @ Exec { .. }] => Some((*event).clone()),
        [Exec {
            pid,
            timestamp,
            ppid,
            pgid,
            ..
        }, ExecArgs { args, .. }] => Some(Exec {
            cmdline: Some(args.clone()),
            timestamp: *timestamp,
            pid: *pid,
            ppid: *ppid,
            pgid: *pgid,
        }),
        [Exec {
            pid,
            timestamp,
            ppid,
            pgid,
            ..
        }, ExecArgs { args: args1, .. }, ExecArgs { args: args2, .. }] => {
            let joined1 = args1.to_string();
            let joined2 = args2.to_string();
            let args = if joined1.len() > joined2.len() {
                args1
            } else {
                args2
            };
            Some(Exec {
                pid: *pid,
                ppid: *ppid,
                pgid: *pgid,
                timestamp: *timestamp,
                cmdline: Some(args.clone()),
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod test {
    use crate::ingest::test::make_simple_events;

    use super::*;

    // TODO: this should be a property test at some point
    #[test]
    fn events_inserted_in_order() {
        let events = make_simple_events(
            0,
            &[
                ("fork", 1, 0),
                ("exec", 1, 0),
                ("exec_args", 1, 0),
                ("setpgid", 1, 0),
                ("setsid", 1, 0),
            ],
        );
        let mut shuffled = events.clone();
        shuffled.swap(0, 3);
        shuffled.swap(2, 3);

        let mut store = EventStore::new();
        store.add_many(1, shuffled.iter());

        let stored = store
            .inner
            .remove(&1)
            .unwrap()
            .into_iter()
            .collect::<Vec<_>>();

        assert_eq!(events, stored);
    }

    #[test]
    fn reports_unfinished_pids() {
        let events = make_simple_events(
            0,
            &[
                ("fork", 1, 0),
                ("exec", 1, 0),
                ("fork", 2, 0),
                ("exec", 2, 0),
                ("fork", 3, 0),
                ("exit", 3, 0), // PID will be finished
            ],
        );

        let mut store = EventStore::new();
        for event in events.iter() {
            store.add(event.pid(), event);
        }

        let unfinished = store.unfinished_pids().collect::<Vec<_>>();
        assert_eq!(unfinished, vec![1, 2]);
    }

    #[test]
    fn returns_ordered_events() {
        let events = make_simple_events(
            0,
            &[
                ("fork", 1, 0),
                ("exec", 1, 0),
                ("exec_args", 1, 0),
                ("setpgid", 1, 0),
                ("setsid", 1, 0),
            ],
        );
        let mut shuffled = events.clone();
        shuffled.swap(0, 3);
        shuffled.swap(2, 3);

        let mut store = EventStore::new();
        store.add_many(1, shuffled.iter());

        let stored = store.events_ordered().collect::<Vec<_>>();

        assert_eq!(events, stored);
    }

    #[test]
    fn returns_ordered_buffers() {
        let events = make_simple_events(
            0,
            &[
                // These are all forks because that's what triggers storing
                // events for a PID. These are all created from one another
                // so they won't get buffered by mistake.
                ("fork", 1, 0),
                ("fork", 2, 1),
                ("fork", 3, 2),
                ("fork", 4, 3),
            ],
        );

        let mut store = EventStore::new();
        for event in events.iter() {
            store.add(event.pid(), event);
        }

        let ordered_pids = store
            .pid_buffers_ordered()
            .map(|(pid, _)| pid)
            .collect::<Vec<_>>();
        assert_eq!(ordered_pids, vec![1, 2, 3, 4]);
    }

    #[test]
    fn exec_args_when_no_args_provided() {
        let event = Event::Exec {
            timestamp: 0,
            pid: 1,
            ppid: 0,
            pgid: 1,
            cmdline: Some(ExecArgsKind::Joined("args".to_string())),
        };
        let events = [&event];
        let filled_in = fill_in_exec_args(&events);
        assert!(filled_in.is_some());
        let filled_in = filled_in.unwrap();
        let Event::Exec {
            cmdline: original_args,
            ..
        } = event
        else {
            panic!();
        };
        let Event::Exec {
            cmdline: filled_in_args,
            ..
        } = filled_in
        else {
            panic!();
        };
        assert_eq!(original_args, filled_in_args);
    }

    #[test]
    fn exec_args_filled_in_from_one_event() {
        let exec = Event::Exec {
            timestamp: 0,
            pid: 1,
            ppid: 0,
            pgid: 1,
            cmdline: None,
        };
        let args = ExecArgsKind::Joined("args".to_string());
        let exec_args = Event::ExecArgs {
            timestamp: 1,
            pid: 1,
            args: args.clone(),
        };
        let events = [&exec, &exec_args];
        let filled_in = fill_in_exec_args(&events);
        assert!(filled_in.is_some());
        let filled_in = filled_in.unwrap();
        let Event::Exec {
            cmdline: Some(filled_in_args),
            ..
        } = filled_in
        else {
            panic!();
        };
        assert_eq!(args, filled_in_args);
    }

    #[test]
    fn exec_args_filled_in_from_two_events() {
        let exec = Event::Exec {
            timestamp: 0,
            pid: 1,
            ppid: 0,
            pgid: 1,
            cmdline: None,
        };
        let shorter_args = ExecArgsKind::Joined("args".to_string());
        let longer_args = ExecArgsKind::Joined("longer args".to_string());
        let exec_args1 = Event::ExecArgs {
            timestamp: 1,
            pid: 1,
            args: shorter_args.clone(),
        };
        let exec_args2 = Event::ExecArgs {
            timestamp: 1,
            pid: 1,
            args: longer_args.clone(),
        };
        let events = [&exec, &exec_args1, &exec_args2];
        let filled_in = fill_in_exec_args(&events);
        assert!(filled_in.is_some());
        let filled_in = filled_in.unwrap();
        let Event::Exec {
            cmdline: Some(filled_in_args),
            ..
        } = filled_in
        else {
            panic!();
        };
        assert_eq!(longer_args, filled_in_args);
    }

    #[test]
    fn exec_args_not_filled_from_bad_number_of_events() {
        assert!(fill_in_exec_args(&[]).is_none());

        let exec = Event::Exec {
            timestamp: 0,
            pid: 1,
            ppid: 0,
            pgid: 1,
            cmdline: None,
        };
        assert!(fill_in_exec_args(&[&exec, &exec]).is_none());

        let args = ExecArgsKind::Joined("args".to_string());
        let exec_args = Event::ExecArgs {
            timestamp: 1,
            pid: 1,
            args: args.clone(),
        };
        assert!(fill_in_exec_args(&[&exec, &exec_args, &exec_args, &exec_args]).is_none());
    }

    #[test]
    fn collapses_buffer_execs_at_end() {
        let mut events =
            make_simple_events(0, &[("fork", 1, 0), ("setpgid", 1, 0), ("exec", 1, 0)]);
        events.push(Event::ExecArgs {
            timestamp: 4,
            pid: 1,
            args: ExecArgsKind::Joined("args".to_string()),
        });
        let mut buffer = VecDeque::new();
        for event in events.iter() {
            buffer.push_back(event.clone());
        }

        let collapsed = collapse_buffer_execs(buffer.iter());
        assert_eq!(collapsed.len(), 3);
        assert!(matches!(collapsed.back().unwrap(), Event::Exec { .. }));
    }

    #[test]
    fn collapses_buffer_execs_in_the_middle() {
        let events = make_simple_events(
            0,
            &[
                ("fork", 1, 0),
                ("setpgid", 1, 0),
                ("exec", 1, 0),
                ("exec_args", 1, 0),
                ("exec_args", 1, 0),
                ("setsid", 1, 0),
            ],
        );
        let mut buffer = VecDeque::new();
        for event in events.iter() {
            buffer.push_back(event.clone());
        }

        let collapsed = collapse_buffer_execs(buffer.iter());
        assert_eq!(collapsed.len(), 4); // events.len() - 2 exec_args
        assert!(matches!(collapsed.back().unwrap(), Event::SetSID { .. }));
    }

    #[test]
    fn iterates_fork_order() {
        let events = make_simple_events(
            0,
            &[
                ("fork", 1, 0),
                ("fork", 2, 1),
                ("fork", 3, 2),
                ("fork", 4, 2),
                ("fork", 5, 3),
                ("fork", 6, 1),
            ],
        );
        let mut store = EventStore::new();
        for event in events.iter() {
            store.add(event.pid(), event);
        }
        let ordered = store
            .buffers_depth_first_fork_order(1)
            .unwrap()
            .map(|(pid, _)| pid)
            .collect::<Vec<_>>();
        let expected = vec![1, 2, 3, 5, 4, 6];
        assert_eq!(ordered, expected);
    }
}
