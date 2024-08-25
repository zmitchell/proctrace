use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

pub type ProcEvents = BTreeMap<i32, VecDeque<Event>>;

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
