---
title: Getting Started
description: How to get up and running with proctrace
---

`proctrace` isn't packaged anywhere yet, so you must build it from source.
This requires a Rust toolchain.
If you have [Flox](https://flox.dev) installed, then simply clone the repository and you're good:

```
$ git clone https://github.com/zmitchell/proctrace
$ flox activate
$ cargo build
```

If you aren't using Flox, then you'll probably need to install Rust via `rustup`.
You'll also need to ensure that `bpftrace` is installed on your Linux machine
(this is also provided by the Flox environment on Linux).

## Take a recording

If you're on Linux, you'll want to make a recording.
You do this simply by calling `proctrace record` with options for where to store the output
and which command you want to run.
See [`proctrace-record`](../../reference/proctrace-record) for more details.
Note that since `bpftrace` requires super user priviledges, this will prompt for your password.

```
$ proctrace record -o events.log -- <your command>
```

Note that if your `bpftrace` isn't installed in a globally accessible location,
such as if you've installed `bpftrace` via Flox or `nix profile`,
this command will fail.
Use the `-b` flag to specify the path to your `bpftrace` executable.

This `events.log` file will contain newline-delimited JSON parsed from the output of a `bpftrace` script.
For example:
```
{"Fork":{"timestamp":777771839,"parent_pid":415779,"child_pid":415790,"parent_pgid":286785}}
{"Exec":{"timestamp":777873759,"pid":415790,"ppid":415779,"pgid":415790,"cmdline":null}}
{"ExecArgs":{"timestamp":777873759,"pid":415790,"args":"flox activate -- sleep 1"}}
{"ExecArgs":{"timestamp":777873759,"pid":415790,"args":"flox activate -- sleep 1"}}
{"Exec":{"timestamp":778236771,"pid":415790,"ppid":415779,"pgid":415790,"cmdline":null}}
{"ExecArgs":{"timestamp":778236771,"pid":415790,"args":"flox activate -- sleep 1"}}
{"ExecArgs":{"timestamp":778236771,"pid":415790,"args":"flox activate -- sleep 1"}}
{"Fork":{"timestamp":821380607,"parent_pid":415790,"child_pid":415802,"parent_pgid":415779}}
...
```

This file contains only those events that can be identified as part of the same process tree as
the command you supplied.
This allows you to see which processes are spawned as part of your command,
which of them takes the longest, etc.

## Render the recording

Now that you have a recording, you need to render it.
You do that with the [`proctrace-render`](../../reference/proctrace-render) command:

```
$ proctrace render -i events.log
```

The default display mode is `sequential`, which prints the events in the order that they occurred.
This looks very similar to the contents of the `events.log` file with some cleanup performed:
```
{"Fork":{"timestamp":777771839,"parent_pid":415779,"child_pid":415790,"parent_pgid":286785}}
{"Exec":{"timestamp":777873759,"pid":415790,"ppid":415779,"pgid":415790,"cmdline":"flox activate -- sleep 1"}}
{"Exec":{"timestamp":778236771,"pid":415790,"ppid":415779,"pgid":415790,"cmdline":"flox activate -- sleep 1"}}
{"Fork":{"timestamp":821380607,"parent_pid":415790,"child_pid":415802,"parent_pgid":415779}}
...
```

You can specify a different display mode via the `-d` flag.

## Render on a different system

Since the `events.log` file is just text, you can record on a Linux system
and render on a macOS system.
This is useful if you want to say take a recording in CI,
then on a failure upload that recording somewhere that a developer can
investigate offline.
