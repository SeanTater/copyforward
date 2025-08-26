#!/usr/bin/env bash
set -euo pipefail

# Build, profile with perf, and generate a flamegraph for the `instrument`
# binary focused on the capped implementation. Usage:
#   ./scripts/profile_capped.sh [output.svg]
# If `flamegraph` (Rust crate) is on PATH it will be used. Otherwise the
# script falls back to `stackcollapse-perf.pl` + `flamegraph.pl` if present.

OUT=${1:-flame_capped.svg}
PERF_DATA=perf.data
PERF_SCRIPT=perf.script

echo "Building instrument (release)..."
cargo build --release --bin instrument

echo "Recording perf data to ${PERF_DATA} (press Ctrl-C to stop)..."
perf record -F 99 -g -- target/release/instrument || true

echo "Generating perf script -> ${PERF_SCRIPT}"
perf script > ${PERF_SCRIPT}

if command -v flamegraph >/dev/null 2>&1; then
    echo "Found 'flamegraph' on PATH — using it to generate ${OUT}"
    flamegraph --perfdata ${PERF_DATA} -o "${OUT}" --title "copyforward - capped"
    echo "Wrote ${OUT}"
else
    echo "'flamegraph' not found. Trying stackcollapse-perf.pl + flamegraph.pl"
    if command -v stackcollapse-perf.pl >/dev/null 2>&1 && command -v flamegraph.pl >/dev/null 2>&1; then
        stackcollapse-perf.pl ${PERF_SCRIPT} > perf.folded
        flamegraph.pl perf.folded > "${OUT}"
        echo "Wrote ${OUT} (via perl tools)"
    else
        echo "No flamegraph toolchain found. Generated ${PERF_SCRIPT} and ${PERF_DATA}."
        echo "Install the 'flamegraph' cargo crate or add the FlameGraph perl scripts to PATH."
        exit 2
    fi
fi

echo "Done. Files: ${PERF_DATA} ${PERF_SCRIPT} ${OUT}"

