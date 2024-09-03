---
date: ingest
section: 1
title: proctrace-ingest
---

# NAME

proctrace-ingest - Convert a raw recording into a processed recording
such that it is ready for rendering

# SYNOPSIS

**proctrace ingest** \<**-i**\|**\--input**\> \[**-o**\|**\--output**\]
\<**-p**\|**\--root-pid**\> \[**-d**\|**\--debug**\]
\[**-h**\|**\--help**\]

# DESCRIPTION

Convert a raw recording into a processed recording such that it is ready
for rendering.

A recording produced in \"raw\" mode cannot be rendered directly, so it
must first be processed into a render-ready form. This subcommand does
that processing.

# OPTIONS

**-i**, **\--input**=*INPUT_PATH*

:   The path to the raw recording to be processed.

    Must either be a path to a file or - to read from stdin.

**-o**, **\--output**=*PATH*

:   Where to write the output (printed to stdout if omitted).

**-p**, **\--root-pid**=*PID*

:   Which PID to use as the root of the process tree.

    A raw recording contains events from the entire system, so the user
    must supply a PID from which to begin tracing a process tree.

**-d**, **\--debug**

:   Whether to display debug output while ingesting

**-h**, **\--help**

:   Print help (see a summary with -h)
