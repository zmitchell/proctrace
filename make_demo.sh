#!/usr/bin/env bash

set -euo pipefail

bpf_path=$(flox activate -- readlink '$(which bpftrace)')
root_pid="$(target/debug/proctrace record -r -o demo_script_raw.log -b "$bpf_path" -- ./demo_script.sh 2>&1 | tail -n 1 | cut -d' ' -f6)"
target/debug/proctrace ingest -i demo_script_raw.log -o demo_script_ingested.log -p "$root_pid"
