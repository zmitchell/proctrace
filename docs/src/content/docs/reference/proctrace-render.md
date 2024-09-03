---
date: render
section: 1
title: proctrace-render
---

# NAME

proctrace-render - Render a recording in the specified display format

# SYNOPSIS

**proctrace render** \[**-d**\|**\--display-mode**\]
\<**-i**\|**\--input**\> \[**-o**\|**\--output**\]
\[**-h**\|**\--help**\]

# DESCRIPTION

Render a recording in the specified display format

# OPTIONS

**-d**, **\--display-mode**=*DISPLAY_MODE* \[default: sequential\]

:   How should the output be rendered.

    For \"sequential\" events will be shown in the order that they were
    received. For \"by-process\" events are shown in order for each
    process, and processes are separated by a blank line. For
    \"mermaid\" the output is the syntax for a Mermaid.js Gantt chart.\

    \
    \[*possible values: *sequential, by-process, mermaid\]

**-i**, **\--input**=*INPUT_PATH*

:   The location where an event recording should be read from.

    Must either be a path to a file or - to read from stdin.

**-o**, **\--output**=*PATH*

:   Where to write the output (printed to stdout if omitted).

**-h**, **\--help**

:   Print help (see a summary with -h)
