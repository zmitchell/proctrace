---
title: Raw Recordings
description: How to get more out of your recordings via first recording in raw mode.
---

In [Getting Started](../getting-started) you actually performed two steps in one:
- Collect raw events from `bpftrace`
- Prune the events that weren't part of the process tree started from your command

The `bpftrace` script is started before the user-supplied command so that we can be sure to
catch the initial `fork` that starts the user-supplied command.
This means that `bpftrace` can't know which PIDs to monitor while it's running
(though even if it could it would be painful).
For this reason we use `bpftrace` as a source of raw events, then do book keeping inside `proctrace`.

A "raw" recording contains these raw events without any of the pruning done by `proctrace`,
and you can take one of these raw recordings with the `proctrace record -r` flag.

## Why take raw recordings?

Simply put, a raw recording can be processed in a variety of ways without needing to take another recording.
This is especially useful if the event you're trying to record is uncommon.

Once you have a raw recording you can turn it into a "normal" recording via the
[`proctrace-ingest`](../../reference/proctrace-ingest) command.
Note that this command requires that you supply the PID for the root of the process tree.

```
$ proctrace ingest -i raw.log --root-pid 12345
```

When you take a recording with `proctrace record -r` it will report the root PID since that command
also starts the user-supplied command.

```
$ proctrace record --raw -- <your command>
...
Process tree root was PID 415790
```

Now, one of the neat things you can do with a raw recording is generate output from a _subset_
of the process tree by supplying a different PID to `--root-pid`.
Say you take a recording during an entire run of your CI suite and a test fails.
You have a recording for the entire run, but you're probably only interested in the part of
the process tree involved in the failure.
You could first view the output for the whole tree to see which part failed,
then you could generate output for the part of the process tree rooted on the failure to
take a more detailed look at the failure.

```
$ proctrace ingest -i failure.log -o all.log --root-pid <actual root PID>
$ # do some investigation, find out that PID 12345 is a parent of the failure
$ proctrace ingest -i failure.log -o 12345.log --root-pid 12345
```

## When not to take a raw recording?

Depending on how busy your system is, these files could get...large.
