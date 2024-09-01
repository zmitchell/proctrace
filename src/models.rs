use std::collections::{BTreeMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

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
        cmdline: Option<Vec<String>>,
    },
    ExecArgs {
        timestamp: u128,
        pid: i32,
        args: String,
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

    pub fn set_timestamp(&mut self, new_ts: u128) {
        match self {
            Event::Fork { timestamp, .. } => *timestamp = new_ts,
            Event::Exec { timestamp, .. } => *timestamp = new_ts,
            Event::ExecArgs { timestamp, .. } => *timestamp = new_ts,
            Event::Exit { timestamp, .. } => *timestamp = new_ts,
            Event::SetSID { timestamp, .. } => *timestamp = new_ts,
            Event::SetPGID { timestamp, .. } => *timestamp = new_ts,
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
}
