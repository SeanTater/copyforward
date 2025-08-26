#!/usr/bin/env bash
set -euo pipefail

export PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1

# Build, profile with perf, and generate a flamegraph for the `instrument`
# binary focused on the capped implementation. Usage:
#   ./scripts/profile_capped.sh [output.svg]
# If `flamegraph` (Rust crate) is on PATH it will be used. Otherwise the
# script falls back to `stackcollapse-perf.pl` + `flamegraph.pl` if present.

OUT=${1:-flame_capped.svg}
PERF_DATA=perf.data
PERF_SCRIPT=perf.script
# BUILD=release|debug (default release)
BUILD_MODE=${BUILD:-release}
# If set to 1, generate flamegraph with --no-inline
NO_INLINE=${NO_INLINE:-0}
# If set to 1, run `perf report --stdio --call-graph dwarf` after recording
RUN_REPORT=${RUN_REPORT:-0}

if [ "$BUILD_MODE" = "debug" ]; then
    echo "Building instrument (debug)..."
    cargo build --bin instrument
    BIN=target/debug/instrument
else
    echo "Building instrument (release)..."
    cargo build --release --bin instrument
    BIN=target/release/instrument
fi

echo "Recording perf data to ${PERF_DATA} (press Ctrl-C to stop)..."
# Use DWARF call-graph unwinding for more accurate stacks
perf record -F 99 -g --call-graph dwarf -- "$BIN" || true

echo "Generating perf script -> ${PERF_SCRIPT}"
perf script > ${PERF_SCRIPT}

if command -v flamegraph >/dev/null 2>&1; then
    echo "Found 'flamegraph' on PATH — using it to generate ${OUT}"
    if [ "$NO_INLINE" = "1" ]; then
        flamegraph --perfdata ${PERF_DATA} --no-inline -o "${OUT}" --title "copyforward - capped"
    else
        flamegraph --perfdata ${PERF_DATA} -o "${OUT}" --title "copyforward - capped"
    fi
    echo "Wrote ${OUT}"
else
    echo "'flamegraph' not found. Trying stackcollapse-perf.pl + flamegraph.pl"
    if command -v stackcollapse-perf.pl >/dev/null 2>&1 && command -v flamegraph.pl >/dev/null 2>&1; then
        stackcollapse-perf.pl ${PERF_SCRIPT} > perf.folded
        if [ "$NO_INLINE" = "1" ]; then
            flamegraph.pl --no-inline perf.folded > "${OUT}"
        else
            flamegraph.pl perf.folded > "${OUT}"
        fi
        echo "Wrote ${OUT} (via perl tools)"
    else
        echo "No flamegraph toolchain found. Generated ${PERF_SCRIPT} and ${PERF_DATA}."
        echo "Install the 'flamegraph' cargo crate or add the FlameGraph perl scripts to PATH."
        exit 2
    fi
fi

if [ "$RUN_REPORT" = "1" ]; then
    echo "Running perf report (stdio)"
    perf report --stdio --call-graph dwarf
fi

echo "Done. Files: ${PERF_DATA} ${PERF_SCRIPT} ${OUT}"
