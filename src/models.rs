use std::collections::BTreeMap;

pub type ProcEvents = BTreeMap<i32, Vec<Event>>;

#[derive(Debug, Clone)]
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
            Event::Exit { timestamp, .. } => *timestamp,
            Event::SetSID { timestamp, .. } => *timestamp,
            Event::SetPGID { timestamp, .. } => *timestamp,
        }
    }

    pub fn set_timestamp(&mut self, new_ts: u128) {
        match self {
            Event::Fork { timestamp, .. } => *timestamp = new_ts,
            Event::Exec { timestamp, .. } => *timestamp = new_ts,
            Event::Exit { timestamp, .. } => *timestamp = new_ts,
            Event::SetSID { timestamp, .. } => *timestamp = new_ts,
            Event::SetPGID { timestamp, .. } => *timestamp = new_ts,
        }
    }

    pub fn is_fork(&self) -> bool {
        matches!(self, Event::Fork { .. })
    }

    pub fn is_exec(&self) -> bool {
        matches!(self, Event::Exec { .. })
    }

    #[allow(dead_code)]
    pub fn is_exit(&self) -> bool {
        matches!(self, Event::Exit { .. })
    }
}
