#!/bin/bash
echo "make sure to install inferno: https://github.com/jonhoo/inferno"

echo "compile with release & debug symbols"
RUSTFLAGS=-g cargo build --bin=gear --release

echo "profiling for 30 seconds"
timeout 30s perf record --call-graph dwarf target/release/gear &>/dev/null

echo "creating the flamegraph"
perf script | ~/.cargo/bin/inferno-collapse-perf > stacks.folded
cat stacks.folded | ~/.cargo/bin/inferno-flamegraph > flamegraph.svg

echo "done, output file is $(pwd)/flamegraph.svg"