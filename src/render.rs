use std::{io::Read, path::Path};

use anyhow::{anyhow, Context};
use regex_lite::Regex;
use serde_json::Deserializer;

use crate::{cli::DisplayMode, ingest::EventIngester, models::Event, writers::NoOpWriter};

type Error = anyhow::Error;

pub fn render(reader: impl Read, mode: DisplayMode) -> Result<(), Error> {
    let ingester = read_events(reader).context("failed to read events from input")?;
    render_events(ingester, mode);
    Ok(())
}

pub fn read_events(reader: impl Read) -> Result<EventIngester<NoOpWriter>, Error> {
    let mut de = Deserializer::from_reader(reader).into_iter::<Event>();
    let first_event = match de.next() {
        Some(Ok(event)) => event,
        Some(Err(err)) => return Err(err.into()),
        None => return Err(anyhow!("input was empty")),
    };
    let Event::Exec { ref pid, .. } = first_event else {
        return Err(anyhow!("first event was not an exec"));
    };
    let mut ingester: EventIngester<NoOpWriter> = EventIngester::new(Some(*pid), None);
    ingester.observe_event(&first_event)?;
    for maybe_event in de {
        match maybe_event {
            Ok(event) => {
                ingester.observe_event(&event)?;
            }
            Err(err) => {
                eprintln!("failed to parse event: {err}");
            }
        }
    }
    Ok(ingester)
}

pub fn render_events<T>(ingester: EventIngester<T>, mode: DisplayMode) {
    // match mode {
    //     DisplayMode::Multiplexed => {
    //         let mut sorted_events = proc_events.into_values().flatten().collect::<Vec<_>>();
    //         sorted_events.sort_by_key(|e| e.timestamp());
    //         if sorted_events.len() > 1 {
    //             let next_ts = sorted_events[1].timestamp();
    //             sorted_events.get_mut(0).unwrap().set_timestamp(next_ts);
    //         }
    //         // let mut prev_ts = 0;
    //         for event in sorted_events.into_iter() {
    //             // let ellapsed_us = (event.timestamp() - prev_ts) / 1_000;
    //             // prev_ts = event.timestamp();
    //             let maybe_json = serde_json::to_string(&event);
    //             match maybe_json {
    //                 Ok(json) => println!("{json}"),
    //                 Err(err) => eprintln!("failed to parse event as JSON: {err}"),
    //             }
    //         }
    //     }
    //     DisplayMode::ByProcess => {
    //         println!("EVENTS");
    //         let mut sorted = proc_events.into_values().collect::<Vec<_>>();
    //         sorted.sort_by_key(|events| events.front().unwrap().timestamp());
    //         for events in sorted.iter() {
    //             for event in events.iter() {
    //                 let maybe_json = serde_json::to_string(&event);
    //                 match maybe_json {
    //                     Ok(json) => println!("{json}"),
    //                     Err(err) => eprintln!("failed to parse event as JSON: {err}"),
    //                 }
    //             }
    //             println!();
    //         }
    //     }
    //     DisplayMode::Mermaid => {
    //         print_mermaid_output(user_cmd_pid, proc_events);
    //     }
    // }
}

// fn print_mermaid_output(root_pid: i32, mut events: ProcEvents) {
//     // We inject a timestamp of 0 for the first event (the user's command starting)
//     // and that will fuck up the Gantt chart, so we need to patch it. I've arbitrarily
//     // chosen the timestamp of the second event.
//     let mut sorted = events
//         .clone()
//         .into_values()
//         .flat_map(|proc_events| proc_events.into_iter())
//         .collect::<Vec<Event>>();
//     sorted.sort_by_key(Event::timestamp);
//     let second_ts = sorted[1].timestamp();
//     events
//         .get_mut(&root_pid)
//         .unwrap()
//         .front_mut()
//         .unwrap()
//         .set_timestamp(second_ts);

//     // There's a bug that catches a bunch of Fork events with no exit right
//     // now. I have no idea what those forks are or why they don't show up
//     // with an exit.
//     events.retain(|_k, v| !matches!(v.back().unwrap(), Event::Fork { .. }));
//     let mut buf = String::new();
//     buf.push_str("gantt\n");
//     buf.push_str("    title Process Trace\n");
//     buf.push_str("    dateFormat x\n"); // pretend like our timestamps are seconds
//     buf.push_str("    axisFormat %S.%L\n"); // put "seconds" on the x-axis
//     buf.push_str("    todayMarker off\n\n"); // time has no meaning
//     recurse_children(root_pid, events, &mut buf, second_ts);
//     println!("{}", buf);
// }

