---
date: record
section: 1
title: proctrace-record
---

# NAME

proctrace-record - Record the process lifecycle events from a command

# SYNOPSIS

**proctrace record** \[**-b**\|**\--bpftrace-path**\] \[**\--debug**\]
\[**-r**\|**\--raw**\] \[**-o**\|**\--output**\] \[**-h**\|**\--help**\]
\[*CMD*\]

# DESCRIPTION

Record the process lifecycle events from a command.

Note that this uses \`bpftrace\` under the hood, and will try to run it
with superuser priviledges (but will not run any other commands with
elevated priviledges). Depending on how youve installed \`bpftrace\` it
may not be in the PATH of the superuser. If this is the case then you
can use the \`\--bpftrace-path\` flag to specify it manually. This is
likely the case if youve installed \`bpftrace\` via \`flox\` or \`nix
profile install\`.

# OPTIONS

**-b**, **\--bpftrace-path**=*PATH* \[default: bpftrace\]

:   The path to a \`bpftrace\` executable.

    Since \`bpftrace\` needs to be run as root, its possible that the
    root user may not have \`bpftrace\` in their path. In that case
    youll need to pass in an explicit path. This is the case if youve
    installed \`bpftrace\` via \`flox\` or \`nix profile\`.

**\--debug**

:   Show each line of output from \`bpftrace\` before it goes through
    filtering.

    This also displays which PIDs are being tracked but have not yet
    exited.

**-r**, **\--raw**

:   Write the raw events from the \`bpftrace\` script instead of the
    processed events.

    This will write events from processes outside of the target process
    tree, but recording this raw output allows you to rerun analysis
    without needing to collect another recording.

**-o**, **\--output**=*PATH*

:   Where to write the output (printed to stdout if omitted).

**-h**, **\--help**

:   Print help (see a summary with -h)

\[*CMD*\]

:   The user-provided command that should be recorded.

    Note that this will print to the terminal if it has output.
    \`proctrace\` does its best to not meddle with the environment of
    the command so that it behaves as you expect.
