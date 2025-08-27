#!/usr/bin/env bash
set -euo pipefail

# Install Python build/test deps using uv
uv pip install maturin pytest numpy

# Build and install the extension in the active venv
export PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1
# Enable Python bindings by default; add tokenizers via FEATURES if desired
: "${FEATURES:=python}"
maturin develop --release --features "${FEATURES}"

# Run the Python tests
pytest tests/python_tests/ -q

echo "All Python tests passed. You can inspect docs via: python -c 'import copyforward; help(copyforward.CopyForward.from_tokens)'"
