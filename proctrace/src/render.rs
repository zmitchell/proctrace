use std::{
    collections::VecDeque,
    io::{Read, Write},
};

use anyhow::{anyhow, Context};
use regex_lite::Regex;
use serde_json::Deserializer;

use crate::{
    cli::DisplayMode,
    ingest::EventIngester,
    models::{Event, ExecArgsKind},
    writers::NoOpWriter,
};

type Error = anyhow::Error;

pub fn render(reader: impl Read, writer: impl Write, mode: DisplayMode) -> Result<(), Error> {
    let ingester = read_events(reader).context("failed to read events from input")?;
    render_events(ingester, writer, mode)
}

pub fn read_events(reader: impl Read) -> Result<EventIngester<NoOpWriter>, Error> {
    let mut de = Deserializer::from_reader(reader).into_iter::<Event>();
    let first_event = match de.next() {
        Some(Ok(event)) => event,
        Some(Err(err)) => return Err(err.into()),
        None => return Err(anyhow!("input was empty")),
    };
    let Event::Fork { ref child_pid, .. } = first_event else {
        return Err(anyhow!("first event was not a fork"));
    };
    let mut ingester: EventIngester<NoOpWriter> = EventIngester::new(Some(*child_pid), None, false);
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

/// Render ingested events.
pub fn render_events<T>(
    mut ingester: EventIngester<T>,
    writer: impl Write,
    mode: DisplayMode,
) -> Result<(), Error> {
    ingester.prepare_for_rendering();
    match mode {
        DisplayMode::Sequential => render_sequential(ingester, writer),
        DisplayMode::ByProcess => render_by_process(ingester, writer),
        DisplayMode::Mermaid => render_mermaid(ingester, writer),
    }
}

fn render_sequential<T>(ingester: EventIngester<T>, mut writer: impl Write) -> Result<(), Error> {
    for event in ingester.into_tracked_events().events_ordered() {
        serde_json::to_writer(&mut writer, &event).context("failed to write event")?;
        writer.write(b"\n").context("write failed")?;
    }
    Ok(())
}

fn render_by_process<T>(ingester: EventIngester<T>, mut writer: impl Write) -> Result<(), Error> {
    for (pid, buffer) in ingester.into_tracked_events().pid_buffers_ordered() {
        let header = extract_displayable_buffer_header(pid, &buffer)
            .context("failed to extract header for PID {pid}")?;
        writer
            .write_all(header.as_bytes())
            .context("write failed")?;
        writer.write(b"\n").context("write failed")?;
        for event in buffer.iter() {
            serde_json::to_writer(&mut writer, event).context("failed to write event")?;
            writer.write(b"\n").context("write failed")?;
        }
        writer.write(b"\n").context("write failed")?;
    }
    Ok(())
}

/// Try to exact some kind of displayable title for the events contained in the buffer.
fn extract_displayable_buffer_header(pid: i32, events: &VecDeque<Event>) -> Result<String, Error> {
    let n_events = events.len();
    if n_events == 0 {
        Err(anyhow!("buffer had no events"))
    } else if n_events == 1 {
        if let Event::Fork {
            parent_pid,
            child_pid,
            ..
        } = events[0]
        {
            // A single fork event, display the fork info
            Ok(format!("PID {child_pid}, forked from {parent_pid}"))
        } else if let Event::Exec { ref cmdline, .. } = events[0] {
            // A single exec event, display the exec args
            Ok(format!(
                "PID {pid}: {}",
                cmdline
                    .as_ref()
                    .map(|args| args.to_string())
                    .unwrap_or("<command unavailable>".to_string())
            ))
        } else {
            unreachable!("all event buffers should begin with either fork or exec");
        }
    } else if matches!(events[0], Event::Fork { .. }) && matches!(events[1], Event::Exec { .. }) {
        // A fork and an initial exec, display the exec args
        let Event::Exec { ref cmdline, .. } = events[1] else {
            unreachable!("just checked that this was an exec");
        };
        Ok(format!(
            "PID {pid}: {}",
            cmdline
                .as_ref()
                .map(|args| args.to_string())
                .unwrap_or("<command unavailable>".to_string())
        ))
    } else if matches!(events[0], Event::Fork { .. }) {
        // A fork followed by something other than exec, display the fork info
        let Event::Fork {
            parent_pid,
            child_pid,
            ..
        } = events[0]
        else {
            unreachable!("just checked that this was a fork");
        };
        Ok(format!("PID {child_pid}, forked from {parent_pid}"))
    } else {
        // No idea what happened here
        Ok(format!("PID {pid}"))
    }
}

fn render_mermaid<T>(ingester: EventIngester<T>, mut writer: impl Write) -> Result<(), Error> {
    // Get anything out of the ingester or event store ahead of time because we're about
    // to consume it
    let root_pid = ingester
        .root_pid()
        .ok_or(anyhow!("tried to render without a root PID"))?;
    let initial_time = ingester
        .tracked_events()
        .pid_start_time(root_pid)
        .ok_or(anyhow!("no events tracked for root PID"))?;

    writer
        .write_all("gantt\n".as_bytes())
        .context("write failed")?;
    writer
        .write_all("    title Process Trace\n".as_bytes())
        .context("write failed")?;
    writer
        .write_all("    dateFormat x\n".as_bytes())
        .context("write failed")?; // pretend like our timestamps are seconds
    writer
        .write_all("    axisFormat %S.%L\n".as_bytes())
        .context("write failed")?; // put "seconds" on the x-axis
    writer
        .write_all("    todayMarker off\n\n".as_bytes())
        .context("write failed")?; // time has no meaning

    for (pid, mut buffer) in ingester
        .into_tracked_events()
        .buffers_depth_first_fork_order(root_pid)?
    {
        let item = parse_buffer(buffer.make_contiguous())
            .with_context(|| format!("failed to parse buffer for PID {pid}"))?;
        render_item(&item, &mut writer, initial_time)?;
    }

    Ok(())
}

#[derive(Debug)]
enum MermaidItem {
    Single(Span),
    ExecGroup(Vec<Span>),
}

#[derive(Debug)]
struct Span {
    pub pid: i32,
    pub label: String,
    pub start: u128,
    pub stop: u128,
}

fn parse_buffer(events: &[Event]) -> Result<MermaidItem, Error> {
    if events.is_empty() {
        return Err(anyhow!("tried to parse empty buffer"));
    }
    let exec_indices = events
        .iter()
        .enumerate()
        .filter_map(|(i, event)| if event.is_exec() { Some(i) } else { None })
        .collect::<Vec<_>>();
    if exec_indices.is_empty() {
        extract_fork_span(events)
    } else if exec_indices.len() == 1 {
        extract_single_exec_span(events, exec_indices[0])
    } else {
        extract_multiple_exec_spans(events, &exec_indices)
    }
}

/// Extracts a [RenderItem] from a buffer that doesn't contain any `exec` events.
fn extract_fork_span(events: &[Event]) -> Result<MermaidItem, Error> {
    let start = events
        .first()
        .ok_or(anyhow!("buffer was empty after checking"))?
        .timestamp();
    let pid = events.first().unwrap().pid();
    let stop = events
        .last()
        .ok_or(anyhow!("buffer was empty after checking"))?
        .timestamp();
    let label = format!("[{pid}] <fork>");
    let span = Span {
        pid,
        start,
        stop,
        label,
    };
    Ok(MermaidItem::Single(span))
}

/// Extracts a [RenderItem] from a buffer that contains a single `exec` event.
fn extract_single_exec_span(events: &[Event], exec_index: usize) -> Result<MermaidItem, Error> {
    let start = events
        .first()
        .ok_or(anyhow!("buffer was empty after checking"))?
        .timestamp();
    let pid = events.first().unwrap().pid();
    let stop = events
        .last()
        .ok_or(anyhow!("buffer was empty after checking"))?
        .timestamp();
    let cmdline = events
        .get(exec_index)
        .and_then(|event| match event {
            Event::Exec { cmdline, .. } => Some(cmdline),
            _ => None,
        })
        .ok_or(anyhow!("failed to find exec for span"))?;
    let label = match cmdline {
        Some(args) => match args {
            ExecArgsKind::Joined(args) => format!("[{pid}] {args}"),
            ExecArgsKind::Args(args) => {
                format!("[{pid}] {}", args.join(" "))
            }
        },
        None => "<command unavailable>".to_string(),
    };
    let span = Span {
        pid,
        start,
        stop,
        label,
    };
    Ok(MermaidItem::Single(span))
}

/// Extracts a [RenderItem] from a buffer that contains multiple `exec` events
fn extract_multiple_exec_spans(
    events: &[Event],
    exec_indices: &[usize],
) -> Result<MermaidItem, Error> {
    let mut spans = vec![];
    let mut ranges = vec![];
    let n_execs = exec_indices.len();
    debug_assert!(n_execs > 1, "should only be called with > 1 execs");
    ranges.push(0..exec_indices[1]);
    for (i, idx) in exec_indices.iter().enumerate() {
        if i == 0 {
            continue;
        } else if i == (n_execs - 1) {
            ranges.push(*idx..events.len());
        } else {
            ranges.push(*idx..(exec_indices[i + 1] + 1));
        }
    }
    for (i, range) in ranges.into_iter().enumerate() {
        // The exec indices we have are relative to the entire buffer,
        // we need to offset it so that it's relative to this slice.
        let slice_index = exec_indices[i] - range.start;
        let MermaidItem::Single(span) = extract_single_exec_span(&events[range], slice_index)?
        else {
            unreachable!("single exec span returned more than one span");
        };
        spans.push(span);
    }
    Ok(MermaidItem::ExecGroup(spans))
}

fn render_item(
    item: &MermaidItem,
    mut writer: impl Write,
    initial_time: u128,
) -> Result<(), Error> {
    match item {
        MermaidItem::Single(span) => {
            render_single_span(span, &mut writer, initial_time).context("failed rendering span")?;
        }
        MermaidItem::ExecGroup(spans) => {
            writer
                .write_all(format!("    section {} execs\n", spans[0].pid).as_bytes())
                .context("failed writing exec group header")?;
            for span in spans.iter() {
                render_single_span(span, &mut writer, initial_time)
                    .context("failed rendering span")?;
            }
            writer
                .write_all("    section other\n".as_bytes())
                .context("write failed")?;
        }
    }
    Ok(())
}

fn render_single_span(
    span: &Span,
    mut writer: impl Write,
    initial_time: u128,
) -> Result<(), Error> {
    let start = (span.start - initial_time) / 1_000_000;
    let duration = (span.stop - span.start) / 1_000_000;
    let line = format!(
        "    {} :active, {}, {}ms\n",
        clean_mermaid_label(&span.label),
        start,
        duration.max(1)
    );
    writer.write_all(line.as_bytes()).context("write failed")?;
    Ok(())
}

fn clean_mermaid_label(label: impl AsRef<str>) -> String {
    if std::env::var("PROCTRACE_LONG_NIX_PATHS").is_ok_and(|val| val == "1") {
        label.as_ref().to_string()
    } else {
        let nix_regex = Regex::new(r"\/nix\/store\/.*\/bin\/").unwrap();
        let denixified = nix_regex.replace_all(label.as_ref(), "<store>/");
        denixified.to_string()
    }
}

#[cfg(test)]
mod test {
    use crate::ingest::test::make_simple_events;

    use super::*;

    #[test]
    fn extracts_fork_span() {
        let events = make_simple_events(0, &[("fork", 1, 0), ("exit", 1, 0)]);
        let item = extract_fork_span(&events).unwrap();
        assert!(matches!(item, MermaidItem::Single(_)));
    }

    #[test]
    fn extracts_single_exec_span() {
        let events = make_simple_events(0, &[("fork", 1, 0), ("exec", 1, 0), ("exit", 1, 0)]);
        let item = extract_single_exec_span(&events, 1).unwrap();
        assert!(matches!(item, MermaidItem::Single(_)));
    }

    #[test]
    fn extracts_multiple_exec_spans() {
        let events = make_simple_events(
            0,
            &[
                ("fork", 1, 0),
                ("exec", 1, 0),
                ("exec", 1, 0),
                ("exec", 1, 0),
                ("exit", 1, 0),
            ],
        );
        let item = extract_multiple_exec_spans(&events, &[1, 2, 3]).unwrap();
        assert!(matches!(item, MermaidItem::ExecGroup(_)));
        let MermaidItem::ExecGroup(spans) = item else {
            panic!()
        };
        assert_eq!(spans.len(), 3);
    }
}