// fn recurse_children(parent: i32, mut events: ProcEvents, buf: &mut String, initial_time: u128) {
//     print_spans_for_process(
//         events.get_mut(&parent).unwrap().make_contiguous(),
//         buf,
//         initial_time,
//     );
//     if let Some(child) = next_child_pid(parent, &events) {
//         recurse_children(child, events, buf, initial_time);
//     } else {
//         events.remove(&parent);
//     }
// }

// fn next_child_pid(parent: i32, events: &ProcEvents) -> Option<i32> {
//     let mut pid_starts = events
//         .iter()
//         .filter(|(pid, _)| **pid != parent)
//         .filter_map(|(pid, proc_events)| {
//             proc_events.front().and_then(|e| {
//                 if let Event::Fork {
//                     timestamp,
//                     parent_pid,
//                     ..
//                 } = e
//                 {
//                     if *parent_pid == parent {
//                         Some((*pid, timestamp))
//                     } else {
//                         None
//                     }
//                 } else {
//                     None
//                 }
//             })
//         })
//         .collect::<Vec<_>>();
//     pid_starts.sort_by_key(|(_, ts)| **ts);
//     pid_starts.first().map(|(pid, _)| *pid)
// }

// fn print_spans_for_process(proc_events: &[Event], buf: &mut String, initial_time: u128) {
//     let default_length_limit = 200;
//     let num_execs = num_execs(proc_events);
//     if num_execs > 1 {
//         let first_exec = proc_events.iter().position(|e| e.is_exec()).unwrap();
//         buf.push_str(
//             format!(
//                 "    section {} execs\n",
//                 exec_command(proc_events.get(first_exec).unwrap(), 10)
//             )
//             .as_str(),
//         );
//         if first_exec != 0 {
//             // Must have started with a `fork`
//             let start = proc_events.first().unwrap();
//             let stop = proc_events.get(first_exec).unwrap();
//             single_exec_span(
//                 start,
//                 stop,
//                 1_000_000,
//                 initial_time,
//                 buf,
//                 default_length_limit,
//                 None,
//             );
//         }
//         for i in 0..num_execs {
//             let idx = i + first_exec;
//             let start = proc_events.get(idx).unwrap();
//             let stop = proc_events.get(idx + 1).unwrap();
//             single_exec_span(
//                 start,
//                 stop,
//                 1_000_000,
//                 initial_time,
//                 buf,
//                 default_length_limit,
//                 None,
//             );
//         }
//         buf.push_str("    section other\n");
//     } else {
//         let start = proc_events.first().unwrap();
//         let label = if proc_events.get(1).unwrap().is_exec() {
//             exec_command(proc_events.get(1).unwrap(), default_length_limit)
//         } else {
//             "fork".to_string()
//         };
//         let stop = proc_events.last().unwrap();
//         single_exec_span(
//             start,
//             stop,
//             1_000_000,
//             initial_time,
//             buf,
//             default_length_limit,
//             Some(label),
//         );
//     }
// }

// fn single_exec_span(
//     start: &Event,
//     stop: &Event,
//     scale: u128,
//     initial_time: u128,
//     buf: &mut String,
//     length_limit: usize,
//     label_override: Option<String>,
// ) {
//     let duration = (stop.timestamp() - start.timestamp()) / scale;
//     let duration = duration.max(1);
//     let shifted_start = (start.timestamp() - initial_time) / scale;
//     let label = if let Some(label) = label_override {
//         label
//     } else if start.is_fork() {
//         "fork".to_string()
//     } else {
//         exec_command(start, length_limit)
//     };
//     buf.push_str(format!("    {} :active, {}, {}ms\n", label, shifted_start, duration).as_str());
// }

// fn num_execs(events: &[Event]) -> usize {
//     events.iter().filter(|e| e.is_exec()).count()
// }

// fn exec_command(event: &Event, limit: usize) -> String {
//     let regex = Regex::new(r"\/nix\/store\/.*\/bin\/").unwrap();
//     let Event::Exec { ref cmdline, .. } = event else {
//         unreachable!("we reached it");
//     };
//     cmdline
//         .clone()
//         .map(|cmds| {
//             let joined = cmds.join(" ");
//             let denixified = regex.replace_all(&joined, "<store>/");
//             if denixified.len() > limit {
//                 printable_cmd(&cmds[0])
//             } else {
//                 denixified.to_string()
//             }
//         })
//         .unwrap_or("proc".to_string())
// }

// // Store paths and long argument lists don't work so well
// fn printable_cmd(cmd: &str) -> String {
//     let path = Path::new(cmd);
//     path.file_name().unwrap().to_string_lossy().to_string()
// }
