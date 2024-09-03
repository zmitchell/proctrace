---
date: sort
section: 1
title: proctrace-sort
---

# NAME

proctrace-sort - Sort the output from a recording

# SYNOPSIS

**proctrace sort** \<**-i**\|**\--input**\> \[**-o**\|**\--output**\]
\[**-h**\|**\--help**\]

# DESCRIPTION

Sort the output from a recording.

The events persisted in a recording may not arrive in timestamp order.
This command reads the events in a recording and sorts them by
timestamp. You dont need to do this yourself unless you want to look at
the raw recording data, the \`render\` command will automatically sort
the events before rendering the output.

# OPTIONS

**-i**, **\--input**=*INPUT_PATH*

:   The path to the recording to be sorted.

    Must either be a path to a file or - to read from stdin.

**-o**, **\--output**=*PATH*

:   Where to write the output (printed to stdout if omitted).

**-h**, **\--help**

:   Print help (see a summary with -h)
